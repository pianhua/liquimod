<script lang="ts">
  import type { ModSort } from "$lib/view";

  let {
    crumbs,
    sort = $bindable(),
    showSort,
    onlaunchgame,
    onlaunchloader,
  }: {
    crumbs: string[];
    sort: ModSort;
    showSort: boolean;
    onlaunchgame: () => void;
    onlaunchloader: () => void;
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
    <button
      class="glass radius-pill h-9 px-4 text-sm flex items-center gap-1.5 cursor-pointer transition-transform hover:scale-[1.03]"
      onclick={onlaunchgame}
    >
      <svg width="11" height="11" viewBox="0 0 11 11" fill="currentColor">
        <path d="M2.5 1.5v8l7-4-7-4z" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round" />
      </svg>
      启动游戏
    </button>
    <button
      class="glass radius-pill h-9 px-4 text-sm flex items-center gap-1.5 cursor-pointer transition-transform hover:scale-[1.03]"
      onclick={onlaunchloader}
    >
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
        <path d="M6 1.5v5M3.8 3.8 6 1.5l2.2 2.3M2 7.5v2a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1v-2" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
      启动加载器
    </button>
  </div>
</div>
