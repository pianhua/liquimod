<script lang="ts">
  import { onMount } from "svelte";
  import {
    api,
    isTauri,
    type ConfigDto,
    type DiagnosticStatusDto,
    type AssetSyncProgressDto,
    type StorageInfoDto,
    type CorePackageStatusDto,
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
  let detectingGame = $state(false);
  let storageInfo = $state<StorageInfoDto | null>(null);
  let storageBusy = $state(false);
  let coreStatuses = $state<CorePackageStatusDto[]>([]);
  let sourceBusy = $state(false);

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
      try {
        storageInfo = await api.getStorageInfo();
      } catch {
        storageInfo = null;
      }
      try {
        coreStatuses = await api.getCorePackageStatus();
      } catch {
        coreStatuses = [];
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

  async function toggleMultipleModWarning(next: boolean) {
    try {
      await api.setWarnMultipleMods(next);
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

  async function addModSource() {
    if (sourceBusy) return;
    if (!isTauri()) {
      toast("浏览器预览不支持原生目录选择，请在桌面版操作");
      return;
    }
    try {
      const path = await open({ directory: true, title: "选择外部 Mod 源目录" });
      if (typeof path !== "string" || !path.trim()) return;
      sourceBusy = true;
      await api.addModSource(path);
      toast("外部 Mod 源已连接；源文件不会被复制或删除");
      onchanged();
    } catch (e) {
      toast(String(e));
    } finally {
      sourceBusy = false;
    }
  }

  async function removeModSource(path: string) {
    if (sourceBusy) return;
    sourceBusy = true;
    try {
      await api.removeModSource(path);
      toast("已解除外部 Mod 源连接，源文件未删除");
      onchanged();
    } catch (e) {
      toast(String(e));
    } finally {
      sourceBusy = false;
    }
  }

  function formatBytes(bytes: number | null | undefined): string {
    if (bytes == null) return "未知";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
    return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
  }

  function compactPath(path: string | null | undefined, root = config?.storage_root): string {
    if (!path) return "未配置";
    if (!root) return path;
    const normalizedPath = path.replace(/[\\/]+/g, "/").replace(/\/$/, "");
    const normalizedRoot = root.replace(/[\\/]+/g, "/").replace(/\/$/, "");
    if (normalizedPath.toLowerCase() === normalizedRoot.toLowerCase()) return "LiquiMod";
    if (normalizedPath.toLowerCase().startsWith(`${normalizedRoot.toLowerCase()}/`)) {
      return `LiquiMod/${normalizedPath.slice(normalizedRoot.length + 1)}`;
    }
    return path;
  }

  async function loadStorageInfo() {
    try {
      storageInfo = await api.getStorageInfo();
    } catch (e) {
      toast(`读取存储信息失败：${e}`);
    }
  }

  async function pickStorageRoot() {
    if (storageBusy) return;
    if (!isTauri()) {
      toast("浏览器预览不支持原生目录选择，请在桌面版操作");
      return;
    }
    try {
      const path = await open({ directory: true, title: "选择 LiquiMod 数据存储盘符或文件夹" });
      if (typeof path !== "string" || !path.trim()) return;
      const confirmed = window.confirm(
        `将把 LiquiMod 核心仓库复制到：\n${path}\n\n旧仓库会暂时保留，确认迁移完成后可在此页面手动清理。继续吗？`,
      );
      if (!confirmed) return;
      storageBusy = true;
      const result = await api.migrateStorage(path);
      toast(`存储迁移完成：已复制 ${result.copied_files} 个文件（${formatBytes(result.copied_bytes)}）`);
      await loadStorageInfo();
      onchanged();
    } catch (e) {
      toast(`存储迁移失败：${e}`);
    } finally {
      storageBusy = false;
    }
  }

  async function cleanupPreviousStorage() {
    if (storageBusy || !storageInfo?.previous_library_root) return;
    const confirmed = window.confirm(
      `确定删除旧仓库吗？\n${storageInfo.previous_library_root}\n\n此操作只会清理已经完成迁移的旧 LiquiMod 仓库，不会影响当前仓库。`,
    );
    if (!confirmed) return;
    storageBusy = true;
    try {
      const bytes = await api.cleanupPreviousLibrary();
      toast(
        bytes > 0
          ? `旧仓库已清理（${formatBytes(bytes)}）`
          : "旧仓库记录已清理（原目录已不存在）",
      );
      await loadStorageInfo();
      onchanged();
    } catch (e) {
      toast(`清理旧仓库失败：${e}`);
    } finally {
      storageBusy = false;
    }
  }

  async function pickExe(which: "game") {
    if (!isTauri()) {
      toast("浏览器预览不支持原生文件选择，请在桌面版操作");
      return;
    }
    try {
      const path = await open({
        directory: false,
        title: "选择游戏主程序",
        filters: [{ name: "可执行文件", extensions: ["exe"] }],
      });
      if (typeof path === "string") {
        await api.chooseGameExe(path);
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

  async function handleRepairDeployment() {
    if (busy) return;
    busy = true;
    try {
      await api.repairDeployment();
      diagStatus = await api.getDiagnosticStatus().catch(() => null);
      toast("Mod 部署已重新对账");
      onchanged();
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }

  async function openWebView2Download() {
    try {
      await api.openWebView2Download();
    } catch (e) {
      toast(String(e));
    }
  }

  async function copyDefenderCommand() {
    if (!diagStatus?.defender_command) return;
    try {
      await navigator.clipboard.writeText(diagStatus.defender_command);
      toast("Defender 排除命令已复制");
    } catch {
      toast("复制失败，请手动选择命令");
    }
  }

  function checkTone(state: string): string {
    if (state === "pass") return "text-emerald-500";
    if (state === "fail") return "text-red-500";
    if (state === "warn") return "text-amber-500";
    return "text-secondary";
  }

  function checkDot(state: string): string {
    if (state === "pass") return "bg-emerald-500";
    if (state === "fail") return "bg-red-500";
    if (state === "warn") return "bg-amber-500";
    return "bg-zinc-400";
  }

  function checkLabel(state: string): string {
    if (state === "pass") return "通过";
    if (state === "fail") return "需要处理";
    if (state === "warn") return "注意";
    return "未确认";
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

  let srmiRelease = $state<import("$lib/api").MigotoReleaseInfoDto | null>(null);
  let xxmiRelease = $state<import("$lib/api").MigotoReleaseInfoDto | null>(null);
  let checkingSrmi = $state(false);
  let checkingXxmi = $state(false);
  let installingCore = $state<"SRMI" | "XXMI" | null>(null);
  let delayDraft = $state(0);
  let tokenDraft = $state("");
  let mirrorDraft = $state("");

  $effect(() => {
    if (config) {
      delayDraft = config.injection_delay_ms ?? 0;
      tokenDraft = config.github_token ?? "";
      mirrorDraft = config.github_mirror ?? "";
    }
  });

  async function autoDetectGame() {
    if (detectingGame) return;
    detectingGame = true;
    try {
      const found = await api.autoDetectGameExe();
      if (found) {
        await api.chooseGameExe(found);
        toast(`✨ 成功探测到游戏路径：${found}`);
        diagStatus = await api.getDiagnosticStatus().catch(() => null);
        onchanged();
      } else {
        toast("未能在常见目录或运行日志中找到 StarRail.exe，请手动选择");
      }
    } catch (e) {
      toast(`探测失败：${e}`);
    } finally {
      detectingGame = false;
    }
  }

  async function handleInitMigoto() {
    try {
      await api.switchToManagedMigoto();
      toast("✨ 已初始化 LiquiMod 本地 3DMigoto 运行入口！");
      diagStatus = await api.getDiagnosticStatus().catch(() => null);
      onchanged();
    } catch (e) {
      toast(`切换失败：${e}`);
    }
  }

  async function handleCheckSrmiUpdate() {
    if (checkingSrmi || installingCore) return;
    checkingSrmi = true;
    try {
      const rel = await api.checkMigotoUpdate();
      srmiRelease = rel;
      if (config?.migoto_version && rel.tag_name && config.migoto_version.trim().toLowerCase() === rel.tag_name.trim().toLowerCase()) {
        toast(`SRMI 当前已是最新版本 (${rel.tag_name})`);
      } else {
        toast(`✨ 查找到 SRMI 最新 Release：${rel.tag_name || rel.name}`);
      }
    } catch (e) {
      toast(`检查 SRMI 更新失败：${e}`);
    } finally {
      checkingSrmi = false;
    }
  }

  async function refreshCoreStatuses() {
    coreStatuses = await api.getCorePackageStatus().catch(() => []);
  }

  async function handleCheckXxmiUpdate() {
    if (checkingXxmi || checkingSrmi || installingCore) return;
    checkingXxmi = true;
    try {
      xxmiRelease = await api.checkXxmiUpdate();
      toast(`XXMI 官方套件：${xxmiRelease.tag_name || xxmiRelease.name}`);
    } catch (e) {
      toast(`检查 XXMI 更新失败：${e}`);
    } finally {
      checkingXxmi = false;
    }
  }

  async function installCore(packageName: "SRMI" | "XXMI") {
    if (installingCore) return;
    installingCore = packageName;
    try {
      if (packageName === "SRMI") await api.installSrmiUpdate();
      else await api.installXxmiUpdate();
      await refreshCoreStatuses();
      toast(`${packageName} 官方核心已安装并完成签名校验`);
      onchanged();
    } catch (e) {
      toast(`${packageName} 核心安装失败：${e}`);
    } finally {
      installingCore = null;
    }
  }

  async function handleMigrateOldMigoto() {
    if (!isTauri()) {
      toast("浏览器预览不支持原生目录选择，请在桌面版操作");
      return;
    }
    try {
      const oldDir = await open({
        directory: true,
        title: "选择包含 Mods 的旧 3DMigoto / SSMI 工作区目录",
      });
      if (typeof oldDir === "string") {
        const res = await api.migrateModsFromOldMigoto(oldDir);
        toast(`✨ 迁移完成！扫描发现 ${res.total_found} 个 Mod，成功迁移导入 ${res.migrated_count} 个`);
        onchanged();
      }
    } catch (e) {
      toast(`迁移失败：${e}`);
    }
  }

  async function saveDelay() {
    try {
      await api.setInjectionDelay(delayDraft);
      toast("已保存注入延迟设置");
      onchanged();
    } catch (e) {
      toast(String(e));
    }
  }

  async function saveGithubSettings() {
    try {
      await api.setGithubToken(tokenDraft);
      await api.setGithubMirror(mirrorDraft);
      toast("已保存云端与镜像加速设置");
      onchanged();
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
  <div class="flex items-center gap-3 px-8 pt-3 pb-4 shrink-0 max-w-3xl w-full mx-auto">
    <button
      class="glass radius-pill pl-2.5 pr-3.5 h-8 text-xs font-semibold flex items-center gap-1 cursor-pointer transition-transform hover:-translate-x-0.5"
      onclick={onback}
    >
      <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
        <path d="M7 1L2.5 5L7 9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
      <span>返回</span>
    </button>
    <h2 class="text-2xl font-bold tracking-tight">偏好设置</h2>
  </div>

  <div class="flex flex-col gap-4 px-8 pb-10 overflow-y-auto flex-1 min-h-0 max-w-3xl w-full mx-auto">
    <!-- 分组 1：外观与偏好 -->
    <section class="glass radius-panel p-6 flex flex-col gap-4">
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

      <div class="border-t border-[var(--glass-stroke)] pt-3.5 flex items-center justify-between gap-3">
        <div>
          <p class="text-sm font-medium">安装后自动启用</p>
          <p class="text-xs text-secondary mt-0.5">解压并导入 Mod 成功后，立即创建软链接部署到游戏</p>
        </div>
        <Toggle checked={config?.auto_enable ?? false} ariaLabel="自动启用" onchange={toggleAutoEnable} />
      </div>

      <div class="border-t border-[var(--glass-stroke)] pt-3.5 flex items-center justify-between gap-3">
        <div>
          <p class="text-sm font-medium">多 Mod 风险提示</p>
          <p class="text-xs text-secondary mt-0.5">同一角色启用两个及以上 Mod 时显示黄灯与详情提示</p>
        </div>
        <Toggle
          checked={config?.warn_multiple_mods ?? true}
          ariaLabel="多 Mod 风险提示"
          onchange={toggleMultipleModWarning}
        />
      </div>

      <div class="border-t border-[var(--glass-stroke)] pt-3.5 flex items-center justify-between gap-3">
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
            class="accent-fill accent-text radius-pill h-8 px-3.5 text-xs font-semibold cursor-pointer disabled:opacity-50"
            disabled={!catNameDraft.trim() || catNameDraft.trim() === config?.character_category_name}
            onclick={saveCatName}
          >
            保存
          </button>
        </div>
      </div>
    </section>

    <!-- 分组 2：游戏与 XXMI/SRMI 集成 -->
    <section class="glass radius-panel p-6 flex flex-col gap-4">
      <div class="flex flex-col gap-1">
        <h3 class="text-xs font-semibold uppercase tracking-wider text-secondary">游戏与 XXMI/SRMI 集成</h3>
        <p class="text-xs text-secondary leading-relaxed">游戏路径、便携运行时与分布式 Mod 源统一在 LiquiMod 内管理</p>
      </div>

      <!-- 快速向导操作卡片栏 (三大平行集成动作，统一优雅毛玻璃与晶体矢量图标) -->
      <div class="grid grid-cols-1 sm:grid-cols-3 gap-2.5">
        <button
          class="glass radius-card p-4 flex flex-col items-center justify-center text-center gap-1.5 cursor-pointer transition-all hover:scale-[1.02] hover:bg-[var(--item-hover)] active:scale-95 group"
          style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
          onclick={handleInitMigoto}
        >
          <div class="w-9 h-9 rounded-xl grid place-items-center bg-[var(--input-bg)] text-[var(--accent)] group-hover:scale-110 transition-transform">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/>
              <polyline points="3.27 6.96 12 12.01 20.73 6.96"/>
              <line x1="12" y1="22.08" x2="12" y2="12"/>
            </svg>
          </div>
          <span class="text-xs font-semibold text-[var(--text)]">初始化本地运行时</span>
          <span class="text-[11px] text-secondary">只创建标准入口，不复制 Mod</span>
        </button>

        <button
          class="glass radius-card p-4 flex flex-col items-center justify-center text-center gap-1.5 cursor-pointer transition-all hover:scale-[1.02] hover:bg-[var(--item-hover)] active:scale-95 group"
          style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
          onclick={handleMigrateOldMigoto}
        >
          <div class="w-9 h-9 rounded-xl grid place-items-center bg-[var(--input-bg)] text-emerald-500 group-hover:scale-110 transition-transform">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h7"/>
              <polyline points="16 3 21 3 21 8"/>
              <line x1="10" y1="14" x2="21" y2="3"/>
            </svg>
          </div>
          <span class="text-xs font-semibold text-[var(--text)]">迁移旧版 Mod</span>
          <span class="text-[11px] text-secondary">批量导入旧目录资产</span>
        </button>

        <button
          class="glass radius-card p-4 flex flex-col items-center justify-center text-center gap-1.5 cursor-pointer transition-all hover:scale-[1.02] hover:bg-[var(--item-hover)] active:scale-95 group"
          style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
          onclick={addModSource}
        >
          <div class="w-9 h-9 rounded-xl grid place-items-center bg-[var(--input-bg)] text-amber-500 group-hover:scale-110 transition-transform">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 3v3m0 12v3M3 12h3m12 0h3m-2.64-6.36l-2.12 2.12m-6.48 6.48l-2.12 2.12m0-10.72l2.12 2.12m6.48 6.48l2.12 2.12"/>
              <circle cx="12" cy="12" r="4"/>
            </svg>
          </div>
          <span class="text-xs font-semibold text-[var(--text)]">连接外部 Mod 源</span>
          <span class="text-[11px] text-secondary">任意磁盘均可索引与启用</span>
        </button>
      </div>

      <!-- 路径配置项列表 -->
      <div class="flex flex-col gap-3 pt-1">
        <!-- 游戏主程序 -->
        <div class="border-t border-[var(--glass-stroke)] pt-3.5 flex flex-col sm:flex-row sm:items-center justify-between gap-2.5">
          <div class="min-w-0 flex-1">
            <p class="text-sm font-medium">游戏主程序</p>
            <p class="text-xs text-secondary truncate mt-0.5 font-mono select-all" title={config?.game_exe ?? "未配置"}>{config?.game_exe ?? "未配置"}</p>
          </div>
          <div class="flex items-center justify-end gap-1.5 shrink-0">
            <button
              class="glass radius-pill h-8 px-3 text-xs font-medium cursor-pointer flex items-center gap-1.5 hover:bg-[var(--item-hover)]"
              disabled={detectingGame}
              onclick={autoDetectGame}
              title="自动从注册表和运行日志嗅探 StarRail.exe"
            >
              {#if detectingGame}
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="animate-spin text-[var(--accent)]">
                  <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67"/>
                </svg>
                <span>探测中…</span>
              {:else}
                <span>🔍</span>
                <span>自动探测</span>
              {/if}
            </button>
            {#if config?.game_exe}
              <button
                class="glass radius-pill w-8 h-8 grid place-items-center cursor-pointer text-secondary hover:text-[var(--text)] hover:bg-[var(--item-hover)]"
                title="在资源管理器中定位"
                onclick={() => config?.game_exe && api.openPathInExplorer(config.game_exe)}
              >
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>
              </button>
            {/if}
            <button class="glass radius-pill h-8 px-3 text-xs font-medium cursor-pointer hover:bg-[var(--item-hover)]" onclick={() => pickExe("game")}>
              选择…
            </button>
          </div>
        </div>

        <!-- XXMI 原生 Hook 核心：不再配置或启动 Loader.exe -->
        <div class="border-t border-[var(--glass-stroke)] pt-3.5 flex flex-col gap-3">
          <div class="flex flex-col sm:flex-row sm:items-start justify-between gap-2.5">
            <div class="min-w-0 flex-1">
              <p class="text-sm font-medium">XXMI 原生 Hook 核心</p>
              <p class="text-xs text-secondary mt-0.5">启动 Mod 时由 3dmloader.dll 直接挂钩游戏进程，不再弹出或依赖 3DMigoto Loader.exe。</p>
            </div>
            <div class="flex items-center gap-1.5 shrink-0">
              <button class="glass radius-pill h-8 px-3 text-xs font-medium cursor-pointer hover:bg-[var(--item-hover)] disabled:opacity-50" disabled={checkingSrmi || installingCore !== null} onclick={handleCheckSrmiUpdate}>
                {checkingSrmi ? "检查中…" : "检查 SRMI 更新"}
              </button>
              <button class="glass radius-pill h-8 px-3 text-xs font-medium cursor-pointer hover:bg-[var(--item-hover)] disabled:opacity-50" disabled={checkingXxmi || installingCore !== null} onclick={handleCheckXxmiUpdate}>
                {checkingXxmi ? "检查中…" : "检查 XXMI 更新"}
              </button>
              <button class="accent-fill accent-text radius-pill h-8 px-3 text-xs font-semibold cursor-pointer disabled:opacity-50" disabled={installingCore !== null} onclick={() => installCore("XXMI")}>
                {installingCore === "XXMI" ? "安装中…" : "安装 / 修复"}
              </button>
            </div>
          </div>
          <div class="grid grid-cols-1 sm:grid-cols-2 gap-2">
            {#each coreStatuses as status (status.package)}
              <div class="p-3 radius-card" style="background: var(--input-bg); box-shadow: inset 0 0 0 0.5px var(--glass-stroke)">
                <div class="flex items-center justify-between gap-2">
                  <span class="text-xs font-semibold">{status.package} 官方套件</span>
                  <span class="text-[11px] font-semibold" class:text-emerald-500={status.ready} class:text-amber-500={!status.ready}>{status.ready ? "已就绪" : "缺少文件"}</span>
                </div>
                <p class="text-[11px] text-secondary font-mono truncate mt-1" title={status.package_dir}>{compactPath(status.package_dir)}</p>
                <div class="flex items-center justify-between gap-2 mt-2">
                  <span class="text-[11px] text-secondary">版本 {status.installed_version ?? "未安装"}</span>
                  <button class="glass radius-pill h-7 px-2.5 text-[11px] cursor-pointer hover:bg-[var(--item-hover)] disabled:opacity-50" disabled={installingCore !== null} onclick={() => installCore(status.package)}>{installingCore === status.package ? "处理中…" : "修复"}</button>
                </div>
              </div>
            {/each}
          </div>
          {#if srmiRelease}<p class="text-[11px] text-secondary">最新 SRMI：{srmiRelease.tag_name || srmiRelease.name} · 下载包签名已由后端校验。</p>{/if}
          {#if xxmiRelease}<p class="text-[11px] text-secondary">最新 XXMI：{xxmiRelease.tag_name || xxmiRelease.name} · 下载包签名已由后端校验。</p>{/if}
        </div>

        <!-- 3Dmigoto Mods 部署目录 -->
        <div class="border-t border-[var(--glass-stroke)] pt-3.5 flex flex-col sm:flex-row sm:items-center justify-between gap-2.5">
          <div class="min-w-0 flex-1">
            <p class="text-sm font-medium">3Dmigoto Mods 运行入口</p>
            <p class="text-xs text-secondary truncate mt-0.5 font-mono" title={config?.mods_dir ?? "未初始化"}>{compactPath(config?.mods_dir, config?.storage_root)}</p>
            <p class="text-[11px] text-secondary mt-1">此目录只保存 Junction 入口，不保存真实 Mod 文件；真实 Mod 可来自 Library 或任意已连接的外部源。</p>
          </div>
          <div class="flex items-center justify-end gap-1.5 shrink-0">
            {#if config?.mods_dir}
              <button
                class="glass radius-pill w-8 h-8 grid place-items-center cursor-pointer text-secondary hover:text-[var(--text)] hover:bg-[var(--item-hover)]"
                title="在资源管理器中打开"
                onclick={() => config?.mods_dir && api.openPathInExplorer(config.mods_dir)}
              >
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>
              </button>
            {/if}
            <span class="px-2.5 h-7 flex items-center radius-pill text-[11px] font-semibold text-emerald-500" style="background: color-mix(in srgb, #10b981 12%, transparent)">
              {config?.mods_dir ? "LiquiMod 托管" : "尚未初始化"}
            </span>
            <button
              class="glass radius-pill h-8 px-3 text-xs font-medium cursor-pointer hover:bg-[var(--item-hover)]"
              onclick={handleInitMigoto}
            >
              {config?.mods_dir ? "修复入口" : "初始化"}
            </button>
          </div>
        </div>

        <!-- 外部 Mod 源：只建立索引与 Junction，不接管源文件 -->
        <div class="border-t border-[var(--glass-stroke)] pt-3.5 flex flex-col gap-2.5">
          <div class="flex flex-col sm:flex-row sm:items-start justify-between gap-2.5">
            <div class="min-w-0 flex-1">
              <p class="text-sm font-medium">外部 Mod 源目录</p>
              <p class="text-xs text-secondary mt-0.5">Mod 可以分散在任意磁盘；添加源目录后会自动扫描、索引并按需建立 Junction。解除连接不会删除源文件。</p>
            </div>
            <button class="accent-fill accent-text radius-pill h-8 px-3 text-xs font-semibold cursor-pointer disabled:opacity-50" disabled={sourceBusy} onclick={addModSource}>{sourceBusy ? "处理中…" : "添加目录"}</button>
          </div>
          {#if config?.mod_sources?.length}
            <div class="flex flex-col gap-1.5">
              {#each config.mod_sources as source (source)}
                <div class="glass radius-card min-h-9 px-3 py-2 flex items-center gap-2">
                  <span class="w-2 h-2 rounded-full bg-emerald-500 shrink-0" aria-hidden="true"></span>
                  <span class="font-mono text-[11px] truncate flex-1" title={source}>{source}</span>
                  <button class="glass radius-pill h-7 px-2.5 text-[11px] text-secondary hover:text-[var(--danger)] hover:bg-[var(--item-hover)] cursor-pointer disabled:opacity-50" aria-label={`解除外部 Mod 源 ${source}`} disabled={sourceBusy} onclick={() => removeModSource(source)}>解除</button>
                </div>
              {/each}
            </div>
          {:else}
            <div class="p-3 radius-card text-xs text-secondary" style="background: var(--input-bg); box-shadow: inset 0 0 0 0.5px var(--glass-stroke)">尚未添加外部源。应用内安装的 Mod 默认保存在当前数据根下的 Library。</div>
          {/if}
        </div>

        <!-- LiquiMod 核心仓库 -->
        <div class="border-t border-[var(--glass-stroke)] pt-3.5 flex flex-col sm:flex-row sm:items-center justify-between gap-2.5">
          <div class="min-w-0 flex-1">
            <p class="text-sm font-medium">LiquiMod 核心仓库（Library）</p>
            <p class="text-xs text-secondary truncate mt-0.5 font-mono" title={config?.library_root ?? "…"}>{compactPath(config?.library_root, config?.storage_root)}</p>
          </div>
          <div class="flex items-center justify-end shrink-0">
            <button
              class="glass radius-pill h-8 px-3 text-xs font-medium cursor-pointer flex items-center gap-1 hover:bg-[var(--item-hover)]"
              onclick={() => config?.library_root && api.openPathInExplorer(config.library_root)}
            >
              <span>📂</span> 打开仓库
            </button>
          </div>
        </div>

        <!-- LiquiMod 数据存储根：迁移只复制并保留旧仓库，避免误删用户数据 -->
        <div class="border-t border-[var(--glass-stroke)] pt-3.5 flex flex-col gap-3">
          <div class="flex flex-col sm:flex-row sm:items-start justify-between gap-2.5">
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-2">
                <p class="text-sm font-medium">数据存储位置</p>
                <span class="px-1.5 py-0.5 radius-pill text-[10px] font-semibold text-[var(--accent)]" style="background: var(--accent-fill)">可迁移</span>
              </div>
              <p class="text-xs text-secondary mt-0.5">默认跟随软件所在盘；Mod 仓库、数据库、日志与托管 3DMigoto 会统一放在这里。</p>
            </div>
            <div class="flex items-center justify-end gap-1.5 shrink-0">
              <button
                class="glass radius-pill h-8 px-3 text-xs font-medium cursor-pointer flex items-center gap-1 hover:bg-[var(--item-hover)] disabled:opacity-50"
                disabled={storageBusy}
                onclick={pickStorageRoot}
              >
                {storageBusy ? "处理中…" : "迁移到…"}
              </button>
              <button
                class="glass radius-pill w-8 h-8 grid place-items-center cursor-pointer text-secondary hover:text-[var(--text)] hover:bg-[var(--item-hover)] disabled:opacity-50"
                title="打开数据存储目录"
                disabled={!storageInfo?.storage_root || storageBusy}
                onclick={() => storageInfo?.storage_root && api.openPathInExplorer(storageInfo.storage_root)}
              >
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>
              </button>
            </div>
          </div>

          <div class="grid grid-cols-1 sm:grid-cols-3 gap-2 text-xs">
            <div class="min-w-0 p-2.5 radius-card" style="background: var(--input-bg); box-shadow: inset 0 0 0 0.5px var(--glass-stroke)">
              <p class="text-secondary">当前数据根</p>
              <p class="font-mono truncate mt-1" title={storageInfo?.storage_root ?? config?.storage_root ?? "读取中…"}>{storageInfo?.storage_root ?? config?.storage_root ?? "读取中…"}</p>
            </div>
            <div class="p-2.5 radius-card" style="background: var(--input-bg); box-shadow: inset 0 0 0 0.5px var(--glass-stroke)">
              <p class="text-secondary">仓库占用</p>
              <p class="font-mono mt-1">{formatBytes(storageInfo?.bytes)} · {storageInfo?.files ?? "—"} 文件</p>
            </div>
            <div class="p-2.5 radius-card" style="background: var(--input-bg); box-shadow: inset 0 0 0 0.5px var(--glass-stroke)">
              <p class="text-secondary">所在盘可用空间</p>
              <p class="font-mono mt-1">{formatBytes(storageInfo?.available_bytes)}</p>
            </div>
          </div>

          {#if storageInfo?.previous_library_root}
            <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2.5 p-3 radius-card text-xs text-amber-600 dark:text-amber-300" style="background: color-mix(in srgb, var(--warning) 12%, transparent); box-shadow: inset 0 0 0 0.5px color-mix(in srgb, var(--warning) 40%, transparent)">
              <div class="min-w-0">
                <p class="font-semibold">迁移备份仍保留（不参与运行）</p>
                <p class="text-[11px] mt-0.5">当前使用新的 LiquiMod/Library；确认内容无误后可清理旧仓库。</p>
                <p class="font-mono truncate mt-0.5" title={storageInfo.previous_library_root}>{storageInfo.previous_library_root}</p>
              </div>
              <button
                class="radius-pill h-8 px-3 text-xs font-medium border border-amber-500/40 hover:bg-amber-500/10 cursor-pointer shrink-0 disabled:opacity-50"
                disabled={storageBusy}
                onclick={cleanupPreviousStorage}
              >
                清理旧仓库
              </button>
            </div>
          {/if}
        </div>
      </div>
    </section>

    <!-- 分组 3：启动与更新设置 -->
    <section class="glass radius-panel p-6 flex flex-col gap-4">
      <div>
        <h3 class="text-xs font-semibold uppercase tracking-wider text-secondary">启动与更新设置</h3>
        <p class="text-xs text-secondary mt-0.5">原生 XXMI Hook 启动、DLL 初始化延迟与官方核心更新</p>
      </div>

      <!-- 注入延迟调节 -->
      <div class="border-t border-[var(--glass-stroke)] pt-3.5 flex flex-col gap-2">
        <div class="flex items-center justify-between">
          <div>
            <p class="text-sm font-medium">XXMI DLL 初始化延迟</p>
            <p class="text-xs text-secondary mt-0.5">按官方 XXMI 方式写入 d3dx.ini 的 [System]，不会启动外部 Loader.exe（默认 500ms）</p>
          </div>
          <span class="text-xs font-mono font-bold text-[var(--accent)] px-2.5 py-0.5 glass radius-pill">
            {delayDraft} ms
          </span>
        </div>
        <div class="flex items-center gap-3">
          <input
            type="range"
            min="0"
            max="3000"
            step="100"
            bind:value={delayDraft}
            onchange={saveDelay}
            class="flex-1 accent-[var(--accent)] cursor-pointer"
          />
        </div>
      </div>

      <!-- 云端镜像与加速通道 -->
      <div class="border-t border-[var(--glass-stroke)] pt-3.5 flex flex-col gap-2.5">
        <div>
          <p class="text-sm font-medium">GitHub 镜像与加速通道 (可选)</p>
          <p class="text-xs text-secondary mt-0.5">国内环境若无法直连 GitHub，可配置镜像前缀 (如 https://ghproxy.net/)</p>
        </div>
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-2">
          <input
            bind:value={mirrorDraft}
            placeholder="镜像加速前缀 (如 https://ghproxy.net)"
            class="h-8 px-3 text-xs bg-transparent outline-none rounded-full"
            style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
          />
          <input
            bind:value={tokenDraft}
            type="password"
            placeholder="GitHub Personal Access Token (防限流)"
            class="h-8 px-3 text-xs bg-transparent outline-none rounded-full"
            style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
          />
        </div>
        <div class="flex justify-end">
          <button
            class="accent-fill accent-text radius-pill h-8 px-3.5 text-xs font-semibold cursor-pointer"
            onclick={saveGithubSettings}
          >
            保存加速配置
          </button>
        </div>
      </div>
    </section>

    <!-- 分组 4：解压密码本 -->
    <section class="glass radius-panel p-6 flex flex-col gap-4">
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

    <!-- 分组 5：维护与诊断 -->
    <section class="glass radius-panel p-6 flex flex-col gap-4">
      <div>
        <h3 class="text-xs font-semibold uppercase tracking-wider text-secondary">维护与系统诊断</h3>
        <p class="text-xs text-secondary mt-0.5">检查组件状态、同步磁盘索引或清理缓存</p>
      </div>

      <!-- 诊断指示灯 -->
      <div class="grid grid-cols-2 gap-2.5 text-xs">
        <div class="p-3 rounded-xl flex items-center justify-between" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke); background: var(--input-bg)">
          <span class="text-secondary font-medium">F10 游戏内热刷新</span>
          <span class="flex items-center gap-1.5 font-medium {diagStatus?.helper_ready ? 'text-emerald-500' : 'text-amber-500'}">
            <span class="w-2 h-2 rounded-full {diagStatus?.helper_ready ? 'bg-emerald-500 shadow-[0_0_6px_rgba(16,185,129,0.5)]' : 'bg-amber-500'}"></span>
            {diagStatus?.helper_ready ? "就绪" : "未启动/缺组件"}
          </span>
        </div>
        <div class="p-3 rounded-xl flex items-center justify-between" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke); background: var(--input-bg)">
          <span class="text-secondary font-medium">3Dmigoto 关联</span>
          <span class="flex items-center gap-1.5 font-medium {diagStatus?.mods_dir_configured ? 'text-emerald-500' : 'text-zinc-400'}">
            <span class="w-2 h-2 rounded-full {diagStatus?.mods_dir_configured ? 'bg-emerald-500 shadow-[0_0_6px_rgba(16,185,129,0.5)]' : 'bg-zinc-400'}"></span>
            {diagStatus?.mods_dir_configured ? "已配置" : "未配置"}
          </span>
        </div>
      </div>

      {#if diagStatus}
        <div class="flex flex-col gap-2">
          {#each diagStatus.checks as check (check.id)}
            <div
              class="radius-card p-3 flex flex-col gap-1.5"
              style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke); background: var(--input-bg)"
            >
              <div class="flex items-center justify-between gap-3 text-xs">
                <span class="flex min-w-0 items-center gap-2 font-medium">
                  <span class="w-2 h-2 shrink-0 rounded-full {checkDot(check.state)}"></span>
                  <span class="truncate">{check.label}</span>
                </span>
                <span class="shrink-0 font-semibold {checkTone(check.state)}">{checkLabel(check.state)}</span>
              </div>
              <p class="text-[11px] text-secondary break-words">{check.detail}</p>
              {#if check.remediation}
                <p class="text-[11px] text-secondary/80 break-words">{check.remediation}</p>
              {/if}
              {#if check.id === "webview2" && check.state === "fail"}
                <button
                  class="self-start glass radius-pill h-7 px-2.5 text-[11px] font-medium cursor-pointer"
                  onclick={openWebView2Download}
                >
                  打开 WebView2 下载页
                </button>
              {/if}
            </div>
          {/each}
        </div>

        {#if diagStatus.filesystem || diagStatus.deploy_strategy}
          <div class="radius-card p-3 flex flex-wrap items-center gap-x-4 gap-y-1 text-[11px]" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke); background: var(--input-bg)">
            {#if diagStatus.filesystem}
              <span class="text-secondary">文件系统：<strong class="text-[var(--text)]">{diagStatus.filesystem}</strong></span>
            {/if}
            {#if diagStatus.deploy_strategy}
              <span class="text-secondary">部署模式：<strong class="text-[var(--accent)]">{diagStatus.deploy_strategy}</strong></span>
            {/if}
          </div>
        {/if}

        {#if diagStatus.defender_command}
          <div class="radius-card p-3 flex flex-col gap-2" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke); background: var(--input-bg)">
            <div class="flex items-center justify-between gap-3">
              <span class="text-xs font-medium">Defender 排除项命令</span>
              <button class="glass radius-pill h-7 px-2.5 text-[11px] font-medium cursor-pointer" onclick={copyDefenderCommand}>复制命令</button>
            </div>
            <code class="text-[10px] text-secondary break-all select-text">{diagStatus.defender_command}</code>
          </div>
        {/if}
      {/if}

      <!-- 维护动作按钮组 -->
      <div class="border-t border-[var(--glass-stroke)] pt-3.5 flex items-center gap-2">
        <button
          class="glass radius-pill h-8 px-3.5 text-xs font-medium cursor-pointer flex items-center gap-1.5 hover:bg-[var(--item-hover)] disabled:opacity-50"
          disabled={busy}
          onclick={handleRescan}
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67"/>
          </svg>
          <span>全库扫描与对齐</span>
        </button>
        <button
          class="glass radius-pill h-8 px-3.5 text-xs font-medium cursor-pointer flex items-center gap-1.5 hover:bg-[var(--item-hover)] disabled:opacity-50"
          disabled={busy}
          onclick={handleCleanCache}
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M3 6h18m-2 0v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6m3 0V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/>
          </svg>
          <span>清理封面缓存</span>
        </button>
        <button
          class="glass radius-pill h-8 px-3.5 text-xs font-medium cursor-pointer flex items-center gap-1.5 hover:bg-[var(--item-hover)] disabled:opacity-50"
          disabled={busy || !diagStatus?.mods_dir_configured}
          onclick={handleRepairDeployment}
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M14.7 6.3a4 4 0 0 0-5.4 5.4L3 18v3h3l6.3-6.3a4 4 0 0 0 5.4-5.4l-2.2 2.2-2.8-.6-.6-2.8 2.6-1.8Z"/>
          </svg>
          <span>修复 Mod 部署</span>
        </button>
      </div>

      <!-- 角色数据云端热更新卡片 -->
      <div class="border-t border-[var(--glass-stroke)] pt-3.5 flex flex-col gap-2.5">
        <div class="flex items-center justify-between">
          <div>
            <p class="text-sm font-medium flex items-center gap-1.5">
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-[var(--accent)]">
                <path d="M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9Z"/>
              </svg>
              <span>崩坏：星穹铁道 角色数据云端同步</span>
            </p>
            <p class="text-xs text-secondary mt-0.5">
              当前版本：<span class="font-mono font-medium text-[var(--accent)]">{localAssetVersion || "内嵌预置版本"}</span>
            </p>
          </div>
          <div class="flex items-center gap-2">
            <button
              class="glass radius-pill h-8 px-3 text-xs font-medium cursor-pointer flex items-center gap-1.5 hover:bg-[var(--item-hover)] disabled:opacity-50"
              disabled={checkingUpdate || syncing}
              onclick={handleCheckAssetUpdate}
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="11" cy="11" r="8"/>
                <line x1="21" y1="21" x2="16.65" y2="16.65"/>
              </svg>
              <span>{checkingUpdate ? "检查中…" : "检查更新"}</span>
            </button>
            <button
              class="accent-fill accent-text radius-pill h-8 px-3.5 text-xs font-semibold cursor-pointer flex items-center gap-1.5 disabled:opacity-50"
              disabled={syncing || checkingUpdate}
              onclick={handleSyncAssets}
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>
              </svg>
              <span>{syncing ? "正在同步…" : "同步星铁数据"}</span>
            </button>
          </div>
        </div>

        {#if syncProgress}
          <div class="p-3.5 radius-card flex flex-col gap-1.5 bg-[var(--glass-tint)]" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)">
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
      <div class="border-t border-[var(--glass-stroke)] pt-3.5 flex flex-col gap-2">
        <div class="flex items-center justify-between">
          <p class="text-sm font-medium">运行日志</p>
          <div class="flex gap-1.5">
            <button class="glass radius-pill h-7 px-2.5 text-xs cursor-pointer" onclick={refreshLog}>刷新</button>
            <button class="glass radius-pill h-7 px-2.5 text-xs cursor-pointer" onclick={copyLog}>复制</button>
          </div>
        </div>
        <pre
          class="text-[11px] font-mono rounded-xl p-3 max-h-40 overflow-auto whitespace-pre-wrap break-all select-text leading-relaxed opacity-85"
          style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke); background: var(--input-bg)"
        >{formatLog(logText) || "（暂无日志）"}</pre>
      </div>

      <!-- 关于信息 -->
      <div class="border-t border-[var(--glass-stroke)] pt-3.5 text-xs text-secondary flex items-center justify-between">
        <span>LiquiMod · 星轨流光 —— 崩坏：星穹铁道 现代化 Mod 管理器</span>
        <span class="font-mono text-[var(--accent)] font-medium">v0.6.0 (Rust + Tauri 2 + Svelte 5)</span>
      </div>
    </section>
  </div>
</div>
