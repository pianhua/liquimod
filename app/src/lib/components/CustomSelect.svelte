<script module lang="ts">
  import type { Component } from "svelte";

  export interface SelectOption<V> {
    value: V;
    label: string;
    icon?: Component<{ size?: number; class?: string }> | string;
  }
</script>

<script lang="ts" generics="T extends string | number | boolean | null">
  import { onMount } from "svelte";
  import { pushEscHandler, registerPopover, notifyPopoverOpened } from "$lib/esc";
  import { IconChevronDown, IconCheckCircle } from "$lib/components/icons";

  let {
    value = $bindable(),
    options,
    placeholder = "请选择",
    size = "sm",
    className = "",
    onChange,
  }: {
    value: T;
    options: SelectOption<T>[];
    placeholder?: string;
    size?: "xs" | "sm" | "md";
    className?: string;
    onChange?: (val: T) => void;
  } = $props();

  let open = $state(false);
  let focusedIndex = $state(-1);
  let hoveredIndex = $state(-1);
  let focusSource = $state<"keyboard" | "pointer" | "none">("none");
  let rootEl: HTMLDivElement | null = $state(null);

  let currentOption = $derived(options.find((o) => o.value === value));

  const closeSelf = () => {
    open = false;
  };

  $effect(() => {
    return registerPopover(closeSelf);
  });

  $effect(() => {
    if (open) {
      notifyPopoverOpened(closeSelf);
      const idx = options.findIndex((o) => o.value === value);
      focusedIndex = idx >= 0 ? idx : 0;
      hoveredIndex = -1;
      focusSource = "none";
      return pushEscHandler(() => {
        open = false;
        return true;
      });
    }
  });

  function toggleOpen(e: MouseEvent) {
    e.stopPropagation();
    open = !open;
  }

  function selectOption(opt: SelectOption<T>, e?: MouseEvent) {
    if (e) e.stopPropagation();
    value = opt.value;
    open = false;
    hoveredIndex = -1;
    focusSource = "none";
    onChange?.(opt.value);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!open) {
      if (e.key === "Enter" || e.key === " " || e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        open = true;
      }
      return;
    }

    if (e.key === "ArrowDown") {
      e.preventDefault();
      focusedIndex = (focusedIndex + 1) % options.length;
      focusSource = "keyboard";
      hoveredIndex = -1;
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      focusedIndex = (focusedIndex - 1 + options.length) % options.length;
      focusSource = "keyboard";
      hoveredIndex = -1;
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      if (focusedIndex >= 0 && focusedIndex < options.length) {
        selectOption(options[focusedIndex]);
      }
    } else if (e.key === "Tab") {
      open = false;
    }
  }

  onMount(() => {
    function onDocClick(e: MouseEvent) {
      if (rootEl && !rootEl.contains(e.target as Node)) {
        open = false;
      }
    }
    window.addEventListener("click", onDocClick);
    return () => {
      window.removeEventListener("click", onDocClick);
    };
  });

  const sizeClasses = {
    xs: "h-6.5 text-[11px] px-2.5 gap-1.5",
    sm: "h-7.5 text-xs px-3 gap-2",
    md: "h-8.5 text-sm px-3.5 gap-2.5",
  };
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  bind:this={rootEl}
  class="relative inline-flex items-center select-none {className}"
  onkeydown={handleKeydown}
  tabindex="-1"
  role="group"
>
  <!-- 主触发按钮胶囊 -->
  <button
    type="button"
    role="combobox"
    aria-expanded={open}
    aria-haspopup="listbox"
    aria-controls="custom-select-listbox"
    class="flex items-center justify-between radius-pill glass-liquid-btn border border-[var(--glass-stroke)] text-[var(--text)] transition-all cursor-pointer shadow-xs focus:outline-none focus:ring-1 focus:ring-[var(--accent)]/50 {sizeClasses[size]}"
    onclick={toggleOpen}
    style={open ? "border-color: var(--accent);" : ""}
  >
    <div class="flex items-center gap-1.5 truncate z-10">
      {#if currentOption?.icon}
        {#if typeof currentOption.icon === "string"}
          <span class="text-xs shrink-0">{currentOption.icon}</span>
        {:else}
          {@const IconComp = currentOption.icon}
          <span class="shrink-0 text-[var(--accent)] flex items-center"><IconComp size={13} /></span>
        {/if}
      {/if}
      <span class="truncate font-medium">{currentOption?.label ?? placeholder}</span>
    </div>

    <!-- 下拉指示箭头 -->
    <IconChevronDown
      size={12}
      class="shrink-0 transition-transform duration-200 ml-1.5 text-secondary z-10 {open ? 'rotate-180 text-[var(--accent)]' : ''}"
    />
  </button>

  <!-- 下拉悬浮菜单面板 (高保真黑曜玻璃卡片，绝对杜绝背景透字) -->
  {#if open}
    <div
      id="custom-select-listbox"
      class="absolute top-full left-0 mt-2 min-w-[150px] max-h-64 overflow-y-auto glass-popover p-1.5 z-50 flex flex-col gap-0.5 animate-slide-up"
      role="listbox"
      tabindex="-1"
      onmouseleave={() => {
        hoveredIndex = -1;
        if (focusSource === "pointer") focusSource = "none";
      }}
    >
      {#each options as opt, idx (opt.value ?? idx)}
        {@const isSelected = opt.value === value}
        {@const isFocused = idx === focusedIndex && focusSource === "keyboard"}
        {@const isHovered = idx === hoveredIndex}
        <button
          type="button"
          role="option"
          aria-selected={isSelected}
          class="flex items-center justify-between w-full px-3 py-2 rounded-xl text-xs text-left cursor-pointer transition-all {isSelected
            ? 'bg-[var(--accent-fill)] text-[var(--accent)] font-semibold'
            : isFocused || isHovered
            ? 'bg-[var(--item-hover)] text-[var(--text)] font-medium'
            : 'text-secondary hover:text-[var(--text)] hover:bg-[var(--item-hover)]'}"
          onclick={(e) => selectOption(opt, e)}
          onmouseenter={() => {
            hoveredIndex = idx;
            focusedIndex = idx;
            focusSource = "pointer";
          }}
        >
          <div class="flex items-center gap-2 truncate">
            {#if opt.icon}
              {#if typeof opt.icon === "string"}
                <span class="text-xs shrink-0">{opt.icon}</span>
              {:else}
                {@const IconComp = opt.icon}
                <span class="shrink-0 flex items-center {isSelected ? 'text-[var(--accent)]' : 'text-secondary'}"><IconComp size={13} /></span>
              {/if}
            {/if}
            <span class="truncate">{opt.label}</span>
          </div>

          {#if isSelected}
            <IconCheckCircle size={13} class="shrink-0 text-[var(--accent)]" />
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>
