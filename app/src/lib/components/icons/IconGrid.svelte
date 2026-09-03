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
    <rect x="3" y="3" width="7.5" height="7.5" rx="1.5" fill="currentColor" fill-opacity="0.25" stroke="none" />
    <rect x="13.5" y="13.5" width="7.5" height="7.5" rx="1.5" fill="currentColor" fill-opacity="0.25" stroke="none" />
  {/if}
  <rect x="3" y="3" width="7.5" height="7.5" rx="1.5" />
  <rect x="13.5" y="3" width="7.5" height="7.5" rx="1.5" />
  <rect x="3" y="13.5" width="7.5" height="7.5" rx="1.5" />
  <rect x="13.5" y="13.5" width="7.5" height="7.5" rx="1.5" />
</svg>
