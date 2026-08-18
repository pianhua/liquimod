<script lang="ts">
  import { isTauri } from "$lib/api";

  let { onsettings }: { onsettings: () => void } = $props();

  async function act(action: "minimize" | "toggleMaximize" | "close") {
    if (!isTauri()) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow()[action]();
  }
</script>

<div
  data-tauri-drag-region
  class="flex items-center justify-between h-11 pl-4 pr-3 shrink-0 select-none"
>
  <div class="flex items-center gap-2" data-tauri-drag-region>
    <span class="w-2 h-2 rounded-full" style="background: var(--accent)"></span>
    <span class="text-[13px] font-semibold tracking-wide" data-tauri-drag-region>LiquiMod</span>
  </div>
  <div class="flex items-center gap-1.5">
    <button
      aria-label="设置"
      class="w-8 h-8 grid place-items-center rounded-full transition-colors hover:bg-[var(--glass-stroke)]"
      onclick={onsettings}
    >
      <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
        <circle cx="7" cy="7" r="2" stroke="currentColor" stroke-width="1.2" />
        <path
          d="M7 1.5v1.6M7 10.9v1.6M1.5 7h1.6M10.9 7h1.6M3.1 3.1l1.1 1.1M9.8 9.8l1.1 1.1M10.9 3.1L9.8 4.2M4.2 9.8L3.1 10.9"
          stroke="currentColor"
          stroke-width="1.2"
          stroke-linecap="round"
        />
      </svg>
    </button>
    <button
      aria-label="鏈€灏忓寲"
      class="w-8 h-8 grid place-items-center rounded-full transition-colors hover:bg-[var(--glass-stroke)]"
      onclick={() => act("minimize")}
    >
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
        <path d="M2 6h8" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
      </svg>
    </button>
    <button
      aria-label="鏈€澶у寲"
      class="w-8 h-8 grid place-items-center rounded-full transition-colors hover:bg-[var(--glass-stroke)]"
      onclick={() => act("toggleMaximize")}
    >
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
        <rect x="2.5" y="2.5" width="7" height="7" rx="1.5" stroke="currentColor" stroke-width="1.2" />
      </svg>
    </button>
    <button
      aria-label="鍏抽棴"
      class="w-8 h-8 grid place-items-center rounded-full transition-colors hover:bg-[var(--danger)] hover:text-white"
      onclick={() => act("close")}
    >
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
        <path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
      </svg>
    </button>
  </div>
</div>
