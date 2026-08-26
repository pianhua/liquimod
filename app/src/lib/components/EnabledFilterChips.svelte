<script lang="ts">
  import type { EnabledFilter } from "$lib/view";

  let {
    value = $bindable("all"),
  }: {
    value: EnabledFilter;
  } = $props();

  const options: [EnabledFilter, string][] = [
    ["all", "全部"],
    ["on", "已启用"],
    ["off", "未启用"],
  ];
</script>

<div class="flex items-center glass-liquid-capsule p-0.5" role="group" aria-label="Mod 状态筛选">
  {#each options as [v, label] (v)}
    {@const active = value === v}
    <button
      class="h-7 px-3 text-xs font-semibold rounded-full cursor-pointer transition-all active:scale-95 z-10 {active
        ? 'accent-fill accent-text shadow-xs font-bold'
        : 'text-secondary hover:text-[var(--text)] hover:bg-[var(--item-hover)]'}"
      aria-pressed={active}
      onclick={() => (value = v)}
    >
      {label}
    </button>
  {/each}
</div>
