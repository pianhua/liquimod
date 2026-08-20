<script lang="ts">
  import { onMount } from "svelte";

  export interface MenuItem {
    id: string;
    label: string;
    icon?: string;
    shortcut?: string;
    danger?: boolean;
    disabled?: boolean;
    divider?: boolean;
    children?: MenuItem[];
    action?: () => void;
  }

  let {
    x,
    y,
    items,
    onclose,
  }: {
    x: number;
    y: number;
    items: MenuItem[];
    onclose: () => void;
  } = $props();

  let menuEl = $state<HTMLDivElement | null>(null);
  let activeSubmenuId = $state<string | null>(null);
  let adjustedX = $state<number | null>(null);
  let adjustedY = $state<number | null>(null);
  let submenuPlacement = $state<"right" | "left">("right");

  let computedX = $derived(adjustedX ?? x);
  let computedY = $derived(adjustedY ?? y);

  $effect(() => {
    if (menuEl) {
      const rect = menuEl.getBoundingClientRect();
      const pad = 10;
      let targetX = x;
      let targetY = y;
      if (targetX + rect.width > window.innerWidth - pad) {
        targetX = Math.max(pad, window.innerWidth - rect.width - pad);
      }
      if (targetY + rect.height > window.innerHeight - pad) {
        targetY = Math.max(pad, window.innerHeight - rect.height - pad);
      }
      adjustedX = targetX;
      adjustedY = targetY;

      if (targetX + rect.width + 160 > window.innerWidth - pad) {
        submenuPlacement = "left";
      } else {
        submenuPlacement = "right";
      }
    }
  });

  onMount(() => {
    // 广播关闭其他可能残留的右键菜单
    window.dispatchEvent(new CustomEvent("liquimod-close-contextmenu"));

    const handleClickOutside = (e: MouseEvent) => {
      if (menuEl && !menuEl.contains(e.target as Node)) {
        onclose();
      }
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onclose();
      }
    };

    const handleWindowBlur = () => {
      onclose();
    };

    const handleCloseOther = () => {
      onclose();
    };

    window.addEventListener("pointerdown", handleClickOutside, { capture: true });
    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("blur", handleWindowBlur);
    window.addEventListener("resize", onclose);
    window.addEventListener("liquimod-close-contextmenu", handleCloseOther);

    return () => {
      window.removeEventListener("pointerdown", handleClickOutside, { capture: true });
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("blur", handleWindowBlur);
      window.removeEventListener("resize", onclose);
      window.removeEventListener("liquimod-close-contextmenu", handleCloseOther);
    };
  });

  function handleItemClick(item: MenuItem, e: MouseEvent) {
    e.stopPropagation();
    if (item.disabled) return;
    if (item.children && item.children.length > 0) {
      activeSubmenuId = activeSubmenuId === item.id ? null : item.id;
      return;
    }
    onclose();
    item.action?.();
  }
</script>

<!-- 全局右键菜单浮层：采用高不透明度 glass-menu 确保遮盖底层复杂立绘 -->
<div
  bind:this={menuEl}
  role="menu"
  tabindex="-1"
  class="fixed z-50 glass-menu radius-panel p-1.5 min-w-[190px] shadow-2xl flex flex-col gap-0.5 select-none outline-none"
  style="left: {computedX}px; top: {computedY}px; transition: none !important;"
  oncontextmenu={(e) => e.preventDefault()}
>
  {#each items as item (item.id)}
    {#if item.divider}
      <div class="h-px my-1 bg-[var(--glass-stroke)] opacity-70"></div>
    {:else}
      <div class="relative">
        <button
          type="button"
          role="menuitem"
          class="w-full h-8 px-2.5 rounded-lg flex items-center justify-between text-xs font-semibold cursor-pointer transition-colors text-left disabled:opacity-40 disabled:cursor-not-allowed
            {item.danger
              ? 'text-[var(--danger)] hover:bg-[rgba(255,69,58,0.15)]'
              : 'hover:bg-[var(--item-hover)] text-[var(--text)]'}"
          disabled={item.disabled}
          onclick={(e) => handleItemClick(item, e)}
          onmouseenter={() => {
            if (item.children) activeSubmenuId = item.id;
            else activeSubmenuId = null;
          }}
        >
          <div class="flex items-center gap-2.5 min-w-0 flex-1">
            {#if item.icon}
              <div class="w-4 h-4 shrink-0 grid place-items-center">
                {#if item.icon === "💖" || item.icon === "fav"}
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="1.5" class="text-rose-500">
                    <path d="M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z"/>
                  </svg>
                {:else if item.icon === "💔" || item.icon === "unfav"}
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="text-secondary">
                    <path d="M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z"/>
                    <line x1="2" y1="2" x2="22" y2="22"/>
                  </svg>
                {:else if item.icon === "🎯" || item.icon === "detail"}
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="text-[var(--accent)]">
                    <circle cx="12" cy="12" r="10"/>
                    <circle cx="12" cy="12" r="6"/>
                    <circle cx="12" cy="12" r="2"/>
                  </svg>
                {:else if item.icon === "⚡" || item.icon === "enable"}
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="text-amber-500">
                    <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>
                  </svg>
                {:else if item.icon === "🚫" || item.icon === "disable"}
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="text-secondary">
                    <circle cx="12" cy="12" r="10"/>
                    <line x1="4.93" y1="4.93" x2="19.07" y2="19.07"/>
                  </svg>
                {:else if item.icon === "📂" || item.icon === "open"}
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="text-sky-500">
                    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
                  </svg>
                {:else if item.icon === "🏷️" || item.icon === "tag"}
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="text-indigo-400">
                    <path d="M12 2H2v10l9.29 9.29c.94.94 2.48.94 3.42 0l6.58-6.58c.94-.94.94-2.48 0-3.42L12 2Z"/>
                    <circle cx="7" cy="7" r="1" fill="currentColor"/>
                  </svg>
                {:else if item.icon === "✏️" || item.icon === "rename"}
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="text-teal-400">
                    <path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/>
                  </svg>
                {:else if item.icon === "🗑️" || item.icon === "uninstall"}
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="text-rose-500">
                    <path d="M3 6h18m-2 0v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6m3 0V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/>
                  </svg>
                {:else}
                  <span class="text-sm leading-none">{item.icon}</span>
                {/if}
              </div>
            {/if}
            <span class="truncate">{item.label}</span>
          </div>

          <div class="flex items-center gap-1.5 shrink-0 ml-2">
            {#if item.shortcut}
              <kbd class="text-[10px] font-mono px-1 py-0.5 rounded bg-[rgba(120,120,128,0.2)] text-secondary">
                {item.shortcut}
              </kbd>
            {/if}
            {#if item.children}
              <svg width="8" height="8" viewBox="0 0 10 10" fill="none" class="text-secondary">
                <path d="M3 1.5L6.5 5L3 8.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
            {/if}
          </div>
        </button>

        <!-- 二级子菜单：即时呈现，无飞入与残影 -->
        {#if item.children && activeSubmenuId === item.id}
          <div
            role="menu"
            tabindex="-1"
            class="absolute top-0 glass-menu radius-panel p-1.5 min-w-[150px] shadow-2xl flex flex-col gap-0.5 select-none outline-none {submenuPlacement === 'left' ? 'right-full mr-1' : 'left-full ml-1'}"
            style="transition: none !important;"
          >
            {#each item.children as sub (sub.id)}
              <button
                type="button"
                role="menuitem"
                class="w-full h-8 px-2.5 rounded-lg flex items-center gap-2 text-xs font-semibold cursor-pointer transition-colors text-left hover:bg-[var(--item-hover)] text-[var(--text)]"
                onclick={(e) => {
                  e.stopPropagation();
                  onclose();
                  sub.action?.();
                }}
              >
                {#if sub.icon}
                  <span class="text-sm shrink-0 w-4 text-center leading-none">{sub.icon}</span>
                {/if}
                <span class="truncate">{sub.label}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  {/each}
</div>
