<script lang="ts">
  import { onMount } from "svelte";
  import { pushEscHandler } from "$lib/esc";
  import { toast } from "$lib/toast.svelte";
  import {
    api,
    getCachedCharacterImage,
    type CharacterSummary,
    type ModDto,
  } from "$lib/api";
  import { IconClose, IconSearch, IconCheckCircle } from "$lib/components/icons";

  let {
    mod,
    currentCharacter,
    characters,
    onClose,
    onReassigned,
  }: {
    mod: ModDto;
    currentCharacter: string;
    characters: CharacterSummary[];
    onClose: () => void;
    onReassigned: (newCharacter: string) => void;
  } = $props();

  let search = $state("");
  let selectedTarget = $state<string | null>(null);
  let isSubmitting = $state(false);
  let inputEl = $state<HTMLInputElement | null>(null);

  // 过滤后的角色列表
  let filteredCharacters = $derived(
    characters.filter((c) => {
      const q = search.trim().toLowerCase();
      if (!q) return true;
      return (
        c.internal_name.toLowerCase().includes(q) ||
        c.display_name.toLowerCase().includes(q)
      );
    }),
  );

  // 是否允许新建角色（当搜索词非空，且现有角色中无完全匹配时）
  let isNewCharacter = $derived(() => {
    const q = search.trim();
    if (!q) return false;
    return !characters.some(
      (c) =>
        c.internal_name.toLowerCase() === q.toLowerCase() ||
        c.display_name.toLowerCase() === q.toLowerCase(),
    );
  });

  onMount(() => {
    inputEl?.focus();
    const popEsc = pushEscHandler(() => {
      onClose();
      return true;
    });
    return () => popEsc();
  });

  async function handleConfirm() {
    const target = selectedTarget ?? (isNewCharacter() ? search.trim() : null);
    if (!target) return;
    if (target === currentCharacter) {
      onClose();
      return;
    }
    isSubmitting = true;
    try {
      await api.reassignMod(mod.id, target);
      toast(`已成功将「${mod.name}」移动到角色【${target}】`);
      onReassigned(target);
      onClose();
    } catch (e) {
      toast(String(e));
    } finally {
      isSubmitting = false;
    }
  }
</script>

<div
  class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/40 backdrop-blur-sm animate-fade-in"
  role="dialog"
  aria-modal="true"
  aria-labelledby="reassign-modal-title"
>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div
    class="glass-floating radius-panel w-full max-w-md p-6 flex flex-col gap-4 shadow-2xl relative border border-[var(--glass-floating-stroke)]"
    style="box-shadow: var(--glass-floating-shadow);"
    onclick={(e) => e.stopPropagation()}
  >
    <!-- 标题与说明 -->
    <div class="flex items-center justify-between shrink-0">
      <div>
        <h3 id="reassign-modal-title" class="text-base font-bold tracking-tight">
          重新分配角色
        </h3>
        <p class="text-xs text-secondary mt-0.5">
          将「{mod.name}」从当前角色【{currentCharacter}】迁移至新角色
        </p>
      </div>
      <button
        class="w-7 h-7 flex items-center justify-center rounded-full text-secondary hover:text-[var(--text)] hover:bg-[var(--item-hover)] cursor-pointer transition-colors"
        onclick={onClose}
        title="关闭 (Esc)"
      >
        <IconClose size={14} />
      </button>
    </div>

    <!-- 搜索输入框 -->
    <div class="relative shrink-0">
      <input
        bind:this={inputEl}
        type="text"
        class="glass radius-pill w-full pl-9 pr-4 h-9 text-sm outline-none bg-transparent text-[var(--text)] placeholder:text-secondary focus:ring-1 focus:ring-[var(--accent)] transition-all"
        placeholder="搜索角色（中文/英文）或输入新角色名…"
        bind:value={search}
        onkeydown={(e) => {
          if (e.key === "Enter") {
            handleConfirm();
          }
        }}
      />
      <IconSearch
        class="absolute left-3 top-2.5 text-secondary pointer-events-none"
        size={15}
      />
    </div>

    <!-- 角色选择列表 -->
    <div class="flex-1 overflow-y-auto min-h-0 flex flex-col gap-1 pr-1">
      {#if isNewCharacter()}
        <button
          type="button"
          class="p-2.5 rounded-xl border flex items-center gap-3 text-left transition-colors cursor-pointer {selectedTarget === search.trim() ? 'border-[var(--accent)] bg-[var(--accent-fill)]' : 'border-dashed border-[var(--glass-stroke)] hover:bg-[var(--item-hover)]'}"
          onclick={() => (selectedTarget = search.trim())}
        >
          <div class="w-10 h-10 rounded-full bg-[var(--accent-fill)] border border-[var(--accent)] flex items-center justify-center text-lg font-bold text-[var(--accent)] shrink-0">
            +
          </div>
          <div class="flex-1 min-w-0">
            <div class="text-sm font-semibold truncate text-[var(--accent)]">
              新建角色「{search.trim()}」
            </div>
            <div class="text-xs text-secondary">
              将为此角色创建新的 Library 目录
            </div>
          </div>
        </button>
      {/if}

      {#each filteredCharacters as char (char.internal_name)}
        {@const isCurrent = char.internal_name === currentCharacter}
        {@const isSelected = selectedTarget === char.internal_name}
        <button
          type="button"
          class="p-2 rounded-xl flex items-center gap-3 text-left transition-colors cursor-pointer {isSelected ? 'bg-[var(--accent-fill)] ring-1 ring-[var(--accent)]' : 'hover:bg-[var(--item-hover)]'} {isCurrent ? 'opacity-50 cursor-not-allowed' : ''}"
          disabled={isCurrent}
          onclick={() => {
            if (!isCurrent) selectedTarget = char.internal_name;
          }}
        >
          {#if char.image}
            <img
              src={getCachedCharacterImage(char.image) || `/images/${char.image}`}
              alt={char.display_name}
              class="w-10 h-10 rounded-full object-cover object-top shrink-0 border border-[var(--glass-stroke)]"
              onerror={(e) => {
                const img = e.currentTarget as HTMLImageElement;
                if (img && !img.dataset.fallback) {
                  img.dataset.fallback = "1";
                  img.src = "/images/Others.png";
                }
              }}
            />
          {:else}
            <div class="w-10 h-10 rounded-full bg-[var(--surface)] border border-[var(--glass-stroke)] flex items-center justify-center text-sm font-bold text-secondary shrink-0">
              {char.display_name.slice(0, 1)}
            </div>
          {/if}
          <div class="flex-1 min-w-0">
            <div class="text-sm font-medium truncate flex items-center gap-1.5">
              <span>{char.display_name}</span>
              {#if char.internal_name !== char.display_name}
                <span class="text-xs text-secondary font-normal">({char.internal_name})</span>
              {/if}
            </div>
            <div class="text-xs text-secondary truncate">{char.total} 个现有 Mod</div>
          </div>
          {#if isCurrent}
            <span class="text-xs text-secondary shrink-0">当前归属</span>
          {:else if isSelected}
            <IconCheckCircle class="text-[var(--accent)] shrink-0" size={16} />
          {/if}
        </button>
      {/each}
      {#if filteredCharacters.length === 0 && !isNewCharacter()}
        <div class="py-8 text-center text-secondary text-sm">
          未找到匹配角色
        </div>
      {/if}
    </div>

    <!-- 底部动作栏 -->
    <div class="flex items-center justify-end gap-2 pt-2 border-t border-[var(--glass-stroke)]">
      <button
        type="button"
        class="glass radius-pill px-4 h-8 text-xs font-medium cursor-pointer transition-colors"
        onclick={onClose}
      >
        取消
      </button>
      <button
        type="button"
        class="accent-fill accent-text radius-pill px-4 h-8 text-xs font-medium cursor-pointer transition-transform active:scale-95 disabled:opacity-40 disabled:pointer-events-none"
        disabled={!selectedTarget && !isNewCharacter() || isSubmitting}
        onclick={handleConfirm}
      >
        {isSubmitting ? "正在迁移…" : "确认迁移"}
      </button>
    </div>
  </div>
</div>
