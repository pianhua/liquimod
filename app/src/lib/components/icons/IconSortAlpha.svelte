<script lang="ts">
  let { size = 13, class: className = "" }: { size?: number; class?: string } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  class={className}
>
  <path
    d="M4 18l4-10 4 10m-6.5-3.5h5"
    stroke="currentColor"
    stroke-width="2.2"
    stroke-linecap="round"
    stroke-linejoin="round"
  />
  <path
    d="M18 6v12m0 0l-3-3m3 3l3-3"
    stroke="currentColor"
    stroke-width="1.8"
    stroke-linecap="round"
    stroke-linejoin="round"
  />
</svg>
