<script lang="ts">
  import { getCachedCharacterImage, resolveCharacterImage, type CharacterSummary } from "$lib/api";

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

  function handleFavoriteClick(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    if (ontogglefavorite) {
      ontogglefavorite(character);
    }
  }
</script>

<div
  role="button"
  tabindex="0"
  class="group relative radius-card overflow-hidden cursor-pointer select-none card-lift outline-none focus-visible:outline-2 focus-visible:outline-[var(--accent)] focus-visible:outline-offset-2 flex flex-col justify-end p-3"
  style="background: var(--glass-tint); border: 0.5px solid var(--glass-stroke); box-shadow: var(--glass-rim), var(--shadow-soft); content-visibility: auto; contain-intrinsic-size: 180px 200px; contain: layout style paint"
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
  <!-- 右上角：喜爱置顶按钮 -->
  <button
    class="absolute top-2.5 right-2.5 z-20 w-8 h-8 glass radius-pill flex items-center justify-center backdrop-blur-md transition-all cursor-pointer focus-visible:opacity-100 focus-visible:outline-2 focus-visible:outline-[var(--accent)] group-focus-within:opacity-100 {character.is_favorite ? 'opacity-100 scale-100' : 'opacity-0 group-hover:opacity-100 hover:scale-110'}"
    style={character.is_favorite
      ? "background: rgba(255, 45, 85, 0.85); color: #fff; box-shadow: 0 2px 8px rgba(255, 45, 85, 0.4)"
      : "color: rgba(255,255,255,0.9)"}
    title={character.is_favorite ? "取消喜爱" : "标为喜爱（置顶）"}
    onclick={handleFavoriteClick}
  >
    <svg width="14" height="14" viewBox="0 0 24 24" fill={character.is_favorite ? "currentColor" : "none"} stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"/>
    </svg>
  </button>
  <!-- 1. 全幅底层立绘（支持动态热更新立绘 + 三级防裂图降级） -->
  {#if character.image}
    <img
      src={displaySrc}
      alt={character.display_name}
      class="absolute inset-0 w-full h-full object-cover object-top transition-transform duration-500 ease-out group-hover:scale-108"
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
      class="absolute inset-0 grid place-items-center text-4xl font-bold text-secondary opacity-30"
      style="background: var(--glass-tint)"
    >
      {character.display_name.slice(0, 1)}
    </div>
  {/if}

  <!-- 2. Apple 级自然渐变遮罩（无生硬横条，底部柔和暗部） -->
  <div
    class="absolute inset-x-0 bottom-0 h-28 pointer-events-none bg-gradient-to-t from-black/80 via-black/35 to-transparent transition-opacity duration-300 group-hover:from-black/90"
  ></div>

  <!-- 3. 全沉浸海报文字排版（自然融于画面） -->
  <div class="relative z-10 w-full flex items-center justify-between gap-1.5 drop-shadow">
    <!-- 左侧：呼吸灯 + 纯白高对比度文字 -->
    <div class="flex items-center gap-1.5 min-w-0 flex-1">
      <span
        class="w-2 h-2 rounded-full shrink-0 transition-transform duration-300 group-hover:scale-125"
        title={character.enabled > 0 ? `${character.enabled} 个 Mod 启用中` : "没有启用的 Mod"}
        style:background={dot.color}
        style:box-shadow={dot.glow}
      ></span>
      <span class="text-[13px] font-bold tracking-tight truncate text-white drop-shadow-sm">
        {character.display_name}
      </span>
    </div>

    <!-- 右侧：超轻薄极简数量气泡 -->
    {#if character.total > 0}
      <span
        class="text-[10px] font-semibold font-mono px-1.5 py-0.5 rounded-full shrink-0 backdrop-blur-md"
        style="background: rgba(255, 255, 255, 0.2); color: #ffffff; box-shadow: inset 0 0 0 0.5px rgba(255, 255, 255, 0.3)"
      >
        {character.enabled}/{character.total}
      </span>
    {/if}
  </div>
</div>
