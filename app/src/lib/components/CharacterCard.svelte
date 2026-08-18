<script lang="ts">
  import { portraitUrl, type CharacterSummary } from "$lib/api";

  let {
    character,
    onclick,
  }: { character: CharacterSummary; onclick: () => void } = $props();

  // 信号灯：恰好 1 个启用 = 绿；2 个及以上 = 黄；0 = 灰
  let dot = $derived(
    character.enabled === 1
      ? { color: "#34c759", glow: "0 0 6px rgba(52,199,89,0.9)" }
      : character.enabled >= 2
        ? { color: "#ffd60a", glow: "0 0 6px rgba(255,214,10,0.9)" }
        : { color: "rgba(142,142,147,0.65)", glow: "none" },
  );
</script>

<div
  role="button"
  tabindex="0"
  class="radius-card relative cursor-pointer transition-all duration-200 hover:scale-[1.03] hover:-translate-y-0.5 active:scale-[0.98] outline-none p-2 flex flex-col gap-2"
  style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke), var(--shadow-soft)"
  {onclick}
  onkeydown={(e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onclick();
    }
  }}
>
  <div class="relative w-full rounded-[14px] overflow-hidden" style="aspect-ratio: 1">
    {#if character.image}
      <img
        src={portraitUrl(character.image)}
        alt={character.display_name}
        class="absolute inset-0 w-full h-full object-cover object-top"
        loading="lazy"
        draggable="false"
      />
    {:else}
      <div class="absolute inset-0 grid place-items-center text-4xl font-bold text-secondary"
        style="background: var(--glass-tint)">
        {character.display_name.slice(0, 1)}
      </div>
    {/if}
  </div>
  <span
    class="absolute top-3.5 right-3.5 w-2.5 h-2.5 rounded-full z-10"
    title={character.enabled > 0 ? `${character.enabled} 个 Mod 启用中` : "没有启用的 Mod"}
    style:background={dot.color}
    style:box-shadow={dot.glow}
  ></span>
  <div class="glass radius-pill px-3 h-9 flex items-center justify-between gap-1.5 shrink-0">
    <span class="text-[13px] font-medium truncate">{character.display_name}</span>
    {#if character.total > 0}
      <span class="text-[11px] text-secondary shrink-0">{character.enabled}/{character.total}</span>
    {/if}
  </div>
</div>
