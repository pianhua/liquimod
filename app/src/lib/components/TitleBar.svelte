<script lang="ts">
  import { onMount } from "svelte";
  import { isTauri } from "$lib/api";

  let {
    onmaximizedchange = () => {},
  }: {
    onmaximizedchange?: (maximized: boolean) => void;
  } = $props();

  let maximized = $state(false);
  let unlistenResize: (() => void) | null = null;

  onMount(() => {
    let disposed = false;

    if (!isTauri()) return;

    void (async () => {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      if (disposed) return;

      const win = getCurrentWindow();
      const syncMaximized = async () => {
        const next = await win.isMaximized();
        if (!disposed) {
          maximized = next;
          onmaximizedchange(next);
        }
      };

      await syncMaximized();
      if (!disposed) {
        unlistenResize = await win.onResized(() => void syncMaximized());
      }
    })();

    return () => {
      disposed = true;
      unlistenResize?.();
      unlistenResize = null;
    };
  });

  async function act(action: "minimize" | "toggleMaximize" | "close") {
    if (!isTauri()) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const win = getCurrentWindow();
    await win[action]();
    if (action === "toggleMaximize") {
      maximized = await win.isMaximized();
      onmaximizedchange(maximized);
    }
  }
</script>

<div
  class="window-titlebar grid grid-cols-[84px_minmax(0,1fr)_84px] items-center h-11 px-3 shrink-0 select-none"
>
  <div class="window-controls flex items-center" aria-label="窗口控制">
    <button
      type="button"
      aria-label="关闭"
      title="关闭"
      class="window-control window-control-close"
      onclick={() => act("close")}
    >
      <svg aria-hidden="true" width="9" height="9" viewBox="0 0 12 12" fill="none">
        <path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      </svg>
    </button>
    <button
      type="button"
      aria-label="最小化"
      title="最小化"
      class="window-control window-control-minimize"
      onclick={() => act("minimize")}
    >
      <svg aria-hidden="true" width="9" height="9" viewBox="0 0 12 12" fill="none">
        <path d="M2.5 6h7" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      </svg>
    </button>
    <button
      type="button"
      aria-label={maximized ? "还原" : "最大化"}
      title={maximized ? "还原" : "最大化"}
      class="window-control window-control-maximize"
      onclick={() => act("toggleMaximize")}
    >
      <svg aria-hidden="true" width="9" height="9" viewBox="0 0 12 12" fill="none">
        <rect x="2.5" y="2.5" width="7" height="7" rx="1.5" stroke="currentColor" stroke-width="1.5" />
      </svg>
    </button>
  </div>
  <div class="window-title min-w-0 justify-self-center flex items-center gap-2" data-tauri-drag-region>
    <span class="w-2 h-2 rounded-full" style="background: var(--accent)"></span>
    <span class="truncate text-[13px] font-semibold tracking-wide" data-tauri-drag-region>LiquiMod</span>
  </div>
  <div class="flex-1 h-full" data-tauri-drag-region></div>
</div>

<style>
  .window-control {
    position: relative;
    display: grid;
    width: 28px;
    height: 28px;
    place-items: center;
    border: 0;
    border-radius: 9999px;
    color: rgba(0, 0, 0, 0.7);
    cursor: pointer;
    transition: background-color 0.16s ease, transform 0.16s ease;
  }

  .window-control::before {
    content: "";
    position: absolute;
    width: 13px;
    height: 13px;
    border-radius: 9999px;
    background: var(--window-control-color);
    box-shadow: inset 0 0.5px 0 rgba(255, 255, 255, 0.58), 0 1px 2px rgba(0, 0, 0, 0.12);
    transition: transform 0.16s ease, filter 0.16s ease;
  }

  .window-control svg {
    position: relative;
    z-index: 1;
    opacity: 0;
    transition: opacity 0.16s ease;
  }

  .window-control:hover::before,
  .window-control:focus-visible::before {
    transform: scale(1.08);
    filter: brightness(0.9) saturate(1.08);
  }

  .window-control:hover svg,
  .window-control:focus-visible svg {
    opacity: 1;
  }

  .window-control:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 65%, transparent);
    outline-offset: 1px;
  }

  .window-control:active {
    transform: scale(0.9);
  }

  .window-control-close {
    --window-control-color: #ff5f57;
  }

  .window-control-minimize {
    --window-control-color: #febc2e;
  }

  .window-control-maximize {
    --window-control-color: #28c840;
  }
</style>
