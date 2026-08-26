<script lang="ts">
  import type { CategoryDto } from "$lib/api";
  import {
    IconPower,
    IconPowerOff,
    IconTag,
    IconUser,
    IconTrash,
    IconClose,
  } from "$lib/components/icons";

  let {
    selectedCount,
    categories,
    destructiveLocked = false,
    onEnableAll,
    onDisableAll,
    onMoveCategory,
    onReassignCharacter,
    onUninstallAll,
    onClearSelection,
  }: {
    selectedCount: number;
    categories: CategoryDto[];
    destructiveLocked?: boolean;
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
    class="absolute bottom-5 left-1/2 -translate-x-1/2 z-30 flex items-center gap-2 p-1.5 glass-floating radius-pill shadow-2xl animate-slide-up select-none border border-[var(--glass-floating-stroke)] backdrop-blur-2xl whitespace-nowrap"
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
      <IconPower size={13} class="text-emerald-500" />
      <span>启用</span>
    </button>

    <!-- 批量禁用 -->
    <button
      type="button"
      class="h-8 px-3 text-xs font-medium rounded-full cursor-pointer flex items-center gap-1.5 transition-all hover:bg-[var(--item-hover)] text-secondary hover:text-[var(--text)] active:scale-95"
      title="禁用所有选中的 Mod"
      onclick={onDisableAll}
    >
      <IconPowerOff size={13} />
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
        <IconTag size={13} />
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
      title={destructiveLocked ? "游戏运行期间暂不支持重新分配角色" : "批量重新分配所属角色"}
      disabled={destructiveLocked}
      onclick={onReassignCharacter}
    >
      <IconUser size={13} />
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
        title={destructiveLocked ? "游戏运行期间暂不支持卸载" : "批量卸载所有选中的 Mod"}
        disabled={destructiveLocked}
        onclick={() => (isConfirmingDelete = true)}
      >
        <IconTrash size={13} class="text-red-400" />
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
      <IconClose size={12} />
    </button>
  </div>
{/if}
