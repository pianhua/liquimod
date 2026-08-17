<script lang="ts">
  import { onMount } from "svelte";
  import { filterCharacters, type CharacterSummary } from "$lib/api";
  import CharacterCard from "$lib/components/CharacterCard.svelte";

  let {
    characters,
    query,
    onselect,
  }: {
    characters: CharacterSummary[];
    query: string;
    onselect: (c: CharacterSummary) => void;
  } = $props();

  let filtered = $derived(filterCharacters(characters, query));

  let gridEl: HTMLDivElement;
  let rowHeight = $state(0);

  onMount(() => {
    if (typeof ResizeObserver === "undefined") return;
    const measure = () => {
      const cs = getComputedStyle(gridEl);
      const cols = cs.gridTemplateColumns.split(" ").filter(Boolean).length;
      if (cols === 0) return;
      const gap = parseFloat(cs.columnGap) || 0;
      const inner = gridEl.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
      rowHeight = (inner - (cols - 1) * gap) / cols;
    };
    const ro = new ResizeObserver(measure);
    ro.observe(gridEl);
    measure();
    return () => ro.disconnect();
  });
</script>

<div
  bind:this={gridEl}
  class="grid grid-cols-[repeat(auto-fill,minmax(170px,1fr))] gap-5 px-8 pt-2 pb-8 overflow-y-auto flex-1 min-h-0 content-start"
  style:grid-auto-rows={rowHeight > 0 ? `${rowHeight}px` : undefined}
>
  {#each filtered as c (c.internal_name)}
    <CharacterCard character={c} onclick={() => onselect(c)} />
  {/each}
  {#if filtered.length === 0}
    <p class="text-secondary col-span-full text-center mt-24">没有匹配的角色</p>
  {/if}
</div>
