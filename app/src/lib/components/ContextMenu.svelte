<script lang="ts">
  import { onMount } from "svelte";
  import {
    IconHeart,
    IconHeartOff,
    IconUser,
    IconPower,
    IconPowerOff,
    IconFolderOpen,
    IconTag,
    IconPencil,
    IconTrash,
    IconChevronRight,
  } from "$lib/components/icons";

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
                  <IconHeart size={13} class="text-rose-500" />
                {:else if item.icon === "💔" || item.icon === "unfav"}
                  <IconHeartOff size={13} class="text-secondary" />
                {:else if item.icon === "🎯" || item.icon === "detail"}
                  <IconUser size={13} class="text-[var(--accent)]" />
                {:else if item.icon === "⚡" || item.icon === "enable"}
                  <IconPower size={13} class="text-amber-500" />
                {:else if item.icon === "🚫" || item.icon === "disable"}
                  <IconPowerOff size={13} class="text-secondary" />
                {:else if item.icon === "📂" || item.icon === "open"}
                  <IconFolderOpen size={13} class="text-sky-500" />
                {:else if item.icon === "🏷️" || item.icon === "tag"}
                  <IconTag size={13} class="text-indigo-400" />
                {:else if item.icon === "✏️" || item.icon === "rename"}
                  <IconPencil size={13} class="text-teal-400" />
                {:else if item.icon === "🗑️" || item.icon === "uninstall"}
                  <IconTrash size={13} class="text-rose-500" />
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
              <IconChevronRight size={10} class="text-secondary" />
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
