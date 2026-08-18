<script lang="ts">
  import { onMount } from "svelte";
  import { api, portraitUrl, type CharacterSummary, type ModDto } from "$lib/api";
  import Toggle from "$lib/components/Toggle.svelte";
  import { open } from "@tauri-apps/plugin-dialog";

  let {
    character,
    modsDirConfigured,
    onback,
    onconfigured,
  }: {
    character: CharacterSummary;
    modsDirConfigured: boolean;
    onback: () => void;
    onconfigured: () => void;
  } = $props();

  let mods = $state<ModDto[]>([]);
  let error = $state("");

  onMount(async () => {
    try {
      mods = await api.listMods(character.internal_name);
    } catch (e) {
      error = String(e);
    }
  });

  async function toggle(mod: ModDto, next: boolean) {
    error = "";
    try {
      await api.setModEnabled(mod.id, next);
      mod.enabled = next;
    } catch (e) {
      error = String(e);
    }
  }

  async function pickModsDir() {
    try {
      const path = await open({ directory: true, title: "选择 3Dmigoto Mods 目录" });
      if (typeof path === "string") {
        await api.chooseModsDir(path);
        onconfigured();
      }
    } catch (e) {
      error = String(e);
    }
  }
</script>

<div class="flex flex-col h-full min-h-0">
  <div class="flex items-center gap-4 px-8 pt-3 pb-4 shrink-0">
    <button
      class="glass radius-pill pl-2.5 pr-3.5 h-8 text-sm flex items-center gap-1 cursor-pointer transition-transform hover:-translate-x-0.5"
      onclick={onback}
    >
      <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
        <path d="M7 1L2.5 5L7 9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
      返回
    </button>
    {#if character.image}
      <img
        src={portraitUrl(character.image)}
        alt=""
        class="w-10 h-10 rounded-full object-cover object-top"
        style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
        draggable="false"
      />
    {/if}
    <h2 class="text-2xl font-bold tracking-tight">{character.display_name}</h2>
    <span class="text-sm text-secondary">{mods.length} 个 Mod</span>
  </div>

  {#if !modsDirConfigured}
    <div class="glass radius-panel mx-8 mb-3 px-4 py-3 flex items-center justify-between shrink-0">
      <span class="text-sm">未配置 3Dmigoto Mods 目录，无法启用 Mod</span>
      <button class="accent-fill accent-text radius-pill px-3.5 h-8 text-sm font-medium cursor-pointer" onclick={pickModsDir}>
        选择目录
      </button>
    </div>
  {/if}
  {#if error}
    <p class="mx-8 mb-2 text-sm shrink-0" style="color: var(--danger)">{error}</p>
  {/if}

  <div class="flex flex-col gap-2.5 px-8 pb-8 overflow-y-auto flex-1 min-h-0 max-w-3xl w-full mx-auto">
    {#each mods as mod (mod.id)}
      <div class="glass radius-card px-5 py-3.5 flex items-center justify-between gap-3">
        <div class="flex items-center gap-3 min-w-0">
          {#if mod.thumb}
            <img
              src={mod.thumb}
              alt=""
              class="w-11 h-11 rounded-xl object-cover shrink-0"
              style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
              draggable="false"
              onerror={(e) => ((e.currentTarget as HTMLImageElement).style.display = "none")}
            />
          {/if}
          <span class="font-medium truncate">{mod.name}</span>
        </div>
        <Toggle
          checked={mod.enabled}
          ariaLabel={`启用 ${mod.name}`}
          onchange={(next) => toggle(mod, next)}
        />
      </div>
    {/each}
    {#if mods.length === 0}
      <p class="text-secondary text-center mt-24">该角色还没有 Mod，拖入压缩包即可安装</p>
    {/if}
  </div>
</div>
