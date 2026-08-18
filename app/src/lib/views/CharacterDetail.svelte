<script lang="ts">
  import { onMount } from "svelte";
  import {
    api,
    isTauri,
    portraitUrl,
    type CategoryDto,
    type CharacterSummary,
    type ModDto,
  } from "$lib/api";
  import { filterMods, type EnabledFilter } from "$lib/view";
  import ModRow from "$lib/components/ModRow.svelte";
  import EnabledFilterChips from "$lib/components/EnabledFilterChips.svelte";
  import { open } from "@tauri-apps/plugin-dialog";

  let {
    character,
    categories,
    modsDirConfigured,
    onback,
    onconfigured,
  }: {
    character: CharacterSummary;
    categories: CategoryDto[];
    modsDirConfigured: boolean;
    onback: () => void;
    onconfigured: () => void;
  } = $props();

  let mods = $state<ModDto[]>([]);
  let error = $state("");
  let enabledFilter = $state<EnabledFilter>("all");

  let shown = $derived(filterMods(mods, "", enabledFilter));

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

  async function renameMod(mod: ModDto, name: string): Promise<boolean> {
    error = "";
    try {
      await api.renameMod(mod.id, name);
      mod.name = name;
      return true;
    } catch (e) {
      error = String(e);
      return false;
    }
  }

  async function uninstallMod(mod: ModDto) {
    error = "";
    try {
      await api.uninstallMod(mod.id);
      mods = mods.filter((m) => m.id !== mod.id);
    } catch (e) {
      error = String(e);
    }
  }

  async function openModDir(mod: ModDto) {
    if (!isTauri()) return;
    try {
      const { openPath } = await import("@tauri-apps/plugin-opener");
      await openPath(mod.path);
    } catch (e) {
      error = String(e);
    }
  }

  async function moveCategory(mod: ModDto, categoryId: number | null) {
    error = "";
    try {
      await api.setModCategory(mod.id, categoryId);
      if (categoryId !== null) {
        // 移出角色视图后从列表消失
        mods = mods.filter((m) => m.id !== mod.id);
      }
      onconfigured(); // 刷新侧边栏计数
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

  <div class="flex items-center justify-between shrink-0 px-8">
    <EnabledFilterChips bind:value={enabledFilter} />
    <span class="text-xs text-secondary">{shown.length}/{mods.length} 个显示</span>
  </div>

  <div class="flex flex-col gap-2.5 px-8 pb-8 overflow-y-auto flex-1 min-h-0 max-w-3xl w-full mx-auto">
    {#each shown as mod (mod.id)}
      <ModRow
        {mod}
        {categories}
        ontoggle={(next) => toggle(mod, next)}
        onrename={(name) => renameMod(mod, name)}
        onuninstall={() => uninstallMod(mod)}
        onopen={() => openModDir(mod)}
        onmove={(cid) => moveCategory(mod, cid)}
      />
    {/each}
    {#if shown.length === 0}
      <p class="text-secondary text-center mt-24">
        {mods.length === 0 ? "该角色还没有 Mod，拖入压缩包即可安装" : "没有匹配的 Mod"}
      </p>
    {/if}
  </div>
</div>
