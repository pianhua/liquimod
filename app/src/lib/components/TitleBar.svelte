<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  const win = getCurrentWindow();
</script>

<div
  data-tauri-drag-region
  class="flex items-center justify-between h-10 px-3 shrink-0 select-none"
>
  <span class="text-sm font-semibold" data-tauri-drag-region>LiquiMod</span>
  <div class="flex gap-2">
    <button aria-label="最小化" class="glass radius-pill w-8 h-8 grid place-items-center" onclick={() => win.minimize()}>–</button>
    <button aria-label="最大化" class="glass radius-pill w-8 h-8 grid place-items-center" onclick={() => win.toggleMaximize()}>□</button>
    <button aria-label="关闭" class="glass radius-pill w-8 h-8 grid place-items-center" onclick={() => win.close()}>×</button>
  </div>
</div>