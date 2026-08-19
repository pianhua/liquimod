<script lang="ts">
  import type { ModSort } from "$lib/view";
  import type { ConflictReportDto, CharacterSortOption } from "$lib/api";
  import PresetMenu from "./PresetMenu.svelte";

  let {
    crumbs,
    query = $bindable(""),
    sort = $bindable(),
    charSort = $bindable("default" as CharacterSortOption),
    isCharGrid = false,
    showSort,
    showSettings = false,
    conflicts = [],
    workMode = "play",
    ontoggleworkmode,
    onlaunchgame,
    onlaunchloader,
    onrefreshgame = undefined,
    ontogglesettings,
    onapplied,
  }: {
    crumbs: string[];
    query?: string;
    sort: ModSort;
    charSort?: CharacterSortOption;
    isCharGrid?: boolean;
    showSort: boolean;
    showSettings?: boolean;
    conflicts?: ConflictReportDto[];
    workMode?: "play" | "dev";
    ontoggleworkmode?: () => void;
    onlaunchgame: () => void;
    onlaunchloader: () => void;
    onrefreshgame?: () => void;
    ontogglesettings: () => void;
    onapplied: () => void;
  } = $props();

  let conflictModalOpen = $state(false);
  let showDevKeyHelp = $state(false);
</script>

<header class="relative z-30 flex items-center justify-between h-14 px-6 shrink-0 gap-4" aria-label="全局控制台">
  <!-- 左侧：面包屑导航 -->
  <nav class="flex items-center text-sm text-secondary min-w-0 max-w-[260px] truncate" aria-label="面包屑">
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="mr-2 shrink-0 opacity-70">
      <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>
      <polyline points="9 22 9 12 15 12 15 22"/>
    </svg>
    {#each crumbs as crumb, i (i)}
      {#if i > 0}<span class="mx-1.5 opacity-40">/</span>{/if}
      <span class={i === crumbs.length - 1 ? "font-semibold truncate text-[var(--text)]" : "truncate"}>
        {crumb}
      </span>
    {/each}
  </nav>

  <!-- 中间：全局超级搜索栏 -->
  <div class="flex-1 max-w-md mx-auto min-w-0">
    <div class="glass radius-pill flex items-center gap-2 pl-3.5 pr-2.5 h-8 w-full transition-all focus-within:shadow-md focus-within:scale-[1.01]"
      style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
    >
      <svg width="12" height="12" viewBox="0 0 13 13" fill="none" class="shrink-0 text-secondary">
        <circle cx="5.5" cy="5.5" r="4" stroke="currentColor" stroke-width="1.3" />
        <path d="M8.8 8.8L12 12" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" />
      </svg>
      <input
        bind:value={query}
        type="search"
        placeholder="搜索角色或 Mod (Ctrl+K)…"
        class="flex-1 min-w-0 bg-transparent outline-none text-xs placeholder:text-[var(--text-secondary)]"
      />
      <kbd class="hidden sm:inline-flex items-center px-1.5 py-0.5 text-[10px] font-mono text-secondary rounded bg-black/5 dark:bg-white/10">
        Ctrl K
      </kbd>
    </div>
  </div>

  <!-- 右侧：全局核心操作组 -->
  <div class="flex items-center gap-2 shrink-0">
    <!-- 冲突预警 Badge -->
    {#if conflicts && conflicts.length > 0}
      <button
        class="radius-pill h-8 px-3 text-xs font-semibold flex items-center gap-1.5 cursor-pointer backdrop-blur-md transition-transform hover:scale-105"
        style="background: rgba(239, 68, 68, 0.16); color: #ef4444; box-shadow: inset 0 0 0 0.5px rgba(239, 68, 68, 0.4)"
        title={`发现 ${conflicts.length} 处 Mod Hash 冲突！点击查看`}
        onclick={() => (conflictModalOpen = true)}
      >
        <span class="w-1.5 h-1.5 rounded-full bg-red-500 animate-ping"></span>
        <span>{conflicts.length} 处冲突</span>
      </button>
    {/if}

    {#if isCharGrid}
      <div class="glass radius-pill h-8 px-2.5 flex items-center">
        <select
          bind:value={charSort}
          aria-label="角色排序"
          class="bg-transparent outline-none text-xs cursor-pointer text-secondary hover:text-[var(--text)] font-medium"
        >
          <option value="default">默认排序 (喜爱置顶)</option>
          <option value="name">名称 (A-Z)</option>
          <option value="mods">Mod 数量</option>
          <option value="enabled">启用优先</option>
          <option value="rarity">星级稀有度</option>
        </select>
      </div>
    {:else if showSort}
      <div class="glass radius-pill h-8 px-2.5 flex items-center">
        <select
          bind:value={sort}
          aria-label="排序方式"
          class="bg-transparent outline-none text-xs cursor-pointer"
        >
          <option value="recent">最近安装</option>
          <option value="name">名称</option>
          <option value="enabled">启用优先</option>
        </select>
      </div>
    {/if}

    <!-- 预设管理入口 -->
    <PresetMenu {onapplied} />

    <!-- 工作模式切换胶囊 (Play / Dev) -->
    <div class="relative flex items-center h-8 glass radius-pill px-1 gap-1">
      <button
        class="h-6 px-2 text-[11px] font-medium flex items-center gap-1 cursor-pointer rounded-full transition-all"
        class:accent-fill={workMode === "play"}
        class:accent-text={workMode === "play"}
        class:text-secondary={workMode !== "play"}
        onclick={() => ontoggleworkmode?.()}
        title={workMode === "play" ? "当前为游玩模式：极致流畅无Dump开销。点击切换" : "点击切换为游玩模式"}
      >
        <span>🎮</span>
        <span>游玩</span>
      </button>
      <button
        class="h-6 px-2 text-[11px] font-medium flex items-center gap-1 cursor-pointer rounded-full transition-all"
        class:accent-fill={workMode === "dev"}
        class:accent-text={workMode === "dev"}
        class:text-secondary={workMode !== "dev"}
        onclick={() => ontoggleworkmode?.()}
        title={workMode === "dev" ? "当前为抓取模式：开启着色器/Hash捕获。点击切换" : "点击切换为抓取模式"}
      >
        <span>🛠️</span>
        <span>抓取</span>
      </button>
      {#if workMode === "dev"}
        <button
          class="w-5 h-5 rounded-full grid place-items-center text-secondary hover:text-[var(--text)] hover:bg-black/5 dark:hover:bg-white/10 cursor-pointer"
          title="查看 3Dmigoto 抓取快捷键"
          onclick={(e) => {
            e.stopPropagation();
            showDevKeyHelp = !showDevKeyHelp;
          }}
        >
          <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <circle cx="12" cy="12" r="10"></circle>
            <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"></path>
            <line x1="12" y1="17" x2="12.01" y2="17"></line>
          </svg>
        </button>

        {#if showDevKeyHelp}
          <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
          <div
            class="absolute right-0 top-full mt-2 w-80 p-4 radius-card shadow-2xl z-50 flex flex-col gap-2.5 text-xs animate-in fade-in zoom-in-95 border border-[var(--glass-stroke)]"
            style="background: var(--bg); color: var(--text);"
            role="dialog"
            tabindex="-1"
            onclick={(e) => e.stopPropagation()}
            onkeydown={(e) => e.stopPropagation()}
          >
            <div class="flex items-center justify-between font-bold text-[var(--accent)]">
              <span class="flex items-center gap-1.5">
                <span>🛠️</span>
                <span>3Dmigoto 抓取小键盘速查</span>
              </span>
              <button
                class="w-5 h-5 rounded-full grid place-items-center text-secondary hover:text-[var(--text)] cursor-pointer"
                onclick={() => (showDevKeyHelp = false)}
              >
                ✕
              </button>
            </div>
            <p class="text-[11px] text-secondary leading-relaxed">
              游戏内使用数字小键盘实时捕获 Hash，已启用自动复制到剪贴板。
            </p>
            <div class="flex flex-col gap-1.5 mt-1 font-mono text-[11px]">
              <div class="flex items-center justify-between py-1 px-2 rounded-lg bg-black/5 dark:bg-white/5">
                <span class="text-secondary font-sans text-xs">切换目标(顶点/索引/着色器)</span>
                <kbd class="font-bold text-[var(--accent)]">Num /</kbd>
              </div>
              <div class="flex items-center justify-between py-1 px-2 rounded-lg bg-black/5 dark:bg-white/5">
                <span class="text-secondary font-sans text-xs">上一个 / 下一个元素</span>
                <div class="flex gap-1">
                  <kbd class="font-bold text-[var(--accent)]">Num 1</kbd>
                  <kbd class="font-bold text-[var(--accent)]">Num 2</kbd>
                </div>
              </div>
              <div class="flex items-center justify-between py-1 px-2 rounded-lg bg-black/5 dark:bg-white/5">
                <span class="text-secondary font-sans text-xs">标记并复制当前 Hash</span>
                <kbd class="font-bold text-[var(--accent)]">Num *</kbd>
              </div>
              <div class="flex items-center justify-between py-1 px-2 rounded-lg bg-black/5 dark:bg-white/5">
                <span class="text-secondary font-sans text-xs">隐藏当前选中的模型</span>
                <kbd class="font-bold text-[var(--accent)]">Num 0</kbd>
              </div>
              <div class="flex items-center justify-between py-1 px-2 rounded-lg bg-black/5 dark:bg-white/5">
                <span class="text-secondary font-sans text-xs">导出 HLSL 反汇编</span>
                <kbd class="font-bold text-[var(--accent)]">Num .</kbd>
              </div>
            </div>
          </div>
        {/if}
      {/if}
    </div>

    <!-- 启动组合控制组 -->
    <div class="flex items-center h-8 glass radius-pill px-0.5">
      <button
        class="h-7 px-3 text-xs font-semibold flex items-center gap-1.5 cursor-pointer rounded-full transition-all hover:bg-black/5 dark:hover:bg-white/10"
        onclick={onlaunchgame}
        title="启动游戏本体"
      >
        <svg width="10" height="10" viewBox="0 0 11 11" fill="currentColor">
          <path d="M2.5 1.5v8l7-4-7-4z" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round" />
        </svg>
        启动游戏
      </button>
      <span class="w-[1px] h-3.5 bg-[var(--glass-stroke)] mx-0.5 opacity-60"></span>
      <button
        class="h-7 px-2.5 text-xs text-secondary flex items-center gap-1 cursor-pointer rounded-full transition-all hover:bg-black/5 dark:hover:bg-white/10 hover:text-[var(--text)]"
        onclick={onlaunchloader}
        title="启动 3Dmigoto Loader"
      >
        <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
          <path d="M6 1.5v5M3.8 3.8 6 1.5l2.2 2.3M2 7.5v2a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1v-2" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
        启动加载器
      </button>
      {#if onrefreshgame}
        <span class="w-[1px] h-3.5 bg-[var(--glass-stroke)] mx-0.5 opacity-60"></span>
        <button
          class="h-7 px-2.5 text-xs text-secondary flex items-center gap-1 cursor-pointer rounded-full transition-all hover:bg-black/5 dark:hover:bg-white/10 hover:text-[var(--text)]"
          onclick={onrefreshgame}
          title="向游戏发送 F10 刷新信号"
        >
          <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67"/>
          </svg>
          刷新
        </button>
      {/if}
    </div>

    <!-- 全局偏好设置按钮 -->
    <button
      class="glass radius-pill w-8 h-8 grid place-items-center cursor-pointer transition-transform hover:rotate-45"
      style={showSettings ? "background: var(--accent-fill); color: var(--accent)" : ""}
      aria-label="设置"
      title="偏好设置"
      onclick={ontogglesettings}
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="3"/>
        <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>
      </svg>
    </button>
  </div>
</header>

<!-- 冲突诊断对话框 -->
{#if conflictModalOpen}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm grid place-items-center p-6"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    onclick={() => (conflictModalOpen = false)}
    onkeydown={(e) => e.key === "Escape" && (conflictModalOpen = false)}
  >
    <div
      class="glass radius-panel p-6 max-w-lg w-full flex flex-col gap-4 shadow-2xl"
      role="document"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-2 text-red-500 font-bold">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
            <line x1="12" y1="9" x2="12" y2="13"/>
            <line x1="12" y1="17" x2="12.01" y2="17"/>
          </svg>
          <h3>Mod 覆盖冲突诊断 ({conflicts.length})</h3>
        </div>
        <button
          class="glass radius-pill w-7 h-7 grid place-items-center cursor-pointer text-secondary"
          aria-label="关闭"
          onclick={() => (conflictModalOpen = false)}
        >
          ✕
        </button>
      </div>

      <p class="text-xs text-secondary">
        以下已启用的 Mod 修改了游戏中相同的模型或贴图 Hash，同时启用可能导致游戏内闪烁或模型撕裂。
      </p>

      <div class="flex flex-col gap-3 max-h-80 overflow-y-auto pr-1">
        {#each conflicts as c (c.hash)}
          <div class="p-3 radius-card flex flex-col gap-2" style="background: rgba(239, 68, 68, 0.08); border: 1px solid rgba(239, 68, 68, 0.2)">
            <div class="flex items-center justify-between text-xs">
              <span class="font-mono font-bold text-red-500">Hash: {c.hash}</span>
              <span class="text-secondary text-[11px] font-mono">{c.section}</span>
            </div>
            <div class="flex flex-col gap-1.5">
              {#each c.conflicting_mods as mod (mod.id)}
                <div class="flex items-center justify-between text-xs py-1 px-2 rounded bg-black/5 dark:bg-white/5">
                  <span class="font-medium truncate">{mod.character} · {mod.name}</span>
                </div>
              {/each}
            </div>
          </div>
        {/each}
      </div>

      <div class="flex justify-end pt-2 border-t border-[var(--glass-stroke)]">
        <button
          class="glass radius-pill h-8 px-4 text-xs font-semibold cursor-pointer"
          onclick={() => (conflictModalOpen = false)}
        >
          知道了
        </button>
      </div>
    </div>
  </div>
{/if}
