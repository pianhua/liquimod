# LiquiMod 里程碑 9 实施计划：布局修复 + 顶部工具条充实

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 修复主人报告的 4 个布局 bug（搜索框溢出压返回键 / 角色卡立绘与信息重叠 / 卡片尺寸随窗口反向缩放 / 预设按钮拥挤），并把顶部工具条充实为「启动游戏 / 启动加载器 / 排序 / 面包屑」。

**主人决策记录：** 预设按钮移到侧边栏底部（新建分类上方）；卡片顶部无需额外内容（主人的"顶部空旷"指窗口顶部工具条，参照 JASM 放启动游戏/启动 3Dmigoto/筛选搜索——搜索已在侧边栏，故工具条补两个启动按钮）。

**既有事实：** 见 `AGENTS.md`（构建/测试/CDP、里程碑 7/8 须知）。改前端文件一律用 Edit/Write 工具。Config 已有 theme/character_category_name 字段与 serde default 惯例。

---

### Task 1: core/app——游戏与加载器路径 + 启动命令

**Files:**
- Modify: `app/src-tauri/src/config.rs`
- Modify: `app/src-tauri/src/commands.rs`
- Modify: `app/src-tauri/src/lib.rs`（注册 4 个命令）
- Test: commands.rs / config.rs 内 tests

- [x] **Step 1: config.rs 加 game_exe / loader_exe**

```rust
    #[serde(default)]
    pub game_exe: Option<PathBuf>,
    #[serde(default)]
    pub loader_exe: Option<PathBuf>,
```

`load_from` fallback 补 `game_exe: None, loader_exe: None`。既有测试中的 Config 字面量（config.rs save_load_roundtrip、commands.rs set_mods_dir_rejects_missing 与 maybe_auto_enable_deploys_when_on）补 `game_exe: None, loader_exe: None`。新测试：

```rust
    #[test]
    fn exe_paths_default_none_and_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(&path, r#"{"library_root":"C:/L","mods_dir":null}"#).unwrap();
        let c = Config::load_from(&path);
        assert_eq!(c.game_exe, None);
        assert_eq!(c.loader_exe, None);
        let mut c = c;
        c.game_exe = Some(PathBuf::from("C:/game/StarRail.exe"));
        c.save_to(&path).unwrap();
        assert_eq!(
            Config::load_from(&path).game_exe,
            Some(PathBuf::from("C:/game/StarRail.exe"))
        );
    }
```

- [x] **Step 2: commands.rs——DTO 与命令**

`ConfigDto` 加 `pub game_exe: Option<String>, pub loader_exe: Option<String>`；`config_dto` 补 `game_exe: c.game_exe.as_ref().map(|p| p.display().to_string()), loader_exe: c.loader_exe.as_ref().map(|p| p.display().to_string()),`。

新增函数与命令：

```rust
/// 选择 exe 路径（设置项通用校验：存在、是文件、.exe 后缀）。
fn set_exe_path(slot: impl FnOnce(&mut Config, Option<PathBuf>), path: PathBuf) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("文件不存在：{}", path.display()));
    }
    if path.extension().and_then(|e| e.to_str()) != Some("exe") {
        return Err("请选择 .exe 可执行文件".to_string());
    }
    Ok(())
}

fn launch_exe(exe: Option<&Path>, what: &str) -> Result<(), String> {
    let Some(exe) = exe else {
        return Err(format!("未配置{what}路径，请在设置中配置"));
    };
    if !exe.is_file() {
        return Err(format!("{what}不存在：{}", exe.display()));
    }
    std::process::Command::new(exe)
        .current_dir(exe.parent().unwrap_or_else(|| Path::new(".")))
        .spawn()
        .map_err(|e| format!("启动{what}失败：{e}"))?;
    tracing::info!("launched {} ({})", what, exe.display());
    Ok(())
}

#[tauri::command]
pub fn choose_game_exe(state: tauri::State<AppState>, path: String) -> Result<ConfigDto, String> {
    let p = PathBuf::from(path);
    if !p.is_file() {
        return Err(format!("文件不存在：{}", p.display()));
    }
    if p.extension().and_then(|e| e.to_str()) != Some("exe") {
        return Err("请选择 .exe 可执行文件".to_string());
    }
    let mut config = state.config.lock().unwrap();
    config.game_exe = Some(p);
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    Ok(config_dto(&config))
}

#[tauri::command]
pub fn choose_loader_exe(state: tauri::State<AppState>, path: String) -> Result<ConfigDto, String> {
    let p = PathBuf::from(path);
    if !p.is_file() {
        return Err(format!("文件不存在：{}", p.display()));
    }
    if p.extension().and_then(|e| e.to_str()) != Some("exe") {
        return Err("请选择 .exe 可执行文件".to_string());
    }
    let mut config = state.config.lock().unwrap();
    config.loader_exe = Some(p);
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    Ok(config_dto(&config))
}

#[tauri::command]
pub fn launch_game(state: tauri::State<AppState>) -> Result<(), String> {
    let exe = state.config.lock().unwrap().game_exe.clone();
    launch_exe(exe.as_deref(), "游戏")
}

#[tauri::command]
pub fn launch_loader(state: tauri::State<AppState>) -> Result<(), String> {
    let exe = state.config.lock().unwrap().loader_exe.clone();
    launch_exe(exe.as_deref(), "加载器")
}
```

（`set_exe_path` 辅助函数若未被使用则不写——两个 choose 命令内联校验即可，遵守 YAGNI；上面已内联，删掉 set_exe_path。）

`lib.rs` 注册：`commands::choose_game_exe, commands::choose_loader_exe, commands::launch_game, commands::launch_loader,`

测试（只测错误路径，不真启动进程）：

```rust
    #[test]
    fn launch_exe_errors_when_unconfigured_or_missing() {
        assert!(launch_exe(None, "游戏").unwrap_err().contains("未配置游戏路径"));
        assert!(launch_exe(Some(Path::new("C:/no/such.exe")), "游戏")
            .unwrap_err()
            .contains("不存在"));
    }
```

- [x] **Step 3: 验证 + 提交**

Run: `cargo test --workspace`、`cargo clippy --workspace --all-targets`、`cargo fmt --all`
Commit: `feat(app): 游戏/加载器路径配置与启动命令`

---

### Task 2: UI 修复四点 + 工具条启动按钮

**Files:**
- Modify: `app/src/lib/components/SearchBar.svelte`（w-72 → w-full）
- Modify: `app/src/lib/components/CharacterCard.svelte`（立绘内缩、信息条分离）
- Modify: `app/src/lib/views/CharacterGrid.svelte`（固定轨宽）
- Modify: `app/src/lib/components/PresetMenu.svelte`（block prop）
- Modify: `app/src/lib/components/Sidebar.svelte`（预设移入底部）
- Modify: `app/src/lib/components/Toolbar.svelte`（启动按钮，移除预设）
- Modify: `app/src/lib/views/Settings.svelte`（启动区）
- Modify: `app/src/lib/api.ts`（ConfigDto 字段 + 4 命令 + mock）
- Modify: `app/src/routes/+page.svelte`（launch 回调）
- Test: `app/src/lib/components/Toolbar.test.ts`（新建）、既有测试适配

- [x] **Step 1: SearchBar 宽度修复**

`app/src/lib/components/SearchBar.svelte` 第 5 行 `w-72` 改 `w-full`；placeholder 改 `搜索…`（现在也过滤 Mod）。

- [x] **Step 2: CharacterCard 重构（立绘与信息完全分离）**

整体替换为：

```svelte
<script lang="ts">
  import { portraitUrl, type CharacterSummary } from "$lib/api";

  let {
    character,
    onclick,
  }: { character: CharacterSummary; onclick: () => void } = $props();

  // 信号灯：恰好 1 个启用 = 绿；2 个及以上 = 黄；0 = 灰
  let dot = $derived(
    character.enabled === 1
      ? { color: "#34c759", glow: "0 0 6px rgba(52,199,89,0.9)" }
      : character.enabled >= 2
        ? { color: "#ffd60a", glow: "0 0 6px rgba(255,214,10,0.9)" }
        : { color: "rgba(142,142,147,0.65)", glow: "none" },
  );
</script>

<div
  role="button"
  tabindex="0"
  class="radius-card relative cursor-pointer transition-all duration-200 hover:scale-[1.03] hover:-translate-y-0.5 active:scale-[0.98] outline-none p-2 flex flex-col gap-2"
  style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke), var(--shadow-soft)"
  {onclick}
  onkeydown={(e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onclick();
    }
  }}
>
  <div class="relative w-full rounded-[14px] overflow-hidden" style="aspect-ratio: 1">
    {#if character.image}
      <img
        src={portraitUrl(character.image)}
        alt={character.display_name}
        class="absolute inset-0 w-full h-full object-cover object-top"
        loading="lazy"
        draggable="false"
      />
    {:else}
      <div class="absolute inset-0 grid place-items-center text-4xl font-bold text-secondary"
        style="background: var(--glass-tint)">
        {character.display_name.slice(0, 1)}
      </div>
    {/if}
  </div>
  <span
    class="absolute top-3.5 right-3.5 w-2.5 h-2.5 rounded-full z-10"
    title={character.enabled > 0 ? `${character.enabled} 个 Mod 启用中` : "没有启用的 Mod"}
    style:background={dot.color}
    style:box-shadow={dot.glow}
  ></span>
  <div class="glass radius-pill px-3 h-9 flex items-center justify-between gap-1.5 shrink-0">
    <span class="text-[13px] font-medium truncate">{character.display_name}</span>
    {#if character.total > 0}
      <span class="text-[11px] text-secondary shrink-0">{character.enabled}/{character.total}</span>
    {/if}
  </div>
</div>
```

信号灯在卡片右上角（`top-3.5 right-3.5` 落在立绘内缩后的框边上，不压立绘主体与名字）；信息条在立绘下方独立成行。

CharacterCard.test.ts 的 `span[title]` 选择器与颜色断言不变，应仍通过；若因结构变化失败按新结构调整选择器。

- [x] **Step 3: CharacterGrid 固定轨宽（卡片不随窗口反向缩放）**

`app/src/lib/views/CharacterGrid.svelte` 改为：

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { filterCharacters, type CharacterSummary } from "$lib/api";
  import CharacterCard from "$lib/components/CharacterCard.svelte";

  // 卡片信息条固定高度：p-2 上下 16 + gap-2 8 + h-9 36 = 60
  const CARD_EXTRA = 60;

  let {
    characters,
    query,
    onselect,
  }: {
    characters: CharacterSummary[];
    query: string;
    onselect: (c: CharacterSummary) => void;
  } = $props();

  let filtered = $derived(filterCharacters(characters, query));

  let gridEl: HTMLDivElement;
  let rowHeight = $state(0);

  onMount(() => {
    if (typeof ResizeObserver === "undefined") return;
    const measure = () => {
      const cs = getComputedStyle(gridEl);
      // 固定轨宽布局：第一条轨道的像素宽即卡片宽
      const first = cs.gridTemplateColumns.split(" ").filter(Boolean)[0];
      const w = parseFloat(first);
      if (w > 0) rowHeight = w + CARD_EXTRA;
    };
    const ro = new ResizeObserver(measure);
    ro.observe(gridEl);
    measure();
    return () => ro.disconnect();
  });
</script>

<div
  bind:this={gridEl}
  class="grid grid-cols-[repeat(auto-fill,180px)] justify-center gap-5 px-6 pt-2 pb-8 overflow-y-auto flex-1 min-h-0 content-start"
  style:grid-auto-rows={rowHeight > 0 ? `${rowHeight}px` : undefined}
>
  {#each filtered as c (c.internal_name)}
    <CharacterCard character={c} onclick={() => onselect(c)} />
  {/each}
  {#if filtered.length === 0}
    <p class="text-secondary col-span-full text-center mt-24">没有匹配的角色</p>
  {/if}
</div>
```

- [x] **Step 4: PresetMenu 加 block prop（供侧边栏全宽使用）**

PresetMenu.svelte 的 props 改为：

```ts
  let { onapplied, block = false }: { onapplied: () => void; block?: boolean } = $props();
```

触发按钮 class 条件化：`class="glass radius-pill h-9 px-4 text-sm flex items-center gap-1.5 cursor-pointer transition-transform hover:scale-[1.03]"` 后加 `class:w-full={block} class:justify-center={block}`。面板定位类 `absolute right-0 top-11` 在 block 时改为 `absolute left-0 right-0 top-11`（宽度跟随侧边栏）：用 `class:left-0={block} class:right-0={!block}` 加原 `right-0` 冲突——直接把面板 class 改为动态：

```svelte
    <div
      class="glass radius-panel absolute top-11 z-50 p-2.5 flex flex-col gap-1"
      class:left-0={block}
      class:right-0={!block}
      class:w-72={!block}
      style={block ? "left: 0; right: 0" : ""}
    >
```

（简化：非 block 时 `right-0 w-72`；block 时 `left-0 right-0`。用三元 class 字符串亦可，选可读性好的写法。）

- [x] **Step 5: Sidebar 底部加预设按钮（新建分类上方）**

import PresetMenu；props 加 `onapplied: () => void`。在底部 shrink-0 容器内、`{#if creating}` 块之前插入：

```svelte
    <div class="pb-1.5">
      <PresetMenu {onapplied} block />
    </div>
```

- [x] **Step 6: Toolbar 移除预设、加启动按钮**

整体替换为：

```svelte
<script lang="ts">
  import type { ModSort } from "$lib/view";

  let {
    crumbs,
    sort = $bindable(),
    showSort,
    onlaunchgame,
    onlaunchloader,
  }: {
    crumbs: string[];
    sort: ModSort;
    showSort: boolean;
    onlaunchgame: () => void;
    onlaunchloader: () => void;
  } = $props();
</script>

<div class="relative z-30 flex items-center justify-between h-12 px-6 shrink-0">
  <nav class="text-sm text-secondary truncate" aria-label="面包屑">
    {#each crumbs as crumb, i (i)}
      {#if i > 0}<span class="mx-1.5 opacity-50">›</span>{/if}
      <span class={i === crumbs.length - 1 ? "font-semibold" : ""} style={i === crumbs.length - 1 ? "color: var(--text)" : ""}>{crumb}</span>
    {/each}
  </nav>
  <div class="flex items-center gap-2.5 shrink-0">
    {#if showSort}
      <div class="glass radius-pill h-9 px-3 flex items-center">
        <select
          bind:value={sort}
          aria-label="排序方式"
          class="bg-transparent outline-none text-sm cursor-pointer"
        >
          <option value="recent">最近安装</option>
          <option value="name">名称</option>
          <option value="enabled">启用优先</option>
        </select>
      </div>
    {/if}
    <button
      class="glass radius-pill h-9 px-4 text-sm flex items-center gap-1.5 cursor-pointer transition-transform hover:scale-[1.03]"
      onclick={onlaunchgame}
    >
      <svg width="11" height="11" viewBox="0 0 11 11" fill="currentColor">
        <path d="M2.5 1.5v8l7-4-7-4z" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round" />
      </svg>
      启动游戏
    </button>
    <button
      class="glass radius-pill h-9 px-4 text-sm flex items-center gap-1.5 cursor-pointer transition-transform hover:scale-[1.03]"
      onclick={onlaunchloader}
    >
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
        <path d="M6 1.5v5M3.8 3.8 6 1.5l2.2 2.3M2 7.5v2a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1v-2" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
      启动加载器
    </button>
  </div>
</div>
```

- [x] **Step 7: api.ts——字段 + 命令 + mock**

ConfigDto 加 `game_exe: string | null; loader_exe: string | null;`。mock get_config/set_theme/set_character_category_name/set_auto_enable 的返回对象都补 `game_exe: null, loader_exe: null`。switch 加：

```ts
      case "choose_game_exe":
      case "choose_loader_exe": {
        const p = String(args?.path ?? "");
        if (!p.toLowerCase().endsWith(".exe")) throw "请选择 .exe 可执行文件";
        return { library_root: "C:/mock/Library", mods_dir: null, auto_enable: false, theme: "auto", character_category_name: "角色", game_exe: null, loader_exe: null } as T;
      }
      case "launch_game":
        throw "未配置游戏路径，请在设置中配置";
      case "launch_loader":
        throw "未配置加载器路径，请在设置中配置";
```

api 对象加：

```ts
  chooseGameExe: (path: string) => call<ConfigDto>("choose_game_exe", { path }),
  chooseLoaderExe: (path: string) => call<ConfigDto>("choose_loader_exe", { path }),
  launchGame: () => call<void>("launch_game"),
  launchLoader: () => call<void>("launch_loader"),
```

- [x] **Step 8: Settings 加「启动」区（行为区之前）**

script 加：

```ts
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
        onchanged();
      }
    } catch (e) {
      toast(String(e));
    }
  }
```

模板在「行为」区前插入：

```svelte
    <section class="glass radius-panel p-5 flex flex-col gap-3">
      <h3 class="text-sm font-semibold text-secondary">启动</h3>
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <p class="text-sm font-medium">游戏主程序</p>
          <p class="text-xs text-secondary truncate">{config?.game_exe ?? "未配置"}</p>
        </div>
        <button class="glass radius-pill h-8 px-3.5 text-sm shrink-0 cursor-pointer" onclick={() => pickExe("game")}>
          选择…
        </button>
      </div>
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <p class="text-sm font-medium">3Dmigoto 加载器</p>
          <p class="text-xs text-secondary truncate">{config?.loader_exe ?? "未配置"}</p>
        </div>
        <button class="glass radius-pill h-8 px-3.5 text-sm shrink-0 cursor-pointer" onclick={() => pickExe("loader")}>
          选择…
        </button>
      </div>
    </section>
```

- [x] **Step 9: +page.svelte 接线**

script 加：

```ts
  async function launchGame() {
    try {
      await api.launchGame();
      toast("已启动游戏");
    } catch (e) {
      toast(String(e));
    }
  }

  async function launchLoader() {
    try {
      await api.launchLoader();
      toast("已启动加载器");
    } catch (e) {
      toast(String(e));
    }
  }
```

Toolbar 调用改为 `<Toolbar {crumbs} bind:sort {showSort} onlaunchgame={launchGame} onlaunchloader={launchLoader} />`；Sidebar 调用加 `onapplied={refresh}`。

- [x] **Step 10: Toolbar.test.ts**

```ts
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import Toolbar from "./Toolbar.svelte";

function props(over: Record<string, unknown> = {}) {
  return {
    crumbs: ["角色", "流萤"],
    sort: "recent" as const,
    showSort: true,
    onlaunchgame: vi.fn(),
    onlaunchloader: vi.fn(),
    ...over,
  };
}

describe("Toolbar", () => {
  it("渲染面包屑与两个启动按钮", () => {
    render(Toolbar, { props: props() });
    expect(screen.getByLabelText("面包屑").textContent).toContain("角色");
    expect(screen.getByText("启动游戏")).toBeTruthy();
    expect(screen.getByText("启动加载器")).toBeTruthy();
  });

  it("启动按钮回调", async () => {
    const p = props();
    render(Toolbar, { props: p });
    await fireEvent.click(screen.getByText("启动游戏"));
    expect(p.onlaunchgame).toHaveBeenCalled();
    await fireEvent.click(screen.getByText("启动加载器"));
    expect(p.onlaunchloader).toHaveBeenCalled();
  });

  it("showSort=false 时不渲染排序", () => {
    render(Toolbar, { props: props({ showSort: false }) });
    expect(screen.queryByLabelText("排序方式")).toBeNull();
  });
});
```

- [x] **Step 11: 验证 + 提交**

检查所有引用 SearchBar/PresetMenu/Toolbar 的测试适配；`cd app; npx vitest run; npm run check; npm run build` 全绿。
Commit: `fix(ui): 搜索框溢出/角色卡立绘信息分离/卡片固定尺寸 + 预设入侧边栏 + 工具条启动按钮`

---

### Task 3: E2E 实测 + 终审（主模型执行）

- [x] **Step 1: 构建真实 exe，CDP 实测**
  1. 设置页返回按钮与搜索框不重叠（getBoundingClientRect 断言）。
  2. 角色卡：立绘矩形与信息条矩形不重叠；信号灯不压名字。
  3. 改窗口大小（resize 720/1600）卡片宽度恒 180px。
  4. 侧边栏底部有预设按钮，点开面板在侧边栏内展开、elementFromPoint 命中。
  5. 工具条有启动游戏/加载器按钮；未配置时点击 toast 报错文案。
  6. 亮/暗各截一张图。
- [x] **Step 2: 终审子代理 + 修 Critical/Important + 收尾 commit**

---

## Self-Review 记录

- 主人四个问题全覆盖：①SearchBar w-72→w-full（Task 2 Step 1）；②立绘内缩+信息条分离（Step 2）；③auto-fill minmax(170px,1fr)→固定 180px 轨（Step 3）；④预设移侧边栏底部（Step 4/5/6）。顶部充实 = 启动游戏/加载器（Task 1 + Step 6/8/9）。
- 命名一致：choose_game_exe/choose_loader_exe/launch_game/launch_loader 两端一致（参数 path 单字无 camelCase 问题）；onlaunchgame/onlaunchloader 在 Toolbar/+page 一致；PresetMenu block prop 在 Sidebar 使用一致。
- CharacterGrid 行高公式 CARD_EXTRA=60 = p-2(16) + gap-2(8) + h-9(36)。
- YAGNI：不做启动参数、不做"启动游戏并注入"联动、不做游戏进程检测按钮态。
