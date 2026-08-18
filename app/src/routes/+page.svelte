<script lang="ts">
  import { onMount } from "svelte";
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

  let config = $state<ConfigDto | null>(null);
  let characters = $state<CharacterSummary[]>([]);
  let query = $state("");
  let selected = $state<CharacterSummary | null>(null);
  let error = $state("");
  let dragHover = $state(false);

  let modTotal = $derived(characters.reduce((n, c) => n + c.total, 0));

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
      if (added > 0 || removed > 0) toast(`检测到仓库变动�?${added} / -${removed}`);
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
  <TitleBar />
  {#if error}
    <div class="glass radius-panel mx-8 mt-2 px-4 py-2.5 text-sm" style="color: var(--danger)">
      {error}
    </div>
  {/if}
  {#if selected}
    <CharacterDetail
      character={selected}
      modsDirConfigured={config?.mods_dir != null}
      onback={() => {
        selected = null;
        refresh();
      }}
      onconfigured={refresh}
    />
  {:else}
    <header class="flex items-end justify-between px-8 pt-3 pb-5 shrink-0">
      <div>
        <h1 class="text-[34px] leading-tight font-bold tracking-tight">角色</h1>
        <p class="text-sm text-secondary mt-0.5">{characters.length} 位角�?· {modTotal} �?Mod</p>
      </div>
      <div class="flex items-center gap-2.5">
        <PresetMenu onapplied={refresh} />
        <SearchBar bind:value={query} />
      </div>
    </header>
    <CharacterGrid {characters} {query} onselect={(c) => (selected = c)} />
  {/if}
  {#if dragHover}
    <div class="fixed inset-3 z-40 pointer-events-none radius-panel"
      style="border: 2px dashed var(--accent, #409CFF); background: rgba(64,156,255,0.06)"></div>
  {/if}
  <InstallOverlay jobs={installJobs} {characters} onInstalled={refresh} />
</div>
