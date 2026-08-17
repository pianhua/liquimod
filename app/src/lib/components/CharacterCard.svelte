<script lang="ts">
  import { portraitUrl, type CharacterSummary } from "$lib/api";

  let {
    character,
    onclick,
  }: { character: CharacterSummary; onclick: () => void } = $props();
</script>

<button
  class="radius-card relative overflow-hidden aspect-[3/4] group cursor-pointer"
  {onclick}
>
  {#if character.image}
    <img
      src={portraitUrl(character.image)}
      alt={character.display_name}
      class="absolute inset-0 w-full h-full object-cover object-top"
      loading="lazy"
    />
  {:else}
    <div class="glass absolute inset-0 grid place-items-center text-4xl font-bold text-secondary">
      {character.display_name.slice(0, 1)}
    </div>
  {/if}
  <div class="absolute inset-x-0 bottom-0 h-1/3 bg-gradient-to-t from-black/45 to-transparent"></div>
  <div class="absolute bottom-2 inset-x-2 flex items-center justify-between">
    <span class="glass radius-pill px-3 py-1 text-sm font-medium text-white">
      {character.display_name}
    </span>
    {#if character.total > 0}
      <span class="glass radius-pill px-2 py-1 text-xs text-white">
        {character.enabled}/{character.total}
      </span>
    {/if}
  </div>
</button>