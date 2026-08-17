<script lang="ts">
  let { value = $bindable("") }: { value: string } = $props();
</script>

<input
  bind:value
  type="search"
  placeholder="搜索角色…"
  class="glass radius-pill px-4 h-9 w-64 outline-none text-sm bg-transparent"
/>