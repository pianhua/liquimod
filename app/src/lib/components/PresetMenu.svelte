<script lang="ts">
  import { api, type PresetDto } from "$lib/api";
  import { toast } from "$lib/toast.svelte";

  let { onapplied, block = false }: { onapplied: () => void; block?: boolean } = $props();

  let open = $state(false);
  let presets = $state<PresetDto[]>([]);
  let newName = $state("");
  let busy = $state(false);

  async function load() {
    try {
      presets = await api.listPresets();
    } catch (e) {
      toast(String(e));
    }
  }

  function toggleOpen() {
    open = !open;
    if (open) void load();
  }

  async function save() {
    const name = newName.trim();
    if (!name || busy) return;
    busy = true;
    try {
      await api.savePreset(name);
      newName = "";
      await load();
      toast(`已保存预设「${name}」`);
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }

  async function apply(p: PresetDto) {
    if (busy) return;
    busy = true;
    try {
      await api.applyPreset(p.id, p.name);
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
      onapplied(); // 成功与部分失败都刷新（后端可能已部分应用）
    }
  }

  async function remove(p: PresetDto) {
    if (busy) return;
    busy = true;
    try {
      await api.deletePreset(p.id);
      await load();
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape" && open) open = false;
  }}
/>

<div class="relative">
  <button
    class="glass radius-pill h-9 px-4 text-sm flex items-center gap-1.5 cursor-pointer transition-transform hover:scale-[1.03]"
    class:w-full={block}
    class:justify-center={block}
    aria-label="预设"
    aria-expanded={open}
    onclick={toggleOpen}
  >
    <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
      <path
        d="M3 1.5h7v10l-3.5-2.6L3 11.5v-10z"
        stroke="currentColor"
        stroke-width="1.2"
        stroke-linejoin="round"
      />
    </svg>
    预设
  </button>
  {#if open}
    <button
      class="fixed inset-0 z-40 cursor-default bg-transparent"
      aria-label="关闭预设菜单"
      tabindex="-1"
      onclick={() => (open = false)}
    ></button>
    <div
      class="glass radius-panel absolute z-50 p-2.5 flex flex-col gap-1"
      class:left-0={block}
      class:right-0={!block}
      class:w-72={!block}
      class:bottom-full={block}
      class:mb-2={block}
      class:top-11={!block}
      style={block ? "left: 0; right: 0" : ""}
    >
      {#each presets as p (p.id)}
        <div class="flex items-center gap-1 rounded-lg px-1.5 py-1 transition-colors hover:bg-[var(--glass-stroke)]">
          <button
            class="flex-1 text-left text-sm px-1.5 py-1 cursor-pointer truncate disabled:opacity-50"
            disabled={busy}
            onclick={() => apply(p)}
          >
            {p.name}
          </button>
          <button
            class="glass radius-pill w-8 h-8 grid place-items-center text-secondary cursor-pointer transition-colors hover:bg-[var(--danger)] hover:text-white disabled:opacity-50 disabled:cursor-default"
            aria-label={`删除预设 ${p.name}`}
            disabled={busy}
            onclick={() => remove(p)}
          >
            <svg width="9" height="9" viewBox="0 0 9 9" fill="none">
              <path d="M2 2l5 5M7 2L2 7" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
            </svg>
          </button>
        </div>
      {:else}
        <p class="text-xs text-secondary px-2.5 py-2">还没有预设，保存当前启用组合试试</p>
      {/each}
      <div class="flex gap-1.5 mt-1 pt-2" style="border-top: 0.5px solid var(--glass-stroke)">
        <input
          bind:value={newName}
          placeholder="保存当前启用为预设…"
          class="flex-1 h-8 px-3 text-sm bg-transparent outline-none rounded-full"
          style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
          onkeydown={(e) => e.key === "Enter" && save()}
        />
        <button
          class="accent-fill accent-text radius-pill h-8 px-3.5 text-sm font-medium cursor-pointer disabled:opacity-50"
          disabled={!newName.trim() || busy}
          onclick={save}
        >
          保存
        </button>
      </div>
    </div>
  {/if}
</div>