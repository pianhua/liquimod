<script lang="ts">
  import { onMount } from "svelte";
  import {
    api,
    type DiagnosticsCenterDto,
    type DiagnosticCheckDto,
    type ModDeploymentState,
    type ModDiagnosticDto,
  } from "$lib/api";
  import { toast } from "$lib/toast.svelte";
  import {
    IconAlertCircle,
    IconAlertTriangle,
    IconArrowLeft,
    IconCheckCircle,
    IconChevronDown,
    IconInfo,
    IconLink,
    IconPackage,
    IconRefresh,
    IconSearch,
    IconShieldCheck,
    IconWrench,
  } from "$lib/components/icons";

  let {
    onback,
    onchanged,
  }: {
    onback: () => void;
    onchanged: () => void;
  } = $props();

  type ModFilter = "all" | "attention" | "enabled" | "external";

  let report = $state<DiagnosticsCenterDto | null>(null);
  let loading = $state(true);
  let refreshing = $state(false);
  let repairBusy = $state(false);
  let error = $state("");
  let query = $state("");
  let filter = $state<ModFilter>("all");
  let expandedHash = $state<string | null>(null);
  let expandedVariable = $state<string | null>(null);
  let requestSeq = 0;

  const filterLabels: Record<ModFilter, string> = {
    all: "全部",
    attention: "需处理",
    enabled: "已启用",
    external: "外部源",
  };

  const stateLabels: Record<ModDeploymentState, string> = {
    disabled: "已禁用",
    deployed: "已部署",
    missing: "缺少部署",
    mismatched: "目标不一致",
    unexpected: "意外部署",
    source_unavailable: "源离线",
    unsupported: "不支持的路径",
    not_configured: "未配置",
  };

  function needsAttention(mod: Pick<ModDiagnosticDto, "deployment_state" | "source_available">): boolean {
    return !mod.source_available || !["disabled", "deployed"].includes(mod.deployment_state);
  }

  function statusIcon(state: ModDeploymentState) {
    if (state === "deployed" || state === "disabled") return IconCheckCircle;
    if (state === "source_unavailable" || state === "unsupported") return IconAlertTriangle;
    return IconAlertCircle;
  }

  function statusClass(state: ModDeploymentState): string {
    if (state === "deployed") return "text-emerald-400 bg-emerald-400/10 border-emerald-400/20";
    if (state === "disabled") return "text-secondary bg-white/[0.04] border-[var(--glass-stroke)]";
    if (state === "source_unavailable" || state === "unsupported") {
      return "text-amber-300 bg-amber-400/10 border-amber-300/20";
    }
    return "text-red-300 bg-red-400/10 border-red-300/20";
  }

  function checkClass(state: DiagnosticCheckDto["state"]): string {
    if (state === "pass") return "text-emerald-400";
    if (state === "warn") return "text-amber-300";
    if (state === "fail") return "text-red-300";
    return "text-secondary";
  }

  function checkIcon(state: DiagnosticCheckDto["state"]) {
    if (state === "pass") return IconCheckCircle;
    if (state === "warn" || state === "fail") return IconAlertTriangle;
    return IconInfo;
  }

  let filteredMods = $derived.by(() => {
    const rows = report?.mods ?? [];
    const normalized = query.trim().toLowerCase();
    return rows.filter((row) => {
      if (filter === "attention" && !needsAttention(row)) return false;
      if (filter === "enabled" && !row.enabled) return false;
      if (filter === "external" && row.storage_kind !== "external") return false;
      if (!normalized) return true;
      return `${row.name} ${row.character}`.toLowerCase().includes(normalized);
    });
  });

  let conflictCount = $derived((report?.hash_conflicts.length ?? 0) + (report?.variable_conflicts.length ?? 0));
  let environmentFailures = $derived(
    (report?.environment.checks ?? []).filter((check) => check.state === "fail").length,
  );
  let environmentWarnings = $derived(
    (report?.environment.checks ?? []).filter((check) => check.state === "warn").length,
  );

  async function refresh(showSpinner = true) {
    const seq = ++requestSeq;
    if (showSpinner) refreshing = true;
    error = "";
    try {
      const next = await api.getDiagnosticsCenter();
      if (seq !== requestSeq) return;
      report = next;
    } catch (e) {
      if (seq === requestSeq) error = String(e);
    } finally {
      if (seq === requestSeq) {
        loading = false;
        refreshing = false;
      }
    }
  }

  async function repairDeployment() {
    if (repairBusy) return;
    repairBusy = true;
    try {
      await api.repairDeployment();
      toast("部署对账完成");
      await refresh(false);
      onchanged();
    } catch (e) {
      toast(`部署修复失败：${e}`);
    } finally {
      repairBusy = false;
    }
  }

  function toggleHash(hash: string) {
    expandedHash = expandedHash === hash ? null : hash;
  }

  function toggleVariable(variable: string) {
    expandedVariable = expandedVariable === variable ? null : variable;
  }

  onMount(() => {
    void refresh(false);
  });
</script>

<div class="flex flex-col flex-1 min-h-0 view-transition">
  <header class="shrink-0 flex items-center justify-between px-6 py-4 border-b border-[var(--glass-stroke)]">
    <div class="flex items-center gap-3 min-w-0">
      <button
        class="w-8 h-8 grid place-items-center rounded-full text-secondary hover:text-[var(--text)] hover:bg-[var(--item-hover)] transition-colors cursor-pointer"
        aria-label="返回资源库"
        title="返回资源库"
        onclick={onback}
      >
        <IconArrowLeft size={16} />
      </button>
      <div class="min-w-0">
        <div class="flex items-center gap-2">
          <IconShieldCheck size={18} class="text-[var(--accent)]" />
          <h1 class="text-base font-semibold tracking-tight">Mod 状态与诊断中心</h1>
        </div>
        <p class="mt-0.5 text-xs text-secondary truncate">只读检查索引、源目录、部署一致性与已启用 Mod 冲突</p>
      </div>
    </div>
    <button
      class="glass-liquid-btn h-8 px-3 flex items-center gap-1.5 text-xs font-medium cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
      onclick={() => refresh()}
      disabled={loading || refreshing}
      aria-busy={refreshing}
    >
      <IconRefresh size={14} class={refreshing ? "animate-spin" : ""} />
      <span>{refreshing ? "检查中…" : "重新检查"}</span>
    </button>
  </header>

  <div class="flex-1 min-h-0 overflow-y-auto px-6 py-5">
    {#if loading}
      <div class="h-full grid place-items-center text-secondary text-sm">正在收集诊断信息…</div>
    {:else if error}
      <div class="max-w-2xl mx-auto mt-10 glass radius-panel p-6 text-center">
        <IconAlertCircle size={28} class="mx-auto text-red-300" />
        <h2 class="mt-3 text-sm font-semibold">诊断读取失败</h2>
        <p class="mt-1 text-xs text-secondary break-words">{error}</p>
        <button class="mt-4 glass-liquid-btn h-8 px-3 text-xs cursor-pointer" onclick={() => refresh()}>
          再试一次
        </button>
      </div>
    {:else if report}
      <div class="max-w-6xl mx-auto space-y-5">
        <section class="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-4 gap-3" aria-label="诊断摘要">
          <div class="glass radius-panel p-4">
            <div class="flex items-center justify-between">
              <span class="text-xs text-secondary">部署健康度</span>
              <IconLink size={15} class={report.deployment.attention_mods ? "text-amber-300" : "text-emerald-400"} />
            </div>
            <div class="mt-2 flex items-end gap-2">
              <strong class="text-2xl font-semibold">{report.deployment.healthy_mods}</strong>
              <span class="pb-0.5 text-xs text-secondary">/ {report.deployment.total_mods} 个 Mod</span>
            </div>
            <p class="mt-1 text-[11px] text-secondary">
              {report.deployment.attention_mods ? `${report.deployment.attention_mods} 个需要处理` : "数据库与磁盘状态一致"}
            </p>
          </div>
          <div class="glass radius-panel p-4">
            <div class="flex items-center justify-between">
              <span class="text-xs text-secondary">启用状态</span>
              <IconPackage size={15} class="text-[var(--accent)]" />
            </div>
            <div class="mt-2 flex items-end gap-2">
              <strong class="text-2xl font-semibold">{report.deployment.enabled_mods}</strong>
              <span class="pb-0.5 text-xs text-secondary">个已启用</span>
            </div>
            <p class="mt-1 text-[11px] text-secondary">诊断不会修改启停状态</p>
          </div>
          <div class="glass radius-panel p-4">
            <div class="flex items-center justify-between">
              <span class="text-xs text-secondary">环境检查</span>
              {#if environmentFailures > 0}
                <IconAlertCircle size={15} class="text-red-300" />
              {:else if environmentWarnings > 0}
                <IconAlertTriangle size={15} class="text-amber-300" />
              {:else}
                <IconCheckCircle size={15} class="text-emerald-400" />
              {/if}
            </div>
            <div class="mt-2 flex items-end gap-2">
              <strong class="text-2xl font-semibold">{environmentFailures + environmentWarnings}</strong>
              <span class="pb-0.5 text-xs text-secondary">个需关注</span>
            </div>
            <p class="mt-1 text-[11px] text-secondary">{report.environment.checks.length} 项检查已完成</p>
          </div>
          <div class="glass radius-panel p-4">
            <div class="flex items-center justify-between">
              <span class="text-xs text-secondary">冲突诊断</span>
              <IconAlertTriangle size={15} class={conflictCount ? "text-amber-300" : "text-emerald-400"} />
            </div>
            <div class="mt-2 flex items-end gap-2">
              <strong class="text-2xl font-semibold">{report.hash_conflicts.length + report.variable_conflicts.length}</strong>
              <span class="pb-0.5 text-xs text-secondary">组冲突</span>
            </div>
            <p class="mt-1 text-[11px] text-secondary">仅诊断，不自动拦截启停</p>
          </div>
        </section>

        <section class="grid grid-cols-1 xl:grid-cols-[minmax(0,1.15fr)_minmax(320px,0.85fr)] gap-4">
          <div class="glass radius-panel p-5">
            <div class="flex items-start justify-between gap-4">
              <div>
                <h2 class="text-sm font-semibold">部署环境</h2>
                <p class="mt-1 text-xs text-secondary">当前诊断只读取状态，不会重建物理部署。</p>
              </div>
              <button
                class="glass-liquid-btn h-8 px-3 flex items-center gap-1.5 text-xs cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
                onclick={repairDeployment}
                disabled={!report.deployment.configured || repairBusy}
                title={!report.deployment.configured ? "请先配置 3Dmigoto Mods 目录" : "按数据库 enabled 状态重新对账"}
              >
                <IconWrench size={14} />
                <span>{repairBusy ? "修复中…" : "修复部署"}</span>
              </button>
            </div>
            <div class="mt-4 grid grid-cols-1 sm:grid-cols-2 gap-2">
              <div class="rounded-xl border border-[var(--glass-stroke)] bg-[var(--card-bg)] px-3 py-2.5">
                <div class="text-[11px] text-secondary">部署策略</div>
                <div class="mt-1 text-xs font-medium">{report.deployment.strategy ?? "未配置"}</div>
              </div>
              <div class="rounded-xl border border-[var(--glass-stroke)] bg-[var(--card-bg)] px-3 py-2.5">
                <div class="text-[11px] text-secondary">文件系统</div>
                <div class="mt-1 text-xs font-medium">{report.deployment.filesystem ?? "未检测"}</div>
              </div>
            </div>
            <div class="mt-4 space-y-1.5">
              {#each report.environment.checks as check (check.id)}
                {@const CheckIcon = checkIcon(check.state)}
                <div class="flex items-start gap-2.5 rounded-lg px-2 py-1.5 hover:bg-[var(--item-hover)]">
                  <CheckIcon size={14} class={checkClass(check.state) + " mt-0.5"} />
                  <div class="min-w-0 flex-1">
                    <div class="flex items-center justify-between gap-3">
                      <span class="text-xs font-medium truncate">{check.label}</span>
                      <span class="text-[10px] text-secondary shrink-0">{check.state === "pass" ? "正常" : check.state === "warn" ? "警告" : check.state === "fail" ? "失败" : "未知"}</span>
                    </div>
                    <p class="mt-0.5 text-[11px] text-secondary break-words">{check.detail}</p>
                    {#if check.remediation}
                      <p class="mt-0.5 text-[11px] text-amber-200/80">建议：{check.remediation}</p>
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          </div>

          <div class="glass radius-panel p-5">
            <div class="flex items-start gap-2.5">
              <IconInfo size={16} class="mt-0.5 text-[var(--accent)]" />
              <div>
                <h2 class="text-sm font-semibold">安全边界</h2>
                <p class="mt-1 text-xs leading-5 text-secondary">状态中心不会自动启用、禁用、安装、卸载或移动 Mod；Hash 与变量冲突只用于诊断，不会阻止操作。</p>
              </div>
            </div>
            <div class="mt-4 space-y-2 text-xs text-secondary">
              <div class="rounded-xl border border-[var(--glass-stroke)] bg-[var(--card-bg)] p-3">
                <div class="font-medium text-[var(--text)]">外部 Mod</div>
                <div class="mt-1">仅读取规范化来源是否在线，离线时不触碰源文件。</div>
              </div>
              <div class="rounded-xl border border-[var(--glass-stroke)] bg-[var(--card-bg)] p-3">
                <div class="font-medium text-[var(--text)]">运行态保护</div>
                <div class="mt-1">修复部署仍遵守游戏运行状态与同卷 Junction 限制。</div>
              </div>
            </div>
          </div>
        </section>

        <section class="glass radius-panel overflow-hidden">
          <div class="px-5 pt-5 pb-3 flex flex-col lg:flex-row lg:items-center justify-between gap-3">
            <div>
              <h2 class="text-sm font-semibold">Mod 部署状态</h2>
              <p class="mt-1 text-xs text-secondary">按数据库状态与实际部署入口对账，外部源离线会单独标记。</p>
            </div>
            <div class="flex flex-wrap items-center gap-2">
              <div class="relative">
                <IconSearch size={14} class="absolute left-2.5 top-1/2 -translate-y-1/2 text-secondary" />
                <input
                  class="h-8 w-48 rounded-full border border-[var(--glass-stroke)] bg-[var(--card-bg)] pl-8 pr-3 text-xs outline-none focus:border-[var(--accent)]"
                  placeholder="搜索 Mod 或角色"
                  aria-label="搜索 Mod 或角色"
                  bind:value={query}
                />
              </div>
              <div class="flex items-center gap-0.5 rounded-full border border-[var(--glass-stroke)] bg-[var(--card-bg)] p-0.5">
                {#each Object.keys(filterLabels) as key}
                  {@const option = key as ModFilter}
                  <button
                    class="h-7 px-2.5 rounded-full text-[11px] cursor-pointer transition-colors {filter === option ? 'bg-[var(--accent-fill)] text-[var(--accent)] font-medium' : 'text-secondary hover:text-[var(--text)]'}"
                    aria-pressed={filter === option}
                    onclick={() => (filter = option)}
                  >
                    {filterLabels[option]}
                  </button>
                {/each}
              </div>
            </div>
          </div>
          {#if filteredMods.length === 0}
            <div class="px-5 py-12 text-center text-xs text-secondary">没有匹配的 Mod</div>
          {:else}
            <div class="border-t border-[var(--glass-stroke)]">
              {#each filteredMods as mod (mod.id)}
                {@const ModStatusIcon = mod.source_available ? statusIcon(mod.deployment_state) : IconAlertTriangle}
                <div class="flex items-center gap-3 px-5 py-3 border-b last:border-b-0 border-[var(--glass-stroke)] hover:bg-[var(--item-hover)] transition-colors">
                  <div class="w-8 h-8 shrink-0 rounded-lg grid place-items-center bg-[var(--accent-fill)] text-[var(--accent)]">
                    <ModStatusIcon size={16} />
                  </div>
                  <div class="min-w-0 flex-1">
                    <div class="flex items-center gap-2 min-w-0">
                      <span class="text-xs font-medium truncate">{mod.name}</span>
                      <span class="text-[10px] text-secondary shrink-0">{mod.character}</span>
                    </div>
                    <p class="mt-0.5 text-[11px] text-secondary truncate">{mod.detail}</p>
                  </div>
                  <span class="{mod.deployment_state === 'source_unavailable' ? 'inline-flex' : 'hidden sm:inline-flex'} text-[10px] px-2 py-1 rounded-full border {statusClass(mod.deployment_state)} shrink-0">{stateLabels[mod.deployment_state]}</span>
                  {#if !mod.source_available && mod.deployment_state !== "source_unavailable"}
                    <span class="inline-flex text-[10px] px-2 py-1 rounded-full border text-amber-300 bg-amber-400/10 border-amber-300/20 shrink-0" title="源目录不可用，依赖源文件的操作不可执行">源离线</span>
                  {/if}
                  <span class="hidden md:inline-flex text-[10px] px-2 py-1 rounded-full border text-secondary border-[var(--glass-stroke)] bg-white/[0.03] shrink-0">{mod.storage_kind === "external" ? "外部" : "托管"}</span>
                </div>
              {/each}
            </div>
          {/if}
        </section>

        <section class="grid grid-cols-1 xl:grid-cols-2 gap-4 pb-2">
          <div class="glass radius-panel p-5">
            <div class="flex items-center justify-between">
              <div>
                <h2 class="text-sm font-semibold">Hash 冲突</h2>
                <p class="mt-1 text-xs text-secondary">已启用 Mod 的静态覆盖项</p>
              </div>
              <span class="text-xs font-mono text-secondary">{report.hash_conflicts.length}</span>
            </div>
            {#if report.hash_conflicts.length === 0}
              <div class="mt-5 flex items-center gap-2 text-xs text-emerald-400"><IconCheckCircle size={14} /> 未发现冲突</div>
            {:else}
              <div class="mt-4 space-y-2">
                {#each report.hash_conflicts as conflict (conflict.hash)}
                  <div class="rounded-xl border border-[var(--glass-stroke)] bg-[var(--card-bg)] overflow-hidden">
                    <button class="w-full flex items-center justify-between gap-3 px-3 py-2.5 text-left cursor-pointer" onclick={() => toggleHash(conflict.hash)}>
                      <span class="min-w-0 text-xs truncate"><code>{conflict.hash}</code><span class="ml-2 text-secondary">{conflict.section || "未命名区段"}</span></span>
                      <IconChevronDown size={14} class={expandedHash === conflict.hash ? "rotate-180 transition-transform" : "transition-transform"} />
                    </button>
                    {#if expandedHash === conflict.hash}
                      <div class="px-3 pb-3 text-[11px] text-secondary space-y-1">
                        {#each conflict.conflicting_mods as mod}
                          <div>{mod.character} / {mod.name}</div>
                        {/each}
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          </div>

          <div class="glass radius-panel p-5">
            <div class="flex items-center justify-between">
              <div>
                <h2 class="text-sm font-semibold">变量冲突</h2>
                <p class="mt-1 text-xs text-secondary">[Constants] 中重复使用的变量</p>
              </div>
              <span class="text-xs font-mono text-secondary">{report.variable_conflicts.length}</span>
            </div>
            {#if report.variable_conflicts.length === 0}
              <div class="mt-5 flex items-center gap-2 text-xs text-emerald-400"><IconCheckCircle size={14} /> 未发现冲突</div>
            {:else}
              <div class="mt-4 space-y-2">
                {#each report.variable_conflicts as conflict (conflict.variable)}
                  <div class="rounded-xl border border-[var(--glass-stroke)] bg-[var(--card-bg)] overflow-hidden">
                    <button class="w-full flex items-center justify-between gap-3 px-3 py-2.5 text-left cursor-pointer" onclick={() => toggleVariable(conflict.variable)}>
                      <span class="min-w-0 text-xs truncate"><code>{conflict.variable}</code><span class="ml-2 text-secondary">{conflict.conflicting_mods.length} 个 Mod</span></span>
                      <IconChevronDown size={14} class={expandedVariable === conflict.variable ? "rotate-180 transition-transform" : "transition-transform"} />
                    </button>
                    {#if expandedVariable === conflict.variable}
                      <div class="px-3 pb-3 text-[11px] text-secondary space-y-1">
                        {#each conflict.conflicting_mods as mod}
                          <div>{mod.character} / {mod.name}</div>
                        {/each}
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        </section>
      </div>
    {/if}
  </div>
</div>
