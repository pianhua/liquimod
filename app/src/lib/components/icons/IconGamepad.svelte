<script lang="ts">
  let { size = 14, class: className = "" }: { size?: number; class?: string } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  class={className}
>
  <path d="M6 12h4m-2-2v4m10-2h.01m-3 0h.01M6.5 6h11A4.5 4.5 0 0 1 22 10.5v3A4.5 4.5 0 0 1 17.5 18H16l-2-2H10l-2 2H6.5A4.5 4.5 0 0 1 2 13.5v-3A4.5 4.5 0 0 1 6.5 6z"/>
</svg>
