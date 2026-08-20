<script lang="ts">
  let { size = 13, class: className = "" }: { size?: number; class?: string } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="currentColor"
  class={className}
>
  <circle cx="9" cy="5" r="2" />
  <circle cx="9" cy="12" r="2" />
  <circle cx="9" cy="19" r="2" />
  <circle cx="15" cy="5" r="2" />
  <circle cx="15" cy="12" r="2" />
  <circle cx="15" cy="19" r="2" />
</svg>
