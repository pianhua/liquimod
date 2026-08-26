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
    <polygon points="7 3 3 7 11 7 7 3" fill="currentColor" fill-opacity="0.22" stroke="none" />
    <polygon points="17 21 21 17 13 17 17 21" fill="currentColor" fill-opacity="0.22" stroke="none" />
  {/if}
  <path d="m3 7 4-4 4 4" />
  <path d="M7 3v18" />
  <path d="m21 17-4 4-4-4" />
  <path d="M17 21V3" />
</svg>
