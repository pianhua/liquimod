<script lang="ts">
  import type { ModSort } from "$lib/view";
  import type { CharacterSortOption } from "$lib/api";
  import PresetMenu from "./PresetMenu.svelte";
  import CustomSelect from "./CustomSelect.svelte";
  import {
    IconHome,
    IconSearch,
    IconClose,
    IconPackage,
    IconGamepad,
    IconWrench,
    IconRocket,
    IconRefresh,
    IconSettings,
    IconHeart,
    IconSort,
    IconZap,
    IconStar,
    IconClock,
    IconSortAlpha,
    IconSortSize,
    IconInfo,
  } from "$lib/components/icons";

  import { pushEscHandler, registerPopover, notifyPopoverOpened } from "$lib/esc";

  let {
    crumbs,
    query = $bindable(""),
    sort = $bindable(),
    charSort = $bindable("default" as CharacterSortOption),
    isCharGrid = false,
    showSort,
    showSettings = false,
    gameRunning = false,
    launchBusy = false,
    workMode = "play",
    ontoggleworkmode,
    onlaunchmodgame,
    onlaunchnativegame,
    onlaunchofficial = undefined,
    onrefreshgame = undefined,
    onimport = undefined,
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
    gameRunning?: boolean;
    launchBusy?: boolean;
    workMode?: "play" | "dev";
    ontoggleworkmode?: () => void;
    onlaunchmodgame: () => void;
    onlaunchnativegame: () => void;
    onlaunchofficial?: () => void;
    onrefreshgame?: () => void;
    onimport?: () => void;
    ontogglesettings: () => void;
    onapplied: () => void;
  } = $props();

  let showDevKeyHelp = $state(false);
  let showSortMenu = $state(false);
  let searchInputEl = $state<HTMLInputElement | null>(null);

  const closeDevKeyHelp = () => {
    showDevKeyHelp = false;
  };

  $effect(() => {
    return registerPopover(closeDevKeyHelp);
  });

  $effect(() => {
    if (showDevKeyHelp) {
      notifyPopoverOpened(closeDevKeyHelp);
      return pushEscHandler(() => {
        showDevKeyHelp = false;
        return true;
      });
    }
  });

  function handleWindowClick() {
    showSortMenu = false;
    showDevKeyHelp = false;
  }

  function onGlobalKeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && (e.key === "k" || e.key === "K")) {
      e.preventDefault();
      searchInputEl?.focus();
      searchInputEl?.select();
    } else if (
      e.key === "/" &&
      !(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement)
    ) {
      e.preventDefault();
      searchInputEl?.focus();
      searchInputEl?.select();
    }
  }
</script>

<svelte:window onclick={handleWindowClick} onkeydown={onGlobalKeydown} />

<header class="relative z-30 flex items-center justify-between h-13 px-6 shrink-0 gap-3 border-b border-[var(--glass-stroke)]" aria-label="全局控制台">
  <!-- 左侧：面包屑导航 -->
  <nav class="flex items-center text-sm text-[var(--text)] min-w-0 max-w-[200px] truncate" aria-label="面包屑">
    <IconHome size={15} class="mr-2 shrink-0 text-[var(--accent)]" />
    {#each crumbs as crumb, i (i)}
      {#if i > 0}<span class="mx-1.5 opacity-40 text-secondary">/</span>{/if}
      <span class={i === crumbs.length - 1 ? "font-semibold truncate text-[var(--text)]" : "truncate text-secondary"}>
        {crumb}
      </span>
    {/each}
  </nav>

  <!-- 中间：全局超级搜索栏 -->
  <div class="flex-1 max-w-sm mx-auto min-w-0">
    <div class="group/search glass-search-capsule flex items-center gap-2 pl-3 pr-2 h-8 w-full">
      <IconSearch size={14} class="shrink-0 text-secondary transition-colors duration-200 group-focus-within/search:text-[var(--accent)]" />
      <input
        bind:this={searchInputEl}
        bind:value={query}
        type="search"
        placeholder="搜索角色或 Mod (Ctrl+K)…"
        class="flex-1 min-w-0 bg-transparent outline-none border-none text-xs text-[var(--text)] placeholder:text-secondary/60"
        onkeydown={(e) => {
          if (e.key === "Escape") {
            if (query) {
              query = "";
              e.stopPropagation();
            } else {
              searchInputEl?.blur();
            }
          }
        }}
      />
      {#if query}
        <button
          class="w-4 h-4 radius-pill grid place-items-center text-secondary hover:text-[var(--text)] hover:bg-[var(--item-hover)] cursor-pointer text-[10px] shrink-0"
          aria-label="清空搜索"
          title="清空 (Esc)"
          onclick={() => (query = "")}
        >
          <IconClose size={12} />
        </button>
      {:else}
        <kbd class="hidden sm:inline-flex items-center px-1.5 py-0.5 text-[10px] font-mono text-secondary rounded-full border select-none" style="background: var(--island-badge); border-color: var(--glass-stroke);">
          Ctrl K
        </kbd>
      {/if}
    </div>
  </div>

  <!-- 右侧：全局核心操作组 -->
  <div class="flex items-center gap-2 shrink-0">
    {#if gameRunning}
      <div
        class="h-8 px-2.5 radius-pill flex items-center gap-1.5 text-xs font-semibold shrink-0"
        style="background: var(--accent-fill); color: var(--accent); box-shadow: inset 0 0 0 0.5px var(--accent);"
        title="支持 Junction Mod 热切换；修改后请手动热重载"
        aria-label="游戏运行中"
      >
        <span class="w-1.5 h-1.5 radius-pill animate-pulse" style="background: var(--accent);"></span>
        <span>游戏运行中</span>
      </div>
    {/if}
    {#if isCharGrid}
      <!-- 角色网格自定义排序下拉菜单 -->
      <CustomSelect
        bind:value={charSort}
        options={[
          { value: "default", label: "默认排序 (喜爱置顶)", icon: IconHeart },
          { value: "name", label: "名称 (A-Z)", icon: IconSortAlpha },
          { value: "mods", label: "Mod 数量", icon: IconPackage },
          { value: "enabled", label: "启用优先", icon: IconZap },
          { value: "rarity", label: "星级稀有度", icon: IconStar },
        ]}
        size="sm"
      />
    {:else if showSort}
      <!-- Mod 列表自定义排序下拉菜单 -->
      <CustomSelect
        bind:value={sort}
        options={[
          { value: "recent", label: "最近安装", icon: IconClock },
          { value: "name", label: "名称 (A-Z)", icon: IconSortAlpha },
          { value: "enabled", label: "启用优先", icon: IconZap },
          { value: "size", label: "文件大小", icon: IconSortSize },
        ]}
        size="sm"
      />
    {/if}

    <!-- 预设管理入口 -->
    <PresetMenu {onapplied} applyDisabled={gameRunning} />

    {#if onimport}
      <button
        class="glass-liquid-btn w-8 h-8 grid place-items-center cursor-pointer active:scale-95 shrink-0 text-[var(--text)] hover:text-white"
        aria-label="导入 Mod 包"
        title="选择 Mod 压缩包导入；如果系统拖放无反应，请使用这里"
        onclick={onimport}
      >
        <span class="z-10 grid place-items-center">
          <IconPackage size={13} />
        </span>
        <span class="sr-only">导入</span>
      </button>
    {/if}

    <!-- 工作模式切换胶囊 (Play / Dev) -->
    <div class="relative flex items-center h-8 glass-mode-capsule p-0.5 gap-0.5">
      <button
        class="h-7 px-2.5 text-xs font-semibold flex items-center gap-1.5 cursor-pointer rounded-full transition-all duration-200"
        class:mode-active-play={workMode === "play"}
        class:mode-inactive={workMode !== "play"}
        onclick={() => ontoggleworkmode?.()}
        title={workMode === "play" ? "当前为游玩模式：极致流畅无Dump开销。点击切换" : "点击切换为游玩模式"}
      >
        <IconGamepad size={13} class={workMode === "play" ? "text-cyan-300 drop-shadow-[0_0_6px_rgba(56,189,248,0.8)]" : "text-secondary"} />
        <span class={workMode === "play" ? "text-white" : "text-secondary"}>游玩</span>
      </button>
      <button
        class="h-7 px-2.5 text-xs font-semibold flex items-center gap-1.5 cursor-pointer rounded-full transition-all duration-200"
        class:mode-active-dev={workMode === "dev"}
        class:mode-inactive={workMode !== "dev"}
        onclick={() => ontoggleworkmode?.()}
        title={workMode === "dev" ? "当前为抓取模式：开启着色器/Hash捕获。点击切换" : "点击切换为抓取模式"}
      >
        <IconWrench size={13} class={workMode === "dev" ? "text-amber-300 drop-shadow-[0_0_6px_rgba(251,191,36,0.8)]" : "text-secondary"} />
        <span class={workMode === "dev" ? "text-white" : "text-secondary"}>抓取</span>
      </button>
      {#if workMode === "dev"}
        <button
          class="w-5 h-5 rounded-full grid place-items-center text-amber-400 hover:text-amber-200 hover:bg-amber-500/20 cursor-pointer ml-0.5 transition-colors"
          title="查看 3Dmigoto 抓取快捷键"
          onclick={(e) => {
            e.stopPropagation();
            showDevKeyHelp = !showDevKeyHelp;
          }}
        >
          <IconInfo size={11} />
        </button>

        {#if showDevKeyHelp}
          <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
          <div
            class="glass-popover absolute right-0 top-full mt-2 w-80 p-4 shadow-2xl z-[100] flex flex-col gap-3 text-xs animate-slide-up text-[var(--text)]"
            role="dialog"
            tabindex="-1"
            onclick={(e) => e.stopPropagation()}
            onkeydown={(e) => e.stopPropagation()}
          >
            <div class="flex items-center justify-between">
              <span class="flex items-center gap-1.5 text-sm font-bold text-[var(--accent)]">
                <IconWrench size={15} />
                <span>3Dmigoto 抓取小键盘速查</span>
              </span>
              <button
                class="w-6 h-6 rounded-full grid place-items-center text-secondary hover:text-[var(--text)] hover:bg-[var(--item-hover)] cursor-pointer"
                onclick={() => (showDevKeyHelp = false)}
              >
                ✕
              </button>
            </div>
            <p class="text-[11px] text-secondary leading-relaxed">
              游戏内使用数字小键盘（NumPad）实时捕获模型与贴图 Hash，已启用自动复制到剪贴板。
            </p>
            <div class="flex flex-col gap-1.5 font-mono text-[11px]">
              <div class="flex items-center justify-between py-1.5 px-2.5 rounded-lg border border-[var(--glass-stroke)]" style="background: var(--card-bg);">
                <span class="text-secondary font-sans text-xs">切换目标(顶点/索引/着色器)</span>
                <kbd class="font-bold text-[var(--accent)] text-xs">Num /</kbd>
              </div>
              <div class="flex items-center justify-between py-1.5 px-2.5 rounded-lg border border-[var(--glass-stroke)]" style="background: var(--card-bg);">
                <span class="text-secondary font-sans text-xs">上一个 / 下一个元素</span>
                <div class="flex gap-1">
                  <kbd class="font-bold text-[var(--accent)] text-xs">Num 1</kbd>
                  <kbd class="font-bold text-[var(--accent)] text-xs">Num 2</kbd>
                </div>
              </div>
              <div class="flex items-center justify-between py-1.5 px-2.5 rounded-lg border border-[var(--glass-stroke)]" style="background: var(--card-bg);">
                <span class="text-secondary font-sans text-xs">标记并复制当前 Hash</span>
                <kbd class="font-bold text-[var(--accent)] text-xs">Num *</kbd>
              </div>
              <div class="flex items-center justify-between py-1.5 px-2.5 rounded-lg border border-[var(--glass-stroke)]" style="background: var(--card-bg);">
                <span class="text-secondary font-sans text-xs">隐藏当前选中的模型</span>
                <kbd class="font-bold text-[var(--accent)] text-xs">Num 0</kbd>
              </div>
              <div class="flex items-center justify-between py-1.5 px-2.5 rounded-lg border border-[var(--glass-stroke)]" style="background: var(--card-bg);">
                <span class="text-secondary font-sans text-xs">导出 HLSL 反汇编</span>
                <kbd class="font-bold text-[var(--accent)] text-xs">Num .</kbd>
              </div>
            </div>
          </div>
        {/if}
      {/if}
    </div>

    <!-- 模组启动主按钮（高饱和度星空蓝宝石独立药丸，高度 32px，彻底消灭双层嵌套鬼影） -->
    <button
      class="glass-liquid-btn-accent h-8 px-3.5 text-xs font-semibold flex items-center gap-1.5 cursor-pointer rounded-full transition-all active:scale-95 z-10 shrink-0"
      onclick={onlaunchmodgame}
      disabled={launchBusy}
      aria-busy={launchBusy}
      title="自动启动 3DMigoto 注入引擎并拉起游戏（模组生效）"
    >
      <span class="z-10 flex items-center gap-1.5">
        <IconRocket size={13} class="text-white drop-shadow-[0_0_6px_rgba(255,255,255,0.7)]" />
        <span>{launchBusy ? "启动中…" : "模组启动"}</span>
      </span>
    </button>

    <!-- 辅助启动与热重载控制胶囊（纯净启动 + 官方启动器 + F10 热重载） -->
    <div class="flex items-center h-8 glass-liquid-capsule px-1 gap-0.5 shrink-0">
      <button
        class="h-7 w-7 grid place-items-center text-secondary hover:text-[var(--text)] cursor-pointer rounded-full transition-all hover:bg-[var(--item-hover)] active:scale-95 z-10"
        onclick={onlaunchnativegame}
        disabled={launchBusy}
        title="直接启动游戏主程序，不注入 3DMigoto（原版纯净游戏）"
      >
        <IconGamepad size={14} />
        <span class="sr-only">纯净启动</span>
      </button>
      {#if onlaunchofficial}
        <span class="w-[1px] h-3.5 bg-[var(--glass-stroke)] mx-0.5 opacity-60 z-10"></span>
        <button
          class="h-7 w-7 grid place-items-center text-secondary hover:text-[var(--text)] cursor-pointer rounded-full transition-all hover:bg-[var(--item-hover)] active:scale-95 z-10"
          onclick={onlaunchofficial}
          disabled={launchBusy}
          title="打开崩铁官方启动器 / HoYoPlay"
        >
          <IconRocket size={14} />
          <span class="sr-only">官方启动器</span>
        </button>
      {/if}
      {#if onrefreshgame}
        <span class="w-[1px] h-3.5 bg-[var(--glass-stroke)] mx-0.5 opacity-60 z-10"></span>
        <button
          class="h-7 w-7 grid place-items-center text-secondary hover:text-[var(--text)] cursor-pointer rounded-full transition-all hover:bg-[var(--item-hover)] active:scale-95 z-10"
          onclick={onrefreshgame}
          disabled={launchBusy}
          title="向游戏发送刷新信号 (F10)"
        >
          <IconRefresh size={14} />
          <span class="sr-only">热重载</span>
        </button>
      {/if}
    </div>

    <!-- 全局偏好设置按钮 -->
    <button
      class="glass-liquid-btn w-8 h-8 grid place-items-center cursor-pointer transition-transform hover:rotate-45 text-[var(--text)]"
      style={showSettings ? "background: var(--accent-fill); color: var(--accent)" : ""}
      aria-label="设置"
      title="偏好设置"
      onclick={ontogglesettings}
    >
      <span class="z-10 grid place-items-center">
        <IconSettings size={15} />
      </span>
    </button>
  </div>
</header>
