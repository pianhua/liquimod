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
      d="M20 20H4a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2z"
      fill="currentColor"
      fill-opacity="0.18"
      stroke="none"
    />
  {/if}
  <path d="M12 11v6" />
  <path d="M9 14h6" />
  <path d="M20 20H4a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2z" />
</svg>
