<script lang="ts">
  import type { CategoryDto } from "$lib/api";

  let {
    selectedCount,
    categories,
    onEnableAll,
    onDisableAll,
    onMoveCategory,
    onReassignCharacter,
    onUninstallAll,
    onClearSelection,
  }: {
    selectedCount: number;
    categories: CategoryDto[];
    onEnableAll: () => void;
    onDisableAll: () => void;
    onMoveCategory: (categoryId: number | null) => void;
    onReassignCharacter: () => void;
    onUninstallAll: () => void;
    onClearSelection: () => void;
  } = $props();

  let showCategoryMenu = $state(false);
  let isConfirmingDelete = $state(false);
</script>

{#if selectedCount > 0}
  <div
    class="fixed bottom-6 left-1/2 -translate-x-1/2 z-40 flex items-center gap-2 p-1.5 glass-floating radius-pill shadow-2xl animate-slide-up select-none border border-[var(--glass-floating-stroke)] backdrop-blur-2xl"
    style="box-shadow: var(--glass-floating-shadow);"
    role="toolbar"
    aria-label="批量操作栏"
  >
    <!-- 选中计数徽章 -->
    <div class="flex items-center gap-1.5 pl-3 pr-2 h-8 text-xs font-semibold text-[var(--accent)] shrink-0">
      <span class="w-2 h-2 rounded-full bg-[var(--accent)] animate-pulse"></span>
      <span>已选中 {selectedCount} 项</span>
    </div>

    <span class="w-[1px] h-4 bg-[var(--glass-stroke)] opacity-60"></span>

    <!-- 批量启用 -->
    <button
      type="button"
      class="h-8 px-3 text-xs font-medium rounded-full cursor-pointer flex items-center gap-1.5 transition-all hover:bg-[var(--item-hover)] text-emerald-500 active:scale-95"
      title="启用所有选中的 Mod"
      onclick={onEnableAll}
    >
      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>
      </svg>
      <span>启用</span>
    </button>

    <!-- 批量禁用 -->
    <button
      type="button"
      class="h-8 px-3 text-xs font-medium rounded-full cursor-pointer flex items-center gap-1.5 transition-all hover:bg-[var(--item-hover)] text-secondary hover:text-[var(--text)] active:scale-95"
      title="禁用所有选中的 Mod"
      onclick={onDisableAll}
    >
      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="10"/>
        <line x1="4.93" y1="4.93" x2="19.07" y2="19.07"/>
      </svg>
      <span>禁用</span>
    </button>

    <!-- 批量分类 -->
    <div class="relative">
      <button
        type="button"
        class="h-8 px-3 text-xs font-medium rounded-full cursor-pointer flex items-center gap-1.5 transition-all hover:bg-[var(--item-hover)] text-secondary hover:text-[var(--text)] active:scale-95"
        title="批量移动到分类"
        onclick={() => (showCategoryMenu = !showCategoryMenu)}
      >
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"/>
          <line x1="7" y1="7" x2="7.01" y2="7"/>
        </svg>
        <span>分类</span>
      </button>

      {#if showCategoryMenu}
        <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
        <div
          class="absolute bottom-full left-0 mb-2 z-50 glass radius-card p-1.5 min-w-[140px] shadow-2xl flex flex-col gap-0.5"
          onclick={(e) => e.stopPropagation()}
        >
          <button
            type="button"
            class="px-2.5 py-1.5 text-xs text-left rounded-lg cursor-pointer transition-colors hover:bg-[var(--item-hover)] text-[var(--text)]"
            onclick={() => {
              onMoveCategory(null);
              showCategoryMenu = false;
            }}
          >
            角色 (默认)
          </button>
          {#each categories as c (c.id)}
            <button
              type="button"
              class="px-2.5 py-1.5 text-xs text-left rounded-lg cursor-pointer transition-colors hover:bg-[var(--item-hover)] text-[var(--text)]"
              onclick={() => {
                onMoveCategory(c.id);
                showCategoryMenu = false;
              }}
            >
              {c.name}
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <!-- 批量分配角色 -->
    <button
      type="button"
      class="h-8 px-3 text-xs font-medium rounded-full cursor-pointer flex items-center gap-1.5 transition-all hover:bg-[var(--item-hover)] text-secondary hover:text-[var(--text)] active:scale-95"
      title="批量重新分配所属角色"
      onclick={onReassignCharacter}
    >
      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="10"/>
        <circle cx="12" cy="12" r="6"/>
        <circle cx="12" cy="12" r="2"/>
      </svg>
      <span>换角色</span>
    </button>

    <!-- 批量卸载 -->
    {#if isConfirmingDelete}
      <div class="flex items-center gap-1 bg-red-500/10 px-1.5 py-0.5 rounded-full border border-red-500/30">
        <span class="text-[11px] text-red-400 pl-1.5">确认卸载{selectedCount}项？</span>
        <button
          type="button"
          class="h-7 px-2.5 text-xs font-semibold bg-red-500 hover:bg-red-600 text-white rounded-full cursor-pointer transition-colors"
          onclick={() => {
            isConfirmingDelete = false;
            onUninstallAll();
          }}
        >
          确定
        </button>
        <button
          type="button"
          class="h-7 px-2 text-xs text-secondary hover:text-[var(--text)] rounded-full cursor-pointer transition-colors"
          onclick={() => (isConfirmingDelete = false)}
        >
          取消
        </button>
      </div>
    {:else}
      <button
        type="button"
        class="h-8 px-3 text-xs font-medium rounded-full cursor-pointer flex items-center gap-1.5 transition-all hover:bg-red-500/10 text-red-400 active:scale-95"
        title="批量卸载所有选中的 Mod"
        onclick={() => (isConfirmingDelete = true)}
      >
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="3 6 5 6 21 6"/>
          <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
        </svg>
        <span>卸载</span>
      </button>
    {/if}

    <span class="w-[1px] h-4 bg-[var(--glass-stroke)] opacity-60"></span>

    <!-- 取消选择 -->
    <button
      type="button"
      class="w-7 h-7 flex items-center justify-center rounded-full text-secondary hover:text-[var(--text)] hover:bg-[var(--item-hover)] cursor-pointer transition-colors"
      title="取消选择 (Esc)"
      onclick={onClearSelection}
    >
      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="18" y1="6" x2="6" y2="18"/>
        <line x1="6" y1="6" x2="18" y2="18"/>
      </svg>
    </button>
  </div>
{/if}
