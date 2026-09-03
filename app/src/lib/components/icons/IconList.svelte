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
    <circle cx="4.5" cy="6" r="1.5" fill="currentColor" fill-opacity="0.3" stroke="none" />
    <circle cx="4.5" cy="12" r="1.5" fill="currentColor" fill-opacity="0.3" stroke="none" />
    <circle cx="4.5" cy="18" r="1.5" fill="currentColor" fill-opacity="0.3" stroke="none" />
  {/if}
  <circle cx="4.5" cy="6" r="1.5" />
  <circle cx="4.5" cy="12" r="1.5" />
  <circle cx="4.5" cy="18" r="1.5" />
  <line x1="9.5" y1="6" x2="20" y2="6" />
  <line x1="9.5" y1="12" x2="20" y2="12" />
  <line x1="9.5" y1="18" x2="20" y2="18" />
</svg>
