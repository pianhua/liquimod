<script lang="ts">
  import type { CategoryDto, ModDto } from "$lib/api";
  import {
    filterMods,
    sortMods,
    type EnabledFilter,
    type ModSort,
  } from "$lib/view";
  import ModCard from "./ModCard.svelte";
  import EnabledFilterChips from "./EnabledFilterChips.svelte";

  let {
    mods,
    categories,
    sort,
    query,
    enabledFilter = $bindable("all"),
    catLabelOf,
    ontoggle,
    onrename,
    onuninstall,
    onopen,
    onmove,
  }: {
    mods: ModDto[];
    categories: CategoryDto[];
    sort: ModSort;
    query: string;
    enabledFilter: EnabledFilter;
    catLabelOf: (m: ModDto) => string;
    ontoggle: (m: ModDto, next: boolean) => void;
    onrename: (m: ModDto, name: string) => Promise<boolean>;
    onuninstall: (m: ModDto) => Promise<void>;
    onopen: (m: ModDto) => void;
    onmove: (m: ModDto, categoryId: number | null) => void;
  } = $props();

  let shown = $derived(sortMods(filterMods(mods, query, enabledFilter), sort));
</script>

<div class="flex flex-col flex-1 min-h-0">
  <div class="px-6 pt-2 pb-1 shrink-0">
    <EnabledFilterChips bind:value={enabledFilter} />
  </div>
  <div class="grid grid-cols-[repeat(auto-fill,minmax(230px,1fr))] gap-5 px-6 pt-2 pb-8 overflow-y-auto flex-1 min-h-0 content-start">
    {#each shown as m (m.id)}
      <ModCard
        mod={m}
        {categories}
        catLabel={catLabelOf(m)}
        ontoggle={(next) => ontoggle(m, next)}
        onrename={(name) => onrename(m, name)}
        onuninstall={() => onuninstall(m)}
        onopen={() => onopen(m)}
        onmove={(cid) => onmove(m, cid)}
      />
    {/each}
    {#if shown.length === 0}
      <p class="text-secondary col-span-full text-center mt-24">
        {mods.length === 0 ? "这里还没有 Mod" : "没有匹配的 Mod"}
      </p>
    {/if}
  </div>
</div>