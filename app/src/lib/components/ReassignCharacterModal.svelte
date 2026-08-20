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
    class="glass radius-panel w-full max-w-md p-6 flex flex-col gap-4 shadow-2xl relative"
    style="box-shadow: var(--shadow-lift);"
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
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <line x1="18" y1="6" x2="6" y2="18" />
          <line x1="6" y1="6" x2="18" y2="18" />
        </svg>
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
      <svg
        class="absolute left-3 top-2.5 text-secondary pointer-events-none"
        width="15"
        height="15"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <circle cx="11" cy="11" r="8" />
        <line x1="21" y1="21" x2="16.65" y2="16.65" />
      </svg>
    </div>

    <!-- 角色选择列表 -->
    <div class="flex flex-col gap-1 max-h-60 overflow-y-auto pr-1">
      {#if isNewCharacter()}
        <button
          type="button"
          class="flex items-center gap-3 px-3 py-2.5 rounded-xl text-left cursor-pointer transition-all border border-dashed border-[var(--accent)] bg-[var(--accent-fill)] text-[var(--accent)] font-medium"
          onclick={() => {
            selectedTarget = search.trim();
          }}
        >
          <div class="w-8 h-8 rounded-full bg-[var(--accent)] text-white flex items-center justify-center font-bold text-xs shrink-0">
            ＋
          </div>
          <div class="flex-1 min-w-0">
            <div class="text-sm font-semibold truncate">新建角色「{search.trim()}」</div>
            <div class="text-xs opacity-80">创建新目录并移入此 Mod</div>
          </div>
        </button>
      {/if}

      {#each filteredCharacters as char (char.internal_name)}
        {@const isCurrent = char.internal_name === currentCharacter}
        {@const isSelected = selectedTarget === char.internal_name}
        <button
          type="button"
          class="flex items-center gap-3 px-3 py-2 rounded-xl text-left cursor-pointer transition-all {isSelected ? 'bg-[var(--accent-fill)] text-[var(--accent)] font-semibold' : 'hover:bg-[var(--item-hover)] text-[var(--text)]'} {isCurrent ? 'opacity-50 cursor-not-allowed' : ''}"
          disabled={isCurrent}
          onclick={() => {
            if (!isCurrent) selectedTarget = char.internal_name;
          }}
        >
          <img
            src={char.image ? (getCachedCharacterImage(char.image) || `/images/${char.image}`) : "/images/Others.png"}
            alt=""
            class="w-8 h-8 rounded-full object-cover object-top shrink-0 bg-black/10"
            onerror={(e) => {
              const img = e.currentTarget as HTMLImageElement;
              if (img && !img.dataset.fallback) {
                img.dataset.fallback = "1";
                img.src = "/images/Others.png";
              }
            }}
          />
          <div class="flex-1 min-w-0">
            <div class="text-sm font-medium truncate flex items-center gap-1.5">
              <span>{char.display_name}</span>
              {#if char.display_name !== char.internal_name}
                <span class="text-xs text-secondary font-normal">({char.internal_name})</span>
              {/if}
            </div>
            <div class="text-xs text-secondary truncate">{char.total} 个现有 Mod</div>
          </div>
          {#if isCurrent}
            <span class="text-xs text-secondary shrink-0">当前归属</span>
          {:else if isSelected}
            <svg class="text-[var(--accent)] shrink-0" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="20 6 9 17 4 12" />
            </svg>
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
