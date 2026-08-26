<script lang="ts">
  import { api, type PresetDto } from "$lib/api";
  import { toast } from "$lib/toast.svelte";
  import { pushEscHandler, registerPopover, notifyPopoverOpened } from "$lib/esc";
  import { IconBookmark, IconClose } from "$lib/components/icons";

  let {
    onapplied,
    block = false,
    applyDisabled = false,
  }: {
    onapplied: () => void;
    block?: boolean;
    applyDisabled?: boolean;
  } = $props();

  let open = $state(false);
  let presets = $state<PresetDto[]>([]);
  let newName = $state("");
  let busy = $state(false);

  const closeSelf = () => {
    open = false;
  };

  $effect(() => {
    return registerPopover(closeSelf);
  });

  $effect(() => {
    if (open) {
      notifyPopoverOpened(closeSelf);
      return pushEscHandler(() => {
        open = false;
        return true;
      });
    }
  });

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
    if (busy || applyDisabled) return;
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
    if (e.key === "Escape" && open) {
      e.stopPropagation();
      open = false;
    }
  }}
/>

<div class="relative">
  <button
    class="glass-liquid-btn h-8 px-3 text-xs flex items-center gap-1.5 cursor-pointer transition-transform hover:scale-[1.02] text-[var(--text)]"
    class:w-full={block}
    class:justify-center={block}
    aria-label="预设"
    aria-expanded={open}
    onclick={toggleOpen}
  >
    <span class="z-10 flex items-center gap-1.5">
      <IconBookmark size={13} class="text-[var(--accent)]" />
      <span>预设</span>
    </span>
  </button>
  {#if open}
    <button
      class="fixed inset-0 z-40 cursor-default bg-transparent"
      aria-label="关闭预设菜单"
      tabindex="-1"
      onclick={() => (open = false)}
    ></button>
    <div
      class="glass-popover absolute z-50 p-3 flex flex-col gap-1.5 shadow-2xl animate-slide-up"
      class:left-0={block}
      class:right-0={!block}
      class:w-72={!block}
      class:bottom-full={block}
      class:mb-2={block}
      class:top-11={!block}
      style={block ? "left: 0; right: 0" : ""}
    >
      <!-- 头部：标题与计数 -->
      <div class="flex items-center justify-between pb-1.5 border-b border-[var(--glass-stroke)] px-1">
        <span class="flex items-center gap-1.5 text-xs font-bold text-[var(--text)]">
          <IconBookmark size={13} class="text-[var(--accent)]" />
          <span>Mod 预设方案</span>
        </span>
        <span class="text-[10px] font-mono text-secondary px-1.5 py-0.5 rounded-full bg-white/5 border border-white/5">
          {presets.length} 个
        </span>
      </div>

      <!-- 预设列表 -->
      <div class="flex flex-col gap-0.5 max-h-52 overflow-y-auto pr-0.5">
        {#each presets as p (p.id)}
          <div class="group/item flex items-center justify-between px-2.5 py-1.5 rounded-xl transition-all hover:bg-[var(--item-hover)]">
            <button
              class="flex items-center gap-2 flex-1 min-w-0 text-left cursor-pointer"
              disabled={busy || applyDisabled}
              title={applyDisabled ? "游戏运行期间暂不支持应用预设" : `应用预设 ${p.name}`}
              onclick={() => apply(p)}
            >
              <span class="w-1.5 h-1.5 rounded-full bg-[var(--accent)] shrink-0 opacity-70 group-hover/item:opacity-100 group-hover/item:scale-125 transition-all"></span>
              <span class="text-xs font-medium text-[var(--text)] truncate">
                {p.name}
              </span>
            </button>
            <button
              class="w-5 h-5 rounded-full grid place-items-center text-secondary/60 opacity-0 group-hover/item:opacity-100 hover:text-[var(--danger)] hover:bg-[var(--danger)]/15 transition-all cursor-pointer shrink-0 ml-1"
              title="删除预设「{p.name}」"
              aria-label="删除预设 {p.name}"
              disabled={busy}
              onclick={(e) => {
                e.stopPropagation();
                remove(p);
              }}
            >
              <IconClose size={11} />
            </button>
          </div>
        {:else}
          <div class="py-4 text-center text-xs text-secondary/70">
            暂无预设，保存当前启用的 Mod 组合
          </div>
        {/each}
      </div>

      <!-- 底部：一体化保存输入胶囊 -->
      <div class="mt-1 pt-2 border-t border-[var(--glass-stroke)]">
        <div class="glass-search-capsule flex items-center h-8 pl-3 pr-1 gap-1">
          <input
            bind:value={newName}
            placeholder="保存当前启用为预设…"
            class="flex-1 min-w-0 bg-transparent text-xs text-[var(--text)] placeholder:text-secondary/60 outline-none border-none"
            onkeydown={(e) => e.key === "Enter" && save()}
          />
          <button
            class="glass-liquid-btn-accent px-3 h-6 text-[11px] font-semibold rounded-full shrink-0 cursor-pointer disabled:opacity-40 disabled:cursor-default"
            disabled={!newName.trim() || busy}
            onclick={save}
          >
            保存
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>
