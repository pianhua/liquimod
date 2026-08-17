<script lang="ts">
  import { portraitUrl, type CharacterSummary } from "$lib/api";

  let {
    character,
    onclick,
  }: { character: CharacterSummary; onclick: () => void } = $props();
</script>

<div
  role="button"
  tabindex="0"
  class="radius-card relative overflow-hidden cursor-pointer transition-all duration-200 hover:scale-[1.03] hover:-translate-y-0.5 active:scale-[0.98] outline-none"
  style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke), var(--shadow-soft)"
  {onclick}
  onkeydown={(e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onclick();
    }
  }}
>
  <div class="w-full" style="padding-top: 100%"></div>
  {#if character.image}
    <img
      src={portraitUrl(character.image)}
      alt={character.display_name}
      class="absolute inset-0 w-full h-full object-cover object-top"
      loading="lazy"
      draggable="false"
    />
  {:else}
    <div class="glass absolute inset-0 grid place-items-center text-4xl font-bold text-secondary">
      {character.display_name.slice(0, 1)}
    </div>
  {/if}
  <div class="absolute inset-x-0 bottom-0 h-2/5 bg-gradient-to-t from-black/55 via-black/20 to-transparent pointer-events-none"></div>
  <div class="absolute bottom-2.5 inset-x-2.5 flex items-end justify-between gap-1.5 pointer-events-none">
    <span class="glass radius-pill px-3 py-1 text-[13px] font-medium text-white truncate">
      {character.display_name}
    </span>
    {#if character.total > 0}
      <span class="glass radius-pill px-2 py-1 text-[11px] text-white shrink-0">
        {character.enabled}/{character.total}
      </span>
    {/if}
  </div>
</div>
