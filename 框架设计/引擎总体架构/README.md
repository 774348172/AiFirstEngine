# 引擎总体架构文档入口

[阶段49实际压力矩阵](阶段完成记录/2026-10-18-371-S2-Execution-Chain-Continuation/49-BM03有效输入与实际压力矩阵.md)与[阶段50终止路径验证完成](阶段完成记录/2026-10-18-371-S2-Execution-Chain-Continuation/50-会话终止后的原生帧截断.md)：371阶段50已补齐scoped Clippy及真实Windows/Vulkan终止分支红绿验证：11帧正常呈现后第12帧原时钟超载，修复阻止旧帧准备/提交并保留首因、exit1。新增回归为test-only，原Release与3项报告消费者按身份复用。阶段51正在沿原有效1×K1包拆分renderSubmit成本，尚无性能改善声明；阶段49原2×K1失败保持。原X1–X6、OS显示access_denied/正式轮次等仍未关闭，S1/371 partial。新授权00:35:48 UTC开始、03:20:48停止新增、03:35:48硬截止，预算/安装/游戏V0.0.5及374 B路保持。

本文档只作为当前入口索引，不记录施工流水账。

[374 Animator与Spine方案B](374-Animator与Spine骨骼网格变形及动作混合-方案B-v1方案.md)：Gate A保持通过；Gate B已启动，隔离后端新增严格资源预检、依赖Cook key、骨骼/IK/transform/path、weighted/linked/deform、skin/draw order、有界裁剪几何与linear PMA归一化。20项B测试加11项A回归共31项通过，Clippy零warning/fmt通过。B整体partial：原Assembler/包/RenderProjection/native GPU尚待串行接入，真实像素与无源包consumer未执行；C–F未启动、未安装。结果见[374-B实际合同](374-B-实际合同与验收.md)，当前施工与唯一归属见49/54。

[371大型游戏级AI原生3D平台方案B](371-大型游戏级AI原生3D平台-方案B-v1方案.md)已有[总合同](371-大型游戏级AI原生3D平台-方案B-v1总合同.md)、基准与基础设计，并完成[总施工文档](施工文档/已完成/371-大型游戏级AI原生3D平台-方案B-总施工文档.md)和[方案/施工审核](371-E-方案复审与施工文档审核.md)。当前状态只见[49](49-当前状态与下一步入口.md)，执行槽与待执行只见[54](54-当前可自动化施工总入口.md)。§0.26本窗合并整改结果与未闭合出口见[22记录](阶段完成记录/2026-10-18-371-S2-Execution-Chain-Continuation/22-动态发布与native消费合并收敛.md)。

顶层 authority：

```text
00-AI-First-Game-Engine-权威产品需求-v1.1.md
00-AI-First-Game-Engine-权威架构设计-v1.1.md
```

旧 `01-目标与核心原则.md` 和 `02-AI功能生成流程.md` 已移入 `历史文档/`，不再是当前入口。

## 当前最重要规则

```text
内部跨域同步统一使用 World Projection / Projection Adapter。
旧 Bridge / Extract / Hydration 名称只作为历史落地名保留。
新增类型时只新增对应 ProjectionAdapter，不新增独立 Bridge 系统。
```

权威规则：

```text
142-M9-Asset-Browser-Productization-v1方案.md
141-M8-Schema-driven-Inspector-Details-System-C-full方案.md
140-M7-Prefab-Workflow-Reusable-Authoring-Object-System-v1方案.md
110-World-Projection-Adapter统一跨域同步规则.md
111-Native-Editor-Real-UI-Present-方案B.md
112-Native-Editor-Text-Rendering-C-min方案.md
113-Native-Editor-FontSystem-v1方案.md
114-Native-Editor-UI-RenderGraph-RHI-收敛方案.md
115-EngineRHI-Trait与RuntimeRenderer迁移方案.md
116-真实WgpuBackend完整v1方案.md
117-Runtime-WGPU-Surface注入-WindowedPlayerPresent-v1方案.md
118-WindowedPlayer-Runtime-v1完整方案.md
121-Native-Editor-Application-Shell方案.md
122-Editor-Authoring-Workspace-C-min方案.md
```

当前可施工入口及待执行队列以[54](54-当前可自动化施工总入口.md)为唯一真相，不在本文复制动态状态。

以下309/254为历史归档摘要，其中“当前”和队列状态仅指当时，不覆盖49/54：

309-A/B/C/D/E 已完成并归档，当前能力到 Headless neutral read、snapshot lease、统一 mutation/CAS、
crash recovery，以及 Editor Scene 完整文档 LWW Save/Save As。309-F C-Atomic Headless Engine Tool
Provider 也已完成：Gate A-I 与 F1-F12 全部通过，真实 Codex Agent Tool Loop 已验收，旧 Gateway 生产路径
已退役，施工文档和阶段记录已归档。当前施工槽与待执行队列均为空；Tower P1-2 为 `stopped_by_user`，禁止恢复。

当前进度：`R2 lifecycle retired / 254 Core simplified`。254-R2 已 superseded 并转为历史，`ProductionCandidateModule` remediation 已取消；退役施工、fresh exact-commit 预检、唯一一次 default/all-features 权威回归、cleanup、完成记录与归档均已闭环。第二次 Fresh Candidate、Real Evaluation 和所有历史 Gate 仍禁止；当前施工与待执行队列均为空。

## 推荐阅读

只看当前状态：

```text
00-阅读顺序.md
00-文档地图.md
49-当前状态与下一步入口.md
54-当前可自动化施工总入口.md
施工文档/README.md
阶段完成记录/README.md
```

理解核心架构：

```text
00-AI-First-Game-Engine-权威产品需求-v1.md
00-AI-First-Game-Engine-权威架构设计-v1.md
03-系统分层与混合数据模型.md
04-引擎能力边界与蓝图.md
05-逻辑系统边界-DSL-IR-RustAOT-ECS.md
06-资源系统架构.md
07-Build-Export-Pipeline.md
08-架构治理层.md
09-热更新能力边界.md
10-技术路线与迁移.md
11-测试与验证系统.md
12-团队协作与版本控制.md
13-AI资源生产管线.md
14-库系统与项目资源归属.md
15-Scene-Entity-Component-Prefab数据模型.md
16-ECS写入与项目规则边界.md
17-Runtime-FrameLoop.md
```

理解落地路线：

```text
18-长期主义实现路径.md
19-Project-Schema-v1.md
20-编辑器工程结构.md
21-Runtime-Core-Boundary.md
22-Canonical-Rule-IR-v1.md
23-IR-Interpreter-MVP.md
24-阶段6收敛-真实RuntimeSystem接入IR.md
25-Asset-DB-Importer-MVP.md
26-Runtime-Replay-MVP.md
27-Renderer-MVP.md
28-AI-Asset-Generation-MVP.md
29-Platform-Build-Profile-MVP.md
30-Build-Graph-Core-v2.md
```

## 文档分层

```text
架构规则：根目录正式方案文档。
施工计划：施工文档/当前。
已完成施工计划：施工文档/已完成。
被替代施工计划：施工文档/历史。
执行结果：阶段完成记录。
旧路线：历史文档。
源码参考：../UE源码参考、../Unity源码参考、../Bevy源码参考、../Godot源码参考。
```

## 当前判断

当前不是继续新增零散 Bridge 的阶段，而是先把已有 `Hydration / RenderExtract / Physics2DBridge / RenderAssetBridge / AuiRenderExtract / SpriteRenderer2D Bridge` 收敛到统一 Projection 规则下。

Native Editor 当前不是继续零散补按钮的阶段，而是先建立完整 `NativeEditorApplication` 应用壳：

```text
WindowEvent -> FocusInputSystem -> EditorCommandSystem -> EditorSession
-> TransactionService -> UiModel rebuild -> DrawList -> Present
```

后续讨论和施工必须先查 `00-文档地图.md`，避免重复讨论已经确认的系统。

