<script lang="ts">
  import { onMount } from "svelte";
  import { initGlobalTooltip, subscribeTooltip, type TooltipInfo } from "$lib/tooltip";

  let info = $state<TooltipInfo | null>(null);

  onMount(() => {
    const unlistenEvents = initGlobalTooltip();
    const unsubscribe = subscribeTooltip((next) => {
      info = next;
    });

    return () => {
      unlistenEvents();
      unsubscribe();
    };
  });

  let transformStyle = $derived.by(() => {
    if (!info) return "";
    const yTrans = info.placement === "bottom" ? "0" : "-100%";
    let xTrans = "-50%";
    if (info.align === "right") xTrans = "-100%";
    else if (info.align === "left") xTrans = "0";
    return `translate(${xTrans}, ${yTrans})`;
  });
</script>

{#if info}
  <div
    role="tooltip"
    class="fixed z-[999999] pointer-events-none select-none px-3 py-1.5 rounded-xl flex items-center gap-2 text-xs transition-opacity duration-150 animate-in fade-in zoom-in-95 glass-floating"
    style="
      left: {info.x}px;
      top: {info.y}px;
      transform: {transformStyle};
      color: var(--text);
    "
  >
    <span class="leading-normal text-left text-[12px] font-normal tracking-tight whitespace-nowrap shrink-0">{info.text}</span>
    {#if info.shortcut}
      <kbd class="px-1.5 py-0.5 rounded-md text-[10px] font-mono font-bold shrink-0 bg-[var(--input-bg)] text-[var(--accent)] border border-[var(--glass-stroke)] shadow-2xs">
        {info.shortcut}
      </kbd>
    {/if}
  </div>
{/if}
