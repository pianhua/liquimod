<script lang="ts">
  let { size = 14, class: className = "" }: { size?: number; class?: string } = $props();
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
  class={className}
>
  <circle cx="12" cy="12" r="10"/>
  <line x1="4.93" y1="4.93" x2="19.07" y2="19.07"/>
</svg>
