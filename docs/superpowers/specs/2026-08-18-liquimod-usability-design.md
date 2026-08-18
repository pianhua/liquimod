# LiquiMod 里程碑 7：可用性攻坚 设计文档

> 日期：2026-08-18 · 状态：已获主人批准范围
> 目标：把已有后端能力全部透到 UI，修掉卡手交互，达到日常可用标准。

## 1. 背景

实测确认：后端扎实（130 tests 全绿，`remove_entry` 等命令早已存在），但前端大量能力未接线、交互缺失：
- B1：进角色再返回，主页滚动位置丢失回顶部（CDP 实测复现）
- B2：UI 无删除 Mod 入口（后端 `remove_entry` 前端零调用）
- B3：空格键启停未实现（设计文档 §4.3 明确承诺）
- 缺口：自动启用设置、日志查看、Mod 行信息密度/行内操作

## 2. 范围

### 2.1 修 Bug（交互记忆）
- **B1 滚动记忆**：`+page.svelte` 离开主页/详情/设置时记录各视图 scrollTop（模块级 Map，key = 视图标识；详情 key 含角色名），返回时 `tick()` 后恢复。搜索词同理保留（已是 $state，组件不销毁即保留——确认视图切换是条件渲染而非销毁；若是 keyed each 重建则需提升 state 到模块级）。
- **B3 空格启停**：详情页 Mod 行可聚焦（tabindex=0），聚焦时 Space/Enter 切换启停（与点击开关同路径，`e.preventDefault()` 防滚动）。

### 2.2 Mod 行内操作（接通后端）
详情页 Mod 行新增三个操作，hover 时浮现（键盘聚焦同样可见，`focus-within`）：
- **打开目录**：复用已授权的 `opener` 插件，`revealItemInDir` 定位到仓库内 Mod 目录。
- **重命名**：行内编辑（名字变输入框，Enter 确认 / Esc 取消）。后端新命令 `rename_mod(id, new_name)`：
  - 校验非空、不含路径分隔符、同角色下不重名
  - 若已启用：先删 Junction → 重命名仓库目录 → 按新名重建 Junction → 更新 DB（全程 op_log 事务）
  - 若未启用：重命名目录 + 更新 DB
- **卸载**：接通既有 `remove_entry`。行内确认（点卸载 → 行内变「确认卸载？删除文件不可恢复 [确认][取消]」），不弹窗。

### 2.3 Mod 行信息密度 + 视觉升级
- Mod 行重设计（主模型亲自设计 UI，子代理机械执行）：
  - 缩略图 56px → **72px 圆角 14px**
  - 名字下增加副行：**大小 · 文件数 · 安装日期**（人话格式：`123 MB`、`42 文件`、`8月12日`）
  - hover/focus-within 浮现三个胶囊小按钮（打开/改名/卸载），右侧开关不变
- DB：`mods` 表加 `size_bytes INTEGER`、`file_count INTEGER`（`ALTER TABLE` 迁移，既存行默认 -1 表示未统计）。
- core `Library::scan()` 扫描时顺带 `walkdir` 统计写入；`ModDto` 加 `size_bytes/file_count/installed_at`（installed_at 已有列，透出即可）。

### 2.4 自动启用设置
- `AppConfig` 加 `auto_enable: bool`（默认 false）。
- 设置页「行为」区加 iOS 开关行。
- 安装命令收尾处：`if config.auto_enable { deploy enable }`（失败 toast 非阻断，Mod 仍入库）。

### 2.5 日志查看
- core/壳已有 tracing？确认现状：若无，app 加 `tracing-appender` 滚动日志到 `%APPDATA%/LiquiMod/logs/liquimod.log`（daily 滚动）。
- 关键操作（安装/启停/删除/预设应用/对账）写 `tracing::info!`。
- 新命令 `read_log() -> String`（读最近 ~200 行，超出截断）。
- 设置页加「日志」区：只读 `<pre>` 玻璃面板 + 「复制」按钮 + 「刷新」按钮。

### 2.6 启动恢复验证
- 确认 app 启动路径调用了 `Library::recover`（op_log 对账）；若无则补上（`lib.rs` setup 内，锁内一次）。

## 3. 明确不做（YAGNI）

- 右键上下文菜单（hover 胶囊已覆盖同能力，右键菜单在 WebView 里体验差）——后续需要再说
- 全局 Mod 概览页、启动游戏按钮、随机 Mod、预设详情页——里程碑 8 候选
- Mod 自定义封面上传——后续
- 大卡片封面式布局改版——本次只把行做宽做密，不动整体列表形态

## 4. 错误处理

- rename 冲突/非法名：人话 toast（"已存在同名 Mod"、"名字不能含 / \\ 等字符"），行内编辑框保持打开
- 卸载失败（文件占用）：toast "删除失败，可能有文件被占用"，行状态不变
- 自动启用部署失败：toast 提示但安装本身算成功
- size 统计失败（权限等）：记 -1，前端显示「—」，不阻断扫描

## 5. 测试策略

- core：rename_mod 单测（未启用/已启用 junction 重建/重名冲突/非法名）；scan 统计 size/file_count 单测
- app：rename_mod 命令测试；read_log 截断测试；auto_enable 安装后自动部署测试
- 前端：ModRow 组件测试（hover 操作出现/卸载确认流/重命名提交取消）；滚动记忆单测（模块 Map 读写）
- E2E（CDP）：真实 app 验证 B1 修复（滚动→进入→返回→位置保持）、卸载按钮可见可用、空格启停、设置页自动启用开关、日志区渲染

## 6. 任务切分（供 writing-plans）

1. core：rename_mod + scan 统计（db 迁移 + 测试）
2. app：rename_mod/read_log 命令 + auto_enable 安装接线 + tracing 滚动日志 + 启动 recover 验证
3. 前端：ModRow 重设计（72px 缩略图/副行信息/hover 操作/卸载确认/重命名行内编辑）+ api.ts 接线
4. 前端：滚动记忆 + 空格启停 + 设置页（自动启用开关 + 日志区）
5. E2E 验证 + 终审
