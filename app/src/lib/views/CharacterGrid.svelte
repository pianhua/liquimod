<script lang="ts">
  import {
    api,
    filterCharacters,
    sortCharacters,
    type CharacterSummary,
    type CharacterSortOption,
  } from "$lib/api";
  import CharacterCard from "$lib/components/CharacterCard.svelte";

  let {
    characters,
    query,
    sort = "default",
    onselect,
    onmenu,
    ontogglefavorite,
  }: {
    characters: CharacterSummary[];
    query: string;
    sort?: CharacterSortOption;
    onselect: (c: CharacterSummary) => void;
    onmenu?: (e: MouseEvent, c: CharacterSummary) => void;
    ontogglefavorite?: (c: CharacterSummary) => void;
  } = $props();

  let selectedElement = $state<string>("all");

  const elements = [
    { id: "all", label: "全部", color: "var(--text)" },
    { id: "Physical", label: "物理", color: "#a6a6a6" },
    { id: "Fire", label: "火", color: "#f84f36" },
    { id: "Ice", label: "冰", color: "#47c7fd" },
    { id: "Lightning", label: "雷", color: "#c65df8" },
    { id: "Wind", label: "风", color: "#00d696" },
    { id: "Quantum", label: "量子", color: "#5b5df8" },
    { id: "Imaginary", label: "虚数", color: "#f4d258" },
  ];

  let filtered = $derived(
    sortCharacters(filterCharacters(characters, query, selectedElement), sort)
  );

  async function handleToggleFavorite(c: CharacterSummary) {
    try {
      const next = await api.toggleFavoriteCharacter(c.internal_name);
      c.is_favorite = next;
      if (ontogglefavorite) ontogglefavorite(c);
    } catch {}
  }
</script>

<div class="flex flex-col flex-1 min-h-0">
  <!-- 属性筛选胶囊栏 -->
  <div class="flex items-center gap-1.5 px-6 pb-2.5 shrink-0 overflow-x-auto select-none no-scrollbar">
    {#each elements as el}
      <button
        class="glass radius-pill h-7 px-3 text-xs font-medium flex items-center gap-1.5 transition-all cursor-pointer {selectedElement === el.id ? 'font-semibold shadow-sm' : 'text-secondary hover:text-[var(--text)]'}"
        style={selectedElement === el.id
          ? `background: var(--accent-fill); color: var(--accent); box-shadow: inset 0 0 0 1px var(--accent)`
          : ""}
        onclick={() => (selectedElement = el.id)}
      >
        {#if el.id !== "all"}
          <span class="w-2 h-2 rounded-full shrink-0" style="background: {el.color}"></span>
        {/if}
        {el.label}
      </button>
    {/each}
  </div>

  <div
    class="grid grid-cols-[repeat(auto-fill,180px)] [grid-auto-rows:200px] justify-center gap-5 px-6 pt-1 pb-8 overflow-y-auto flex-1 min-h-0 content-start will-change-scroll"
    style="contain: layout style"
  >
    {#each filtered as c (c.internal_name)}
      <CharacterCard
        character={c}
        onclick={() => onselect(c)}
        {onmenu}
        ontogglefavorite={() => handleToggleFavorite(c)}
      />
    {/each}
    {#if filtered.length === 0}
      <p class="text-secondary col-span-full text-center mt-24">没有匹配的角色</p>
    {/if}
  </div>
</div>
