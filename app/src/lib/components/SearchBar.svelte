<script lang="ts">
  import { IconSearch } from "$lib/components/icons";

  let { value = $bindable("") }: { value: string } = $props();
</script>

<div class="glass radius-pill flex items-center gap-2 pl-3.5 pr-4 h-9 w-full">
  <IconSearch size={13} class="shrink-0 text-secondary" />
  <input
    bind:value
    type="search"
    placeholder="搜索…"
    class="flex-1 min-w-0 bg-transparent outline-none text-sm placeholder:text-[var(--text-secondary)]"
  />
</div>
