<script lang="ts">
  import { onMount, tick } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { listen } from "@tauri-apps/api/event";
  import {
    api,
    isTauri,
    type CategoryDto,
    type CharacterSummary,
    type ConfigDto,
    type ModDto,
    type CharacterSortOption,
  } from "$lib/api";
  import { toast } from "$lib/toast.svelte";
  import { applyTheme } from "$lib/theme";
  import { viewKey, type EnabledFilter, type ModSort, type View } from "$lib/view";
  import { dispatchEscape } from "$lib/esc";
  import { enqueueInstalls, installJobs } from "$lib/install.svelte";
  import InstallOverlay from "$lib/components/InstallOverlay.svelte";
  import TitleBar from "$lib/components/TitleBar.svelte";
  import WindowResizeHandles from "$lib/components/WindowResizeHandles.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Toolbar from "$lib/components/Toolbar.svelte";
  import CharacterGrid from "$lib/views/CharacterGrid.svelte";
  import CharacterDetail from "$lib/views/CharacterDetail.svelte";
  import Settings from "$lib/views/Settings.svelte";
  import ModCardGrid from "$lib/components/ModCardGrid.svelte";

  let config = $state<ConfigDto | null>(null);
  let homeCharacters = $state<CharacterSummary[]>([]);
  let characters = $state<CharacterSummary[]>([]);
  let categories = $state<CategoryDto[]>([]);
  let view = $state<View>({ kind: "home" });
  let viewMods = $state<ModDto[]>([]);
  let query = $state("");
  let sort = $state<ModSort>("recent");
  let charSort = $state<CharacterSortOption>("default");
  let enabledFilter = $state<EnabledFilter>("all");
  let showSettings = $state(false);
  let error = $state("");
  let dragHover = $state(false);
  let gameRunning = $state(false);
  let launchBusy = $state(false);
  let launchStage = $state("");
  let windowMaximized = $state(false);

  let charCatName = $derived(config?.character_category_name ?? "角色");
  let homeCharModTotal = $derived(homeCharacters.reduce((n, c) => n + c.total, 0));
  let charModTotal = $derived(characters.reduce((n, c) => n + c.total, 0));
  let crumbs = $derived.by((): string[] => {
    switch (view.kind) {
      case "home":
        return [charCatName];
      case "type":
        return [view.name];
      case "character":
        return [view.categoryName ?? charCatName, view.display];
    }
  });
  let isCharGrid = $derived.by((): boolean => {
    const v = view;
    if (v.kind === "home") return true;
    if (v.kind === "type") {
      const cat = categories.find((c) => c.id === v.id);
      return cat?.kind === "lightcone" || cat?.kind === "portrait";
    }
    return false;
  });
  let showSort = $derived(view.kind === "type" && !isCharGrid);
  let selectedCharacter = $derived.by((): CharacterSummary | null => {
    // view 在 navigate 等处被赋值，TS 不在闭包内收窄其属性；先快照为局部常量
    const v = view;
    if (v.kind !== "character") return null;
    return (
      characters.find((c) => c.internal_name === v.name) ?? {
        internal_name: v.name,
        display_name: v.display,
        image: null,
        total: 0,
        enabled: 0,
      }
    );
  });

  // 滚动记忆：display:none 会被浏览器重置 scrollTop，按视图显式保存/恢复
  let contentEl = $state<HTMLDivElement | null>(null);
  const scrollMem = new Map<string, number>();

  function saveScroll() {
    const sc = contentEl?.querySelector(".overflow-y-auto");
    if (sc) scrollMem.set(viewKey(view), sc.scrollTop);
  }

  async function restoreScroll() {
    await tick();
    const sc = contentEl?.querySelector(".overflow-y-auto");
    if (sc) sc.scrollTop = scrollMem.get(viewKey(view)) ?? 0;
  }

  async function loadViewMods() {
    if (view.kind === "type" && !isCharGrid) {
      viewMods = await api.listCategoryMods(view.id);
    }
  }

  let refreshSeq = 0;

  async function refresh() {
    const seq = ++refreshSeq;
    error = "";
    try {
      const cfg = await api.getConfig();
      if (seq !== refreshSeq) return;
      config = cfg;
      const gameStatus = await api.getGameStatus().catch(() => ({ running: false }));
      if (seq !== refreshSeq) return;
      gameRunning = gameStatus.running;
      applyTheme(config.theme);

      const cats = await api.listCategories();
      if (seq !== refreshSeq) return;
      categories = cats;

      const homeChars = await api.getCharacters(null);
      if (seq !== refreshSeq) return;
      homeCharacters = homeChars;

      const catId = view.kind === "type" ? view.id : (view.kind === "character" ? (view.categoryId ?? null) : null);
      const curChars = (catId == null) ? homeChars : await api.getCharacters(catId);
      if (seq !== refreshSeq) return;
      characters = curChars;

      await loadViewMods();
      if (seq !== refreshSeq) return;

    } catch (e) {
      if (seq === refreshSeq) {
        error = String(e);
      }
    }
  }

  const viewSearchMemory = new Map<string, string>();

  $effect(() => {
    // 持续记录当前视图的搜索词
    const k = viewKey(view);
    viewSearchMemory.set(k, query);
  });

  async function navigate(v: View) {
    if (!showSettings) saveScroll();
    showSettings = false;
    // 保存当前视图搜索词
    viewSearchMemory.set(viewKey(view), query);
    view = v;
    viewMods = [];
    // 恢复目标视图之前所保存的搜索词
    query = viewSearchMemory.get(viewKey(v)) ?? "";
    restoreScroll();
    await refresh();
    await restoreScroll();
  }

  function onBackFromCharacter(v: View) {
    if (v.kind === "character" && v.categoryId != null) {
      navigate({ kind: "type", id: v.categoryId, name: v.categoryName ?? "分类" });
    } else {
      navigate({ kind: "home" });
    }
  }

  function onSelectCharacter(v: View, c: CharacterSummary) {
    if (v.kind === "type") {
      navigate({ kind: "character", name: c.internal_name, display: c.display_name, categoryId: v.id, categoryName: v.name });
    } else {
      navigate({ kind: "character", name: c.internal_name, display: c.display_name });
    }
  }

  function openSettings() {
    // 仅在非设置页时采样滚动（设置打开时内容区可能已隐藏，scrollTop 已被浏览器归零）
    if (!showSettings) saveScroll();
    showSettings = true;
  }

  async function closeSettings() {
    showSettings = false;
    await refresh();
    await restoreScroll();
  }

  function catLabelOf(m: ModDto): string {
    if (m.category_id == null) return charCatName;
    return categories.find((c) => c.id === m.category_id)?.name ?? "其他";
  }

  /** 当前视图的安装目标：某角色详情 → 该角色名；实体分类视图 → 该分类 kind；home → null（弹窗选）。 */
  let installTarget = $derived.by((): string | null => {
    const v = view; // 快照以让 TS 收窄属性
    if (v.kind === "character") return v.name;
    if (v.kind === "type") {
      const c = categories.find((x) => x.id === v.id);
      return c?.kind ?? null;
    }
    return null;
  });

  async function toggleViewMod(m: ModDto, next: boolean) {
    try {
      await api.setModEnabled(m.id, next);
      m.enabled = next;
    } catch (e) {
      toast(String(e));
    }
  }

  async function renameViewMod(m: ModDto, name: string): Promise<boolean> {
    try {
      await api.renameMod(m.id, name);
      m.name = name;
      return true;
    } catch (e) {
      toast(String(e));
      return false;
    }
  }

  async function uninstallViewMod(m: ModDto) {
    try {
      await api.uninstallMod(m.id);
      await refresh();
    } catch (e) {
      toast(String(e));
    }
  }

  async function openViewMod(m: ModDto) {
    try {
      await api.openModFolder(m.id);
    } catch (e) {
      toast(String(e));
    }
  }

  async function moveViewMod(m: ModDto, categoryId: number | null) {
    try {
      await api.setModCategory(m.id, categoryId);
      await refresh();
    } catch (e) {
      toast(String(e));
    }
  }

  async function toggleWorkMode() {
    const current = config?.work_mode ?? "play";
    const next = current === "play" ? "dev" : "play";
    try {
      config = await api.setWorkMode(next);
      if (next === "dev") {
        toast("🛠️ 已切换为【抓取开发模式】：已启用 Hash 捕获与剪贴板 Dump，可在小键盘抓取");
      } else {
        toast("🎮 已切换为【游玩模式】：纯净流畅零开销，不产生 Dump 缓存");
      }
    } catch (e) {
      toast(`切换模式失败：${e}`);
    }
  }

  async function launchGame() {
    if (launchBusy) return;
    launchBusy = true;
    launchStage = "准备启动";
    try {
      const res = await api.launchGame();
      const modeLabel = config?.work_mode === "dev" ? "🛠️ 抓取开发模式" : "🎮 游玩模式";
      toast(`✨ ${res.message} (${modeLabel})`);
    } catch (e) {
      toast(String(e));
      if (String(e).includes("未配置")) openSettings();
    } finally {
      launchBusy = false;
      launchStage = "";
    }
  }

  async function launchGameNative() {
    try {
      const res = await api.launchGameNative();
      toast(`🕹️ ${res.message}`);
    } catch (e) {
      toast(String(e));
      if (String(e).includes("未配置")) openSettings();
    }
  }

  async function launchOfficialLauncher() {
    try {
      const res = await api.launchOfficialLauncher();
      toast(`🌐 ${res.message}`);
    } catch (e) {
      toast(String(e));
    }
  }

  async function refreshGame() {
    try {
      await api.triggerRefreshGame();
      toast("已发送 F10 游戏刷新信号");
    } catch (e) {
      toast(String(e));
    }
  }

  async function importModPackage() {
    if (!isTauri()) {
      toast("浏览器预览不支持原生文件选择，请在桌面版操作");
      return;
    }
    try {
      const selected = await open({
        multiple: true,
        directory: false,
        title: "选择 Mod 压缩包",
        filters: [{ name: "Mod 压缩包", extensions: ["zip", "7z", "rar"] }],
      });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      enqueueInstalls(paths, installTarget, refresh);
    } catch (e) {
      toast(String(e));
    }
  }

  import ContextMenu, { type MenuItem } from "$lib/components/ContextMenu.svelte";

  let contextMenu = $state<{
    x: number;
    y: number;
    items: MenuItem[];
  } | null>(null);

  async function handleRescanLibrary() {
    try {
      const res = await api.rescanLibrary();
      toast(`全库已对齐：+${res.added} / -${res.removed}`);
      await refresh();
    } catch (e) {
      toast(String(e));
    }
  }

  function handleCharacterContextMenu(e: MouseEvent, c: CharacterSummary) {
    contextMenu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        {
          id: "toggle-fav",
          label: c.is_favorite ? "取消标为喜爱" : "标为喜爱（置顶）",
          icon: c.is_favorite ? "💔" : "💖",
          action: async () => {
            try {
              const next = await api.toggleFavoriteCharacter(c.internal_name);
              c.is_favorite = next;
              toast(next ? `已将 ${c.display_name} 标为喜爱` : `已取消 ${c.display_name} 的喜爱`);
              await refresh();
            } catch (err) {
              toast(String(err));
            }
          },
        },
        { id: "d0", label: "", divider: true },
        {
          id: "open-detail",
          label: `进入 ${c.display_name} 工作台`,
          icon: "🎯",
          action: () => onSelectCharacter(view, c),
        },
        {
          id: "enable-all",
          label: `全开 ${c.display_name} Mod`,
          icon: "⚡",
          action: async () => {
            try {
              const list = await api.listMods(c.internal_name);
              let count = 0;
              for (const m of list) {
                if (!m.enabled) {
                  await api.setModEnabled(m.id, true);
                  count++;
                }
              }
              toast(`已为 ${c.display_name} 启用 ${count} 个 Mod`);
              await refresh();
            } catch (err) {
              toast(String(err));
            }
          },
        },
        {
          id: "disable-all",
          label: `全关 ${c.display_name} Mod`,
          icon: "🚫",
          action: async () => {
            try {
              const list = await api.listMods(c.internal_name);
              let count = 0;
              for (const m of list) {
                if (m.enabled) {
                  await api.setModEnabled(m.id, false);
                  count++;
                }
              }
              toast(`已为 ${c.display_name} 禁用 ${count} 个 Mod`);
              await refresh();
            } catch (err) {
              toast(String(err));
            }
          },
        },
        { id: "d1", label: "", divider: true },
        {
          id: "open-folder",
          label: "在资源管理器中打开角色目录",
          icon: "📂",
          action: async () => {
            if (config?.library_root) {
              const p = `${config.library_root}\\mods\\${c.internal_name}`;
              await api.openPathInExplorer(p);
            }
          },
        },
      ],
    };
  }

  function handleViewModContextMenu(e: MouseEvent, mod: ModDto) {
    const categoryItems: MenuItem[] = [
      {
        id: "cat-char",
        label: "角色 (默认)",
        action: () => moveViewMod(mod, null),
      },
      ...categories.map((c) => ({
        id: `cat-${c.id}`,
        label: c.name,
        action: () => moveViewMod(mod, c.id),
      })),
    ];

    contextMenu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        {
          id: "toggle",
          label: mod.enabled ? "禁用此 Mod" : "启用此 Mod",
          icon: mod.enabled ? "🚫" : "⚡",
          shortcut: "Space",
          action: () => toggleViewMod(mod, !mod.enabled),
        },
        {
          id: "open",
          label: "在资源管理器中定位",
          icon: "📂",
          action: () => openViewMod(mod),
        },
        {
          id: "move",
          label: "移动到分类…",
          icon: "🏷️",
          children: categoryItems,
        },
        {
          id: "rename",
          label: "重命名…",
          icon: "✏️",
          action: () => {
            const newName = window.prompt("请输入新 Mod 名称：", mod.name);
            if (newName && newName.trim() && newName.trim() !== mod.name) {
              renameViewMod(mod, newName.trim());
            }
          },
        },
        { id: "d1", label: "", divider: true },
        {
          id: "uninstall",
          label: "卸载此 Mod",
          icon: "🗑️",
          danger: true,
          shortcut: "Del",
          action: () => uninstallViewMod(mod),
        },
      ],
    };
  }

  let sidebarCollapsed = $state(false);

  function handleKeydown(e: KeyboardEvent) {
    const target = e.target as HTMLElement | null;
    const inInput = target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable);

    if (e.key === "F10") {
      e.preventDefault();
      refreshGame();
      return;
    }
    if (e.key === "F5") {
      e.preventDefault();
      handleRescanLibrary();
      return;
    }
    if ((e.ctrlKey || e.metaKey) && (e.key.toLowerCase() === "k" || e.key.toLowerCase() === "f")) {
      e.preventDefault();
      const input = document.querySelector('input[type="search"]') as HTMLInputElement | null;
      if (input) {
        input.focus();
        input.select();
      }
      return;
    }
    if (e.key === "Escape") {
      if (dispatchEscape()) {
        e.preventDefault();
        return;
      }
      if (contextMenu) {
        contextMenu = null;
        return;
      }
      if (showSettings) {
        closeSettings();
        return;
      }
      const searchInput = document.querySelector('input[type="search"]') as HTMLInputElement | null;
      if (searchInput && document.activeElement === searchInput) {
        searchInput.blur();
        return;
      }
      if (view.kind === "character") {
        onBackFromCharacter(view);
        return;
      }
      if (query) {
        query = "";
        return;
      }
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handleKeydown);
    void refresh();
    if (!isTauri()) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    import("@tauri-apps/api/webviewWindow").then(({ getCurrentWebviewWindow }) => {
      getCurrentWebviewWindow()
        .onDragDropEvent((event) => {
          const t = event.payload.type;
          if (t === "enter" || t === "over") dragHover = true;
          else if (t === "leave") dragHover = false;
          else if (t === "drop") {
            dragHover = false;
            if (event.payload.paths.length > 0)
              enqueueInstalls(event.payload.paths, installTarget, refresh);
          }
        })
        .then((u) => {
          if (cancelled) u();
          else unlisten = u;
        })
        .catch((e) => {
          console.error("[file-drop] native listener registration failed", e);
          toast("系统拖放不可用，请使用顶部“导入”按钮选择 Mod 压缩包");
        });
    });
    let unlistenChanged: (() => void) | undefined;
    let unlistenToast: (() => void) | undefined;
    let unlistenAssetsUpdated: (() => void) | undefined;
    let unlistenGameStatus: (() => void) | undefined;
    let unlistenLaunchProgress: (() => void) | undefined;
    listen<{ added: number; removed: number }>("library-changed", (e) => {
      if (cancelled) return;
      const { added, removed } = e.payload;
      if (added > 0 || removed > 0) toast(`检测到仓库变动：+${added} / -${removed}`);
      refresh();
    })
      .then((u) => {
        if (cancelled) u();
        else unlistenChanged = u;
      })
      .catch(() => {});
    listen<string>("liquimod-toast", (e) => {
      if (cancelled) return;
      toast(e.payload);
    })
      .then((u) => {
        if (cancelled) u();
        else unlistenToast = u;
      })
      .catch(() => {});
    listen<string>("launch-progress", (e) => {
      if (cancelled) return;
      launchStage = e.payload;
      toast(`启动进度：${e.payload}`, 1800);
    })
      .then((u) => {
        if (cancelled) u();
        else unlistenLaunchProgress = u;
      })
      .catch(() => {});
    listen<import("$lib/api").GameStatusDto>("game-status-changed", (e) => {
      if (cancelled) return;
      gameRunning = e.payload.running;
    })
      .then((u) => {
        if (cancelled) u();
        else unlistenGameStatus = u;
      })
      .catch(() => {});
    listen("game-assets-updated", () => {
      if (cancelled) return;
      refresh();
    })
      .then((u) => {
        if (cancelled) u();
        else unlistenAssetsUpdated = u;
      })
      .catch(() => {});
    return () => {
      window.removeEventListener("keydown", handleKeydown);
      cancelled = true;
      unlisten?.();
      unlistenChanged?.();
      unlistenToast?.();
      unlistenAssetsUpdated?.();
      unlistenGameStatus?.();
      unlistenLaunchProgress?.();
    };
  });
</script>

<div class="window-frame" class:window-frame-maximized={windowMaximized}>
  <div class="window-shell h-full min-h-0 overflow-hidden">
    <div class="app-ambient-aurora" aria-hidden="true">
      <div class="aurora-blob aurora-blob-1"></div>
      <div class="aurora-blob aurora-blob-2"></div>
      <div class="aurora-blob aurora-blob-3"></div>
      <div class="aurora-blob aurora-blob-4"></div>
    </div>
    <div class="window-glass glass-island flex flex-col h-full min-h-0 overflow-hidden">
    <TitleBar onmaximizedchange={(maximized) => (windowMaximized = maximized)} />
    {#if error}
      <div class="glass radius-panel mx-4 mt-1 px-4 py-2.5 text-sm shrink-0" style="color: var(--danger)">
        {error}
      </div>
    {/if}
    <div class="flex flex-1 min-h-0" style="contain: layout style">
      <Sidebar
        {view}
        {categories}
        {charCatName}
        charCount={homeCharModTotal}
        bind:collapsed={sidebarCollapsed}
        onnavigate={navigate}
      />
      <div bind:this={contentEl} class="relative flex flex-col flex-1 min-w-0 min-h-0 overflow-hidden" style="contain: layout style">
      <!-- 正常工作区 (保活：通过 class:hidden 隐藏，杜绝 DOM 销毁与返回滚动抖动，同时彻底阻止透底重叠) -->
      <div class="flex flex-col flex-1 min-w-0 min-h-0" class:hidden={showSettings}>
        <Toolbar
          {crumbs}
          bind:query
          bind:sort
          bind:charSort
          {isCharGrid}
          {showSort}
          {showSettings}
          {gameRunning}
          {launchBusy}
          workMode={config?.work_mode ?? "play"}
          ontoggleworkmode={toggleWorkMode}
          onlaunchmodgame={launchGame}
          onlaunchnativegame={launchGameNative}
          onlaunchofficial={launchOfficialLauncher}
          onrefreshgame={refreshGame}
          onimport={importModPackage}
          ontogglesettings={() => (showSettings ? closeSettings() : openSettings())}
          onapplied={refresh}
        />
        {#if isCharGrid}
          <CharacterGrid
            {characters}
            {query}
            sort={charSort}
            warnMultipleEnabled={config?.warn_multiple_mods ?? true}
            onselect={(c) => onSelectCharacter(view, c)}
            onmenu={handleCharacterContextMenu}
            ontogglefavorite={refresh}
          />
        {:else if view.kind === "character" && selectedCharacter}
          <CharacterDetail
            character={selectedCharacter}
            {categories}
            categoryId={view.categoryId}
            categoryName={view.categoryName}
            modsDirConfigured={config?.mods_dir != null}
            warnMultipleEnabled={config?.warn_multiple_mods ?? true}
            {gameRunning}
            {query}
            onback={() => onBackFromCharacter(view)}
            onconfigured={refresh}
          />
        {:else if view.kind === "type"}
          <ModCardGrid
            mods={viewMods}
            {categories}
            {sort}
            {query}
            bind:enabledFilter
            {catLabelOf}
            ontoggle={toggleViewMod}
            onrename={renameViewMod}
            onuninstall={uninstallViewMod}
            onopen={openViewMod}
            onmove={moveViewMod}
            onmenu={handleViewModContextMenu}
          />
        {/if}
      </div>

      <!-- 专属全屏设置视图 (独占工作区，0 重叠，完美适配亮暗主题) -->
      {#if showSettings}
        <div class="flex flex-col flex-1 min-w-0 min-h-0 view-transition">
          <Settings {config} onback={closeSettings} onchanged={refresh} />
        </div>
      {/if}
      </div>
    </div>
  </div>
  </div>
  <WindowResizeHandles />
  {#if dragHover}
    <div class="fixed inset-3 z-40 pointer-events-none radius-panel"
      style="border: 2px dashed var(--accent, #409CFF); background: rgba(64,156,255,0.06)"></div>
  {/if}
  <InstallOverlay jobs={installJobs} {characters} {categories} onInstalled={refresh} />

  {#if contextMenu}
    <ContextMenu
      x={contextMenu.x}
      y={contextMenu.y}
      items={contextMenu.items}
      onclose={() => (contextMenu = null)}
    />
  {/if}
</div>
