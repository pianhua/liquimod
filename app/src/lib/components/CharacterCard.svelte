<script lang="ts">
  import { getCachedCharacterImage, resolveCharacterImage, type CharacterSummary } from "$lib/api";
  import { IconHeart } from "$lib/components/icons";

  let {
    character,
    warnMultipleEnabled = true,
    onclick,
    onmenu,
    ontogglefavorite,
  }: {
    character: CharacterSummary;
    warnMultipleEnabled?: boolean;
    onclick: () => void;
    onmenu?: (e: MouseEvent, character: CharacterSummary) => void;
    ontogglefavorite?: (character: CharacterSummary) => void;
  } = $props();

  let customSrc = $state<string | null>(null);

  let displaySrc = $derived(
    customSrc || (character.image ? (getCachedCharacterImage(character.image) || `/images/${character.image}`) : "")
  );

  $effect(() => {
    let active = true;
    const imgName = character.image;
    if (imgName) {
      resolveCharacterImage(imgName, "Honkai").then((src) => {
        if (active && src) customSrc = src;
      });
    }
    return () => {
      active = false;
    };
  });

  // iOS 信号灯：启用 = 苹果翠绿；多启用 = 琥珀暖黄；未启用 = 柔和灰
  let dot = $derived(
    character.enabled === 1
      ? { color: "#34c759", glow: "0 0 6px rgba(52,199,89,0.9)" }
      : character.enabled >= 2 && warnMultipleEnabled
        ? { color: "#ffd60a", glow: "0 0 6px rgba(255,214,10,0.9)" }
        : character.enabled >= 1
          ? { color: "#34c759", glow: "0 0 6px rgba(52,199,89,0.9)" }
        : { color: "#9b9ba2", glow: "none" },
  );

  let cardEl = $state<HTMLElement | null>(null);
  let isHovered = $state(false);
  let rotateX = $state(0);
  let rotateY = $state(0);
  let shineX = $state(50);
  let shineY = $state(50);
  let rafId: number | null = null;

  function handleFavoriteClick(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    if (ontogglefavorite) {
      ontogglefavorite(character);
    }
  }

  function handlePointerMove(e: PointerEvent) {
    if (!cardEl) return;
    const rect = cardEl.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    // 归一化到 [-1, 1] 区间
    const nx = (x / rect.width - 0.5) * 2;
    const ny = (y / rect.height - 0.5) * 2;

    // 最大微倾斜角度（度数，柔和优雅）
    const maxTilt = 5.5;

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
</script>

<div
  bind:this={cardEl}
  role="button"
  tabindex="0"
  class="group relative radius-card overflow-hidden cursor-pointer select-none outline-none focus-visible:outline-2 focus-visible:outline-[var(--accent)] will-change-transform flex flex-col justify-end"
  style="
    perspective: 800px;
    transform-style: preserve-3d;
    border: 1px solid var(--glass-stroke);
    transform: {isHovered
      ? `perspective(800px) rotateX(${rotateX.toFixed(2)}deg) rotateY(${rotateY.toFixed(2)}deg) translateY(-3px) scale3d(1.02, 1.02, 1.02)`
      : 'perspective(800px) rotateX(0deg) rotateY(0deg) translateY(0px) scale3d(1, 1, 1)'};
    transition: {isHovered
      ? 'transform 0.08s ease-out, box-shadow 0.2s ease-out'
      : 'transform 0.5s cubic-bezier(0.16, 1, 0.3, 1), box-shadow 0.5s cubic-bezier(0.16, 1, 0.3, 1)'};
    box-shadow: {isHovered
      ? `${(-rotateY * 1.2).toFixed(1)}px ${(rotateX * 1.2 + 8).toFixed(1)}px 20px rgba(0, 0, 0, 0.22), 0 16px 36px rgba(0, 0, 0, 0.28)`
      : '0 4px 10px rgba(0, 0, 0, 0.1), 0 12px 24px rgba(0, 0, 0, 0.14)'};
  "
  onpointermove={handlePointerMove}
  onpointerleave={handlePointerLeave}
  onpointercancel={handlePointerLeave}
  {onclick}
  oncontextmenu={(e) => {
    if (onmenu) {
      e.preventDefault();
      e.stopPropagation();
      onmenu(e, character);
    }
  }}
  onkeydown={(e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onclick();
    }
  }}
>
  <!-- 1. 全幅无界立绘（精准 100% 满铺裁切，彻底消除任何缝隙） -->
  {#if character.image}
    <img
      src={displaySrc}
      alt={character.display_name}
      class="absolute inset-0 w-full h-full object-cover object-top pointer-events-none will-change-transform"
      style="
        transform: {isHovered
          ? `translate3d(${(-rotateY * 0.45).toFixed(1)}px, ${(rotateX * 0.45).toFixed(1)}px, 0px) scale(1.06)`
          : 'translate3d(0, 0, 0) scale(1.01)'};
        transition: {isHovered ? 'transform 0.08s ease-out' : 'transform 0.5s cubic-bezier(0.16, 1, 0.3, 1)'};
      "
      loading="lazy"
      draggable="false"
      onerror={(e) => {
        const img = e.currentTarget as HTMLImageElement;
        if (img && !img.dataset.fallback) {
          img.dataset.fallback = "1";
          img.src = "/images/Others.png";
        }
      }}
    />
  {:else}
    <div
      class="absolute inset-0 grid place-items-center text-4xl font-bold text-secondary opacity-30 pointer-events-none rounded-[inherit]"
      style="background: rgba(255, 255, 255, 0.05)"
    >
      {character.display_name.slice(0, 1)}
    </div>
  {/if}

  <!-- 2. 全息柔和流光反光层（大范围羽化次表面微光，告别硬白光斑，暗色极度自然） -->
  <div
    class="absolute inset-0 pointer-events-none rounded-[inherit] z-10 transition-opacity duration-500 ease-out"
    style="
      opacity: {isHovered ? 1 : 0};
      background: radial-gradient(ellipse 260px 200px at {shineX.toFixed(1)}% {shineY.toFixed(1)}%, rgba(255, 255, 255, 0.16) 0%, rgba(255, 255, 255, 0.04) 50%, transparent 80%);
      mix-blend-mode: soft-light;
    "
  ></div>

  <!-- 3. 右上角：高对比度晶体喜爱置顶按钮（黑白背景下均 100% 清晰可见） -->
  <button
    class="absolute top-2.5 right-2.5 z-20 w-7 h-7 rounded-full flex items-center justify-center backdrop-blur-md transition-all cursor-pointer focus-visible:opacity-100 focus-visible:outline-2 focus-visible:outline-[var(--accent)] group-focus-within:opacity-100 shadow-[0_2px_8px_rgba(0,0,0,0.35)] {character.is_favorite
      ? 'opacity-100 scale-100'
      : 'opacity-0 group-hover:opacity-100 hover:scale-110'}"
    style={character.is_favorite
      ? "background: linear-gradient(135deg, #ff2d55 0%, #e11d48 100%); color: #ffffff; box-shadow: 0 2px 10px rgba(244, 63, 94, 0.5);"
      : "background: rgba(0, 0, 0, 0.55); border: 1px solid rgba(255, 255, 255, 0.3); color: rgba(255, 255, 255, 0.95);"}
    title={character.is_favorite ? "取消喜爱" : "标为喜爱（置顶）"}
    onclick={handleFavoriteClick}
  >
    <IconHeart size={13} class={character.is_favorite ? "text-white fill-white" : "text-white"} />
  </button>

  <!-- 4. 自然柔和暗部过渡（纯透明渐变，保证文字清晰可读且不破坏立绘） -->
  <div
    class="absolute inset-x-0 bottom-0 h-20 pointer-events-none rounded-b-[inherit] bg-gradient-to-t from-black/75 via-black/30 to-transparent"
  ></div>

  <!-- 5. 底部自然透明文字排版（带内边距 p-2.5，随 3D 浮雕上浮） -->
  <div
    class="relative z-10 w-full flex items-center justify-between gap-1.5 p-2.5 transition-transform duration-100"
    style="transform: {isHovered ? 'translate3d(0, 0, 16px)' : 'translate3d(0, 0, 0)'};"
  >
    <!-- 左侧：呼吸灯 + 纯白立体文字 -->
    <div class="flex items-center gap-1.5 min-w-0 flex-1">
      <span
        class="w-2 h-2 rounded-full shrink-0 transition-transform duration-300 group-hover:scale-125"
        title={character.enabled > 0 ? `${character.enabled} 个 Mod 启用中` : "没有启用的 Mod"}
        style:background={dot.color}
        style:box-shadow={dot.glow}
      ></span>
      <span class="text-xs font-bold tracking-tight truncate text-white drop-shadow-[0_1.5px_3px_rgba(0,0,0,0.85)]">
        {character.display_name}
      </span>
    </div>

    <!-- 右侧：方案1：纯净无框双重微物理字影浮雕排版（告别生硬黑框，任何背景下均 100% 清晰） -->
    {#if character.total > 0}
      <span
        class="text-[11px] font-mono tracking-tight font-semibold shrink-0 select-none text-white/95 pr-0.5"
        style="text-shadow: 0 1px 2px rgba(0, 0, 0, 0.95), 0 0 6px rgba(0, 0, 0, 0.85);"
      >{character.enabled}/{character.total}</span>
    {/if}
  </div>
</div>
