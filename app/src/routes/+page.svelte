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

  async function refresh() {
    config = await api.getConfig();
    characters = await api.getCharacters();
  }

  onMount(refresh);
</script>

<div class="flex flex-col h-screen">
  <TitleBar />
  {#if selected}
    <CharacterDetail
      character={selected}
      modsDirConfigured={config?.mods_dir != null}
      onback={() => (selected = null)}
      onconfigured={refresh}
    />
  {:else}
    <header class="flex items-end justify-between px-5 pb-2">
      <h1 class="text-3xl font-bold">角色</h1>
      <SearchBar bind:value={query} />
    </header>
    <CharacterGrid {characters} {query} onselect={(c) => (selected = c)} />
  {/if}
</div>