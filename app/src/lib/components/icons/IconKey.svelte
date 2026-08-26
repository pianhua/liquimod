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
    <circle cx="7.5" cy="16.5" r="4.5" fill="currentColor" fill-opacity="0.18" stroke="none" />
  {/if}
  <circle cx="7.5" cy="16.5" r="4.5" />
  <path d="m10.5 13.5 8-8" />
  <path d="m15.5 8.5 2 2" />
  <path d="m18.5 5.5 2 2" />
</svg>
