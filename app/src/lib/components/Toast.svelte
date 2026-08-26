<script lang="ts">
  import { fly } from "svelte/transition";
  import { toasts } from "$lib/toast.svelte";
  import { IconCheckCircle, IconAlertTriangle, IconInfo } from "$lib/components/icons";

  function getToastIcon(msg: string) {
    if (msg.includes("失败") || msg.includes("错误") || msg.includes("未") || msg.includes("不能")) {
      return IconAlertTriangle;
    }
    if (msg.includes("成功") || msg.includes("已") || msg.includes("完成")) {
      return IconCheckCircle;
    }
    return IconInfo;
  }
</script>

<div class="fixed bottom-4 right-4 z-[60] flex flex-col items-end gap-2 pointer-events-none">
  {#each toasts as t (t.id)}
    {@const IconComp = getToastIcon(t.message)}
    <div
      transition:fly={{ y: 12, duration: 200 }}
      class="glass-floating radius-pill px-3.5 h-9 flex items-center gap-2 text-xs font-medium text-[var(--text)] border border-[var(--glass-stroke)] shadow-2xl"
      style="background: var(--glass-floating-bg); backdrop-filter: blur(24px); -webkit-backdrop-filter: blur(24px);"
      role="status"
    >
      <IconComp size={14} class="shrink-0 {t.message.includes('失败') || t.message.includes('错误') ? 'text-rose-400' : 'text-[var(--accent)]'}" />
      <span>{t.message}</span>
    </div>
  {/each}
</div>
