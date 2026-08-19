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
</script>

{#if info}
  <div
    role="tooltip"
    class="fixed z-[999999] pointer-events-none select-none max-w-xs px-2.5 py-1.5 radius-pill flex items-center gap-1.5 text-center text-xs font-medium tracking-tight animate-in fade-in zoom-in-95 duration-150"
    style="
      left: {info.x}px;
      top: {info.y}px;
      transform: translate(-50%, {info.placement === 'bottom' ? '0' : '-100%'});
      background: var(--glass-floating-bg);
      backdrop-filter: blur(28px) saturate(1.8);
      -webkit-backdrop-filter: blur(28px) saturate(1.8);
      border: 0.5px solid var(--glass-stroke);
      box-shadow: 0 10px 30px rgba(0, 0, 0, 0.18), 0 2px 6px rgba(0, 0, 0, 0.08);
      color: var(--text);
    "
  >
    <span class="leading-tight">{info.text}</span>
    {#if info.shortcut}
      <kbd class="px-1.5 py-0.5 rounded text-[10px] font-mono font-bold shrink-0 bg-[var(--input-bg)] text-[var(--accent)] border border-[var(--glass-stroke)] shadow-xs">
        {info.shortcut}
      </kbd>
    {/if}
  </div>
{/if}
