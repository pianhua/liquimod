<script lang="ts">
  let { size = 16, class: className = "" }: { size?: number; class?: string } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.8"
  stroke-linecap="round"
  stroke-linejoin="round"
  class="shrink-0 {className}"
>
  <polyline points="6 9 12 15 18 9" />
</svg>
