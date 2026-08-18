<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, isTauri, type CharacterSummary, type ConfigDto } from "$lib/api";
  import { toast } from "$lib/toast.svelte";
  import { enqueueInstalls, installJobs } from "$lib/install.svelte";
  import InstallOverlay from "$lib/components/InstallOverlay.svelte";
  import TitleBar from "$lib/components/TitleBar.svelte";
  import SearchBar from "$lib/components/SearchBar.svelte";
  import PresetMenu from "$lib/components/PresetMenu.svelte";
  import CharacterGrid from "$lib/views/CharacterGrid.svelte";
  import CharacterDetail from "$lib/views/CharacterDetail.svelte";
  import Settings from "$lib/views/Settings.svelte";

  let config = $state<ConfigDto | null>(null);
  let characters = $state<CharacterSummary[]>([]);
  let query = $state("");
  let selected = $state<CharacterSummary | null>(null);
  let showSettings = $state(false);
  let error = $state("");
  let dragHover = $state(false);

  let modTotal = $derived(characters.reduce((n, c) => n + c.total, 0));

  // 主页滚动记忆：display:none 会被浏览器重置 scrollTop，需显式保存/恢复
  let homeEl = $state<HTMLDivElement | null>(null);
  let homeScroll = 0;

  function saveHomeScroll() {
    homeScroll = homeEl?.querySelector(".overflow-y-auto")?.scrollTop ?? 0;
  }

  async function showHome() {
    await tick();
    const sc = homeEl?.querySelector(".overflow-y-auto");
    if (sc) sc.scrollTop = homeScroll;
  }

  function openCharacter(c: CharacterSummary) {
    saveHomeScroll();
    selected = c;
  }

  async function closeDetail() {
    selected = null;
    await refresh();
    await showHome();
  }

  function openSettings() {
    // 仅在主页可见时采样滚动；详情页里开设置时主页已隐藏（scrollTop 已被浏览器归零）
    if (!showSettings && selected === null) saveHomeScroll();
    showSettings = true;
  }

  async function closeSettings() {
    showSettings = false;
    await refresh();
    await showHome();
  }

  async function refresh() {
    error = "";
    try {
      config = await api.getConfig();
      characters = await api.getCharacters();
    } catch (e) {
      error = String(e);
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
    <div class="glass radius-panel mx-8 mt-2 px-4 py-2.5 text-sm" style="color: var(--danger)">
      {error}
    </div>
  {/if}
  <div bind:this={homeEl} class:hidden={showSettings || selected !== null} class="flex flex-col flex-1 min-h-0">
    <header class="flex items-end justify-between px-8 pt-3 pb-5 shrink-0">
      <div>
        <h1 class="text-[34px] leading-tight font-bold tracking-tight">角色</h1>
        <p class="text-sm text-secondary mt-0.5">{characters.length} 位角色 · {modTotal} 个 Mod</p>
      </div>
      <div class="flex items-center gap-2.5">
        <PresetMenu onapplied={refresh} />
        <SearchBar bind:value={query} />
      </div>
    </header>
    <CharacterGrid {characters} {query} onselect={openCharacter} />
  </div>
  {#if showSettings}
    <Settings {config} onback={closeSettings} onchanged={refresh} />
  {:else if selected}
    <CharacterDetail
      character={selected}
      modsDirConfigured={config?.mods_dir != null}
      onback={closeDetail}
      onconfigured={refresh}
    />
  {/if}
  {#if dragHover}
    <div class="fixed inset-3 z-40 pointer-events-none radius-panel"
      style="border: 2px dashed var(--accent, #409CFF); background: rgba(64,156,255,0.06)"></div>
  {/if}
  <InstallOverlay jobs={installJobs} {characters} onInstalled={refresh} />
</div>
