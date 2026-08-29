# 编辑器交互设计

本目录保存 AI 原生编辑器的 UI / UX 模板。

当前确认方向：

```text
以 Unity Editor 的基础布局为主
在 Unity 式工作流上增加 AI 能力
不要一开始设计一套完全陌生、过重的编辑器 UI
```

正式定位：

```text
Unity-like Editor Shell
+
AI-native Production Layer
```

也就是：

```text
基础编辑器体验对齐 Unity
AI 能力作为增强层加入
不重新发明一套陌生编辑器 UI
```

Unity 的 UI 已经验证了高效的专业工作流：

```text
面板稳定
信息密度高
鼠标路径短
对象选择 -> Inspector 修改 的链路顺
Project / Console / Scene 的位置已被大量开发者习惯
插件窗口可以自然 Dock
```

因此本引擎不在基础面板布局上强行创新。  
真正差异应该来自 AI 生产能力，而不是打破用户熟悉的编辑器结构。

## 当前模板

模板图：

```text
unity-clean-ai-editor-template.png
unity-style-ai-editor-template.png
```

生成脚本：

```text
unity-clean-ai-editor-template.ps1
unity-style-ai-editor-template.ps1
```

当前优先采用：

```text
unity-clean-ai-editor-template.png
```

该模板基于 Unity 2022 风格截图重新生成，只保留轻量 AI 入口，不引入大型 AI 工作区。

## 设计原则

保留 Unity 用户熟悉的结构：

```text
Hierarchy
Scene / Game
Inspector
Project
Console
Toolbar
```

在此基础上增加 AI：

```text
Ask AI toolbar button
AI menu
Scene View AI Overlay
Inspector component AI button
Dockable AI Assistant window
Bottom AI Tasks / Plan Preview
Project Context status
```

Scene View 渲染边界：

```text
Scene View 世界内容由 Runtime Renderer 渲染。
Scene View AI Overlay、工具按钮、选择框、提示气泡由 Editor UI Renderer 渲染。
Scene Viewport 通过 Runtime Renderer 输出的 viewport texture 嵌入编辑器面板。
AI Overlay 不直接绘制游戏世界，也不直接修改 Runtime / Project。
AI Overlay 只能显示解释、候选操作和可点击命令入口；具体修改仍走 UiCommand / Patch Plan / Validation。
```

正式 UI 规则：

```text
1. 默认布局对齐 Unity
2. 不用大面积营销式、卡片式、网页式 UI
3. 不让 AI 面板占据主操作区
4. AI 入口必须贴近用户当前上下文
5. AI 输出必须可审查、可预览、可验证、可撤销
6. AI 生成内容通过 Inspector / Project / Console / Scene 等原生面板展示
7. 用户可以像使用 Unity 一样手动操作，也可以随时叫 AI 介入
```

基础面板保持：

```text
Hierarchy 还是 Hierarchy
Scene / Game 还是 Scene / Game
Inspector 还是 Inspector
Project 还是 Project
Console 还是 Console
Toolbar 还是 Toolbar
```

AI 只在这些位置增加：

```text
入口
提示
预览
验证
应用流程
```

## AI 增强点

Toolbar：

```text
AI
Ask
Generate
Validate
Build with AI
```

Menu：

```text
AI
AI Assets
AI Logic
AI Fix
AI Build
```

Hierarchy：

```text
右键对象 -> Ask AI
右键对象 -> Generate Child
右键对象 -> Explain
右键对象 -> Refactor
```

Scene / Game：

```text
AI Preview
Before / After
Validation Overlay
Generated Placement Preview
```

Inspector：

```text
组件级 AI Fix
字段级 Explain / Generate / Optimize
Add Component with AI
Generate Binding
```

Project：

```text
Generate Asset
Regenerate
Find References
Explain Asset
Create AssetSet
Check Missing References
```

Console：

```text
Ask AI About Error
Generate Patch Plan
Trace to Object / Script / DSL / Asset
```

Bottom Dock：

```text
AI Plan
AI Tasks
Validation Report
Build Report
```

## AI 功能嵌入方式

AI 不是一个巨大的独立工作区。  
AI 应该像 Unity EditorWindow 一样可停靠、可关闭、可切换 tab。

基础交互：

```text
选中对象
  -> AI 读取上下文
  -> 用户输入目标
  -> AI 生成 Spec / Plan
  -> Scene / Inspector / Project 显示预览和影响
  -> 用户 Review Diff
  -> Apply to Project
```

## AI Tab 职责边界

AI 是一个独立 Editor Tab。  
它的层级类似 Unity 的 Scene / Game / Console / Project。

正式规则：

```text
AI Tab 负责生成、说明、链接、任务记录
Project / Inspector 负责资源预览
Scene 负责场景预览
Game 负责运行预览
Console 负责错误预览
Graph / DSL 面板负责逻辑预览
Build Report 负责构建预览
```

AI Tab 不负责复杂预览。  
它不能变成一个包办所有事情的大型工作区。

### AI Tab 负责什么

AI Tab 负责：

```text
用户输入自然语言
展示 AI 正在做什么
展示生成结果
展示错误 / 建议 / 操作记录
给出可点击跳转链接
提供继续修改入口
提供停止 / 继续 / 权限模式入口
```

例如用户输入：

```text
生成一组红色敌机资源
```

AI Tab 显示：

```text
已生成 3 个资源：
- enemy_small_red.sprite   打开
- enemy_medium_red.sprite  打开
- enemy_boss_red.sprite    打开

已绑定：
- RedEnemyAssetSet         打开
- Enemy_Medium Prefab      打开

验证：
- 移动端尺寸通过
- 风格一致性通过
- 1 个碰撞范围建议人工确认
```

### AI Tab 不负责什么

AI Tab 不负责：

```text
复杂资源预览
复杂场景预览
复杂 Game 运行预览
复杂 Inspector 参数编辑
复杂 Graph 编辑
复杂 Build 报告细节
```

这些仍然交给 Unity-like 原生面板。

### 跳转规则

AI Tab 中的结果必须提供可点击链接。  
点击后跳转到对应原生面板：

```text
资源 -> Project / Asset Preview / Inspector
Prefab -> Project 并 Inspector 选中 Prefab
场景对象 -> Scene 并选中对象
逻辑 -> DSL / Graph / Inspector
运行效果 -> Game
错误 -> Console
构建结果 -> Build Report
```

### 预览规则

资源预览沿用 Unity-like 规则：

```text
Project 里选中资源
Inspector / Preview 区看详情
Scene 里看场景对象
Game 里看运行效果
Console 里看错误
Build Report 里看构建结果
```

AI 只告诉用户：

```text
生成了什么
改了什么
在哪里看
需不需要确认
有没有风险
```

### AI Tab 推荐结构

AI Tab 保持轻量：

```text
顶部：自然语言输入框
中间：任务流 / 结果消息 / 可点击链接
底部：当前任务状态 / 停止 / 继续 / 权限模式
```

它更像：

```text
任务中心
结果列表
超链接导航
```

而不是：

```text
资源预览器
场景编辑器
Graph 编辑器
Inspector 替代品
```

## AI 执行权限模式

编辑器提供两种 AI 执行模式：

```text
完全放权模式
询问确认模式
```

### 完全放权模式

完全放权模式下，AI 可以在当前项目权限范围内自动执行允许的操作。

适合：

```text
个人项目
原型阶段
低风险资源生成
批量整理
重复性修复
用户明确希望 AI 自动推进
```

但即使完全放权，也必须保留底线：

```text
必须生成操作记录
必须可撤销
必须可回滚
必须进入 Asset / Patch 历史
高风险操作仍要触发保护
```

高风险操作包括：

```text
删除资源
删除场景对象
修改 Build / 发布配置
修改热更版本
修改 Standard Module
修改权限 / 密钥 / 签名配置
覆盖大量资源
影响多人协作主分支
```

这些操作不能因为完全放权就静默执行。

### 询问确认模式

询问确认模式下，AI 在执行命令前弹出确认。

确认弹窗必须提供：

```text
允许本次
同类型命令全部允许
拒绝本次
查看 Diff / Plan
```

其中：

```text
允许本次 = 只放过当前这一条 AI 操作
同类型命令全部允许 = 当前会话或当前项目策略内，后续同类型命令自动放过
拒绝本次 = 不执行当前操作
查看 Diff / Plan = 先看影响范围、改动内容、验证结果
```

同类型命令必须按语义分类，而不是按按钮名称分类。

示例：

```text
generate_asset.sprite
generate_asset.audio
edit_asset.material
modify_component.value
add_component
generate_dsl.logic_rule
compile_logic_backend
run_validation
build_preview
fix_console_error
delete_asset
publish_hot_update
```

用户选择“同类型命令全部允许”时，引擎要记录：

```text
command_type
scope
duration
risk_level
created_by
created_at
```

scope 可以是：

```text
当前对象
当前场景
当前 AssetSet
当前系统
当前项目
当前会话
```

duration 可以是：

```text
本次会话
今天
当前项目永久
直到用户撤销
```

默认建议：

```text
同类型命令全部允许 = 当前会话内生效
```

除非用户明确选择项目级长期规则。

## 权限风险分级

AI 操作按风险分级：

```text
Level 0: 只读
Level 1: 可撤销的小改动
Level 2: 影响资源 / Prefab / Scene / DSL 的普通改动
Level 3: 批量修改、删除、Build、热更、发布配置
Level 4: 密钥、签名、权限、Standard Module、原生代码、商店发布
```

建议默认策略：

```text
Level 0: 自动允许
Level 1: 可在完全放权模式下自动执行
Level 2: 完全放权可执行，询问模式需确认
Level 3: 必须确认，并展示 Diff / Plan
Level 4: 必须强确认，不能被“同类型全部允许”静默放过
```

## 权限体验原则

AI 权限系统不能变成频繁打断用户的弹窗机器。  
所以必须支持：

```text
放过一次
同类型命令全部放过
按风险等级自动处理
按项目 / 会话记录偏好
随时撤销授权
随时查看 AI 操作历史
```

但它也不能为了顺滑而牺牲项目安全。  
因此必须保证：

```text
所有 AI 执行都可追踪
所有写操作都进入 Patch / Asset Revision
所有高风险操作都必须可审查
所有批量操作必须可回滚
```

## AI 历史与回滚

AI 可以完全放权，也可以获得同类型命令的持续授权。  
因此引擎必须记录 AI 做过什么，并且保证可撤销、可回滚、可审查。

但这些复杂性不能直接暴露给用户。  

正式规则：

```text
内部可以复杂
用户界面必须简单
```

用户不应该理解这些底层概念：

```text
Undo
Transaction
Asset Revision
Patch History
Release Snapshot
```

这些属于引擎内部机制，不应该成为用户心智负担。

### 用户看到的历史系统

用户只看到：

```text
AI 历史
查看改动
撤销
回到这里
```

每一次 AI 操作显示为自然语言记录：

```text
16:20 AI 修复了控制台错误
16:35 AI 生成了敌机资源
16:50 AI 修改了商店系统
```

每条记录只提供少量操作：

```text
查看
撤销这次 AI 操作
回到这里
保存为版本
```

示例：

```text
AI 生成了火焰技能
影响：5 个资源、1 个 Prefab、1 条技能规则
状态：已验证
操作：查看 / 撤销 / 回到这里
```

### 引擎内部机制

内部仍然必须分层：

```text
普通编辑操作 -> Unity-like Undo
AI 操作 -> AI Operation Transaction
资源版本 -> Asset Revision
项目级变更 -> Patch History
发布版本 -> Build / Release Snapshot
```

原因是 AI 操作可能同时修改：

```text
资源
Prefab
Scene
DSL
IR Rule / Rust AOT Rule
Build Profile
Asset Graph
```

如果内部没有事务和版本，撤销和回滚会不可靠。

### 用户层与引擎层映射

用户层：

```text
AI 历史
查看改动
撤销
回到这里
```

引擎层：

```text
Undo
Transaction
Asset Revision
Patch History
Release Snapshot
Validation Report
Build Report
```

映射规则：

```text
AI 历史记录
  -> Patch Transaction
  -> Asset Revision
  -> DSL Change
  -> Scene / Prefab Change
  -> Validation Report
```

用户不需要知道底层具体撤销了什么文件。  
引擎负责把一次 AI 操作当成一个可审查、可撤销的整体。

### 体验原则

AI 历史不能做成专业版本控制 UI。  
它应该像产品时间线：

```text
谁做了什么
为什么做
影响了什么
结果是否通过验证
能否撤销
能否回到这里
```

最终目标：

```text
复杂性由引擎吃掉
用户只面对简单、可信、可撤销的 AI 操作历史
```

## 为什么这样设计

过度重新设计编辑器会带来两个问题：

```text
Unity / UE 用户迁移成本高
AI 面板容易压过正常编辑器操作
```

当前路线更稳：

```text
传统编辑器负责对象操作
AI 负责意图理解、方案生成、差异预览、验证和应用
```

换句话说：

```text
Unity 的布局负责降低迁移成本
AI 原生流程负责拉开产品差异
```

## 后续需要确认

```text
AI Assistant 是默认停靠在 Inspector 下方，还是右侧独立 Tab？
AI Tasks 是否作为底部固定窗口？
Scene View 的 AI Overlay 显示哪些内容？
Inspector 每个 Component 是否都提供 AI 小按钮？
资源生成候选是放在 Project 窗口、AI Tasks，还是单独弹窗？
```
