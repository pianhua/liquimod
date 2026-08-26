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
    <polygon points="12 19 5 12 12 5 12 19" fill="currentColor" fill-opacity="0.18" stroke="none" />
  {/if}
  <line x1="19" y1="12" x2="5" y2="12" />
  <polyline points="12 19 5 12 12 5" />
</svg>
