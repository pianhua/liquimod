<script lang="ts">
  import type { CategoryDto, ModDto } from "$lib/api";
  import Toggle from "./Toggle.svelte";
  import CategoryMenu from "./CategoryMenu.svelte";

  let {
    mod,
    categories,
    catLabel,
    ontoggle,
    onrename,
    onuninstall,
    onopen,
    onmove,
    onmenu,
  }: {
    mod: ModDto;
    categories: CategoryDto[];
    catLabel: string;
    ontoggle: (next: boolean) => void;
    onrename: (name: string) => Promise<boolean>;
    onuninstall: () => Promise<void>;
    onopen: () => void;
    onmove: (categoryId: number | null) => void;
    onmenu?: (e: MouseEvent, mod: ModDto) => void;
  } = $props();

  let renaming = $state(false);
  let draft = $state("");
  let confirming = $state(false);
  let busy = $state(false);
  let cancelled = $state(false);
  let imgError = $state(false);

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

  function onCardKeydown(e: KeyboardEvent) {
    if (renaming || confirming) return;
    if (e.key !== " " && e.key !== "Enter") return;
    if ((e.target as HTMLElement).closest("button, input, select")) return;
    e.preventDefault();
    ontoggle(!mod.enabled);
  }
  function focusOn(el: HTMLElement) {
    el.focus();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  role="listitem"
  tabindex="0"
  aria-label={mod.name}
  class="group glass radius-card overflow-hidden outline-none transition-all duration-200 ease-out hover:scale-[1.02] hover:-translate-y-1 hover:shadow-xl focus-visible:shadow-[inset_0_0_0_2px_var(--accent)]"
  onkeydown={onCardKeydown}
  oncontextmenu={(e) => {
    if (onmenu) {
      e.preventDefault();
      e.stopPropagation();
      onmenu(e, mod);
    }
  }}
>
  {#if confirming}
    <div class="aspect-video grid place-items-center px-4">
      <p class="text-sm text-center">确认卸载 <span class="font-medium">{mod.name}</span>？<br />文件将被删除</p>
    </div>
    <div class="px-4 pb-4 flex items-center justify-center gap-2">
      <button
        class="radius-pill h-8 px-3.5 text-sm font-medium text-white cursor-pointer disabled:opacity-50"
        style="background: var(--danger)"
        disabled={busy}
        onclick={confirmUninstall}
      >
        确认卸载
      </button>
      <button class="glass radius-pill h-8 px-3.5 text-sm cursor-pointer" onclick={() => (confirming = false)}>
        取消
      </button>
    </div>
  {:else}
    <div class="relative aspect-video overflow-hidden">
      {#if mod.thumb && !imgError}
        <img
          src={mod.thumb}
          alt=""
          class="w-full h-full object-cover transition-transform duration-300 group-hover:scale-105"
          draggable="false"
          onerror={() => (imgError = true)}
        />
      {:else}
        <div class="w-full h-full grid place-items-center text-3xl font-semibold text-secondary"
          style="background: var(--glass-tint)">
          {mod.name.slice(0, 1)}
        </div>
      {/if}
      {#if !renaming}
        <div class="absolute top-2 right-2 flex gap-1.5 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
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
    </div>
    <div class="px-4 py-3">
      {#if renaming}
        <input
          bind:value={draft}
          aria-label={`新名字 ${mod.name}`}
          class="w-full h-8 px-3 text-sm font-medium bg-transparent outline-none rounded-full"
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
        <p class="font-medium text-sm truncate">{mod.name}</p>
        <p class="text-xs text-secondary mt-0.5">
          {fmtSize(mod.size_bytes)} · {mod.file_count < 0 ? "—" : mod.file_count} 文件 · {fmtDate(mod.installed_at)}
        </p>
      {/if}
      {#if !renaming}
        <div class="flex items-center justify-between mt-2">
          <span class="text-[11px] text-secondary radius-pill px-2 py-0.5" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)">{catLabel}</span>
          <Toggle
            checked={mod.enabled}
            ariaLabel={`启用 ${mod.name}`}
            onchange={(next) => ontoggle(next)}
          />
        </div>
      {/if}
    </div>
  {/if}
</div>
