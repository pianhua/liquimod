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
  } from "$lib/api";
  import { toast } from "$lib/toast.svelte";
  import { applyTheme } from "$lib/theme";
  import { viewKey, type ModSort, type View } from "$lib/view";
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
  let characters = $state<CharacterSummary[]>([]);
  let categories = $state<CategoryDto[]>([]);
  let view = $state<View>({ kind: "home" });
  let viewMods = $state<ModDto[]>([]);
  let query = $state("");
  let sort = $state<ModSort>("recent");
  let showSettings = $state(false);
  let error = $state("");
  let dragHover = $state(false);

  let charCatName = $derived(config?.character_category_name ?? "角色");
  let charModTotal = $derived(characters.reduce((n, c) => n + c.total, 0));
  let allCount = $derived(charModTotal + categories.reduce((n, c) => n + c.mod_count, 0));
  let uncatCount = $derived(
    characters.find((c) => c.internal_name === "Others")?.total ?? 0,
  );
  let crumbs = $derived.by((): string[] => {
    switch (view.kind) {
      case "home":
        return [charCatName];
      case "all":
        return ["全部 Mod"];
      case "uncat":
        return ["未分类"];
      case "category":
        return [view.name];
      case "character":
        return [charCatName, view.display];
    }
  });
  let showSort = $derived(view.kind === "all" || view.kind === "uncat" || view.kind === "category");
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
    if (view.kind === "category") viewMods = await api.listCategoryMods(view.id);
    else if (view.kind === "all") viewMods = await api.listAllMods();
    else if (view.kind === "uncat") viewMods = await api.listUncategorizedMods();
  }

  async function refresh() {
    error = "";
    try {
      config = await api.getConfig();
      applyTheme(config.theme);
      characters = await api.getCharacters();
      categories = await api.listCategories();
      await loadViewMods();
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
    await refresh();
    await restoreScroll();
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
    return categories.find((c) => c.id === m.category_id)?.name ?? "未分类";
  }

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
    if (!isTauri()) return;
    try {
      const { openPath } = await import("@tauri-apps/plugin-opener");
      await openPath(m.path);
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

  onMount(() => {
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
              enqueueInstalls(event.payload.paths, refresh);
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
    return () => {
      cancelled = true;
      unlisten?.();
      unlistenChanged?.();
      unlistenToast?.();
    };
  });
</script>

<div class="flex flex-col h-screen">
  <TitleBar onsettings={openSettings} />
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
      {allCount}
      charCount={charModTotal}
      {uncatCount}
      bind:query
      onnavigate={navigate}
      onchanged={refresh}
    />
    <div bind:this={contentEl} class="flex flex-col flex-1 min-w-0 min-h-0">
      {#if showSettings}
        <Settings {config} onback={closeSettings} onchanged={refresh} />
      {:else}
        <Toolbar {crumbs} bind:sort {showSort} onapplied={refresh} />
        {#if view.kind === "home"}
          <header class="px-6 pt-1 pb-3 shrink-0">
            <h1 class="text-2xl font-bold tracking-tight">{charCatName}</h1>
            <p class="text-xs text-secondary mt-0.5">{characters.length} 位 · {charModTotal} 个 Mod</p>
          </header>
          <CharacterGrid
            {characters}
            {query}
            onselect={(c) => navigate({ kind: "character", name: c.internal_name, display: c.display_name })}
          />
        {:else if view.kind === "character" && selectedCharacter}
          <CharacterDetail
            character={selectedCharacter}
            {categories}
            modsDirConfigured={config?.mods_dir != null}
            onback={() => navigate({ kind: "home" })}
            onconfigured={refresh}
          />
        {:else}
          <ModCardGrid
            mods={viewMods}
            {categories}
            {sort}
            {query}
            {catLabelOf}
            ontoggle={toggleViewMod}
            onrename={renameViewMod}
            onuninstall={uninstallViewMod}
            onopen={openViewMod}
            onmove={moveViewMod}
          />
        {/if}
      {/if}
    </div>
  </div>
  {#if dragHover}
    <div class="fixed inset-3 z-40 pointer-events-none radius-panel"
      style="border: 2px dashed var(--accent, #409CFF); background: rgba(64,156,255,0.06)"></div>
  {/if}
  <InstallOverlay jobs={installJobs} {characters} onInstalled={refresh} />
</div>
