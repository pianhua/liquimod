<script lang="ts">
  import { onMount, tick } from "svelte";
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
  import {
    filterMods,
    mergeVisibleOrder,
    sortMods,
    type EnabledFilter,
    type ModSort,
  } from "$lib/view";
  import ModRow from "$lib/components/ModRow.svelte";
  import ModDetailPane from "$lib/components/ModDetailPane.svelte";
  import EnabledFilterChips from "$lib/components/EnabledFilterChips.svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { toast } from "$lib/toast.svelte";
  import { pushEscHandler } from "$lib/esc";
  import { enqueueInstalls } from "$lib/install.svelte";
  import ReassignCharacterModal from "$lib/components/ReassignCharacterModal.svelte";
  import BatchActionBar from "$lib/components/BatchActionBar.svelte";
  import CustomSelect from "$lib/components/CustomSelect.svelte";
  import IconGrip from "$lib/components/icons/IconGrip.svelte";
  import IconClock from "$lib/components/icons/IconClock.svelte";
  import IconSortAlpha from "$lib/components/icons/IconSortAlpha.svelte";
  import IconZap from "$lib/components/icons/IconZap.svelte";
  import IconSortSize from "$lib/components/icons/IconSortSize.svelte";

  let {
    character,
    categories,
    categoryId = null,
    categoryName = undefined,
    modsDirConfigured,
    warnMultipleEnabled = true,
    gameRunning = false,
    query = "",
    onback,
    onconfigured,
  }: {
    character: CharacterSummary;
    categories: CategoryDto[];
    categoryId?: number | null;
    categoryName?: string;
    modsDirConfigured: boolean;
    warnMultipleEnabled?: boolean;
    gameRunning?: boolean;
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
  let allCharacters = $state<CharacterSummary[]>([]);
  let reassignTargetMod = $state<ModDto | null>(null);
  let checkedModIds = $state<Set<number>>(new Set());
  let lastAnchorId = $state<number | null>(null);
  let error = $state("");
  let enabledFilter = $state<EnabledFilter>("all");
  let sort = $state<ModSort>("recent");
  let selectedModId = $state<number | null>(null);
  let detailWidth = $state(getInitialDetailWidth());
  let isDragging = $state(false);

  let shown = $derived(sortMods(filterMods(mods, query, enabledFilter), sort));
  let enabledCount = $derived(mods.filter((mod) => mod.enabled).length);
  let selectedMod = $derived(
    shown.find((m) => m.id === selectedModId) ?? (shown.length > 0 ? shown[0] : null)
  );

  async function refreshMods() {
    try {
      mods = await api.listMods(character.internal_name, categoryId);
      if (mods.length > 0 && (!selectedModId || !mods.some((m) => m.id === selectedModId))) {
        selectedModId = mods[0].id;
      }
    } catch (e) {
      error = String(e);
    }
  }

  function handleRowSelect(e: MouseEvent, mod: ModDto) {
    selectedModId = mod.id;
    if (e.ctrlKey || e.metaKey) {
      const next = new Set(checkedModIds);
      if (next.has(mod.id)) {
        next.delete(mod.id);
      } else {
        next.add(mod.id);
      }
      checkedModIds = next;
      lastAnchorId = mod.id;
    } else if (e.shiftKey && lastAnchorId != null) {
      const anchorIdx = shown.findIndex((m) => m.id === lastAnchorId);
      const currIdx = shown.findIndex((m) => m.id === mod.id);
      if (anchorIdx !== -1 && currIdx !== -1) {
        const [start, end] = [Math.min(anchorIdx, currIdx), Math.max(anchorIdx, currIdx)];
        const next = new Set(checkedModIds);
        for (let i = start; i <= end; i++) {
          next.add(shown[i].id);
        }
        checkedModIds = next;
      }
    } else {
      if (checkedModIds.size > 0) {
        checkedModIds = new Set();
      }
      lastAnchorId = mod.id;
    }
  }

  function handleRowCheck(mod: ModDto, checked: boolean) {
    const next = new Set(checkedModIds);
    if (checked) {
      next.add(mod.id);
    } else {
      next.delete(mod.id);
    }
    checkedModIds = next;
    lastAnchorId = mod.id;
  }

  function selectAll() {
    checkedModIds = new Set(shown.map((m) => m.id));
  }

  function clearSelection() {
    checkedModIds = new Set();
  }

  async function batchEnable() {
    const targets = mods.filter((m) => checkedModIds.has(m.id) && !m.enabled);
    if (targets.length === 0) return;
    let success = 0;
    let failed = 0;
    for (const m of targets) {
      try {
        await api.setModEnabled(m.id, true);
        m.enabled = true;
        success++;
      } catch {
        failed++;
      }
    }
    if (failed === 0) {
      toast(`已批量启用 ${success} 个 Mod`);
    } else {
      toast(`已启用 ${success} 个 Mod，${failed} 个失败`);
    }
    onconfigured();
  }

  async function batchDisable() {
    const targets = mods.filter((m) => checkedModIds.has(m.id) && m.enabled);
    if (targets.length === 0) return;
    let success = 0;
    let failed = 0;
    for (const m of targets) {
      try {
        await api.setModEnabled(m.id, false);
        m.enabled = false;
        success++;
      } catch {
        failed++;
      }
    }
    if (failed === 0) {
      toast(`已批量禁用 ${success} 个 Mod`);
    } else {
      toast(`已禁用 ${success} 个 Mod，${failed} 个失败`);
    }
    onconfigured();
  }

  async function batchMoveCategory(cid: number | null) {
    const targets = mods.filter((m) => checkedModIds.has(m.id));
    if (targets.length === 0) return;
    let success = 0;
    let failed = 0;
    for (const m of targets) {
      try {
        await api.setModCategory(m.id, cid);
        m.category_id = cid;
        success++;
      } catch {
        failed++;
      }
    }
    if (failed === 0) {
      toast(`已批量移动 ${success} 个 Mod 分类`);
    } else {
      toast(`已移动 ${success} 个 Mod 分类，${failed} 个失败`);
    }
    await refreshMods();
  }

  async function batchUninstall() {
    const targets = mods.filter((m) => checkedModIds.has(m.id));
    if (targets.length === 0) return;
    let success = 0;
    let failed = 0;
    for (const m of targets) {
      try {
        await api.uninstallMod(m.id);
        success++;
      } catch {
        failed++;
      }
    }
    if (failed === 0) {
      toast(`已成功卸载 ${success} 个 Mod`);
    } else {
      toast(`已卸载 ${success} 个 Mod，${failed} 个失败`);
    }
    clearSelection();
    await refreshMods();
  }

  onMount(async () => {
    await refreshMods();
    try {
      allCharacters = await api.getCharacters();
    } catch {}
  });

  async function changeVariant(mod: ModDto, variant: string | null): Promise<void> {
    error = "";
    await api.setModVariant(mod.id, variant);
    mod.active_variant = variant;
    await refreshMods();
    onconfigured();
  }
  async function toggle(mod: ModDto, next: boolean) {
    error = "";
    try {
      await api.setModEnabled(mod.id, next);
      mod.enabled = next;
      onconfigured();
    } catch (e) {
      error = String(e);
    }
  }

  async function enableAll() {
    error = "";
    let success = 0;
    let failed = 0;
    for (const m of shown) {
      if (!m.enabled) {
        try {
          await api.setModEnabled(m.id, true);
          m.enabled = true;
          success++;
        } catch {
          failed++;
        }
      }
    }
    if (failed === 0) {
      toast(`已批量启用 ${success} 个 Mod`);
    } else {
      toast(`已启用 ${success} 个 Mod，${failed} 个失败`);
    }
    onconfigured();
  }

  async function disableAll() {
    error = "";
    let success = 0;
    let failed = 0;
    for (const m of shown) {
      if (m.enabled) {
        try {
          await api.setModEnabled(m.id, false);
          m.enabled = false;
          success++;
        } catch {
          failed++;
        }
      }
    }
    if (failed === 0) {
      toast(`已批量禁用 ${success} 个 Mod`);
    } else {
      toast(`已禁用 ${success} 个 Mod，${failed} 个失败`);
    }
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
      toast(mod.storage_kind === "external" ? "已断开外部 Mod，源文件保持不变" : "Mod 已卸载");
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

  function updateSelectedMod(patch: Partial<ModDto>) {
    if (selectedModId == null) return;
    mods = mods.map((m) => (m.id === selectedModId ? { ...m, ...patch } : m));
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
    if (!checkedModIds.has(mod.id)) {
      selectedModId = mod.id;
    }

    // 多选状态下的批量右键菜单
    if (checkedModIds.has(mod.id) && checkedModIds.size > 1) {
      const count = checkedModIds.size;
      const batchCategoryItems: MenuItem[] = [
        {
          id: "batch-cat-char",
          label: "角色 (默认)",
          action: () => batchMoveCategory(null),
        },
        ...categories.map((c) => ({
          id: `batch-cat-${c.id}`,
          label: c.name,
          action: () => batchMoveCategory(c.id),
        })),
      ];

      contextMenu = {
        x: e.clientX,
        y: e.clientY,
        items: [
          {
            id: "batch-enable",
            label: `批量启用 (${count} 项)`,
            icon: "⚡",
            action: batchEnable,
          },
          {
            id: "batch-disable",
            label: `批量禁用 (${count} 项)`,
            icon: "🚫",
            action: batchDisable,
          },
          {
            id: "batch-move",
            label: `批量移动到分类…`,
            icon: "🏷️",
            children: batchCategoryItems,
          },
          { id: "d1", label: "", divider: true },
          {
            id: "batch-uninstall",
            label: `批量卸载 (${count} 项)`,
            icon: "🗑️",
            danger: true,
            disabled: gameRunning,
            action: () => {
              if (window.confirm(`确定要批量卸载选中的 ${count} 个 Mod 吗？`)) {
                batchUninstall();
              }
            },
          },
          {
            id: "clear-sel",
            label: "取消选择",
            shortcut: "Esc",
            action: clearSelection,
          },
        ],
      };
      return;
    }

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
          id: "toggle-fav",
          label: mod.is_favorite ? "取消标为喜爱" : "标为喜爱 (置顶)",
          icon: mod.is_favorite ? "💔" : "💖",
          action: () => toggleFavoriteMod(mod),
        },
        { id: "d0", label: "", divider: true },
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
          id: "reassign",
          label: "重新分配角色…",
          icon: "🎯",
          disabled: gameRunning,
          action: () => {
            reassignTargetMod = mod;
          },
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
          disabled: gameRunning,
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
          label: mod.storage_kind === "external" ? "断开外部连接" : "卸载此 Mod",
          icon: "🗑️",
          danger: true,
          disabled: gameRunning,
          shortcut: "Del",
          action: () => uninstallMod(mod),
        },
      ],
    };
  }

  let draggingModId = $state<number | null>(null);
  let dragOrderIds = $state<number[] | null>(null);
  let dragPreviewRect = $state<{ left: number; width: number; height: number } | null>(null);
  let dragPreviewTop = $state(0);
  let dragPointerOffsetY = 0;
  let listContainerEl: HTMLElement | null = $state(null);
  let dragOverlayHostEl: HTMLElement | null = $state(null);
  let displayedMods = $derived(
    dragOrderIds
      ? dragOrderIds
          .map((id) => shown.find((mod) => mod.id === id))
          .filter((mod): mod is ModDto => mod != null)
      : shown,
  );
  let draggedMod = $derived(
    draggingModId == null ? null : mods.find((mod) => mod.id === draggingModId) ?? null,
  );

  async function toggleFavoriteMod(mod: ModDto) {
    try {
      const next = await api.toggleFavoriteMod(mod.id);
      mod.is_favorite = next;
      toast(next ? `已将「${mod.name}」标为喜爱并置顶` : `已取消「${mod.name}」的喜爱`);
    } catch (e) {
      toast(String(e));
    }
  }

  function handleStartPointerDrag(e: PointerEvent, mod: ModDto) {
    if (e.button !== 0) return;
    e.preventDefault();

    const row = (e.currentTarget as HTMLElement | null)?.closest<HTMLElement>('[role="listitem"]');
    if (!row || !dragOverlayHostEl || !shown.some((item) => item.id === mod.id)) return;
    const rect = row.getBoundingClientRect();
    const hostRect = dragOverlayHostEl.getBoundingClientRect();

    draggingModId = mod.id;
    dragOrderIds = shown.map((item) => item.id);
    dragPreviewRect = {
      left: rect.left - hostRect.left,
      width: rect.width,
      height: rect.height,
    };
    dragPointerOffsetY = e.clientY - rect.top;
    dragPreviewTop = rect.top - hostRect.top;
    document.body.style.userSelect = "none";

    function onPointerMove(ev: PointerEvent) {
      if (draggingModId == null || !dragOrderIds || !listContainerEl || !dragOverlayHostEl) return;
      ev.preventDefault();
      dragPreviewTop =
        ev.clientY - dragPointerOffsetY - dragOverlayHostEl.getBoundingClientRect().top;

      const listRect = listContainerEl.getBoundingClientRect();
      const edge = 44;
      if (ev.clientY < listRect.top + edge) {
        listContainerEl.scrollTop -= Math.ceil((listRect.top + edge - ev.clientY) / 6);
      } else if (ev.clientY > listRect.bottom - edge) {
        listContainerEl.scrollTop += Math.ceil((ev.clientY - (listRect.bottom - edge)) / 6);
      }

      const rows = Array.from(listContainerEl.querySelectorAll<HTMLElement>('[data-mod-id]'))
        .filter((item) => Number(item.dataset.modId) !== draggingModId);
      let insertionIndex = 0;
      for (const item of rows) {
        const itemRect = item.getBoundingClientRect();
        if (ev.clientY > itemRect.top + itemRect.height / 2) insertionIndex++;
      }

      const nextOrder = dragOrderIds.filter((id) => id !== draggingModId);
      nextOrder.splice(insertionIndex, 0, draggingModId);
      if (nextOrder.some((id, index) => id !== dragOrderIds?.[index])) {
        dragOrderIds = nextOrder;
      }
    }

    async function onPointerUp() {
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
      window.removeEventListener("pointercancel", onPointerUp);
      document.body.style.userSelect = "";

      const visibleIds = dragOrderIds;
      const orderChanged = visibleIds?.some((id, index) => id !== shown[index]?.id) ?? false;
      if (orderChanged && visibleIds) {
        const fullIds = mergeVisibleOrder(
          sortMods(mods, sort).map((item) => item.id),
          visibleIds,
        );
        const positions = new Map(fullIds.map((id, index) => [id, index]));
        mods = mods.map((item) => ({ ...item, sort_order: positions.get(item.id) ?? item.sort_order }));
        sort = "custom";
        await tick();
        draggingModId = null;
        dragOrderIds = null;
        dragPreviewRect = null;
        try {
          await api.reorderMods(fullIds);
          toast("已更新 Mod 自定义排序");
        } catch (err) {
          toast(String(err));
          await refreshMods();
        }
      } else {
        draggingModId = null;
        dragOrderIds = null;
        dragPreviewRect = null;
      }
    }

    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", onPointerUp);
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

  $effect(() => {
    if (checkedModIds.size > 0) {
      return pushEscHandler(() => {
        clearSelection();
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
      if ((e.ctrlKey || e.metaKey) && (e.key === "a" || e.key === "A")) {
        e.preventDefault();
        selectAll();
        return;
      }
      if (e.key === "Escape" && checkedModIds.size > 0) {
        e.preventDefault();
        clearSelection();
        return;
      }
      if (e.key === " " && selectedMod && checkedModIds.size <= 1) {
        e.preventDefault();
        toggle(selectedMod, !selectedMod.enabled);
        return;
      }
      if (e.key === "Delete") {
        e.preventDefault();
        if (gameRunning) return;
        if (checkedModIds.size > 1) {
          if (window.confirm(`确定要批量卸载选中的 ${checkedModIds.size} 个 Mod 吗？`)) {
            batchUninstall();
          }
        } else if (selectedMod) {
          uninstallMod(selectedMod);
        }
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

  async function handleImportArchive() {
    if (!isTauri()) return;
    try {
      const selected = await open({
        multiple: true,
        directory: false,
        filters: [{ name: "Mod 压缩包", extensions: ["zip", "7z", "rar"] }],
      });
      if (selected) {
        const paths = Array.isArray(selected) ? selected : [selected];
        enqueueInstalls(paths, character.internal_name, refreshMods);
      }
    } catch (e) {
      toast(String(e));
    }
  }

  async function handleImportFolder() {
    if (!isTauri()) return;
    try {
      const selected = await open({
        multiple: true,
        directory: true,
      });
      if (selected) {
        const paths = Array.isArray(selected) ? selected : [selected];
        enqueueInstalls(paths, character.internal_name, refreshMods);
      }
    } catch (e) {
      toast(String(e));
    }
  }

  async function handleConnectFolder() {
    if (!isTauri()) {
      toast("浏览器预览不支持原生目录选择，请在桌面版操作");
      return;
    }
    try {
      const selected = await open({
        multiple: true,
        directory: true,
        title: "选择要直接连接的 Mod 文件夹",
      });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      let connected = 0;
      const failures: string[] = [];
      for (const path of paths) {
        try {
          await api.connectExternalMod(path, character.internal_name);
          connected += 1;
        } catch (e) {
          failures.push(String(e));
        }
      }
      await refreshMods();
      onconfigured();
      if (failures.length === 0) {
        toast(`已连接 ${connected} 个外部 Mod，源文件不会被复制`);
      } else {
        toast(`已连接 ${connected} 个，${failures.length} 个失败：${failures[0]}`);
      }
    } catch (e) {
      toast(String(e));
    }
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

    <!-- 顶部动作组：导入 Mod + 批量操作 -->
    <div class="flex items-center gap-2 shrink-0">
      <!-- 导入 Mod 复合动作胶囊 (支持选择压缩包与文件夹) -->
      <div class="flex items-center glass radius-pill p-0.5">
        <button
          class="h-7 px-2.5 text-xs font-medium flex items-center gap-1.5 cursor-pointer rounded-full transition-all hover:bg-[var(--item-hover)] hover:text-[var(--text)] text-[var(--accent)] active:scale-95"
          title={gameRunning ? "游戏运行期间暂不支持导入" : "选择本地 Mod 压缩包 (.zip / .7z / .rar) 导入到当前角色"}
          disabled={gameRunning}
          onclick={handleImportArchive}
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
            <polyline points="17 8 12 3 7 8"/>
            <line x1="12" y1="3" x2="12" y2="15"/>
          </svg>
          <span>导入压缩包</span>
        </button>
        <span class="w-[1px] h-3 bg-[var(--glass-stroke)] opacity-60"></span>
        <button
          class="h-7 px-2.5 text-xs font-medium flex items-center gap-1.5 cursor-pointer rounded-full transition-all hover:bg-[var(--item-hover)] hover:text-[var(--text)] text-emerald-500 active:scale-95"
          title={gameRunning ? "游戏运行期间暂不支持导入" : "选择本地 Mod 文件夹导入到当前角色"}
          disabled={gameRunning}
          onclick={handleImportFolder}
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
          </svg>
          <span>导入文件夹</span>
        </button>
        <span class="w-[1px] h-3 bg-[var(--glass-stroke)] opacity-60"></span>
        <button
          class="h-7 px-2.5 text-xs font-medium flex items-center gap-1.5 cursor-pointer rounded-full transition-all hover:bg-[var(--item-hover)] hover:text-[var(--text)] text-secondary active:scale-95"
          title={gameRunning ? "游戏运行期间暂不支持连接外部目录" : "直接使用外部 Mod 文件夹，不复制源文件"}
          disabled={gameRunning}
          onclick={handleConnectFolder}
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/>
            <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>
          </svg>
          <span>连接外部</span>
        </button>
      </div>

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
  {#if warnMultipleEnabled && enabledCount >= 2}
    <div
      class="mx-8 mb-3 px-3.5 py-2.5 radius-card flex items-center gap-2.5 shrink-0 text-xs"
      style="background: var(--warning-fill); color: var(--warning); box-shadow: inset 0 0 0 1px var(--warning-stroke)"
      role="status"
    >
      <span class="w-2 h-2 rounded-full shrink-0" style="background: var(--warning)"></span>
      <span><strong>{enabledCount} 个 Mod 同时启用</strong>，可能存在资源覆盖风险；如游戏表现异常，可优先从这里逐个排查。</span>
    </div>
  {/if}

  <!-- 主体区域：左右主从分栏 + 拖拽调节器 -->
  <div class="flex-1 min-h-0 flex flex-row px-8 pb-6 gap-0">
    <!-- 左侧列表区 -->
    <div
      bind:this={dragOverlayHostEl}
      class="flex-1 min-w-[280px] flex flex-col min-h-0 pr-3 relative"
      data-drag-overlay-host
    >
      <div class="flex items-center justify-between shrink-0 mb-3 gap-2">
        <EnabledFilterChips bind:value={enabledFilter} />
        <div class="flex items-center gap-2">
          <CustomSelect
            bind:value={sort}
            options={[
              { value: "custom", label: "自定义拖拽", icon: IconGrip },
              { value: "recent", label: "最新安装", icon: IconClock },
              { value: "name", label: "按名称 A-Z", icon: IconSortAlpha },
              { value: "enabled", label: "启用状态置顶", icon: IconZap },
              { value: "size", label: "按文件大小", icon: IconSortSize },
            ]}
            size="xs"
          />
          <span class="text-xs text-secondary shrink-0">{shown.length}/{mods.length}</span>
        </div>
      </div>

      <!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_no_noninteractive_tabindex -->
      <div
        bind:this={listContainerEl}
        role="region"
        aria-label="Mod 列表"
        class="flex flex-col gap-2.5 overflow-y-auto flex-1 min-h-0 pr-1.5 p-1 -m-1 outline-none pb-12 select-none {draggingModId !== null ? 'cursor-grabbing' : ''}"
        tabindex="0"
        onkeydown={onListKeydown}
      >
        {#each displayedMods as mod (mod.id)}
          <ModRow
            {mod}
            {categories}
            selected={selectedMod?.id === mod.id}
            checked={checkedModIds.has(mod.id)}
            isMultiSelectMode={checkedModIds.size > 0}
            dragPlaceholder={draggingModId === mod.id}
            mutationLocked={gameRunning}
            onstartdrag={handleStartPointerDrag}
            ontoggle={(next) => toggle(mod, next)}
            ontogglefavorite={() => toggleFavoriteMod(mod)}
            onrename={(name) => renameMod(mod, name)}
            onuninstall={() => uninstallMod(mod)}
            onopen={() => openModDir(mod)}
            onmove={(cid) => moveCategory(mod, cid)}
            onselect={(e) => handleRowSelect(e, mod)}
            oncheck={(checked) => handleRowCheck(mod, checked)}
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
                {mods.length === 0 ? "直接将压缩包（.zip / .7z / .rar）或文件夹拖入窗口即可自动安装" : "请尝试切换上面的筛选状态"}
              </p>
            </div>
          </div>
        {/if}
      </div>

      {#if draggedMod && dragPreviewRect}
        <div
          class="absolute z-[100] pointer-events-none"
          style={`left: ${dragPreviewRect.left}px; top: ${dragPreviewTop}px; width: ${dragPreviewRect.width}px; height: ${dragPreviewRect.height}px;`}
          aria-hidden="true"
          data-drag-preview
        >
          <ModRow
            mod={draggedMod}
            {categories}
            dragPreview={true}
            mutationLocked={gameRunning}
            ontoggle={() => {}}
            onrename={async () => false}
            onuninstall={async () => {}}
            onopen={() => {}}
            onmove={() => {}}
          />
        </div>
      {/if}

      <!-- 底部悬浮批量操作栏（绝对居中在列表正下方，永不跨越到右侧详情面板） -->
      <BatchActionBar
        selectedCount={checkedModIds.size}
        {categories}
        destructiveLocked={gameRunning}
        onEnableAll={batchEnable}
        onDisableAll={batchDisable}
        onMoveCategory={batchMoveCategory}
        onReassignCharacter={() => {
          if (selectedMod) reassignTargetMod = selectedMod;
        }}
        onUninstallAll={batchUninstall}
        onClearSelection={clearSelection}
      />
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
        variantLocked={gameRunning}
        mutationLocked={gameRunning}
        ontoggle={(next) => selectedMod && toggle(selectedMod, next)}
        onrename={(name) => selectedMod ? renameMod(selectedMod, name) : Promise.resolve(false)}
        onuninstall={() => selectedMod ? uninstallMod(selectedMod) : Promise.resolve()}
        onopen={() => selectedMod && openModDir(selectedMod)}
        onmove={(cid) => selectedMod && moveCategory(selectedMod, cid)}
        onvariantchange={(variant) => selectedMod ? changeVariant(selectedMod, variant) : Promise.resolve()}
        onmodchange={updateSelectedMod}
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

  {#if reassignTargetMod}
    <ReassignCharacterModal
      mod={reassignTargetMod}
      currentCharacter={character.internal_name}
      characters={allCharacters}
      onClose={() => (reassignTargetMod = null)}
      onReassigned={async (target) => {
        // 如果批量选中，将选中的其余项也一起迁移
        if (checkedModIds.size > 1) {
          const others = mods.filter((m) => checkedModIds.has(m.id) && m.id !== reassignTargetMod?.id);
          for (const m of others) {
            try {
              await api.reassignMod(m.id, target);
            } catch {}
          }
        }
        clearSelection();
        await refreshMods();
      }}
    />
  {/if}
</div>
