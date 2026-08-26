<script lang="ts">
  import type { CategoryDto } from "$lib/api";
  import type { View } from "$lib/view";
  import {
    IconUser,
    IconFolder,
    IconPackage,
    IconSidebar,
    IconLayers,
    IconTag,
    IconSparkles,
  } from "$lib/components/icons";

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
</script>

<aside
  class="shrink-0 flex flex-col min-h-0 h-full py-3.5 border-r border-[var(--glass-stroke)] transition-[width,padding] duration-300 ease-[cubic-bezier(0.16,1,0.3,1)] select-none {collapsed ? 'w-[58px] px-1.5' : 'w-44 px-2'}"
  style="background: var(--sidebar-bg); contain: layout style; will-change: width"
  aria-label="分类导航"
>
  <!-- 顶部 Header：分类导航标题与收起/展开控制按钮 -->
  <div class="shrink-0 flex items-center {collapsed ? 'justify-center' : 'justify-between'} px-1 pb-2">
    {#if !collapsed}
      <span class="text-xs font-bold tracking-tight text-secondary px-1 select-none">
        资源导航
      </span>
    {/if}
    <button
      class="w-7 h-7 rounded-lg flex items-center justify-center text-secondary hover:text-[var(--text)] hover:bg-[var(--item-hover)] active:scale-95 transition-all cursor-pointer"
      title={collapsed ? "展开侧边栏" : "收起侧边栏"}
      aria-label={collapsed ? "展开侧边栏" : "收起侧边栏"}
      onclick={() => (collapsed = !collapsed)}
    >
      <IconSidebar size={15} class="opacity-80 hover:opacity-100 transition-opacity" />
    </button>
  </div>

  <nav class="flex-1 min-h-0 overflow-y-auto flex flex-col gap-0.5 pr-0.5" aria-label="资源导航树">
    <!-- 1. 核心视图 -->
    {#if !collapsed}
      <div class="px-2 pt-0.5 pb-0.5 text-[10px] font-semibold text-secondary uppercase tracking-[0.06em]">
        核心库
      </div>
    {/if}

    <button
      class="w-full flex items-center gap-2 h-8 radius-pill text-xs cursor-pointer transition-all {collapsed ? 'justify-center px-0' : 'justify-between px-2.5'}"
      style={isActive("home")
        ? "background: var(--accent-fill); color: var(--accent); font-weight: 600"
        : ""}
      aria-current={isActive("home") ? "page" : undefined}
      title={charCatName}
      onclick={() => onnavigate({ kind: "home" })}
    >
      <div class="flex items-center gap-2 min-w-0">
        <IconUser size={15} class="shrink-0" />
        {#if !collapsed}
          <span class="truncate">{charCatName}</span>
        {/if}
      </div>
      {#if !collapsed}
        <span class="text-[11px] font-mono text-secondary shrink-0">{charCount}</span>
      {/if}
    </button>

    <!-- 分割线 -->
    {#if fixedTypes.length > 0}
      <div class="my-1.5 shrink-0 border-t border-[var(--glass-stroke)] opacity-50"></div>
    {/if}

    <!-- 2. 资源实体分类 -->
    {#if !collapsed}
      <div class="px-2 pb-0.5 text-[10px] font-semibold text-secondary uppercase tracking-[0.06em]">
        资源分类
      </div>
    {/if}

    {#each fixedTypes as c (c.id)}
      <button
        class="w-full flex items-center gap-2 h-8 radius-pill text-xs cursor-pointer transition-all {collapsed ? 'justify-center px-0' : 'justify-between px-2.5'}"
        style={isActive(String(c.id))
          ? "background: var(--accent-fill); color: var(--accent); font-weight: 600"
          : ""}
        aria-current={isActive(String(c.id)) ? "page" : undefined}
        title={c.name}
        onclick={() => onnavigate({ kind: "type", id: c.id, name: c.name })}
      >
        <div class="flex items-center gap-2 min-w-0">
          <div class="w-3.5 h-3.5 shrink-0 grid place-items-center">
            {#if c.kind === "lightcone"}
              <IconSparkles size={14} />
            {:else if c.kind === "portrait"}
              <IconLayers size={14} />
            {:else if c.kind === "scene"}
              <IconTag size={14} />
            {:else if c.kind === "npc"}
              <IconUser size={14} />
            {:else}
              <IconPackage size={14} />
            {/if}
          </div>
          {#if !collapsed}
            <span class="truncate">{c.name}</span>
          {/if}
        </div>
        {#if !collapsed}
          <span class="text-[11px] font-mono text-secondary shrink-0">{c.mod_count}</span>
        {/if}
      </button>
    {/each}

    {#if customTypes.length > 0}
      <div class="my-1.5 shrink-0 border-t border-[var(--glass-stroke)] opacity-50"></div>
      {#if !collapsed}
        <div class="px-2 pb-0.5 text-[10px] font-semibold text-secondary uppercase tracking-[0.06em]">
          自定义
        </div>
      {/if}
      {#each customTypes as c (c.id)}
        <button
          class="w-full flex items-center gap-2 h-8 radius-pill text-xs cursor-pointer transition-all {collapsed ? 'justify-center px-0' : 'justify-between px-2.5'}"
          style={isActive(String(c.id))
            ? "background: var(--accent-fill); color: var(--accent); font-weight: 600"
            : ""}
          aria-current={isActive(String(c.id)) ? "page" : undefined}
          title={c.name}
          onclick={() => onnavigate({ kind: "type", id: c.id, name: c.name })}
        >
          <div class="flex items-center gap-2 min-w-0">
            <div class="w-3.5 h-3.5 shrink-0 grid place-items-center">
              <IconFolder size={14} />
            </div>
            {#if !collapsed}
              <span class="truncate">{c.name}</span>
            {/if}
          </div>
          {#if !collapsed}
            <span class="text-[11px] font-mono text-secondary shrink-0">{c.mod_count}</span>
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