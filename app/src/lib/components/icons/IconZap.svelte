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
  <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>
</svg>
