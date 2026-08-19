<script lang="ts">
  import type { CategoryDto } from "$lib/api";
  import type { View } from "$lib/view";

  let {
    view,
    categories,
    charCatName,
    charCount,
    collapsed = $bindable(false),
    onnavigate,
  }: {
    view: View;
    categories: CategoryDto[];
    charCatName: string;
    charCount: number;
    collapsed?: boolean;
    onnavigate: (v: View) => void;
  } = $props();

  function isActive(key: string): boolean {
    if (key === "home") return view.kind === "home" || view.kind === "character";
    return view.kind === "type" && String(view.id) === key;
  }

  let fixedTypes = $derived(
    categories
      .filter((c) => c.kind != null)
      .sort((a, b) => a.ord - b.ord),
  );

  let customTypes = $derived(
    categories
      .filter((c) => c.kind == null)
      .sort((a, b) => a.ord - b.ord),
  );

  function getCategoryIcon(kind: string | null): string {
    switch (kind) {
      case "lightcone": return "🗡️";
      case "portrait": return "🖼️";
      case "scene": return "🏞️";
      case "npc": return "👥";
      case "other": return "📦";
      default: return "📁";
    }
  }
</script>

<aside
  class="shrink-0 flex flex-col min-h-0 py-3 transition-[width,padding] duration-150 ease-out select-none {collapsed ? 'w-16 px-2' : 'w-52 px-3'}"
  style="contain: layout style; will-change: width"
  aria-label="分类导航"
>
  <!-- 顶部 Header：分类导航标题与收起/展开控制按钮 -->
  <div class="shrink-0 flex items-center {collapsed ? 'justify-center' : 'justify-between'} px-1 pb-2">
    {#if !collapsed}
      <span class="text-xs font-bold tracking-tight text-secondary px-1.5 select-none">
        资源导航
      </span>
    {/if}
    <button
      class="w-8 h-8 glass radius-pill flex items-center justify-center text-secondary hover:text-[var(--text)] cursor-pointer transition-transform hover:scale-105"
      title={collapsed ? "展开侧边栏" : "收起侧边栏"}
      aria-label={collapsed ? "展开侧边栏" : "收起侧边栏"}
      onclick={() => (collapsed = !collapsed)}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
        <line x1="9" y1="3" x2="9" y2="21"/>
        {#if collapsed}
          <polyline points="13 10 15 12 13 14"/>
        {:else}
          <polyline points="15 10 13 12 15 14"/>
        {/if}
      </svg>
    </button>
  </div>

  <nav class="flex-1 min-h-0 overflow-y-auto flex flex-col gap-1 pr-0.5" aria-label="资源导航树">
    <!-- 1. 核心视图 -->
    {#if !collapsed}
      <div class="px-2.5 pt-1 pb-1 text-[11px] font-semibold text-secondary uppercase tracking-wider">
        核心库
      </div>
    {/if}

    <button
      class="flex items-center gap-2.5 h-9 radius-card text-sm cursor-pointer transition-all {collapsed ? 'justify-center px-0' : 'justify-between px-3'}"
      style={isActive("home")
        ? "background: var(--accent-fill); color: var(--accent); font-weight: 600"
        : ""}
      aria-current={isActive("home") ? "page" : undefined}
      title={charCatName}
      onclick={() => onnavigate({ kind: "home" })}
    >
      <div class="flex items-center gap-2.5 min-w-0">
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="shrink-0">
          <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/>
          <circle cx="12" cy="7" r="4"/>
        </svg>
        {#if !collapsed}
          <span class="truncate">{charCatName}</span>
        {/if}
      </div>
      {#if !collapsed}
        <span class="text-xs text-secondary shrink-0">{charCount}</span>
      {/if}
    </button>

    <!-- 分割线 -->
    {#if fixedTypes.length > 0}
      <div class="my-2 shrink-0 border-t border-[var(--glass-stroke)] opacity-60"></div>
    {/if}

    <!-- 2. 资源实体分类 -->
    {#if !collapsed}
      <div class="px-2.5 pb-1 text-[11px] font-semibold text-secondary uppercase tracking-wider">
        资源分类
      </div>
    {/if}

    {#each fixedTypes as c (c.id)}
      <button
        class="w-full flex items-center gap-2.5 h-9 radius-card text-sm cursor-pointer transition-all {collapsed ? 'justify-center px-0' : 'justify-between px-3'}"
        style={isActive(String(c.id))
          ? "background: var(--accent-fill); color: var(--accent); font-weight: 600"
          : ""}
        aria-current={isActive(String(c.id)) ? "page" : undefined}
        title={c.name}
        onclick={() => onnavigate({ kind: "type", id: c.id, name: c.name })}
      >
        <div class="flex items-center gap-2.5 min-w-0">
          <span class="text-sm shrink-0">{getCategoryIcon(c.kind)}</span>
          {#if !collapsed}
            <span class="truncate">{c.name}</span>
          {/if}
        </div>
        {#if !collapsed}
          <span class="text-xs text-secondary shrink-0">{c.mod_count}</span>
        {/if}
      </button>
    {/each}

    {#if customTypes.length > 0}
      <div class="my-2 shrink-0 border-t border-[var(--glass-stroke)] opacity-60"></div>
      {#if !collapsed}
        <div class="px-2.5 pb-1 text-[11px] font-semibold text-secondary uppercase tracking-wider">
          自定义
        </div>
      {/if}
      {#each customTypes as c (c.id)}
        <button
          class="w-full flex items-center gap-2.5 h-9 radius-card text-sm cursor-pointer transition-all {collapsed ? 'justify-center px-0' : 'justify-between px-3'}"
          style={isActive(String(c.id))
            ? "background: var(--accent-fill); color: var(--accent); font-weight: 600"
            : ""}
          aria-current={isActive(String(c.id)) ? "page" : undefined}
          title={c.name}
          onclick={() => onnavigate({ kind: "type", id: c.id, name: c.name })}
        >
          <div class="flex items-center gap-2.5 min-w-0">
            <span class="text-sm shrink-0">📁</span>
            {#if !collapsed}
              <span class="truncate">{c.name}</span>
            {/if}
          </div>
          {#if !collapsed}
            <span class="text-xs text-secondary shrink-0">{c.mod_count}</span>
          {/if}
        </button>
      {/each}
    {/if}
  </nav>

  <!-- 底部预留区域（为后续 IP 形象与品牌视觉资产展示预留位置） -->
  <div class="shrink-0 mt-auto">
    <!-- IP / Visual Asset Placeholder -->
  </div>
</aside>