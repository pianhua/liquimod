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
      d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"
      fill="currentColor"
      fill-opacity="0.1"
      stroke="none"
    />
  {/if}
  <line x1="2" y1="2" x2="22" y2="22" />
  <path d="M16.5 16.5 12 21.23l-7.78-7.78a5.5 5.5 0 0 1 0-7.78L5 4.88" />
  <path d="M8.66 3.14A5.5 5.5 0 0 1 12 5.67l1.06-1.06a5.5 5.5 0 0 1 7.78 7.78l-1.06 1.06" />
</svg>
