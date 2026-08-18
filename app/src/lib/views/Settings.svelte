<script lang="ts">
  import { onMount } from "svelte";
  import { api, isTauri, type ConfigDto } from "$lib/api";
  import { applyTheme } from "$lib/theme";
  import { toast } from "$lib/toast.svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import Toggle from "$lib/components/Toggle.svelte";

  let {
    config,
    onback,
    onchanged,
  }: {
    config: ConfigDto | null;
    onback: () => void;
    onchanged: () => void;
  } = $props();

  let passwords = $state<string[]>([]);
  let newPassword = $state("");
  let busy = $state(false);
  let logText = $state("");

  onMount(async () => {
    try {
      passwords = await api.listPasswords();
    } catch (e) {
      toast(String(e));
    }
    try {
      logText = await api.readLog();
    } catch {
      logText = "";
    }
  });

  let catNameDraft = $state("");

  $effect(() => {
    if (config && !catNameDraft) catNameDraft = config.character_category_name;
  });

  async function pickTheme(t: string) {
    try {
      const c = await api.setTheme(t);
      applyTheme(c.theme);
      onchanged();
    } catch (e) {
      toast(String(e));
    }
  }

  async function saveCatName() {
    const v = catNameDraft.trim();
    if (!v || v === config?.character_category_name) return;
    try {
      await api.setCharacterCategoryName(v);
      toast("已更新分类名称");
      onchanged();
    } catch (e) {
      toast(String(e));
    }
  }

  async function toggleAutoEnable(next: boolean) {
    try {
      await api.setAutoEnable(next);
      onchanged();
    } catch (e) {
      toast(String(e));
    }
  }

  async function refreshLog() {
    try {
      logText = await api.readLog();
    } catch (e) {
      toast(String(e));
    }
  }

  async function copyLog() {
    try {
      await navigator.clipboard.writeText(logText);
      toast("日志已复制");
    } catch {
      toast("复制失败");
    }
  }

  async function pickModsDir() {
    try {
      const path = await open({ directory: true, title: "选择 3Dmigoto Mods 目录" });
      if (typeof path === "string") {
        await api.chooseModsDir(path);
        toast("已更新 Mods 目录");
        onchanged();
      }
    } catch (e) {
      toast(String(e));
    }
  }

  async function openLibrary() {
    if (!isTauri()) return;
    if (!config) {
      toast("配置尚未加载");
      return;
    }
    try {
      const { openPath } = await import("@tauri-apps/plugin-opener");
      await openPath(config.library_root);
    } catch (e) {
      toast(String(e));
    }
  }

  async function addPassword() {
    const v = newPassword.trim();
    if (!v || busy) return;
    busy = true;
    try {
      await api.addPassword(v);
      newPassword = "";
      passwords = await api.listPasswords();
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }

  async function removePassword(v: string) {
    if (busy) return;
    busy = true;
    try {
      await api.removePassword(v);
      passwords = await api.listPasswords();
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
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
    <h2 class="text-2xl font-bold tracking-tight">设置</h2>
  </div>

  <div class="flex flex-col gap-3 px-8 pb-8 overflow-y-auto flex-1 min-h-0 max-w-2xl w-full mx-auto">
    <section class="glass radius-panel p-5 flex flex-col gap-3">
      <h3 class="text-sm font-semibold text-secondary">目录</h3>
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <p class="text-sm font-medium">Mod 仓库（Library）</p>
          <p class="text-xs text-secondary truncate">{config?.library_root ?? "…"}</p>
        </div>
        <button
          class="glass radius-pill h-8 px-3.5 text-sm shrink-0 cursor-pointer"
          onclick={openLibrary}
        >
          打开
        </button>
      </div>
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <p class="text-sm font-medium">3Dmigoto Mods 目录</p>
          <p class="text-xs text-secondary truncate">{config?.mods_dir ?? "未配置"}</p>
        </div>
        <button
          class="glass radius-pill h-8 px-3.5 text-sm shrink-0 cursor-pointer"
          onclick={pickModsDir}
        >
          选择…
        </button>
      </div>
    </section>

    <section class="glass radius-panel p-5 flex flex-col gap-3">
      <h3 class="text-sm font-semibold text-secondary">外观</h3>
      <div class="flex items-center justify-between gap-3">
        <p class="text-sm font-medium">主题</p>
        <div class="flex gap-1 p-0.5 radius-pill" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)">
          {#each [["auto", "跟随系统"], ["light", "亮色"], ["dark", "暗色"]] as [value, label] (value)}
            <button
              class="radius-pill h-7 px-3 text-xs cursor-pointer transition-colors"
              class:accent-fill={config?.theme === value}
              class:accent-text={config?.theme === value}
              onclick={() => pickTheme(value)}
            >
              {label}
            </button>
          {/each}
        </div>
      </div>
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <p class="text-sm font-medium">角色分类名称</p>
          <p class="text-xs text-secondary">不同游戏叫法不同（如「机体」「干员」）</p>
        </div>
        <div class="flex gap-1.5 shrink-0">
          <input
            bind:value={catNameDraft}
            aria-label="角色分类名称"
            class="h-8 w-28 px-3 text-sm bg-transparent outline-none rounded-full"
            style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
            onkeydown={(e) => e.key === "Enter" && saveCatName()}
          />
          <button
            class="accent-fill accent-text radius-pill h-8 px-3.5 text-sm font-medium cursor-pointer disabled:opacity-50"
            disabled={!catNameDraft.trim() || catNameDraft.trim() === config?.character_category_name}
            onclick={saveCatName}
          >
            保存
          </button>
        </div>
      </div>
    </section>

    <section class="glass radius-panel p-5 flex flex-col gap-3">
      <h3 class="text-sm font-semibold text-secondary">解压密码本</h3>
      <p class="text-xs text-secondary">安装加密压缩包时自动逐个尝试</p>
      {#each passwords as p (p)}
        <div class="flex items-center justify-between rounded-xl px-3 py-2"
          style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)">
          <span class="text-sm font-mono">{p}</span>
          <button
            class="w-6 h-6 grid place-items-center rounded-full text-secondary cursor-pointer transition-colors hover:bg-[var(--danger)] hover:text-white disabled:opacity-50 disabled:cursor-default"
            aria-label={`移除密码 ${p}`}
            disabled={busy}
            onclick={() => removePassword(p)}
          >
            <svg width="9" height="9" viewBox="0 0 9 9" fill="none">
              <path d="M2 2l5 5M7 2L2 7" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
            </svg>
          </button>
        </div>
      {:else}
        <p class="text-xs text-secondary">空</p>
      {/each}
      <div class="flex gap-1.5 mt-1">
        <input
          bind:value={newPassword}
          placeholder="添加解压密码…"
          class="flex-1 h-8 px-3 text-sm bg-transparent outline-none rounded-full"
          style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
          onkeydown={(e) => e.key === "Enter" && addPassword()}
        />
        <button
          class="accent-fill accent-text radius-pill h-8 px-3.5 text-sm font-medium cursor-pointer disabled:opacity-50"
          disabled={!newPassword.trim() || busy}
          onclick={addPassword}
        >
          添加
        </button>
      </div>
    </section>

    <section class="glass radius-panel p-5 flex items-center justify-between">
      <div>
        <h3 class="text-sm font-semibold text-secondary">行为</h3>
        <p class="text-sm font-medium mt-1">自动启用</p>
        <p class="text-xs text-secondary">安装成功后立即部署到 Mods 目录</p>
      </div>
      <Toggle checked={config?.auto_enable ?? false} ariaLabel="自动启用" onchange={toggleAutoEnable} />
    </section>

    <section class="glass radius-panel p-5 flex flex-col gap-3">
      <div class="flex items-center justify-between">
        <h3 class="text-sm font-semibold text-secondary">日志</h3>
        <div class="flex gap-2">
          <button class="glass radius-pill h-7 px-3 text-xs cursor-pointer" onclick={refreshLog}>刷新</button>
          <button class="glass radius-pill h-7 px-3 text-xs cursor-pointer" onclick={copyLog}>复制</button>
        </div>
      </div>
      <pre
        class="text-xs font-mono rounded-xl p-3 max-h-48 overflow-auto whitespace-pre-wrap break-all select-text"
        style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
      >{logText || "（暂无日志）"}</pre>
    </section>
  </div>
</div>
