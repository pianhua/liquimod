<script lang="ts">
  import { onMount } from "svelte";
  import {
    api,
    isTauri,
    portraitUrl,
    getCachedCharacterImage,
    resolveCharacterImage,
    type CategoryDto,
    type CharacterSummary,
    type ModDto,
  } from "$lib/api";
  import { filterMods, type EnabledFilter } from "$lib/view";
  import ModRow from "$lib/components/ModRow.svelte";
  import ModDetailPane from "$lib/components/ModDetailPane.svelte";
  import EnabledFilterChips from "$lib/components/EnabledFilterChips.svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { toast } from "$lib/toast.svelte";
  import { pushEscHandler } from "$lib/esc";

  let {
    character,
    categories,
    categoryId = null,
    categoryName = undefined,
    modsDirConfigured,
    query = "",
    onback,
    onconfigured,
  }: {
    character: CharacterSummary;
    categories: CategoryDto[];
    categoryId?: number | null;
    categoryName?: string;
    modsDirConfigured: boolean;
    query?: string;
    onback: () => void;
    onconfigured: () => void;
  } = $props();

  let customAvatar = $state<string | null>(null);

  let resolvedAvatar = $derived(
    customAvatar || (character.image ? (getCachedCharacterImage(character.image) || `/images/${character.image}`) : "")
  );

  $effect(() => {
    let active = true;
    const imgName = character.image;
    if (imgName) {
      resolveCharacterImage(imgName, "Honkai").then((src) => {
        if (active && src) customAvatar = src;
      });
    }
    return () => {
      active = false;
    };
  });

  function getInitialDetailWidth(): number {
    if (typeof window === "undefined") return 440;
    try {
      const saved = localStorage.getItem("liquimod_detail_pane_width");
      if (saved) {
        const w = parseInt(saved, 10);
        if (!isNaN(w) && w >= 320 && w <= 750) return w;
      }
    } catch {}
    return 440;
  }

  let mods = $state<ModDto[]>([]);
  let error = $state("");
  let enabledFilter = $state<EnabledFilter>("all");
  let selectedModId = $state<number | null>(null);
  let radioMode = $state(false);
  let detailWidth = $state(getInitialDetailWidth());
  let isDragging = $state(false);

  let shown = $derived(filterMods(mods, query, enabledFilter));
  let selectedMod = $derived(
    shown.find((m) => m.id === selectedModId) ?? (shown.length > 0 ? shown[0] : null)
  );

  onMount(async () => {
    try {
      mods = await api.listMods(character.internal_name, categoryId);
      if (mods.length > 0) {
        selectedModId = mods[0].id;
      }
    } catch (e) {
      error = String(e);
    }
  });

  async function toggle(mod: ModDto, next: boolean) {
    error = "";
    try {
      if (next && radioMode) {
        // 单选互斥模式：开启当前 Mod 时，自动关闭该角色的其余所有已启用 Mod
        const others = mods.filter((m) => m.id !== mod.id && m.enabled);
        await api.setModEnabled(mod.id, true);
        mod.enabled = true;
        for (const other of others) {
          try {
            await api.setModEnabled(other.id, false);
            other.enabled = false;
          } catch {
            // 忽略非关键单项错误
          }
        }
        toast(`已启用「${mod.name}」（互斥模式已关闭同角色其他外观）`);
      } else {
        await api.setModEnabled(mod.id, next);
        mod.enabled = next;
      }
      onconfigured();
    } catch (e) {
      error = String(e);
    }
  }

  async function enableAll() {
    error = "";
    let count = 0;
    for (const m of shown) {
      if (!m.enabled) {
        try {
          await api.setModEnabled(m.id, true);
          m.enabled = true;
          count++;
        } catch {}
      }
    }
    toast(`已批量启用 ${count} 个 Mod`);
    onconfigured();
  }

  async function disableAll() {
    error = "";
    let count = 0;
    for (const m of shown) {
      if (m.enabled) {
        try {
          await api.setModEnabled(m.id, false);
          m.enabled = false;
          count++;
        } catch {}
      }
    }
    toast(`已批量禁用 ${count} 个 Mod`);
    onconfigured();
  }

  async function pickModsDir() {
    try {
      const path = await open({ directory: true, title: "选择 3Dmigoto Mods 目录" });
      if (typeof path === "string") {
        await api.chooseModsDir(path);
        onconfigured();
      }
    } catch (e) {
      error = String(e);
    }
  }

  async function renameMod(mod: ModDto, name: string): Promise<boolean> {
    error = "";
    try {
      await api.renameMod(mod.id, name);
      mod.name = name;
      return true;
    } catch (e) {
      error = String(e);
      return false;
    }
  }

  async function uninstallMod(mod: ModDto) {
    error = "";
    try {
      await api.uninstallMod(mod.id);
      mods = mods.filter((m) => m.id !== mod.id);
      if (selectedModId === mod.id) {
        const remaining = filterMods(mods, "", enabledFilter);
        selectedModId = remaining.length > 0 ? remaining[0].id : null;
      }
      onconfigured();
    } catch (e) {
      error = String(e);
    }
  }

  async function openModDir(mod: ModDto) {
    try {
      await api.openModFolder(mod.id);
    } catch (e) {
      error = String(e);
    }
  }

  async function moveCategory(mod: ModDto, categoryId: number | null) {
    error = "";
    try {
      await api.setModCategory(mod.id, categoryId);
      if (categoryId !== null) {
        mods = mods.filter((m) => m.id !== mod.id);
        if (selectedModId === mod.id) {
          const remaining = filterMods(mods, "", enabledFilter);
          selectedModId = remaining.length > 0 ? remaining[0].id : null;
        }
      }
      onconfigured();
    } catch (e) {
      error = String(e);
    }
  }

  import ContextMenu, { type MenuItem } from "$lib/components/ContextMenu.svelte";

  let contextMenu = $state<{
    x: number;
    y: number;
    items: MenuItem[];
  } | null>(null);

  function handleModContextMenu(e: MouseEvent, mod: ModDto) {
    selectedModId = mod.id;
    const categoryItems: MenuItem[] = [
      {
        id: "cat-char",
        label: "角色 (默认)",
        action: () => moveCategory(mod, null),
      },
      ...categories.map((c) => ({
        id: `cat-${c.id}`,
        label: c.name,
        action: () => moveCategory(mod, c.id),
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
          action: () => toggle(mod, !mod.enabled),
        },
        {
          id: "open",
          label: "在资源管理器中定位",
          icon: "📂",
          action: () => openModDir(mod),
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
              renameMod(mod, newName.trim());
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
          action: () => uninstallMod(mod),
        },
      ],
    };
  }

  function onListKeydown(e: KeyboardEvent) {
    if (shown.length <= 1) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const currentIndex = shown.findIndex((m) => m.id === selectedMod?.id);
      if (e.key === "ArrowDown") {
        const nextIndex = (currentIndex + 1) % shown.length;
        selectedModId = shown[nextIndex].id;
      } else {
        const prevIndex = (currentIndex - 1 + shown.length) % shown.length;
        selectedModId = shown[prevIndex].id;
      }
    }
  }

  $effect(() => {
    if (contextMenu) {
      return pushEscHandler(() => {
        contextMenu = null;
        return true;
      });
    }
  });

  onMount(() => {
    function handleKey(e: KeyboardEvent) {
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)) {
        return;
      }
      if (e.key === " " && selectedMod) {
        e.preventDefault();
        toggle(selectedMod, !selectedMod.enabled);
        return;
      }
      if (e.key === "Delete" && selectedMod) {
        e.preventDefault();
        uninstallMod(selectedMod);
        return;
      }
    }

    window.addEventListener("keydown", handleKey);
    return () => {
      window.removeEventListener("keydown", handleKey);
    };
  });

  function startDrag(e: MouseEvent) {
    e.preventDefault();
    isDragging = true;
    const startX = e.clientX;
    const startWidth = detailWidth;
    let rafId: number | null = null;

    function onMouseMove(moveEvent: MouseEvent) {
      if (rafId !== null) cancelAnimationFrame(rafId);
      rafId = requestAnimationFrame(() => {
        const delta = startX - moveEvent.clientX;
        const newWidth = Math.max(340, Math.min(750, startWidth + delta));
        detailWidth = newWidth;
      });
    }

    function onMouseUp() {
      if (rafId !== null) cancelAnimationFrame(rafId);
      isDragging = false;
      localStorage.setItem("liquimod_detail_pane_width", String(detailWidth));
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    }

    window.addEventListener("mousemove", onMouseMove, { passive: true });
    window.addEventListener("mouseup", onMouseUp, { once: true });
  }
</script>

<div class="flex flex-col h-full min-h-0 {isDragging ? 'cursor-col-resize select-none' : ''}">
  <!-- 头部：返回与角色信息 -->
  <div class="flex items-center justify-between gap-4 px-8 pt-2 pb-3 shrink-0">
    <div class="flex items-center gap-3.5 min-w-0">
      <button
        class="glass radius-pill pl-2.5 pr-3.5 h-8 text-xs font-semibold flex items-center gap-1 cursor-pointer transition-transform hover:-translate-x-0.5"
        onclick={onback}
      >
        <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
          <path d="M7 1L2.5 5L7 9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
        {categoryName ? `返回${categoryName}` : "返回角色库"}
      </button>

      {#if character.image}
        <img
          src={resolvedAvatar}
          alt=""
          class="w-9 h-9 rounded-full object-cover object-top shrink-0"
          style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
          draggable="false"
          onerror={(e) => {
            const img = e.currentTarget as HTMLImageElement;
            if (img && !img.dataset.fallback) {
              img.dataset.fallback = "1";
              img.src = "/images/Others.png";
            }
          }}
        />
      {/if}

      <div class="flex items-baseline gap-2 truncate">
        <h2 class="text-xl font-bold tracking-tight truncate">{character.display_name}</h2>
        <span class="text-xs text-secondary shrink-0">{mods.length} 个 Mod</span>
      </div>
    </div>

    <!-- 顶部动作组：单选互斥换装模式 + 批量操作 -->
    <div class="flex items-center gap-2 shrink-0">
      <label class="glass radius-pill h-8 px-3 text-xs flex items-center gap-2 cursor-pointer transition-colors"
        style={radioMode ? "background: var(--accent-fill); color: var(--accent); font-weight: 600" : ""}
        title="开启后，启用某个外观会自动关闭该角色的其余外观，实现一键换装"
      >
        <input type="checkbox" bind:checked={radioMode} class="hidden" />
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="16 3 21 3 21 8"/>
          <line x1="4" y1="20" x2="21" y2="3"/>
          <polyline points="21 16 21 21 16 21"/>
          <line x1="15" y1="15" x2="21" y2="21"/>
          <line x1="4" y1="4" x2="9" y2="9"/>
        </svg>
        <span>单选互斥换装</span>
      </label>

      <div class="flex items-center glass radius-pill p-0.5">
        <button
          class="h-7 px-2.5 text-xs text-secondary hover:text-[var(--text)] hover:bg-[var(--item-hover)] rounded-full cursor-pointer transition-colors"
          title="启用当前筛选出的所有 Mod"
          onclick={enableAll}
        >
          全开
        </button>
        <span class="w-[1px] h-3 bg-[var(--glass-stroke)] opacity-60"></span>
        <button
          class="h-7 px-2.5 text-xs text-secondary hover:text-[var(--text)] hover:bg-[var(--item-hover)] rounded-full cursor-pointer transition-colors"
          title="禁用当前筛选出的所有 Mod"
          onclick={disableAll}
        >
          全关
        </button>
      </div>
    </div>
  </div>

  {#if !modsDirConfigured}
    <div class="glass radius-panel mx-8 mb-3 px-4 py-3 flex items-center justify-between shrink-0">
      <span class="text-sm">未配置 3Dmigoto Mods 目录，无法启用 Mod</span>
      <button class="accent-fill accent-text radius-pill px-3.5 h-8 text-sm font-medium cursor-pointer" onclick={pickModsDir}>
        选择目录
      </button>
    </div>
  {/if}
  {#if error}
    <p class="mx-8 mb-2 text-sm shrink-0" style="color: var(--danger)">{error}</p>
  {/if}

  <!-- 主体区域：左右主从分栏 + 拖拽调节器 -->
  <div class="flex-1 min-h-0 flex flex-row px-8 pb-6 gap-0">
    <!-- 左侧列表区 -->
    <div class="flex-1 min-w-[280px] flex flex-col min-h-0 pr-3">
      <div class="flex items-center justify-between shrink-0 mb-3">
        <EnabledFilterChips bind:value={enabledFilter} />
        <span class="text-xs text-secondary">{shown.length}/{mods.length} 个显示</span>
      </div>

      <!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_no_noninteractive_tabindex -->
      <div
        role="region"
        aria-label="Mod 列表"
        class="flex flex-col gap-2.5 overflow-y-auto flex-1 min-h-0 pr-1 outline-none"
        tabindex="0"
        onkeydown={onListKeydown}
      >
        {#each shown as mod (mod.id)}
          <ModRow
            {mod}
            {categories}
            selected={selectedMod?.id === mod.id}
            ontoggle={(next) => toggle(mod, next)}
            onrename={(name) => renameMod(mod, name)}
            onuninstall={() => uninstallMod(mod)}
            onopen={() => openModDir(mod)}
            onmove={(cid) => moveCategory(mod, cid)}
            onselect={() => (selectedModId = mod.id)}
            onmenu={handleModContextMenu}
          />
        {/each}
        {#if shown.length === 0}
          <div class="flex-1 border-2 border-dashed border-[var(--glass-stroke)] radius-card grid place-items-center text-secondary py-16 px-6 text-center">
            <div class="flex flex-col items-center gap-2">
              <div class="w-12 h-12 rounded-full grid place-items-center text-xl font-bold" style="background: var(--glass-tint)">
                📦
              </div>
              <p class="text-sm font-medium text-[var(--text)]">
                {mods.length === 0 ? "暂无 Mod" : "无匹配项"}
              </p>
              <p class="text-xs text-secondary">
                {mods.length === 0 ? "直接将压缩包（.zip / .7z / .rar）拖入窗口即可自动安装" : "请尝试切换上面的筛选状态"}
              </p>
            </div>
          </div>
        {/if}
      </div>
    </div>

    <!-- 自由拖拽分栏手柄 (Splitter) -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      role="separator"
      aria-orientation="vertical"
      class="w-3 -mx-1.5 shrink-0 flex items-center justify-center cursor-col-resize group z-10 select-none"
      onmousedown={startDrag}
      title="拖拽调节详情面板宽度"
    >
      <div class="w-[3px] h-12 rounded-full bg-[var(--splitter-bg)] group-hover:bg-[var(--accent)] group-hover:h-20 transition-all"></div>
    </div>

    <!-- 右侧大图与属性检查器面板 (丝滑右侧滑入与弹性展开) -->
    <div
      class="shrink-0 h-full flex flex-col min-h-0 pl-3 animate-in fade-in slide-in-from-right-4 duration-300 ease-out {isDragging ? '' : 'transition-[width] duration-150'}"
      style={`width: ${detailWidth}px`}
    >
      <ModDetailPane
        mod={selectedMod}
        {categories}
        {character}
        ontoggle={(next) => selectedMod && toggle(selectedMod, next)}
        onrename={(name) => selectedMod ? renameMod(selectedMod, name) : Promise.resolve(false)}
        onuninstall={() => selectedMod ? uninstallMod(selectedMod) : Promise.resolve()}
        onopen={() => selectedMod && openModDir(selectedMod)}
        onmove={(cid) => selectedMod && moveCategory(selectedMod, cid)}
      />
    </div>
  </div>

  {#if contextMenu}
    <ContextMenu
      x={contextMenu.x}
      y={contextMenu.y}
      items={contextMenu.items}
      onclose={() => (contextMenu = null)}
    />
  {/if}
</div>
