<script lang="ts">
  import { onMount, tick } from "svelte";
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
  let conflicts = $state<import("$lib/api").ConflictReportDto[]>([]);

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

  async function refresh() {
    error = "";
    try {
      config = await api.getConfig();
      applyTheme(config.theme);
      categories = await api.listCategories();
      homeCharacters = await api.getCharacters(null);
      const catId = view.kind === "type" ? view.id : (view.kind === "character" ? (view.categoryId ?? null) : null);
      characters = (catId == null) ? homeCharacters : await api.getCharacters(catId);
      await loadViewMods();
      conflicts = await api.getActiveConflicts().catch(() => []);
    } catch (e) {
      error = String(e);
    }
  }

  async function navigate(v: View) {
    if (!showSettings) saveScroll();
    showSettings = false;
    view = v;
    viewMods = [];
    query = "";
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

  async function launchGame() {
    try {
      await api.launchGame();
      toast("已启动游戏");
    } catch (e) {
      toast(String(e));
      if (String(e).includes("未配置")) openSettings();
    }
  }

  async function launchLoader() {
    try {
      await api.launchLoader();
      toast("已启动加载器");
    } catch (e) {
      toast(String(e));
      if (String(e).includes("未配置")) openSettings();
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
        if (query) {
          query = "";
        }
        return;
      }
      if (query) {
        query = "";
        return;
      }
      if (view.kind === "character") {
        onBackFromCharacter(view);
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
        .catch(() => {});
    });
    let unlistenChanged: (() => void) | undefined;
    let unlistenToast: (() => void) | undefined;
    let unlistenAssetsUpdated: (() => void) | undefined;
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
    };
  });
</script>

<div class="flex flex-col h-screen">
  <TitleBar />
  {#if error}
    <div class="glass radius-panel mx-6 mt-1 px-4 py-2.5 text-sm shrink-0" style="color: var(--danger)">
      {error}
    </div>
  {/if}
  <div class="flex flex-1 min-h-0">
    <Sidebar
      {view}
      {categories}
      {charCatName}
      charCount={homeCharModTotal}
      bind:collapsed={sidebarCollapsed}
      onnavigate={navigate}
    />
    <div bind:this={contentEl} class="flex flex-col flex-1 min-w-0 min-h-0" style="contain: layout style">
      {#if showSettings}
        <Settings {config} onback={closeSettings} onchanged={refresh} />
      {:else}
        <Toolbar
          {crumbs}
          bind:query
          bind:sort
          bind:charSort
          {isCharGrid}
          {showSort}
          {showSettings}
          {conflicts}
          onlaunchgame={launchGame}
          onlaunchloader={launchLoader}
          onrefreshgame={refreshGame}
          ontogglesettings={() => (showSettings ? closeSettings() : openSettings())}
          onapplied={refresh}
        />
        {#if isCharGrid}
          <header class="px-6 pt-1 pb-3 shrink-0">
            <h1 class="text-2xl font-bold tracking-tight">{view.kind === "home" ? charCatName : view.name}</h1>
            <p class="text-xs text-secondary mt-0.5">{characters.length} 位 · {charModTotal} 个 Mod</p>
          </header>
          <CharacterGrid
            {characters}
            {query}
            sort={charSort}
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
      {/if}
    </div>
  </div>
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
