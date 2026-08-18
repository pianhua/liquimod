<script lang="ts">
  import PresetMenu from "./PresetMenu.svelte";
  import type { ModSort } from "$lib/view";

  let {
    crumbs,
    sort = $bindable(),
    showSort,
    onapplied,
  }: {
    crumbs: string[];
    sort: ModSort;
    showSort: boolean;
    onapplied: () => void;
  } = $props();
</script>

<div class="relative z-30 flex items-center justify-between h-12 px-6 shrink-0">
  <nav class="text-sm text-secondary truncate" aria-label="面包屑">
    {#each crumbs as crumb, i (i)}
      {#if i > 0}<span class="mx-1.5 opacity-50">›</span>{/if}
      <span class={i === crumbs.length - 1 ? "font-semibold" : ""} style={i === crumbs.length - 1 ? "color: var(--text)" : ""}>{crumb}</span>
    {/each}
  </nav>
  <div class="flex items-center gap-2.5 shrink-0">
    {#if showSort}
      <div class="glass radius-pill h-9 px-3 flex items-center">
        <select
          bind:value={sort}
          aria-label="排序方式"
          class="bg-transparent outline-none text-sm cursor-pointer"
        >
          <option value="recent">最近安装</option>
          <option value="name">名称</option>
          <option value="enabled">启用优先</option>
        </select>
      </div>
    {/if}
    <PresetMenu {onapplied} />
  </div>
</div>
