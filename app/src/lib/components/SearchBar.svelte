<script lang="ts">
  let { value = $bindable("") }: { value: string } = $props();
</script>

<div class="glass radius-pill flex items-center gap-2 pl-3.5 pr-4 h-9 w-72">
  <svg width="13" height="13" viewBox="0 0 13 13" fill="none" class="shrink-0 text-secondary">
    <circle cx="5.5" cy="5.5" r="4" stroke="currentColor" stroke-width="1.3" />
    <path d="M8.8 8.8L12 12" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" />
  </svg>
  <input
    bind:value
    type="search"
    placeholder="搜索角色…"
    class="flex-1 min-w-0 bg-transparent outline-none text-sm placeholder:text-[var(--text-secondary)]"
  />
</div>
