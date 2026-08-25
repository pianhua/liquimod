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

<div class="flex gap-1 p-0.5 radius-pill w-fit" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)">
  {#each options as [v, label] (v)}
    <button
      class="radius-pill h-7 px-3 text-xs cursor-pointer transition-colors"
      class:accent-fill={value === v}
      class:accent-text={value === v}
      aria-pressed={value === v}
      onclick={() => (value = v)}
    >
      {label}
    </button>
  {/each}
</div>
