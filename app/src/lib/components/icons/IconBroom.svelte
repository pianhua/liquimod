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
    <path
      d="m4 16 5 5 8-8-5-5Z"
      fill="currentColor"
      fill-opacity="0.22"
      stroke="none"
    />
  {/if}
  <path d="m14 11 6-6-3-3-6 6" />
  <path d="m4 16 5 5 8-8-5-5Z" />
  <path d="M3 21h4" />
</svg>
