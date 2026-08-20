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
  class={className}
>
  <path d="M12 3v3m0 12v3M3 12h3m12 0h3m-2.64-6.36l-2.12 2.12m-6.48 6.48l-2.12 2.12m0-10.72l2.12 2.12m6.48 6.48l2.12 2.12"/>
  <circle cx="12" cy="12" r="4"/>
</svg>
