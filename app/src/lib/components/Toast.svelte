<script lang="ts">
  import { fly } from "svelte/transition";
  import { toasts } from "$lib/toast.svelte";
</script>

<div class="fixed bottom-4 right-4 z-[60] flex flex-col items-end gap-2 pointer-events-none">
  {#each toasts as t (t.id)}
    <div
      transition:fly={{ y: 12, duration: 200 }}
      class="glass-floating radius-pill px-4 h-9 flex items-center text-sm font-medium text-[var(--text)] border border-[var(--glass-stroke)] shadow-2xl"
      style="background: var(--glass-floating-bg); backdrop-filter: blur(24px); -webkit-backdrop-filter: blur(24px);"
      role="status"
    >
      {t.message}
    </div>
  {/each}
</div>
