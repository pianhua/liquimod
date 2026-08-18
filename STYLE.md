# LiquiMod UI 视觉规范（Design Tokens & Guidelines）

> LiquiMod 的「磐石」级视觉基线。Svelte 5 + Tailwind v4 + 全局 CSS 变量。所有 UI 组件必须遵循本规范；新增代码若无理由脱离，视为视觉回归。
>
> 原则一句话：**统一分级，而非千篇一律——同层必同规格，跨层用层级区分，不允许同一语境下混淆的散点。**
> （“统一”不等于“所有东西都一样”；明确分层的档位照档位走，同一档内严格一致。）

## 1. 核心：两个分层系统

### 1.1 按钮圆形角层级（Radius Scale）
| 档 | 圆角 | 适用 |
|---|---|---|
| `radius-pill` | 9999px（∞） | 胶囊按钮（导航、主操作、图标圆形按钮）、输入框、toggle/头像/信号灯等「对象」 |
| `radius-card` | 18px | 卡片容器（角色卡、Mod 卡/行） |
| `radius-panel` | 20px | 浮层面板（菜单、设置区块） |
| `radius-window` | 26px | 窗口整体 |
| **矩形按钮/菜单项** | **8px（`rounded-lg`）** | 下拉菜单项、矩形操作按钮、密码行、日志框 |

**禁止：** 组件内硬写 `rounded-xl`(12px) 等矩形按钮杂值；卡片/面板必须用 `radius-*` 全局类而非 `rounded-` 造值。

### 1.2 按钮尺寸三层高度（Height Scale）
| 档 | 高度 | 用途 |
|---|---|---|
| `h-9`（36px） | 主按钮、主输入框 | 工具条启动按钮、搜索框、顶层操作 |
| `h-8`（32px） | 标准按钮、标准输入 | 「选择…」「保存」「添加」等 |
| `h-7`（28px） | 小控制器 | 分段胶囊、小选项 |

- **图标操作按钮 → 统一 `w-8 h-8 glass radius-pill`**（32px 玻璃圆钮）。如：打开/重命名/移到分类/卸载/删除预设/移除密码/分类操作菜单。危险类（卸载/删除/移除）加 `hover:bg-[var(--danger)] hover:text-white`。
- **不做** 24px 透明小图标按钮——那是被淘汰的旧规格（历史残留）。
- 返回按钮、窗口控制钮（TitleBar）、toggle、头像、信号灯是**独立组件语境**，不并入上表。

## 2. 颜色 & 主题

- 全部走 `app.css` 的 CSS 变量，禁止硬编码色值。核心变量：
  - `--glass-bg` `--glass-stroke` `--glass-highlight` `--glass-tint`（玻璃）
  - `--surface` `--text` `--text-secondary` `--accent` `--accent-fill` `--danger`
  - `--shadow-soft` `--shadow-lift` `--blob-a` `--blob-b`
- 主题由 `document.documentElement.dataset.theme`（auto/light/dark）驱动；`auto` 监听 `prefers-color-scheme`。
- **必须亮/暗两套都测**（CDP 截图 + 识图复核对比度），尤其玻璃底上的次级文字/灰色状态（参考：灰色信号灯曾在亮色下过淡，现统一 `#9b9ba2` + 1px 描边）。

## 3. 尺寸 & 间距

- 间距用 Tailwind 4px 步进刻度：`p-0.5(2) p-1(4) p-1.5(6) p-2(8) p-2.5(10) p-3(12) p-3.5(14) p-4(16) p-5(20) p-8(32)`。
- 区块间距：内容列 `gap-3`；卡片间 `gap-2.5/5`；页面/侧边栏边距 `px-6/px-8`。
- 元素间距：`gap-0.5…gap-4` 按需，但同一列表/卡片内保持一致。

## 4. 文本层级

| 用途 | 字号 |
|---|---|
| 页标题 `<h2>` | `text-2xl`（24px，bold, tracking-tight） |
| 区块小标题 | `text-sm font-semibold text-secondary` |
| 主文本 / 按钮 | `text-sm`(14px) |
| 次要 / 元数据 / 计数 / 时间 | `text-xs`(12px), `text-secondary` |
| 卡片内超小调整 | `text-[13px]` 主名 / `text-[11px]` 计数（仅 CharacterCard 特例） |
| 大占位符 | `text-3xl/4xl`（无图首字母） |

## 5. 玻璃质感（`.glass`）

- 卡片/行/胶囊/面板用 `.glass`（半透明 bg + blur + 1 细边 + 软阴影）。需要抬升用 `--shadow-lift`。
- 纯文本/图标按钮在玻璃上 hover 用 `hover:bg-[var(--glass-stroke)]`；危险 hover 用 `var(--danger)` + 白字。
- **浮层面板的祖先必须自带定位与 z-index**（`relative z-30`+），transform 的卡片会建层叠上下文盖住无定位面板。

## 6. 明确分层（这些不是“混乱”）

| 组件 | 规格 | 理由 |
|---|---|---|
| toggle 开关 | 44×28 `radius-pill`，滑块 24×24 | 独立开关控件 |
| 角色头像 | 40×40 `rounded-full` | 头部对象 |
| 信号灯 | 10×10 `rounded-full` + 状态色 + 1px 描边 | 状态点，位于信息胶囊内左侧 |
| 返回按钮 | `radius-pill` 胶囊 | 导航控件档 |

## 7. 可访问性与稳定

- `aria-label` 覆盖所有纯图标按钮；`role=button` 元素须有 `tabindex` + `onkeydown`。
- 避免 `display:none` 依赖滚动位置（会清零 scrollTop）——用滚动记忆 Map 显式保存恢复。
- 改前端文件一律用 Edit/Write 工具（勿 PowerShell 写中文）。
- WebView2 的 grid 行高与 Chromium 有引擎差异，依赖内容撑高的行高须 ResizeObserver 实测。