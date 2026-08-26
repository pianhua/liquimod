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
    <rect width="6" height="18" x="3" y="3" rx="2" fill="currentColor" fill-opacity="0.25" stroke="none" />
  {/if}
  <rect width="18" height="18" x="3" y="3" rx="2" />
  <path d="M9 3v18" />
</svg>
