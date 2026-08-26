<script lang="ts">
  let { size = 16, class: className = "", duotone = true }: { size?: number; class?: string; duotone?: boolean } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.6"
  stroke-linecap="round"
  stroke-linejoin="round"
  class="shrink-0 {className}"
>
  {#if duotone}
    <circle cx="4" cy="14" r="2.5" fill="currentColor" fill-opacity="0.25" stroke="none" />
    <circle cx="12" cy="8" r="2.5" fill="currentColor" fill-opacity="0.25" stroke="none" />
    <circle cx="20" cy="16" r="2.5" fill="currentColor" fill-opacity="0.25" stroke="none" />
  {/if}
  <line x1="4" y1="21" x2="4" y2="3" />
  <line x1="12" y1="21" x2="12" y2="3" />
  <line x1="20" y1="21" x2="20" y2="3" />
  <circle cx="4" cy="14" r="2.5" />
  <circle cx="12" cy="8" r="2.5" />
  <circle cx="20" cy="16" r="2.5" />
</svg>
