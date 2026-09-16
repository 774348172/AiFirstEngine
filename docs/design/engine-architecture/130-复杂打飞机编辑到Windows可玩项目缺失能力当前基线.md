# 130-复杂打飞机编辑到 Windows 可玩项目缺失能力当前基线

## 1. 文档目的

本文是后续讨论“复杂打飞机从编辑器创作到 Windows 桌面可玩项目”的统一基线。

后续讨论、方案、施工文档都应优先对齐本文，不再偏离到零散按钮、局部 UI、单个小功能，除非该功能明确服务于本文列出的缺失能力。

本文不是打飞机玩法设计文档，也不允许把以下概念做成引擎内置 API：

```text
Player
Enemy
Bullet
Health
Damage
Score
Wave
Weapon
Boss
Drop
```

这些属于项目侧，由 Project Schema / Project Rule / Prefab / Asset / AUI / Sample Project 定义。

引擎只补通用底座能力：

```text
Entity
Component
ComponentValue
Field Path
Query
CommandBuffer
Prefab
AssetRef
RuntimePackage
Input Action
Physics2D
SpriteRenderer2D
AUI
Trace
Report
Build
Windowed Player
```

## 2. 当前已经完成的能力

截至 `129-Editor Build / Export Workspace v1`，已经具备：

```text
Native Editor Window / UI / FontSystem / WGPU present
Project Launcher / Open / Create / Recent Projects
Scene Editing C-min
Inspector Field Editing 基础
Project Browser 基础
AI Panel 基础
Asset Import / Asset DB / AI Image Generation 基础
RuntimePackageBuilder C-min
RuntimePackageLoader
RuntimeScene -> ECS World Hydration
Engine Gameplay Foundation C-min
Physics2D Foundation C-min
SpriteRenderer2D / RenderProjection 基础
RuntimeRenderer / EngineRHI / WgpuBackend smoke
WindowedPlayer headless gate
DesktopExportPipeline
Native Editor Build Export panel
Export / Output / Report UI command
DesktopExportReport / package-manifest / player report
```

现在已经从：

```text
底层可以导出
```

推进到：

```text
编辑器中可以点击 Export，并能读取导出反馈
```

但这仍然不是完整可玩的 Windows 桌面游戏闭环。

## 3. 完整目标链路

最终目标链路必须是：

```text
用户自然语言 / AI 生成请求
  -> Project Schema / Rule / Prefab / Scene / Asset / AUI / Input
  -> Editor Authoring Workspace 可查看、可编辑、可保存
  -> Project Saved Document
  -> Asset DB / Importer / Cook
  -> RuntimePackageBuilder
  -> RuntimePackageValidationReport
  -> Desktop Package Builder
  -> Native Windowed Player exe + data
  -> OS window / input / time / frame loop
  -> RuntimePackage load
  -> RuntimeScene World Hydration
  -> Project Rule Execute
  -> Physics2D / Input / AUI / RenderProjection
  -> RuntimeRenderer / EngineRHI / WgpuBackend
  -> Surface Present
  -> Trace / Report / Golden Scenario
```

凡是靠 fixture、测试假数据、headless-only、临时 smoke、只生成 report 而没有用户级闭环的，都只能算阶段验证，不算完整完成。

## 4. 当前最重要缺失能力总表

| 编号 | 缺失能力 | 层级 | 当前状态 | 为什么阻塞可玩桌面项目 | 优先级 |
|---|---|---|---|---|---|
| M1 | Project Authoring Workspace v1 | Editor | 有多个 C-min 面板，但未形成统一创作流 | 用户无法像 Unity 一样自然组织 Scene / Asset / Prefab / Rule / AUI / Input / Build | P0 |
| M2 | Project Rule Authoring / Compile / Runtime Execute | Logic / Build / Runtime | 有 ProjectLogicRunner 和 fixture，产品化规则链路不足 | 子弹移动、生成实体、碰撞响应、得分等项目规则无法由编辑器 / AI 稳定进入 Player | P0 |
| M3 | RuntimePackage -> Native Windowed Player exe | Runtime / Platform | 有 headless gate，真实用户 Player 体验不足 | 用户需要双击 exe 打开窗口并连续运行游戏 | P0 |
| M4 | Runtime Asset Cook -> GPU Resource Binding | Asset / Render / RHI | 有 descriptor / smoke，真实纹理和材质绑定不足 | 图片、sprite、特效、UI 贴图无法稳定显示 | P0 |
| M5 | Sprite2D 产品级运行链路 | Render / Runtime | SpriteRenderer2D 基础存在，产品级相机、排序、透明、材质不足 | 飞机、子弹、敌人、背景、爆炸都依赖稳定 2D 渲染 | P0 |
| M6 | Desktop Build And Run 体验 | Build / Editor | 已有 Export 面板，但不完整 | 还缺真实打开输出、运行导出结果、失败定位、用户级 build/run 流程 | P0 |
| M7 | Prefab Workflow | Editor / Runtime | 有基础实例化能力，编辑器产品流不足 | 复杂项目需要复用对象、批量实例、统一更新 | P1 |
| M8 | Schema-driven Inspector | Editor | 有字段编辑基础，复杂对象 / 数组 / 自定义组件支持不足 | AI 和用户都需要编辑任意项目组件字段 | P1 |
| M9 | Asset Browser 产品化 | Editor / Asset | 有 ProjectBrowser 基础，资产管理能力不足 | 图片、音频、prefab、scene、rule、AUI 需要统一管理和引用 | P1 |
| M10 | Input Mapping Authoring -> Runtime | Editor / Input / Runtime | Runtime input 基础存在，编辑器 authoring 不完整 | 用户需要配置键盘、鼠标、手柄并进入 Player | P1 |
| M11 | Physics2D Collider Authoring / Visualization | Editor / Physics | Physics2D Foundation 存在，可视化和编辑不足 | 碰撞体、触发区、命中调试需要可见、可改、可诊断 | P1 |
| M12 | AUI HUD Authoring / Binding / Runtime Present | UI / Runtime | AUI 方向存在，工作流不完整 | 血量、分数、暂停、结算等 HUD 需要编辑和运行时显示 | P1 |
| M13 | Unified Report Panel | Editor / Diagnostics | 有 Console 和多个 report，统一展示不足 | Build / Asset / Rule / Runtime / Render 错误需要用户和 AI 快速定位 | P1 |
| M14 | Save / Reload / Rebuild 一致性门禁 | Editor / Build / QA | 有局部保存和回归，完整门禁不足 | 编辑器改完、保存、重开、打包、运行必须一致 | P1 |
| M15 | Exported Game Golden Scenario | Test / QA | 有模块级测试，导出产物级验收不足 | 必须证明导出的 exe 真的能运行、输入、碰撞、渲染、产生日志 | P1 |
| M16 | AI Project Patch Entry | AI / Editor | 有 AI Panel 和资源生成基础，完整项目 patch 入口不足 | 用户自然语言需要能修改 Scene / Prefab / Rule / Asset / AUI / Input | P2 |
| M17 | 复杂样例项目资产与规则集 | Sample / QA | 有 fixture，没有真实长期样例项目 | 需要一个真实项目持续验证所有系统组合是否还能工作 | P2 |

## 5. P0 必须优先打通的五条链

### 5.1 Project Authoring Workspace v1

目标不是再补一个小面板，而是建立完整编辑工作区：

```text
Project
  -> Scene
  -> Asset
  -> Prefab
  -> Rule
  -> AUI
  -> Input
  -> Play
  -> Build
```

验收：

```text
用户能打开项目。
能在同一个编辑工作区查看和编辑 Scene / Asset / Prefab / Rule / AUI / Input。
能保存项目。
能触发 Play / Build。
AI 能读取同一套结构化 workspace state。
```

### 5.2 项目规则进入 Runtime

复杂打飞机玩法不能进入引擎 API，但项目规则必须进入 Runtime。

目标：

```text
Project Rule Source
  -> Rule Manifest
  -> Compile / Register
  -> ProjectLogicRunner
  -> FrameLoop
  -> ECS Query / Write / CommandBuffer
  -> Trace
```

第一版可以只做 Rust AOT registered rule gate，但必须保留长期 IR/RustAOT 边界。

验收：

```text
项目规则根据输入 action spawn 一个 prefab-like entity。
Runtime 执行规则后 World 发生变化。
Trace 能说明读取、写入、命令和错误。
```

### 5.3 RuntimePackage 进入真实 Windowed Player

目标：

```text
Player exe
  -> OS window
  -> RuntimePackage load
  -> EngineHostLoop
  -> Input / Time / FrameLoop
  -> Render / RHI / Surface Present
```

验收：

```text
给定 RuntimePackage 目录。
双击或命令启动 player exe。
打开真实窗口。
连续运行多帧。
能看到 package 中的 sprite。
输入能改变 World。
错误写入 report。
```

### 5.4 Runtime Asset Cook -> GPU Resource Binding

目标：

```text
Imported Asset
  -> Cooked Asset
  -> RuntimeAssetIndex
  -> Runtime load
  -> GPU upload
  -> Material / Texture / Mesh / Sprite binding
  -> Draw
```

验收：

```text
导入一张图片。
放到 Scene 中作为 Sprite2D。
构建 RuntimePackage。
Player 中显示真实图片，而不是测试颜色块或 smoke geometry。
缺失资源时能报告 AssetRef / cooked path / binding stage。
```

### 5.5 Sprite2D 产品级运行链路

目标：

```text
SpriteRenderer2D
  -> Camera2D
  -> Layer / Sorting
  -> Transparent Blend
  -> Material / Texture
  -> Batch / DrawPlan
  -> RHI
```

验收：

```text
背景、主体、多个移动物体、UI 层级可以稳定显示。
透明 sprite 正确混合。
排序规则简单明确。
Trace 能说明一个 sprite 为什么没有显示。
```

## 6. P1 产品化能力

P0 打通后，复杂项目还需要这些能力达到可编辑、可修改、可调试：

```text
Prefab Editor
Schema-driven Inspector
Asset Browser
Input Mapping Editor
Physics2D Collider Visualization
AUI HUD Workflow
Play / Stop / Build And Run UI
Unified Report Panel
Save / Reload / Rebuild Consistency Gate
Exported Game Golden Scenario
```

这些能力可以分阶段做，但讨论时必须挂到 Project Authoring Workspace 的统一流程里，不能再次散成无穷的小按钮。

## 7. P2 AI 和样例项目能力

AI 相关能力不应该靠临时 prompt 拼接，而应该进入项目 patch 流：

```text
Natural Language
  -> AI Plan
  -> Project Patch
  -> Validate
  -> Apply Transaction
  -> Preview / Play / Build
  -> Trace / Report
```

需要支持：

```text
修改 Scene
修改 Prefab
修改 Project Rule
导入 / 生成 Asset
修改 AUI
修改 Input Mapping
解释失败原因
生成可回滚 transaction
```

复杂样例项目的目的不是把打飞机做成引擎内置玩法，而是作为长期回归项目：

```text
验证编辑器创作流。
验证 RuntimePackage。
验证 Player exe。
验证真实资源渲染。
验证项目规则执行。
验证 AUI / Input / Physics2D / Report。
```

## 8. 后续讨论和施工规则

后续讨论必须遵守：

```text
1. 先判断问题属于本文哪一个 M 编号。
2. 如果不属于任何 M 编号，先说明为什么需要新增缺失能力。
3. 不为打飞机项目增加引擎专用 API。
4. 优先讨论大系统，不再围绕单个按钮反复拆小点。
5. 每个大系统必须有：
   - 目标链路
   - 引擎侧 / 项目侧边界
   - 与 Unity / UE / Godot / Bevy 对比
   - 最小验收场景
   - 可施工 gate
   - 每 gate 测试命令
6. 已完成系统不重新讨论，除非实现证明不可行或用户明确要求推翻。
```

## 9. 推荐下一步

下一步优先讨论：

```text
Project Authoring Workspace v1
```

原因：

```text
现在已经有 Build Export 面板。
真正缺的是把 Scene / Asset / Prefab / Rule / AUI / Input / Play / Build 串成一个用户可用的创作工作区。
如果没有这个统一工作区，继续补底层或小面板，都会让能力越来越散，用户仍然不能完整做出可玩的桌面项目。
```

建议下一份方案文档直接围绕：

```text
M1 Project Authoring Workspace v1
```

并同时覆盖它如何衔接：

```text
M2 Project Rule
M3 Windowed Player
M4 Asset Cook / GPU Binding
M5 Sprite2D
M6 Build And Run
```
