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
    <polygon points="16 3 21 8 16 13 16 3" fill="currentColor" fill-opacity="0.18" stroke="none" />
    <polygon points="8 21 3 16 8 11 8 21" fill="currentColor" fill-opacity="0.18" stroke="none" />
  {/if}
  <path d="M21 8H7a4 4 0 0 0 0 8h1" />
  <polyline points="16 3 21 8 16 13" />
  <path d="M3 16h14a4 4 0 0 0 0-8h-1" />
  <polyline points="8 21 3 16 8 11" />
</svg>
