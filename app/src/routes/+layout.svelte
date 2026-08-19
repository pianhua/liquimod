<script lang="ts">
  import "../app.css";
  import Toast from "$lib/components/Toast.svelte";
  import TooltipRoot from "$lib/components/TooltipRoot.svelte";
  let { children } = $props();
</script>

{@render children()}
<Toast />
<TooltipRoot />
