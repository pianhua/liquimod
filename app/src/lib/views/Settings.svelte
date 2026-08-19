<script lang="ts">
  import { onMount } from "svelte";
  import {
    api,
    isTauri,
    type ConfigDto,
    type DiagnosticStatusDto,
    type AssetSyncProgressDto,
  } from "$lib/api";
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
  let diagStatus = $state<DiagnosticStatusDto | null>(null);
  let localAssetVersion = $state<string | null>(null);
  let syncing = $state(false);
  let checkingUpdate = $state(false);
  let syncProgress = $state<AssetSyncProgressDto | null>(null);

  onMount(() => {
    let unlistenProgress: (() => void) | undefined;

    (async () => {
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
      try {
        diagStatus = await api.getDiagnosticStatus();
      } catch {
        diagStatus = null;
      }
      try {
        localAssetVersion = await api.getLocalAssetVersion();
      } catch {
        localAssetVersion = null;
      }

      if (isTauri()) {
        const { listen } = await import("@tauri-apps/api/event");
        unlistenProgress = await listen<AssetSyncProgressDto>("asset-sync-progress", (e) => {
          syncProgress = e.payload;
        });
      }
    })();

    return () => {
      unlistenProgress?.();
    };
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

  /// 日志时间戳是 UTC（tracing 默认 RFC3339），展示时转本地时间。
  function formatLog(text: string): string {
    return text.replace(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z/gm, (iso) => {
      const d = new Date(iso);
      if (Number.isNaN(d.getTime())) return iso;
      const p = (n: number) => String(n).padStart(2, "0");
      return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
    });
  }

  async function copyLog() {
    try {
      await navigator.clipboard.writeText(formatLog(logText));
      toast("日志已复制");
    } catch {
      toast("复制失败");
    }
  }

  async function import3dMigoto() {
    try {
      const path = await open({ directory: true, title: "选择 3Dmigoto 根目录（包含 d3dx.ini）" });
      if (typeof path === "string") {
        await api.import3dMigotoDir(path);
        toast("已成功识别并导入 3Dmigoto 配置！");
        diagStatus = await api.getDiagnosticStatus().catch(() => null);
        onchanged();
      }
    } catch (e) {
      toast(String(e));
    }
  }

  async function pickModsDir() {
    try {
      const path = await open({ directory: true, title: "选择 3Dmigoto Mods 目录" });
      if (typeof path === "string") {
        await api.chooseModsDir(path);
        toast("已更新 Mods 目录");
        diagStatus = await api.getDiagnosticStatus().catch(() => null);
        onchanged();
      }
    } catch (e) {
      toast(String(e));
    }
  }

  async function pickExe(which: "game" | "loader") {
    try {
      const path = await open({
        directory: false,
        title: which === "game" ? "选择游戏主程序" : "选择 3Dmigoto 加载器",
        filters: [{ name: "可执行文件", extensions: ["exe"] }],
      });
      if (typeof path === "string") {
        if (which === "game") await api.chooseGameExe(path);
        else await api.chooseLoaderExe(path);
        toast("已更新路径");
        diagStatus = await api.getDiagnosticStatus().catch(() => null);
        onchanged();
      }
    } catch (e) {
      toast(String(e));
    }
  }

  async function handleRescan() {
    if (busy) return;
    busy = true;
    try {
      const res = await api.rescanLibrary();
      toast(`全库扫描完成：新增 ${res.added} 个，移除 ${res.removed} 个`);
      onchanged();
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }

  async function handleCleanCache() {
    if (busy) return;
    busy = true;
    try {
      const count = await api.cleanCache();
      toast(`已清理 ${count} 个无用封面缓存`);
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }

  async function handleCheckAssetUpdate() {
    if (checkingUpdate || syncing) return;
    checkingUpdate = true;
    try {
      const res = await api.checkGameAssetsUpdate("Honkai");
      if (res.has_update) {
        toast(`发现星铁数据新版本：${res.remote_version}，点击「同步星铁数据」即可更新`);
      } else {
        toast(`当前星铁数据已是最新版本 (${res.local_version || "内嵌"})`);
      }
    } catch (e) {
      toast(`检查更新失败：${e}`);
    } finally {
      checkingUpdate = false;
    }
  }

  async function handleSyncAssets() {
    if (syncing || busy) return;
    syncing = true;
    syncProgress = {
      stage: "checking",
      percent: 5,
      current_file: null,
      downloaded_count: 0,
      total_count: 0,
      message: "正在连接云端并测速最优镜像...",
    };
    try {
      const res = await api.syncGameAssets("Honkai");
      localAssetVersion = res.version;
      toast(`星铁角色数据同步完成！版本: ${res.version}`);
      onchanged();
    } catch (e) {
      toast(`数据同步失败：${e}`);
    } finally {
      syncing = false;
      setTimeout(() => {
        syncProgress = null;
      }, 4000);
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
      toast("已添加解压密码");
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
      toast("已移除解压密码");
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

  <div class="flex flex-col gap-4 px-8 pb-10 overflow-y-auto flex-1 min-h-0 max-w-2xl w-full mx-auto">
    <!-- 分组 1：外观与偏好 -->
    <section class="glass radius-panel p-5 flex flex-col gap-3.5">
      <h3 class="text-xs font-semibold uppercase tracking-wider text-secondary">外观与偏好</h3>
      
      <div class="flex items-center justify-between gap-3">
        <div>
          <p class="text-sm font-medium">界面主题</p>
          <p class="text-xs text-secondary mt-0.5">选择高对比度亮色、纯净深色或跟随系统</p>
        </div>
        <div class="flex gap-1 p-0.5 radius-pill shrink-0" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)">
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

      <div class="border-t border-[var(--glass-stroke)] pt-3 flex items-center justify-between gap-3">
        <div>
          <p class="text-sm font-medium">安装后自动启用</p>
          <p class="text-xs text-secondary mt-0.5">解压并导入 Mod 成功后，立即创建软链接部署到游戏</p>
        </div>
        <Toggle checked={config?.auto_enable ?? false} ariaLabel="自动启用" onchange={toggleAutoEnable} />
      </div>

      <div class="border-t border-[var(--glass-stroke)] pt-3 flex items-center justify-between gap-3">
        <div class="min-w-0">
          <p class="text-sm font-medium">角色分类别名</p>
          <p class="text-xs text-secondary mt-0.5">侧边栏与面包屑的默认角色大类显示名</p>
        </div>
        <div class="flex gap-1.5 shrink-0">
          <input
            bind:value={catNameDraft}
            aria-label="角色分类名称"
            class="h-8 w-28 px-3 text-xs bg-transparent outline-none rounded-full"
            style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
            onkeydown={(e) => e.key === "Enter" && saveCatName()}
          />
          <button
            class="accent-fill accent-text radius-pill h-8 px-3 text-xs font-semibold cursor-pointer disabled:opacity-50"
            disabled={!catNameDraft.trim() || catNameDraft.trim() === config?.character_category_name}
            onclick={saveCatName}
          >
            保存
          </button>
        </div>
      </div>
    </section>

    <!-- 分组 2：游戏与 3Dmigoto 集成 -->
    <section class="glass radius-panel p-5 flex flex-col gap-3.5">
      <div class="flex items-center justify-between gap-3">
        <div>
          <h3 class="text-xs font-semibold uppercase tracking-wider text-secondary">游戏与 3Dmigoto 集成</h3>
          <p class="text-xs text-secondary mt-0.5">自动识别 d3dx.ini 或手动配置启动与部署路径</p>
        </div>
        <button
          class="accent-fill accent-text radius-pill h-8 px-3.5 text-xs font-semibold shrink-0 cursor-pointer transition-transform hover:scale-[1.02]"
          onclick={import3dMigoto}
        >
          ✨ 一键识别 3DM 目录
        </button>
      </div>

      <div class="border-t border-[var(--glass-stroke)] pt-3 flex items-center justify-between gap-3">
        <div class="min-w-0 flex-1">
          <p class="text-sm font-medium">游戏主程序</p>
          <p class="text-xs text-secondary truncate mt-0.5 font-mono" title={config?.game_exe ?? "未配置"}>{config?.game_exe ?? "未配置"}</p>
        </div>
        <div class="flex items-center gap-1.5 shrink-0">
          {#if config?.game_exe}
            <button
              class="glass radius-pill w-8 h-8 grid place-items-center cursor-pointer text-secondary hover:text-[var(--text)]"
              title="在资源管理器中定位"
              onclick={() => config?.game_exe && api.openPathInExplorer(config.game_exe)}
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>
            </button>
          {/if}
          <button class="glass radius-pill h-8 px-3 text-xs font-medium shrink-0 cursor-pointer" onclick={() => pickExe("game")}>
            选择…
          </button>
        </div>
      </div>

      <div class="border-t border-[var(--glass-stroke)] pt-3 flex items-center justify-between gap-3">
        <div class="min-w-0 flex-1">
          <p class="text-sm font-medium">3Dmigoto 加载器</p>
          <p class="text-xs text-secondary truncate mt-0.5 font-mono" title={config?.loader_exe ?? "未配置"}>{config?.loader_exe ?? "未配置"}</p>
        </div>
        <div class="flex items-center gap-1.5 shrink-0">
          {#if config?.loader_exe}
            <button
              class="glass radius-pill w-8 h-8 grid place-items-center cursor-pointer text-secondary hover:text-[var(--text)]"
              title="在资源管理器中定位"
              onclick={() => config?.loader_exe && api.openPathInExplorer(config.loader_exe)}
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>
            </button>
          {/if}
          <button class="glass radius-pill h-8 px-3 text-xs font-medium shrink-0 cursor-pointer" onclick={() => pickExe("loader")}>
            选择…
          </button>
        </div>
      </div>

      <div class="border-t border-[var(--glass-stroke)] pt-3 flex items-center justify-between gap-3">
        <div class="min-w-0 flex-1">
          <p class="text-sm font-medium">3Dmigoto Mods 部署目录</p>
          <p class="text-xs text-secondary truncate mt-0.5 font-mono" title={config?.mods_dir ?? "未配置"}>{config?.mods_dir ?? "未配置"}</p>
        </div>
        <div class="flex items-center gap-1.5 shrink-0">
          {#if config?.mods_dir}
            <button
              class="glass radius-pill w-8 h-8 grid place-items-center cursor-pointer text-secondary hover:text-[var(--text)]"
              title="在资源管理器中打开"
              onclick={() => config?.mods_dir && api.openPathInExplorer(config.mods_dir)}
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>
            </button>
          {/if}
          <button
            class="glass radius-pill h-8 px-3 text-xs font-medium shrink-0 cursor-pointer"
            onclick={pickModsDir}
          >
            选择…
          </button>
        </div>
      </div>

      <div class="border-t border-[var(--glass-stroke)] pt-3 flex items-center justify-between gap-3">
        <div class="min-w-0 flex-1">
          <p class="text-sm font-medium">LiquiMod 核心仓库（Library）</p>
          <p class="text-xs text-secondary truncate mt-0.5 font-mono" title={config?.library_root ?? "…"}>{config?.library_root ?? "…"}</p>
        </div>
        <button
          class="glass radius-pill h-8 px-3 text-xs font-medium shrink-0 cursor-pointer flex items-center gap-1"
          onclick={() => config?.library_root && api.openPathInExplorer(config.library_root)}
        >
          <span>📂</span> 打开仓库
        </button>
      </div>
    </section>

    <!-- 分组 3：解压密码本 -->
    <section class="glass radius-panel p-5 flex flex-col gap-3.5">
      <div>
        <h3 class="text-xs font-semibold uppercase tracking-wider text-secondary">解压密码本</h3>
        <p class="text-xs text-secondary mt-0.5">安装加密压缩包时将自动轮询密码本，无需每次重复输入</p>
      </div>

      {#if passwords.length > 0}
        <div class="flex flex-wrap gap-2">
          {#each passwords as p (p)}
            <div class="glass radius-pill pl-3 pr-1.5 h-7 flex items-center gap-1.5 text-xs font-mono group">
              <span>{p}</span>
              <button
                class="w-5 h-5 rounded-full grid place-items-center text-secondary hover:text-[var(--danger)] hover:bg-[rgba(255,69,58,0.12)] cursor-pointer transition-colors"
                aria-label={`移除密码 ${p}`}
                disabled={busy}
                onclick={() => removePassword(p)}
              >
                <svg width="8" height="8" viewBox="0 0 9 9" fill="none">
                  <path d="M2 2l5 5M7 2L2 7" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
                </svg>
              </button>
            </div>
          {/each}
        </div>
      {:else}
        <p class="text-xs text-secondary py-1 italic">（暂无记录的解压密码）</p>
      {/if}

      <div class="flex gap-2 pt-1">
        <input
          bind:value={newPassword}
          placeholder="添加解压密码…"
          class="flex-1 h-8 px-3.5 text-xs bg-transparent outline-none rounded-full"
          style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
          onkeydown={(e) => e.key === "Enter" && addPassword()}
        />
        <button
          class="accent-fill accent-text radius-pill h-8 px-4 text-xs font-semibold cursor-pointer disabled:opacity-50"
          disabled={!newPassword.trim() || busy}
          onclick={addPassword}
        >
          添加
        </button>
      </div>
    </section>

    <!-- 分组 4：维护与诊断 -->
    <section class="glass radius-panel p-5 flex flex-col gap-3.5">
      <div>
        <h3 class="text-xs font-semibold uppercase tracking-wider text-secondary">维护与系统诊断</h3>
        <p class="text-xs text-secondary mt-0.5">检查组件状态、同步磁盘索引或清理缓存</p>
      </div>

      <!-- 诊断指示灯 -->
      <div class="grid grid-cols-2 gap-2 text-xs">
        <div class="p-2.5 rounded-lg flex items-center justify-between" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)">
          <span class="text-secondary">F10 游戏内热刷新</span>
          <span class="flex items-center gap-1.5 font-medium {diagStatus?.helper_ready ? 'text-emerald-500' : 'text-amber-500'}">
            <span class="w-1.5 h-1.5 rounded-full {diagStatus?.helper_ready ? 'bg-emerald-500' : 'bg-amber-500'}"></span>
            {diagStatus?.helper_ready ? "就绪" : "未启动/缺组件"}
          </span>
        </div>
        <div class="p-2.5 rounded-lg flex items-center justify-between" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)">
          <span class="text-secondary">3Dmigoto 关联</span>
          <span class="flex items-center gap-1.5 font-medium {diagStatus?.mods_dir_configured ? 'text-emerald-500' : 'text-zinc-400'}">
            <span class="w-1.5 h-1.5 rounded-full {diagStatus?.mods_dir_configured ? 'bg-emerald-500' : 'bg-zinc-400'}"></span>
            {diagStatus?.mods_dir_configured ? "已配置" : "未配置"}
          </span>
        </div>
      </div>

      <!-- 维护动作按钮组 -->
      <div class="border-t border-[var(--glass-stroke)] pt-3 flex items-center gap-2">
        <button
          class="glass radius-pill h-8 px-3.5 text-xs font-medium cursor-pointer flex items-center gap-1.5 hover:bg-[var(--glass-hover)] disabled:opacity-50"
          disabled={busy}
          onclick={handleRescan}
        >
          <span>🔄</span> 全库扫描与对齐
        </button>
        <button
          class="glass radius-pill h-8 px-3.5 text-xs font-medium cursor-pointer flex items-center gap-1.5 hover:bg-[var(--glass-hover)] disabled:opacity-50"
          disabled={busy}
          onclick={handleCleanCache}
        >
          <span>🧹</span> 清理封面缓存
        </button>
      </div>

      <!-- 角色数据云端热更新卡片 -->
      <div class="border-t border-[var(--glass-stroke)] pt-3 flex flex-col gap-2.5">
        <div class="flex items-center justify-between">
          <div>
            <p class="text-sm font-medium flex items-center gap-1.5">
              <span>☁️</span> 崩坏：星穹铁道 角色数据云端同步
            </p>
            <p class="text-xs text-secondary mt-0.5">
              当前版本：<span class="font-mono">{localAssetVersion || "内嵌预置版本"}</span>
            </p>
          </div>
          <div class="flex items-center gap-2">
            <button
              class="glass radius-pill h-8 px-3 text-xs font-medium cursor-pointer flex items-center gap-1.5 hover:bg-[var(--glass-hover)] disabled:opacity-50"
              disabled={checkingUpdate || syncing}
              onclick={handleCheckAssetUpdate}
            >
              <span>🔍</span> {checkingUpdate ? "检查中…" : "检查更新"}
            </button>
            <button
              class="accent-fill accent-text radius-pill h-8 px-3.5 text-xs font-semibold cursor-pointer flex items-center gap-1.5 disabled:opacity-50"
              disabled={syncing || checkingUpdate}
              onclick={handleSyncAssets}
            >
              <span>⚡</span> {syncing ? "正在同步…" : "同步星铁数据"}
            </button>
          </div>
        </div>

        {#if syncProgress}
          <div class="p-3 rounded-lg flex flex-col gap-1.5 bg-[var(--glass-tint)]" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)">
            <div class="flex items-center justify-between text-xs">
              <span class="text-secondary truncate max-w-[320px]">{syncProgress.message}</span>
              <span class="font-mono font-medium">{syncProgress.percent}%</span>
            </div>
            <div class="w-full h-1.5 rounded-full bg-[var(--glass-stroke)] overflow-hidden">
              <div
                class="h-full transition-all duration-300 rounded-full"
                style="width: {syncProgress.percent}%; background: var(--accent, #409CFF)"
              ></div>
            </div>
          </div>
        {/if}
      </div>

      <!-- 日志预览 -->
      <div class="border-t border-[var(--glass-stroke)] pt-3 flex flex-col gap-2">
        <div class="flex items-center justify-between">
          <p class="text-sm font-medium">运行日志</p>
          <div class="flex gap-1.5">
            <button class="glass radius-pill h-7 px-2.5 text-xs cursor-pointer" onclick={refreshLog}>刷新</button>
            <button class="glass radius-pill h-7 px-2.5 text-xs cursor-pointer" onclick={copyLog}>复制</button>
          </div>
        </div>
        <pre
          class="text-[11px] font-mono rounded-lg p-3 max-h-40 overflow-auto whitespace-pre-wrap break-all select-text leading-relaxed opacity-85"
          style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
        >{formatLog(logText) || "（暂无日志）"}</pre>
      </div>

      <!-- 关于信息 -->
      <div class="border-t border-[var(--glass-stroke)] pt-3 text-xs text-secondary flex items-center justify-between">
        <span>LiquiMod —— 崩坏：星穹铁道 Mod 管理器</span>
        <span class="font-mono">v0.1.0 (Rust + Tauri 2 + Svelte 5)</span>
      </div>
    </section>
  </div>
</div>
