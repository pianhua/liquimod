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
    <rect width="20" height="14" x="2" y="5" rx="2" fill="currentColor" fill-opacity="0.18" stroke="none" />
  {/if}
  <rect width="20" height="14" x="2" y="5" rx="2" />
  <path d="M6 9h.01M10 9h.01M14 9h.01M18 9h.01M6 13h.01M18 13h.01M8 15h8" />
</svg>
