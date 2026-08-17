<script lang="ts">
  import { isTauri } from "$lib/api";

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
