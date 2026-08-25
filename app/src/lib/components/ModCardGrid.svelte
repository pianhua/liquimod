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
    onmenu,
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
    onmenu?: (e: MouseEvent, m: ModDto) => void;
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
        {onmenu}
      />
    {/each}
    {#if shown.length === 0}
      <div class="col-span-full border-2 border-dashed border-[var(--glass-stroke)] radius-card flex flex-col items-center justify-center text-secondary py-16 px-6 text-center my-6">
        <div class="w-12 h-12 rounded-full grid place-items-center text-xl font-bold mb-2" style="background: var(--glass-tint)">
          📦
        </div>
        <p class="text-sm font-medium text-[var(--text)]">
          {mods.length === 0 ? "暂无 Mod" : "无匹配项"}
        </p>
        <p class="text-xs text-secondary mt-1">
          {mods.length === 0 ? "直接将压缩包（.zip / .7z）拖入窗口即可自动安装至此分类" : "请尝试调整上方筛选状态或搜索关键词"}
        </p>
      </div>
    {/if}
  </div>
</div>