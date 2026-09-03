<script lang="ts">
  import type { CategoryDto, ModDto } from "$lib/api";
  import Toggle from "./Toggle.svelte";
  import { IconStar, IconLink, IconFolder, IconSparkles } from "$lib/components/icons";

  let {
    mod,
    categories,
    selected = false,
    checked = false,
    isMultiSelectMode = false,
    mutationLocked = false,
    ontoggle,
    ontogglefavorite,
    onselect,
    oncheck,
    onmenu,
  }: {
    mod: ModDto;
    categories: CategoryDto[];
    selected?: boolean;
    checked?: boolean;
    isMultiSelectMode?: boolean;
    mutationLocked?: boolean;
    ontoggle: (next: boolean) => void;
    ontogglefavorite?: () => void;
    onselect?: (e: MouseEvent) => void;
    oncheck?: (checked: boolean) => void;
    onmenu?: (e: MouseEvent, mod: ModDto) => void;
  } = $props();

  let imgError = $state(false);
  let isExternal = $derived(mod.storage_kind === "external");
  let sourceOffline = $derived(isExternal && mod.source_available === false);

  function fmtSize(b: number): string {
    if (b < 0) return "—";
    if (b < 1024) return `${b} B`;
    if (b < 1024 * 1024) return `${(b / 1024).toFixed(0)} KB`;
    if (b < 1024 * 1024 * 1024) return `${(b / 1024 / 1024).toFixed(1)} MB`;
    return `${(b / 1024 / 1024 / 1024).toFixed(2)} GB`;
  }

  let catLabel = $derived.by(() => {
    if (mod.category_id == null) return "角色";
    const c = categories.find((x) => x.id === mod.category_id);
    return c ? c.name : "未分类";
  });

  // 3D 柔和物理倾斜与次表面流光层（完全参考 CharacterCard 高级交互）
  let cardEl = $state<HTMLElement | null>(null);
  let isHovered = $state(false);
  let rotateX = $state(0);
  let rotateY = $state(0);
  let shineX = $state(50);
  let shineY = $state(50);
  let rafId: number | null = null;

  function handlePointerMove(e: PointerEvent) {
    if (!cardEl) return;
    const rect = cardEl.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    const nx = (x / rect.width - 0.5) * 2;
    const ny = (y / rect.height - 0.5) * 2;
    const maxTilt = 4.5;

    if (rafId) cancelAnimationFrame(rafId);
    rafId = requestAnimationFrame(() => {
      rotateX = -ny * maxTilt;
      rotateY = nx * maxTilt;
      shineX = (x / rect.width) * 100;
      shineY = (y / rect.height) * 100;
      isHovered = true;
    });
  }

  function handlePointerLeave() {
    if (rafId) cancelAnimationFrame(rafId);
    rotateX = 0;
    rotateY = 0;
    isHovered = false;
  }

  function onCardClick(e: MouseEvent) {
    if ((e.target as HTMLElement).closest("button, input, select, a")) return;
    onselect?.(e);
  }

  function onCardKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === " ") {
      if ((e.target as HTMLElement).closest("button, input, select, a")) return;
      e.preventDefault();
      onselect?.(e as unknown as MouseEvent);
    }
  }

  function handleFavoriteClick(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    ontogglefavorite?.();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_no_noninteractive_tabindex -->
<div
  bind:this={cardEl}
  role="listitem"
  tabindex="0"
  aria-label={mod.name}
  data-selected={selected}
  class="group relative radius-card overflow-hidden cursor-pointer select-none outline-none focus-visible:outline-2 focus-visible:outline-[var(--accent)] will-change-transform flex flex-col justify-end"
  style="
    perspective: 800px;
    transform-style: preserve-3d;
    border: {selected ? '2px solid var(--accent)' : '1px solid var(--glass-stroke)'};
    transform: {isHovered
      ? `perspective(800px) rotateX(${rotateX.toFixed(2)}deg) rotateY(${rotateY.toFixed(2)}deg) translateY(-3px) scale3d(1.02, 1.02, 1.02)`
      : 'perspective(800px) rotateX(0deg) rotateY(0deg) translateY(0px) scale3d(1, 1, 1)'};
    transition: {isHovered
      ? 'transform 0.08s ease-out, box-shadow 0.2s ease-out'
      : 'transform 0.5s cubic-bezier(0.16, 1, 0.3, 1), box-shadow 0.5s cubic-bezier(0.16, 1, 0.3, 1)'};
    box-shadow: {selected
      ? '0 0 0 2px var(--accent), 0 8px 24px rgba(59, 130, 246, 0.35)'
      : isHovered
        ? `${(-rotateY * 1.2).toFixed(1)}px ${(rotateX * 1.2 + 8).toFixed(1)}px 20px rgba(0, 0, 0, 0.22), 0 16px 36px rgba(0, 0, 0, 0.28)`
        : '0 4px 10px rgba(0, 0, 0, 0.1), 0 12px 24px rgba(0, 0, 0, 0.14)'};
  "
  onpointermove={handlePointerMove}
  onpointerleave={handlePointerLeave}
  onpointercancel={handlePointerLeave}
  onclick={onCardClick}
  onkeydown={onCardKeydown}
  oncontextmenu={(e) => {
    if (onmenu) {
      e.preventDefault();
      e.stopPropagation();
      onmenu(e, mod);
    }
  }}
>
  <!-- 1. 全幅无界立绘/截图（100% 满铺裁切，彻底告别死板白色底框） -->
  {#if mod.thumb && !imgError}
    <img
      src={mod.thumb}
      alt={mod.name}
      class="absolute inset-0 w-full h-full object-cover object-center pointer-events-none will-change-transform"
      style="
        transform: {isHovered
          ? `translate3d(${(-rotateY * 0.4).toFixed(1)}px, ${(rotateX * 0.4).toFixed(1)}px, 0px) scale(1.06)`
          : 'translate3d(0, 0, 0) scale(1.01)'};
        transition: {isHovered ? 'transform 0.08s ease-out' : 'transform 0.5s cubic-bezier(0.16, 1, 0.3, 1)'};
      "
      draggable="false"
      onerror={() => (imgError = true)}
    />
  {:else}
    <div
      class="absolute inset-0 grid place-items-center relative overflow-hidden pointer-events-none rounded-[inherit]"
      style="background: linear-gradient(135deg, rgba(255,255,255,0.06) 0%, rgba(255,255,255,0.02) 100%)"
    >
      <div class="flex flex-col items-center gap-1.5 text-secondary/60">
        <IconFolder size={36} class="opacity-50" />
        <span class="text-sm font-semibold tracking-wide">{mod.name.slice(0, 1)}</span>
      </div>
    </div>
  {/if}

  <!-- 2. 全息柔和流光反光层（次表面微光，极度自然透明） -->
  <div
    class="absolute inset-0 pointer-events-none rounded-[inherit] z-10 transition-opacity duration-500 ease-out"
    style="
      opacity: {isHovered ? 1 : 0};
      background: radial-gradient(ellipse 260px 200px at {shineX.toFixed(1)}% {shineY.toFixed(1)}%, rgba(255, 255, 255, 0.16) 0%, rgba(255, 255, 255, 0.04) 50%, transparent 80%);
      mix-blend-mode: soft-light;
    "
  ></div>

  <!-- 3. 顶部浮层：多选勾选框与外部/变体角标 -->
  <div class="absolute top-2.5 left-2.5 z-20 flex items-center gap-1.5 pointer-events-auto">
    <!-- 多选 Checkbox -->
    <button
      type="button"
      class="w-6 h-6 rounded-md flex items-center justify-center shrink-0 transition-all cursor-pointer backdrop-blur-md focus-visible:opacity-100 focus-visible:outline-2 focus-visible:outline-[var(--accent)] {checked
        ? 'bg-[var(--accent)] text-white shadow-sm'
        : 'border border-white/35 hover:border-white bg-black/45 text-transparent opacity-0 group-hover:opacity-100 group-focus-within:opacity-100'} {isMultiSelectMode ? '!opacity-100' : ''}"
      onclick={(e) => {
        e.stopPropagation();
        oncheck?.(!checked);
      }}
      title={checked ? "取消选择" : "多选"}
      aria-label="选择此 Mod"
    >
      {#if checked}
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="20 6 9 17 4 12"/>
        </svg>
      {/if}
    </button>

    {#if isExternal}
      <span
        class="px-1.5 py-0.5 radius-pill text-[9px] font-medium flex items-center gap-0.5 backdrop-blur-md shadow-sm {sourceOffline
          ? 'bg-rose-500/50 text-rose-100 border border-rose-400/60'
          : 'bg-black/55 text-amber-300 border border-amber-400/50'}"
        title={sourceOffline ? "外部源当前离线不可用" : "外部关联 Mod"}
      >
        <IconLink size={9} />
        <span>外部</span>
      </span>
    {/if}

    {#if mod.active_variant}
      <span
        class="px-1.5 py-0.5 radius-pill text-[9px] font-medium flex items-center gap-0.5 bg-black/55 text-purple-300 border border-purple-400/50 backdrop-blur-md shadow-sm"
        title={`当前变体：${mod.active_variant}`}
      >
        <IconSparkles size={9} />
        <span>{mod.active_variant}</span>
      </span>
    {/if}
  </div>

  <!-- 4. 右上角：高对比度晶体喜爱置顶按钮（完全对齐 CharacterCard） -->
  <button
    type="button"
    class="absolute top-2.5 right-2.5 z-20 w-7 h-7 rounded-full flex items-center justify-center backdrop-blur-md transition-all cursor-pointer focus-visible:opacity-100 focus-visible:outline-2 focus-visible:outline-[var(--accent)] group-focus-within:opacity-100 shadow-[0_2px_8px_rgba(0,0,0,0.35)] {mod.is_favorite
      ? 'opacity-100 scale-100'
      : 'opacity-0 group-hover:opacity-100 hover:scale-110'}"
    style={mod.is_favorite
      ? "background: linear-gradient(135deg, #ff2d55 0%, #e11d48 100%); color: #ffffff; box-shadow: 0 2px 10px rgba(244, 63, 94, 0.5);"
      : "background: rgba(0, 0, 0, 0.55); border: 1px solid rgba(255, 255, 255, 0.3); color: rgba(255, 255, 255, 0.95);"}
    title={mod.is_favorite ? "取消喜爱" : "标为喜爱（置顶）"}
    aria-label={mod.is_favorite ? "取消喜爱" : "标为喜爱"}
    onclick={handleFavoriteClick}
  >
    <IconStar size={13} filled={Boolean(mod.is_favorite)} class="text-white" />
  </button>

  <!-- 5. 自然柔和暗部过渡（纯透明物理黑渐变，完全融入立绘与暗部） -->
  <div
    class="absolute inset-x-0 bottom-0 h-28 pointer-events-none rounded-b-[inherit] bg-gradient-to-t from-black/90 via-black/45 to-transparent"
  ></div>

  <!-- 6. 底部自然悬浮文字与控制排版（随 3D 浮雕上浮，纯白立体字影） -->
  <div
    class="relative z-10 w-full p-2.5 flex flex-col gap-1.5 transition-transform duration-100"
    style="transform: {isHovered ? 'translate3d(0, 0, 14px)' : 'translate3d(0, 0, 0)'};"
  >
    <!-- 标题行：左侧呼吸状态灯 + Mod 标题 -->
    <div class="flex items-center gap-1.5 min-w-0">
      <span
        class="w-2 h-2 rounded-full shrink-0 transition-transform duration-300 group-hover:scale-125"
        style:background={mod.enabled ? "#34c759" : "#9b9ba2"}
        style:box-shadow={mod.enabled ? "0 0 6px rgba(52,199,89,0.9)" : "none"}
        title={mod.enabled ? "已启用" : "未启用"}
      ></span>
      <span
        class="text-xs font-bold tracking-tight truncate text-white drop-shadow-[0_1.5px_3px_rgba(0,0,0,0.9)] flex-1 min-w-0"
        title={mod.name}
      >
        {mod.name}
      </span>
    </div>

    <!-- 辅助信息与微晶启闭开关 -->
    <div class="flex items-center justify-between gap-1 text-[11px] text-white/80">
      <span
        class="truncate font-mono text-[10px] text-white/85"
        style="text-shadow: 0 1px 2px rgba(0, 0, 0, 0.95);"
      >
        {fmtSize(mod.size_bytes)} · {catLabel}
      </span>

      <div onclick={(e) => e.stopPropagation()} role="presentation" class="shrink-0 scale-90 -mr-1">
        <Toggle
          checked={mod.enabled}
          disabled={sourceOffline}
          ariaLabel={`启用 ${mod.name}`}
          onchange={(next) => ontoggle(next)}
        />
      </div>
    </div>
  </div>
</div>
