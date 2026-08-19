<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import Toast from "$lib/components/Toast.svelte";
  import TooltipRoot from "$lib/components/TooltipRoot.svelte";
  let { children } = $props();

  onMount(() => {
    function handleContextMenu(e: MouseEvent) {
      const target = e.target as HTMLElement | null;
      const isInput = target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable);
      if (!isInput) {
        e.preventDefault();
      }
    }
    window.addEventListener("contextmenu", handleContextMenu);
    return () => {
      window.removeEventListener("contextmenu", handleContextMenu);
    };
  });
</script>

{@render children()}
<Toast />
<TooltipRoot />
