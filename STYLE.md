# LiquiMod 视觉规范与设计系统 (Design System & Guidelines)

> **版本**：v2.0 · 空间流体玻璃与触感交互规范  
> **技术栈**：Svelte 5 (Runes) + Tailwind CSS v4 + 全局 CSS 变量架构  
> **核心原则**：**统一分级，而非千篇一律——同层必同规格，跨层用层级区分，不允许同一语境下混淆的散点。**  
> 所有新增与修改的 UI 组件必须严格遵循本规范；任何未遵照规范的私造类名或不规则尺寸均视为**视觉回归**。

---

## 1. 核心分层几何系统 (Geometry Scale)

### 1.1 容器与圆角分层 (Radius Scale)

系统严格定义 5 个圆角层级，杜绝硬编码 `rounded-xl` / `rounded-2xl` 等杂值：

| 规范类名 | 物理像素 | 语义与适用对象 |
| :--- | :--- | :--- |
| `radius-window` | **26px** | 应用程序顶层主窗口整体外轮廓 |
| `radius-panel` | **20px** | 独立浮层面板、偏好设置卡片组、弹窗对话框、右键下拉浮层 |
| `radius-card` | **18px** | 实体对象卡片（角色海报卡、Mod 列表行容器、2x2 统计卡片、画廊缩略图） |
| `rounded-lg` | **8px** | 矩形下拉菜单项、输入框微容器、密码记录项、日志控制台 |
| `radius-pill` | **9999px (∞)** | 所有交互按钮（主操作、导航、图标胶囊圆钮）、搜索框外框、状态指示器、Toggle 开关 |

> **⚠️ 铁律**：除下拉菜单内部条目使用 `rounded-lg (8px)` 外，所有独立图标按钮与操作按钮必须使用 `radius-pill` 胶囊圆角。

---

### 1.2 按钮尺寸与高度三层体系 (Height Scale)

全系统操作按钮严格划分为三层高度，同层内部严格同规格：

| 档位 | 高度规格 | 适用场景与典型代表 | 边距与排版 |
| :--- | :--- | :--- | :--- |
| **主要控制档** | `h-9` (36px) | 顶栏搜索栏、模组启动主按钮、重点向导动作 | `px-4 text-sm font-semibold` |
| **标准操作档** | `h-8` (32px) | 全局标准按钮（「选择…」「保存」「添加」「全开/全关」）、**所有图标按钮** | `px-3 text-xs font-medium` |
| **紧凑微控档** | `h-7` (28px) | 分段控制器（Play/Dev 模式胶囊）、属性筛选标签、画廊辅助微钮 | `px-2.5 text-xs font-medium` |

#### 🔘 图标操作按钮统一铁律（严禁违规）
- **所有单图标操作按钮统一规格**：`w-8 h-8 glass radius-pill`（32px 高透毛玻璃胶囊圆钮）。
- **覆盖范围**：打开目录、重命名、移到分类、卸载、删除预设、移除密码、收藏置顶、分类管理等。
- **危险操作**：涉及卸载、删除等破坏性按钮，统一附加 `hover:bg-[var(--danger)] hover:text-white`。
- **禁用历史残留**：严禁私自编写 24px / 28px（`w-6`/`w-7`）的粗糙小按钮。

---

## 2. 物理动效与触感反馈 (Physics & Micro-Interactions)

### 2.1 苹果果冻触感开关 (Fluid Jelly Switch)
- **组件规范**：全系统开关必须统一使用 `$lib/components/Toggle.svelte`。
- **物理拉伸 (`:active`)**：按住滑块时，白色圆钮横向弹性拉伸至 `28px`，松开时伴随 `cubic-bezier(0.34, 1.56, 0.64, 1)` 弹性果冻回弹。
- **多层光影**：滑块具备 3 层立体环境光投影与高透微边框，确保在任何壁纸与深浅主题下均立体分明。

### 2.2 按钮物理按压微缩放
- **按压反馈**：全局按钮与 `[role="button"]` 在 `:active:not(:disabled)` 状态下执行 `transform: scale(0.96)`，过渡时间 `0.12s` 弹性贝塞尔曲线。
- **禁用态物理规整**：`button:disabled` 与 `[aria-disabled="true"]` 统一设为 `opacity: 0.45`、`cursor: not-allowed` 并屏蔽 `pointer-events`。

### 2.3 晶体聚焦光晕 (Focus Glow)
- **表单聚焦规范**：所有 `input`、`textarea`、`select` 在 `:focus-visible` 时激活苹果半透明晶体光晕：
  ```css
  box-shadow: 0 0 0 2px var(--accent-fill), 0 0 0 1px var(--accent);
  ```

### 2.4 全局毛玻璃 Tooltip 系统 (Liquid Glass Tooltip)
- **去原生化**：彻底拦截粗糙的 Windows 原生直角黑框 `title`，全局由 `<TooltipRoot />` 统一接管渲染。
- **动效与避障**：`200ms` 物理悬停防抖延迟，具备视口边缘智能翻转自适应；
- **晶体徽章**：自动识别括号内的快捷键提示（如 `(Ctrl+K)` / `(Esc)`），智能解析并渲染为立体晶体 `<kbd>` 徽标。

---

## 3. 色板体系与主题 Tokens (Color & Theming)

全系统禁止硬编码 `#hex` 或 `rgb()` 颜色，必须使用 `app.css` 定义的 CSS 变量：

| CSS Token | 浅色主题 (Light) | 深色主题 (Dark) | 语义与用途 |
| :--- | :--- | :--- | :--- |
| `--surface` | `#f2f2f7` | `#15161f` | 应用程序基底背景色 |
| `--text` | `#1c1c1e` | `#f2f2f7` | 主标题与正文主要文本色 |
| `--text-secondary` | `#6e6e73` | `#98989f` | 次要元数据、说明、时间戳 |
| `--accent` | `#0a84ff` (iOS Blue) | `#409cff` | 品牌主色、激活态高亮、主焦点 |
| `--accent-fill` | `rgba(10, 132, 255, 0.14)` | `rgba(64, 156, 255, 0.16)` | 激活标签背景填充、选区高亮底色 |
| `--danger` | `#ff3b30` | `#ff453a` | 危险操作、冲突报警、删除动作 |
| `--glass-bg` | `rgba(255, 255, 255, 0.28)` | `rgba(28, 30, 42, 0.38)` | 毛玻璃卡片与容器半透底色 |
| `--glass-stroke` | `rgba(255, 255, 255, 0.45)` | `rgba(255, 255, 255, 0.14)` | 晶体容器 0.5px 精致微边框 |
| `--glass-floating-bg` | `rgba(255, 255, 255, 0.92)` | `rgba(24, 26, 38, 0.94)` | 浮层菜单、弹窗高饱和防穿透底色 |

> **🎨 双主题验收铁律**：所有 UI 改动必须在 Light（明亮）和 Dark（暗黑）两种模式下分别验证对比度，禁止出现亮色下文字过淡或暗色下边缘泛白发灰的问题。

---

## 4. 文本层级与排版规范 (Typography Scale)

| 语义角色 | Tailwind 类名 | 字号 / 字重 | 典型应用 |
| :--- | :--- | :--- | :--- |
| **页面主标题** | `text-2xl font-bold` | 24px / 700 | 视图主标题（如角色库、光锥等） |
| **卡片/详情标题** | `text-xl font-bold` | 20px / 700 | 角色详情页名、Mod 详情大标题 |
| **区块分组标题** | `text-xs font-semibold uppercase tracking-wider text-secondary` | 12px / 600 | 设置页分组头部、属性分类标题 |
| **标准正文 / 按钮** | `text-sm font-medium` | 14px / 500 | 列表项主名、主操作按钮文字 |
| **次级元数据 / 辅助** | `text-xs text-secondary` | 12px / 400 | 文件体积、文件数、修改日期、备注 |
| **微标签 / 徽章** | `text-[10px] font-mono font-semibold` | 10px / 600 | 数量胶囊气泡、快捷键 `<kbd>` 徽章 |

---

## 5. 视图状态机与架构规范 (Architecture Invariants)

1. **搜索记忆全链路 (`viewSearchMemory`)**：
   - 视图切换时必须通过 `viewSearchMemory` 独立快照当前视图的搜索词；
   - 用户从角色列表搜索进入详情页、再点击返回时，上一级的搜索过滤结果必须 **100% 精准恢复**。
2. **滚动位置防丢失**：
   - Chromium 内核在元素脱离文档流或 `display:none` 时会清空 `scrollTop`；
   - 切换视图或打开全屏设置前，必须显式调用 `saveScroll()`，渲染完成后调用 `restoreScroll()` 恢复。
3. **层叠上下文 (Stacking Context)**：
   - 所有下拉菜单、浮层面板的祖先容器必须自带定位与层级声明（如 `relative z-30`），避免带 `transform` 动画的子卡片盖住浮层。
4. **WebView2 渲染引擎避坑**：
   - WebView2 的 CSS Grid 行高计算会无视内部 item 的 `aspect-ratio` 撑高；
   - 网格列表必须显式写入 `[grid-auto-rows:200px]` 或借助 `ResizeObserver` 计算行高。

---

## 6. 开发者代码审查清单 (Review Checklist)

在提交任何前端代码或发起 PR 前，请逐项核对：

- [ ] **按钮规格**：图标按钮是否为 `w-8 h-8 glass radius-pill`？操作按钮高度是否严格处于 `h-9` / `h-8` / `h-7` 三档之一？
- [ ] **圆角规范**：是否使用了 `radius-card`(18px) / `radius-panel`(20px) / `radius-pill`，而没有硬编码 `rounded-xl`？
- [ ] **开关交互**：是否统一使用了具备果冻拉伸特性的 `<Toggle />` 组件？
- [ ] **输入框体验**：是否支持 `Esc` 清空/退出？是否具备晶体 Focus Glow 聚焦环？
- [ ] **色板与主题**：是否 100% 采用 CSS 变量？是否在亮色与暗色模式下均通过了对比度测试？
- [ ] **无障碍与交互**：纯图标按钮是否全部配备了有意义的 `aria-label` 与 `title` 提示？
- [ ] **自动化测试**：本地与 CI 环境运行 `npm test && npm run check` 是否保持 0 错误 0 警告？