<script lang="ts">
  let { size = 13, class: className = "", duotone = true }: { size?: number; class?: string; duotone?: boolean } = $props();
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
    <rect
      x="3"
      y="14"
      width="6"
      height="6"
      rx="1.5"
      fill="currentColor"
      fill-opacity="0.22"
      stroke="none"
    />
    <rect
      x="3"
      y="4"
      width="10"
      height="6"
      rx="1.5"
      fill="currentColor"
      fill-opacity="0.22"
      stroke="none"
    />
  {/if}
  <rect x="3" y="14" width="6" height="6" rx="1.5" />
  <rect x="3" y="4" width="10" height="6" rx="1.5" />
  <path d="M18 6v12m0 0l-3-3m3 3l3-3" stroke-width="1.8" />
</svg>
