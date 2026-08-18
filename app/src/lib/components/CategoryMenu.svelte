<script lang="ts">
  import type { CategoryDto } from "$lib/api";

  let {
    categories,
    current,
    label,
    onpick,
  }: {
    categories: CategoryDto[];
    current: number | null;
    label: string;
    onpick: (id: number | null) => void;
  } = $props();

  let open = $state(false);

  function pick(id: number | null) {
    open = false;
    if (id !== current) onpick(id);
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape" && open) open = false;
  }}
/>

<div class="relative">
  <button
    class="glass radius-pill w-8 h-8 grid place-items-center cursor-pointer"
    aria-label={label}
    title="移到分类"
    aria-expanded={open}
    onclick={() => (open = !open)}
  >
    <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
      <path d="M1.5 3.5a1 1 0 0 1 1-1h2.6l1 1.2h5.4a1 1 0 0 1 1 1v5.8a1 1 0 0 1-1 1H2.5a1 1 0 0 1-1-1v-6Z" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round" />
      <path d="M5 8h3.5M7.3 6.8 8.7 8l-1.4 1.2" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
  </button>
  {#if open}
    <button
      class="fixed inset-0 z-40 cursor-default bg-transparent"
      aria-label="关闭分类菜单"
      tabindex="-1"
      onclick={() => (open = false)}
    ></button>
    <div class="glass radius-panel absolute right-0 top-9 z-50 w-44 p-1.5 flex flex-col gap-0.5">
      <button
        class="text-left text-sm px-2.5 py-1.5 rounded-lg cursor-pointer transition-colors hover:bg-[var(--glass-stroke)] flex items-center justify-between"
        onclick={() => pick(null)}
      >
        <span>角色（默认）</span>
        {#if current === null}<span class="accent-text text-xs">✓</span>{/if}
      </button>
      {#each categories as c (c.id)}
        <button
          class="text-left text-sm px-2.5 py-1.5 rounded-lg cursor-pointer transition-colors hover:bg-[var(--glass-stroke)] flex items-center justify-between"
          onclick={() => pick(c.id)}
        >
          <span class="truncate">{c.name}</span>
          {#if current === c.id}<span class="accent-text text-xs">✓</span>{/if}
        </button>
      {/each}
      {#if categories.length === 0}
        <p class="text-xs text-secondary px-2.5 py-1.5">还没有自定义分类，在左侧边栏底部新建</p>
      {/if}
    </div>
  {/if}
</div>
