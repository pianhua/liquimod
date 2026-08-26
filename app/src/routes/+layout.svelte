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

    // 优雅呈现已完成第一帧渲染并恢复窗口坐标的无闪烁窗口
    import("@tauri-apps/api/webviewWindow").then(({ getCurrentWebviewWindow }) => {
      try {
        const win = getCurrentWebviewWindow();
        void win.show();
      } catch {}
    }).catch(() => {});

    return () => {
      window.removeEventListener("contextmenu", handleContextMenu);
    };
  });
</script>

<div class="fixed inset-0 pointer-events-none overflow-hidden app-ambient-aurora" aria-hidden="true">
  <div class="aurora-blob aurora-blob-1"></div>
  <div class="aurora-blob aurora-blob-2"></div>
  <div class="aurora-blob aurora-blob-3"></div>
  <div class="aurora-blob aurora-blob-4"></div>
</div>

<div class="relative z-10 flex flex-col h-full">
  {@render children()}
</div>
<Toast />
<TooltipRoot />
