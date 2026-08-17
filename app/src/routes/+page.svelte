<script lang="ts">
  import { onMount } from "svelte";
  import { api, type CharacterSummary, type ConfigDto } from "$lib/api";
  import TitleBar from "$lib/components/TitleBar.svelte";
  import SearchBar from "$lib/components/SearchBar.svelte";
  import CharacterGrid from "$lib/views/CharacterGrid.svelte";
  import CharacterDetail from "$lib/views/CharacterDetail.svelte";

  let config = $state<ConfigDto | null>(null);
  let characters = $state<CharacterSummary[]>([]);
  let query = $state("");
  let selected = $state<CharacterSummary | null>(null);
  let error = $state("");

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

  onMount(refresh);
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
        <p class="text-sm text-secondary mt-0.5">{characters.length} 位角色 · {modTotal} 个 Mod</p>
      </div>
      <SearchBar bind:value={query} />
    </header>
    <CharacterGrid {characters} {query} onselect={(c) => (selected = c)} />
  {/if}
</div>
