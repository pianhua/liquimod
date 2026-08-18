<script lang="ts">
  import { onMount } from "svelte";
  import { filterCharacters, type CharacterSummary } from "$lib/api";
  import CharacterCard from "$lib/components/CharacterCard.svelte";

  // 卡片信息条固定高度：p-2 上下 16 + gap-2 8 + h-9 36 = 60
  const CARD_EXTRA = 60;

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
      // 固定轨宽布局：第一条轨道的像素宽即卡片宽
      const first = cs.gridTemplateColumns.split(" ").filter(Boolean)[0];
      const w = parseFloat(first);
      if (w > 0) rowHeight = w + CARD_EXTRA;
    };
    const ro = new ResizeObserver(measure);
    ro.observe(gridEl);
    measure();
    return () => ro.disconnect();
  });
</script>

<div
  bind:this={gridEl}
  class="grid grid-cols-[repeat(auto-fill,180px)] justify-center gap-5 px-6 pt-2 pb-8 overflow-y-auto flex-1 min-h-0 content-start"
  style:grid-auto-rows={rowHeight > 0 ? `${rowHeight}px` : undefined}
>
  {#each filtered as c (c.internal_name)}
    <CharacterCard character={c} onclick={() => onselect(c)} />
  {/each}
  {#if filtered.length === 0}
    <p class="text-secondary col-span-full text-center mt-24">没有匹配的角色</p>
  {/if}
</div>