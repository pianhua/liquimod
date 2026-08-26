<script lang="ts">
  import type { CategoryDto } from "$lib/api";
  import { IconFolderPlus, IconCheckCircle } from "$lib/components/icons";

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
    if (e.key === "Escape" && open) {
      e.stopPropagation();
      open = false;
    }
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
    <IconFolderPlus size={13} />
  </button>
  {#if open}
    <button
      class="fixed inset-0 z-40 cursor-default bg-transparent"
      aria-label="关闭分类菜单"
      tabindex="-1"
      onclick={() => (open = false)}
    ></button>
    <div class="glass-floating radius-panel absolute right-0 top-9 z-50 w-44 p-1.5 flex flex-col gap-0.5">
      <button
        class="text-left text-sm px-2.5 py-1.5 rounded-lg cursor-pointer transition-colors hover:bg-[var(--glass-stroke)] flex items-center justify-between"
        onclick={() => pick(null)}
      >
        <span>角色（默认）</span>
        {#if current === null}<IconCheckCircle size={13} class="text-[var(--accent)]" />{/if}
      </button>
      {#each categories as c (c.id)}
        <button
          class="text-left text-sm px-2.5 py-1.5 rounded-lg cursor-pointer transition-colors hover:bg-[var(--glass-stroke)] flex items-center justify-between"
          onclick={() => pick(c.id)}
        >
          <span class="truncate">{c.name}</span>
          {#if current === c.id}<IconCheckCircle size={13} class="text-[var(--accent)]" />{/if}
        </button>
      {/each}
      {#if categories.length === 0}
        <p class="text-xs text-secondary px-2.5 py-1.5">还没有自定义分类，在左侧边栏底部新建</p>
      {/if}
    </div>
  {/if}
</div>
