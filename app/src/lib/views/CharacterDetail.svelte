<script lang="ts">
  import { onMount } from "svelte";
  import { api, type CharacterSummary, type ModDto } from "$lib/api";
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
    mods = await api.listMods(character.internal_name);
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
    const path = await open({ directory: true, title: "选择 3Dmigoto Mods 目录" });
    if (typeof path === "string") {
      await api.chooseModsDir(path);
      onconfigured();
    }
  }
</script>

<div class="flex flex-col h-full">
  <div class="flex items-center gap-3 px-5 pt-2">
    <button class="glass radius-pill px-3 h-8 text-sm" onclick={onback}>‹ 返回</button>
    <h2 class="text-xl font-bold">{character.display_name}</h2>
  </div>

  {#if !modsDirConfigured}
    <div class="glass radius-panel mx-5 mt-3 p-3 flex items-center justify-between">
      <span class="text-sm">未配置 3Dmigoto Mods 目录，无法启用 Mod</span>
      <button class="accent-fill accent-text radius-pill px-3 h-8 text-sm font-medium" onclick={pickModsDir}>
        选择目录
      </button>
    </div>
  {/if}
  {#if error}
    <p class="mx-5 mt-2 text-sm" style="color: var(--accent)">{error}</p>
  {/if}

  <div class="flex flex-col gap-2 p-5 overflow-y-auto">
    {#each mods as mod (mod.id)}
      <div class="glass radius-card px-4 py-3 flex items-center justify-between">
        <span class="font-medium">{mod.name}</span>
        <Toggle checked={mod.enabled} onchange={(next) => toggle(mod, next)} />
      </div>
    {/each}
    {#if mods.length === 0}
      <p class="text-secondary text-center mt-16">该角色还没有 Mod，拖入压缩包即可安装</p>
    {/if}
  </div>
</div>