# LiquiMod 里程碑 8 设计：自定义分类 + 全新布局 + 亮色主题

日期：2026-08-18 ｜ 状态：已与主人逐块确认（布局/后端/视觉/信号灯）

## 1. 背景与目标

主人反馈：
1. 缺少分类与筛选——Mod 类型很多（角色 Mod 只是其一），且**不同游戏叫法不同，分类必须由用户自定义**；
2. 开屏一堆角色堆在一起不好找；预设按钮及弹出菜单被卡片背景图遮挡，形同虚设；
3. 按钮、卡片的布局不舒服、不好看也不实用——需要**全新 UI 布局**；
4. 上亮色主题；软件以中文为主。

追加需求：角色卡上加**信号灯**——该角色恰好 1 个 Mod 启用=绿，2 个及以上=黄，0 个=灰。

风格基调不变：iOS 26 液态玻璃。

## 2. 布局（已与主人确认：经典左侧边栏）

```
┌────────────────────────────────────────────────────┐
│ 标题栏（LiquiMod · F10 · ⚙设置 · ─ ▢ ✕）            │
├──────────┬─────────────────────────────────────────┤
│ 侧边栏    │  内容区                                 │
│ 🔍 搜索框 │  ┌────────────────────────────────┐    │
│ ──────── │  │ 工具条：面包屑 · 排序 · [✨预设] │    │
│ 全部 Mod  │  └────────────────────────────────┘    │
│ 未分类    │                                        │
│ ──────── │  「角色」分类 → 角色卡片网格            │
│ 角色      │  其他分类 → Mod 大卡片网格              │
│ 武器      │  全部 Mod → 跨分类 Mod 大卡片网格       │
│ ＋新建分类│                                        │
└──────────┴─────────────────────────────────────────┘
```

- **侧边栏**：搜索框置顶；内置条目「全部 Mod」「未分类」；下方自定义分类（名称 + Mod 计数），右键（或长按/⋯按钮）重命名/删除/上下调序；底部「＋新建分类」就地添加（行内输入，同 ModRow 重命名模式）。
- **工具条**：面包屑（如「角色 › 流萤」）+ 排序（最近安装/名称/启用优先）+ 预设按钮。
- **视图模型**：主页 `view = home（角色分类） | category(id) | all | character(name)`；设置仍为覆盖层。滚动记忆按 view key 存 Map（沿用 display:none 会清 scrollTop 的教训，显式保存/恢复）。
- **搜索**：过滤当前视图内容（角色网格按角色名，Mod 网格按 Mod 名）；空结果显示玻璃占位文案。

## 3. 数据模型与后端

### core
- 新表 `categories(id INTEGER PRIMARY KEY, name TEXT UNIQUE NOT NULL, ord INTEGER NOT NULL)`。
- `mods` 加列 `category_id INTEGER NULL REFERENCES categories(id)`（迁移：ALTER + 吞 duplicate column，照惯例）。
- **角色分类 = 虚拟视图**：库根平铺的 Mod（现状）视为「角色」分类，不落库；`category_id IS NULL` 且位于角色目录下的 Mod 属于角色视图。拖到自定义分类的 Mod 不参与角色推断。
  - 自定义分类的目录形态：`library_root/categories/<分类名>/<Mod目录>`？——**否**。为保持 watcher/对账简单，自定义分类 Mod 仍在库根平铺，仅用 DB 列区分；「未分类」= 未被角色推断命中且 category_id IS NULL。
  - 角色分类显示名可在设置中改（存 config `character_category_name`，默认「角色」），满足不同游戏叫法。
- 命令：`list_categories() -> Vec<CategoryDto{id,name,ord,mod_count}>`、`create_category(name)`、`rename_category(id,name)`、`delete_category(id)`（非空需前端确认，Mod 批量移回未分类）、`move_category(id,delta)` 调序、`set_mod_category(id, category_id: Option<i64>)`、`list_all_mods() -> Vec<ModDto>`（跨分类）。
- `ModDto` 加 `category_id: Option<i64>`；`list_mods(character)` 不变（角色视图）。
- 删除分类：非空时后端直接执行"Mod 移回未分类 + 删行"，确认逻辑在前端（行内确认模式）。

### 前端
- 新组件：`Sidebar.svelte`、`Toolbar.svelte`、`ModCardGrid.svelte`（大卡片）、`ModCard.svelte`、`CategoryMenu.svelte`（"移到分类…"浮层）、`SignalDot.svelte`（角色卡信号灯）。
- `api.ts` 加上述命令 + mock 内存实现（含分类 CRUD 与归类）。
- 状态：`view` 联合类型 + `scrollMemory: Map<string, number>`。

## 4. 视觉

### Mod 大卡片（category/all 视图）
- 16:9 大缩略图（无图：玻璃占位 + 首字符），悬停微放大（overflow hidden + scale 1.05）。
- 下方信息区：名称（截断）/ 大小 · 文件数 / 分类标签 · Toggle。
- 悬停浮起（translateY -2px、投影加深、边框高光）；操作按钮（重命名/移到分类/卸载/打开目录）悬停浮出于卡片右上角；卸载沿用行内二次确认（卡片变确认态）。
- 键盘：聚焦卡片 Space/Enter 启停（沿用 ModRow 模式）。

### 角色卡优化 + 信号灯
- 底部信息区改独立玻璃条（blur 更强，与背景图分离），名称/计数/徽标基线对齐——根治"被图片背景遮挡"同类问题。
- **SignalDot**：右上角 8px 圆点 + 光晕——enabled==1 绿（#34c759）、>=2 黄（#ffd60a）、==0 灰（rgba 灰、无光晕）。title 提示「N 个 Mod 启用中」。

### 预设菜单遮挡修复（根因）
`PresetMenu` 面板 `position:fixed` 的包含块被祖先 `backdrop-filter` 改写，z 序被卡片背景图压住。修复：预设入口移入工具条，面板改 `position:absolute` 相对工具条定位，显式 z-index 高于内容区卡片。

### 亮色主题
- 全部颜色抽 CSS 变量（`--bg/--surface/--surface-hover/--text/--text-2/--accent/--border/--shadow` 等），挂 `:root[data-theme=light|dark]`；`data-theme` 由设置驱动，`auto` 时监听 `prefers-color-scheme`。
- config 加 `theme: "auto"|"light"|"dark"`（默认 auto），设置页三选一。
- 亮色调参：背景 `#eef1f6` + 克制的淡色径向渐变球；玻璃面 `rgba(255,255,255,.55)` + blur 20px + saturate 1.4 + `inset 0 1px 0 rgba(255,255,255,.8)` 内高光；投影 `0 8px 30px rgba(30,40,60,.12)`；文字 `#1c2333`/`#5a6478`；强调色保持蓝紫系提饱和。暗色参数不变。

### 中文化
残留英文全部替换（Inspect/Reload/占位文案等）；窗口标题保持 LiquiMod。

## 5. 错误处理

| 场景 | 行为 |
|---|---|
| 分类名重名/空/含路径分隔符 | 后端 `InvalidInput`；前端行内红字 |
| 删除非空分类 | 前端二次确认「N 个 Mod 将移回未分类」 |
| set_mod_category 目标分类不存在 | `NotFound`，前端 toast |
| 搜索无结果 | 玻璃占位「没有匹配的 Mod」 |

## 6. 测试

- core：分类 CRUD/调序/删除移回未分类/重名拒绝；`list_all_mods` 聚合；迁移幂等。
- app：命令 DTO 对齐；`set_mod_category` 校验。
- ui：Sidebar 渲染/新建分类行内输入/分类右键菜单；ModCard 操作与启停；SignalDot 三色逻辑；Toolbar 面包屑与排序；主题切换 data-theme 属性。
- E2E：真实 exe CDP 实测——新建分类→归类→切换视图；角色卡信号灯三色；预设菜单不被遮挡；亮色主题截图目测；搜索过滤。

## 7. YAGNI 声明（明确不做）

- 多标签制、子分类、分类图标/颜色自定义、拖拽归类（悬停菜单已够）、分类维度预设过滤、暗色参数重调、详情页大改（仅换卡片样式接入）。

## 8. 里程碑内任务划分

1. core 分类模型 + 命令 + 测试
2. app 壳命令 + config.theme/character_category_name + DTO
3. 主题系统（CSS 变量重构 + 亮色 + 切换）
4. 新布局骨架（Sidebar/Toolbar/view 状态/滚动记忆/预设修复）
5. ModCard 网格 + CategoryMenu + SignalDot + 角色卡优化
6. 中文化收尾 + E2E 实测 + 终审
