<script lang="ts">
  import type { CategoryDto, ModDto } from "$lib/api";
  import Toggle from "./Toggle.svelte";
  import CategoryMenu from "./CategoryMenu.svelte";

  let {
    mod,
    categories,
    selected = false,
    ontoggle,
    onrename,
    onuninstall,
    onopen,
    onmove,
    onselect,
    onmenu,
  }: {
    mod: ModDto;
    categories: CategoryDto[];
    selected?: boolean;
    ontoggle: (next: boolean) => void;
    onrename: (name: string) => Promise<boolean>;
    onuninstall: () => Promise<void>;
    onopen: () => void;
    onmove: (categoryId: number | null) => void;
    onselect?: () => void;
    onmenu?: (e: MouseEvent, mod: ModDto) => void;
  } = $props();

  let renaming = $state(false);
  let draft = $state("");
  let confirming = $state(false);
  let busy = $state(false);
  let cancelled = $state(false);

  function fmtSize(b: number): string {
    if (b < 0) return "—";
    if (b < 1024) return `${b} B`;
    if (b < 1024 * 1024) return `${(b / 1024).toFixed(0)} KB`;
    if (b < 1024 * 1024 * 1024) return `${(b / 1024 / 1024).toFixed(1)} MB`;
    return `${(b / 1024 / 1024 / 1024).toFixed(2)} GB`;
  }

  function fmtDate(ts: number): string {
    const d = new Date(ts * 1000);
    return `${d.getMonth() + 1}月${d.getDate()}日`;
  }

  function startRename() {
    draft = mod.name;
    renaming = true;
  }

  async function commitRename() {
    if (cancelled) {
      cancelled = false;
      return;
    }
    const v = draft.trim();
    if (!v || v === mod.name || busy) {
      renaming = false;
      return;
    }
    busy = true;
    try {
      const ok = await onrename(v);
      if (ok) renaming = false;
    } finally {
      busy = false;
    }
  }

  async function confirmUninstall() {
    if (busy) return;
    busy = true;
    try {
      await onuninstall();
    } finally {
      busy = false;
      confirming = false;
    }
  }

  function focusOn(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  function onRowKeydown(e: KeyboardEvent) {
    if (renaming || confirming) return;
    const target = e.target as HTMLElement | null;
    if (target?.closest("button, input, [role='switch']")) return;
    if (e.key === " ") {
      e.preventDefault();
      ontoggle(!mod.enabled);
    } else if (e.key === "Enter") {
      e.preventDefault();
      onselect?.();
    }
  }

  function onRowClick(e: MouseEvent) {
    // 若点击来自 Toggle、按钮、输入框或菜单则不重复触发选择
    const target = e.target as HTMLElement | null;
    if (target?.closest("button, input, [role='switch'], [role='menu']")) return;
    onselect?.();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  role="listitem"
  tabindex="0"
  aria-label={mod.name}
  class="group glass radius-card px-4 py-3.5 flex items-center gap-3.5 outline-none transition-all cursor-pointer focus-visible:shadow-[inset_0_0_0_2px_var(--accent)]"
  class:selected-row={selected}
  onclick={onRowClick}
  ondblclick={onopen}
  onkeydown={onRowKeydown}
  oncontextmenu={(e) => {
    if (onmenu) {
      e.preventDefault();
      e.stopPropagation();
      onmenu(e, mod);
    }
  }}
>
  {#if confirming}
    <div class="flex-1 flex items-center justify-between gap-3 min-w-0">
      <p class="text-sm truncate">
        确认卸载 <span class="font-medium">{mod.name}</span>？文件将被删除
      </p>
      <div class="flex items-center gap-2 shrink-0">
        <button
          class="radius-pill h-8 px-3.5 text-sm font-medium text-white cursor-pointer disabled:opacity-50"
          style="background: var(--danger)"
          disabled={busy}
          onclick={confirmUninstall}
        >
          确认卸载
        </button>
        <button
          class="glass radius-pill h-8 px-3.5 text-sm cursor-pointer"
          onclick={() => (confirming = false)}
        >
          取消
        </button>
      </div>
    </div>
  {:else}
    {#if mod.thumb}
      <img
        src={mod.thumb}
        alt=""
        class="w-14 h-14 rounded-xl object-cover shrink-0"
        style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
        draggable="false"
        onerror={(e) => ((e.currentTarget as HTMLImageElement).style.display = "none")}
      />
    {:else}
      <div
        class="w-14 h-14 rounded-xl shrink-0 grid place-items-center text-lg font-semibold text-secondary"
        style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke); background: var(--glass-tint)"
      >
        {mod.name.slice(0, 1)}
      </div>
    {/if}

    <!-- 基础信息列：固定紧凑宽度，让备注前移且全列表绝对对齐 -->
    <div class="w-44 sm:w-52 md:w-60 shrink-0 min-w-0 flex flex-col justify-center">
      {#if renaming}
        <input
          bind:value={draft}
          aria-label={`新名字 ${mod.name}`}
          class="w-full h-8 px-3 text-[15px] font-medium bg-transparent outline-none rounded-full"
          style="box-shadow: inset 0 0 0 1.5px var(--accent)"
          onkeydown={(e) => {
            if (e.key === "Enter") commitRename();
            else if (e.key === "Escape") {
              cancelled = true;
              renaming = false;
            }
          }}
          onblur={commitRename}
          use:focusOn
        />
      {:else}
        <p class="font-semibold truncate text-[15px] text-[var(--text)] leading-snug" title={mod.name}>{mod.name}</p>
        <p class="text-[13px] text-secondary mt-0.5 truncate leading-tight">
          {fmtSize(mod.size_bytes)} · {mod.file_count < 0 ? "—" : mod.file_count} 文件 · {fmtDate(mod.installed_at)}
        </p>
      {/if}
    </div>

    <!-- 中间备注列：严格对齐的专属独立展示列，自然前靠 -->
    <div class="flex-1 min-w-0 px-2 flex items-center">
      {#if !renaming && mod.note}
        <div
          class="inline-flex items-center gap-2 max-w-lg min-w-0 py-1 px-2.5 rounded-lg bg-[var(--input-bg)] text-xs text-secondary transition-colors group-hover:text-[var(--text)] group-hover:bg-[var(--item-hover)] select-text"
          style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
          title={`备注：${mod.note}`}
        >
          <span class="text-[10px] font-semibold text-secondary/80 uppercase tracking-wide shrink-0 select-none">
            备注
          </span>
          <span class="w-[1px] h-3 bg-[var(--glass-stroke)] shrink-0"></span>
          <span class="truncate">{mod.note}</span>
        </div>
      {/if}
    </div>

    {#if !renaming}
      <div
        class="flex items-center gap-1.5 shrink-0 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100"
      >
        <button
          class="glass radius-pill w-8 h-8 grid place-items-center cursor-pointer"
          aria-label={`打开目录 ${mod.name}`}
          title="打开目录"
          onclick={onopen}
        >
          <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
            <path d="M1.5 3.5a1 1 0 0 1 1-1h2.6l1 1.2h5.4a1 1 0 0 1 1 1v5.8a1 1 0 0 1-1 1H2.5a1 1 0 0 1-1-1v-6Z" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round" />
          </svg>
        </button>
        <button
          class="glass radius-pill w-8 h-8 grid place-items-center cursor-pointer"
          aria-label={`重命名 ${mod.name}`}
          title="重命名"
          onclick={startRename}
        >
          <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
            <path d="M8.6 2.2 10.8 4.4 4.7 10.5l-2.9.7.7-2.9 6.1-6.1Z" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round" />
          </svg>
        </button>
        <CategoryMenu
          {categories}
          current={mod.category_id}
          label={`移到分类 ${mod.name}`}
          onpick={onmove}
        />
        <button
          class="glass radius-pill w-8 h-8 grid place-items-center cursor-pointer transition-colors hover:text-white hover:bg-[var(--danger)]"
          aria-label={`卸载 ${mod.name}`}
          title="卸载"
          onclick={() => (confirming = true)}
        >
          <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
            <path d="M2 3.5h9M5 3.5V2.3a.8.8 0 0 1 .8-.8h1.4a.8.8 0 0 1 .8.8v1.2M3.2 3.5l.5 7a1 1 0 0 0 1 .9h3.6a1 1 0 0 0 1-.9l.5-7" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>
      </div>
    {/if}

    <Toggle
      checked={mod.enabled}
      ariaLabel={`启用 ${mod.name}`}
      onchange={(next) => ontoggle(next)}
    />
  {/if}
</div>

<style>
  .selected-row {
    box-shadow: inset 0 0 0 1.5px var(--accent), 0 6px 20px var(--accent-glow) !important;
    background: var(--glass-tint) !important;
  }
</style>
