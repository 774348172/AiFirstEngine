# 179-Complex Shooter Real Authoring-to-Playable Vertical Slice v1 方案

## 1. 系统是什么

`Complex Shooter Real Authoring-to-Playable Vertical Slice v1` 是复杂打飞机项目从编辑器到 Windows 可运行产物的真实纵切验收系统。

它回答的问题是：

```text
一个真实项目是否能从编辑器 authoring 入口打开？
编辑器是否能读到 Scene / Asset / Prefab / Rule / Input / AUI / Build 的工作区状态？
同一个项目是否能继续进入 DesktopExportPipeline？
导出的 RuntimePackage 是否能加载？
导出的 Player 路径是否能运行 headless/native surface gate？
最终报告是否能告诉用户和 AI 哪一段通了、哪一段还缺？
```

它不是打飞机玩法内置 API，不把 `Player / Enemy / Bullet / Health / Score` 写进引擎。复杂打飞机只作为真实样例项目和长期回归项目。

## 2. 为什么现在要做

当前已经完成了大量独立能力：

```text
Project Authoring Workspace domain summary
Complex Shooter Authoring Workflow v1
DesktopExportPipeline
RuntimePackageBuilder / Loader
Windowed Player headless gate
Exported Windows Player Process Verification Gate
Editor Visual Regression / Golden Image Gate
```

但当前最大缺口是：这些能力还没有形成一个用户级主流程。已有 `project_e2e_gate` 能从项目目录直接导出和运行，但它没有把“编辑器打开真实项目并形成 authoring/workflow 状态”纳入同一条验收链。

所以本系统要补的是：

```text
EditorSession open real project
  -> EditorUiModel / Workspace / AuthoringWorkflow
  -> Save/Build readiness check
  -> Existing complex project E2E export/player chain
  -> Unified vertical-slice report
```

## 3. 和其它引擎的对应关系

### 3.1 Unity

对应流程：

```text
Unity Hub / Project Open
  -> ProjectBrowser
  -> Scene / Hierarchy / Inspector
  -> Prefab / AssetDatabase / Input / UI
  -> Play Mode
  -> Build Settings / BuildPipeline
  -> Windows Player
```

Unity 的重点不是每个模块独立自证，而是项目能被编辑器打开、编辑、播放、构建。

### 3.2 Unreal Engine

对应流程：

```text
Project Browser / .uproject
  -> LevelEditor / ContentBrowser / Details
  -> Blueprint or C++ gameplay module
  -> PIE
  -> Cook / Package / BuildCookRun
  -> Windows packaged game
```

UE 的启发是：Cook / Package / Run 必须和编辑器项目状态、内容浏览、关卡状态在同一个产品闭环里，而不是只验证底层命令。

### 3.3 Godot

对应流程：

```text
Project Manager
  -> EditorNode
  -> FileSystemDock / SceneTreeDock / Inspector
  -> Run Current Scene / Run Project
  -> Export Project
```

Godot 的启发是：项目入口、场景、资源、运行、导出都围绕一个中心编辑器状态组织。

### 3.4 我们

我们的对应流程：

```text
Project Launcher / EditorSession
  -> ProjectAuthoringWorkspaceModel
  -> AuthoringWorkflowModel
  -> DesktopExportPipeline
  -> RuntimePackageBuilder / Loader
  -> runtime_player_winit headless/native gate
  -> VerticalSliceReport
```

差别是：我们额外要求 AI 友好，报告必须结构化说明每个 domain 的状态和下一步缺口。

## 4. 方案对比

### 4.1 方案 A：继续补单个模块

做法：

```text
继续分别补 Asset / Sprite / AUI / Input / Build 的局部功能。
```

优点：

```text
局部改动小。
每次测试目标明确。
```

缺点：

```text
继续回到无穷细节。
无法证明真实项目主流程能跑通。
用户仍然不知道如何从编辑器做出可玩项目。
```

结论：不采用。

### 4.2 方案 B：只扩展现有 project_e2e_gate

做法：

```text
继续从项目目录直接进入 export/package/player。
增加更多 metrics。
```

优点：

```text
实现最快。
复用现有 gate。
```

缺点：

```text
仍然绕过编辑器 authoring 入口。
不能证明 EditorSession / Workspace / Workflow 对真实项目有效。
容易变成 fixture 级测试。
```

结论：不单独采用，但复用其 export/player 链路。

### 4.3 方案 C-min：真实编辑器 authoring 入口 + 复用现有 E2E 链路

做法：

```text
EditorSession open project
  -> build EditorUiModel
  -> collect workspace / workflow readiness
  -> validate required domains
  -> run existing complex project E2E gate
  -> merge into authoring-to-playable report
```

优点：

```text
覆盖真实编辑器入口。
复用已有 export/player gate，不重复造系统。
报告对用户和 AI 友好。
能发现“目录能打包但编辑器打不开/看不到/状态不对”的问题。
保持引擎只提供通用底座，不新增项目玩法 API。
```

缺点：

```text
第一版仍然不是完整商业级编辑器 walkthrough。
默认仍以 headless/native gate 验证 Player，不强制真实 OS window。
```

结论：采用方案 C-min。

## 5. 正式架构规则

### 5.1 归属规则

```text
project_e2e_gate
  承载复杂项目纵切验收。

editor_core
  负责 EditorSession / EditorUiModel / Workspace / AuthoringWorkflow。

runtime_cli / runtime_player_winit
  负责导出产物和 Player 运行验证。

engine_runtime
  负责 RuntimePackage / World / FrameLoop / Render / Input 等底座。
```

不新增独立 crate。第一版在 `project_e2e_gate` 内新增 vertical slice 模块，避免验证系统散掉。

### 5.2 用户级流程规则

纵切报告必须包含：

```text
editor_open_project
editor_workspace_readiness
authoring_workflow_readiness
desktop_export
runtime_package_load
headless_player_run
optional_real_window_gap
```

其中 `desktop_export / runtime_package_load / headless_player_run` 复用已有 `run_complex_project_e2e_gate` 的结果，不重写底层。

### 5.3 引擎侧和项目侧边界

引擎侧只验证通用能力：

```text
Project open
Scene document
Asset count
Prefab count
Rule manifest count
Input mapping count
AUI document count
Build/export status
Runtime package entity count
Frames/present/draw metrics
Diagnostics/gaps
```

项目侧才拥有：

```text
Player
Enemy
Bullet
Score
Weapon
Wave
Boss
Drop
```

纵切报告可以读取这些项目文件数量或 entity 数量，但不能把它们变成引擎 API。

### 5.4 AI 友好规则

报告必须结构化输出：

```text
status
steps
metrics
workspace_domains
authoring_workflow_steps
blocking_gaps
artifacts
diagnostics
next_actions
```

AI 不需要解析屏幕文字，也不需要猜哪个步骤失败。

### 5.5 不做范围

第一版不做：

```text
完整商业级打飞机玩法。
完整可视化 Rule Graph Editor。
完整真实 OS window mandatory gate。
完整 PNG / pixel diff。
安装包、签名、发布商店。
项目侧 gameplay 专用 API。
```

## 6. 第一版验收标准

必须证明：

```text
真实 samples/complex_shooter_project 能被 EditorSession 打开。
打开后 EditorUiModel.mode == AuthoringWorkspace。
Project / Asset / Scene / Prefab / Rule / Input / AUI / Build / Report domain 可读取。
AuthoringWorkflowModel 可生成，且 project/assets/scene/rules/input/aui 至少有内容或状态。
同一项目继续通过 existing E2E export/player chain。
最终 report 可序列化、可写入 artifact。
失败时输出明确 diagnostic。
```

## 7. 自审

### 7.1 是否合乎规格

结论：通过。

理由：

```text
用户确认进入下一步大系统讨论。
系统围绕复杂打飞机真实编辑到 Windows 可玩闭环。
没有继续扩展测试系统细节，而是回到真实 authoring / build / run 主流程。
```

### 7.2 是否合乎既有规则

结论：通过。

理由：

```text
对齐 130 缺失能力基线。
不重新讨论已完成系统，只复用 project_e2e_gate。
遵守大系统优先，不围绕单个按钮。
```

### 7.3 是否合乎长期主义

结论：通过。

理由：

```text
建立真实项目级纵切报告，后续每个系统都能挂到这条主链路验收。
不新增项目玩法 API。
不新增分散测试 crate。
```

### 7.4 是否方便实现

结论：通过。

理由：

```text
EditorSession 已能 open project 并自动打开默认 scene。
EditorUiModel 已能生成 ProjectAuthoringWorkspaceModel / AuthoringWorkflowModel。
project_e2e_gate 已能 export/package/headless-player。
```

### 7.5 是否合理且能实现

结论：通过。

理由：

```text
C-min 第一版只把已有能力串成一条产品级纵切，不要求一次补完商业级编辑器。
报告结构可以长期扩展真实窗口、截图、AI patch、golden scenario。
```

## 8. 最终结论

采用方案 C-min：

```text
Editor authoring readiness
  + existing complex project E2E export/player chain
  + unified authoring-to-playable vertical slice report
```

下一步生成施工文档并开始施工。
