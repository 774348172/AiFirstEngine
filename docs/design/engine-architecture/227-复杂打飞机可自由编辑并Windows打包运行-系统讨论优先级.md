# 227-复杂打飞机可自由编辑并 Windows 打包运行：系统讨论优先级

> 状态：正式讨论优先级文档。
> 来源：`G:\gameEngin\4.8AI审查目录\02-目标-复杂打飞机可自由编辑并Windows打包运行-优先级路线图.md`。
> 校准日期：2026-07-09。
> 适用目标：用户能在编辑器里自由编辑复杂打飞机项目，并打包成 Windows 可执行文件；运行窗口里能真实看到飞机、能操作、能攻击、能死亡、能计分。

本文只决定“下一轮应该优先讨论哪些系统”。它不是方案文档，也不是施工文档。每个系统正式进入施工前，仍必须按 skill 流程执行：

```text
Context Scan
-> 其它引擎/源码参考
-> 本项目现状扫描
-> 2-3 个方案
-> 推荐方案
-> 正式方案文档
-> 审查/自审
-> 施工文档
-> 施工与测试
-> 阶段完成记录
```

## 1. 当前状态校准

4.8 路线图中的部分前置项已经被后续施工关闭，因此本优先级以当前入口文档和阶段完成记录为准。

已完成前置：

```text
225 Project Authoring Asset Completeness / Prefab-Rule Assetization Gate v1
  已完成：复杂打飞机 sample 已有 PrefabAsset、Scene PrefabInstance 和 Rule authoring assets。

226 PrefabInstance RuntimePackage Bake / Authoring Prefab Instance Expansion v1
  已完成：Scene 中预放的 engine.prefab_instance 已 bake 为 flattened RuntimeScene entities。

224 Editor Play Apply Runtime Change To Authoring v1
  已完成：Play Mode 临时 Inspector 修改可 Preview/Confirm 写回当前 authoring Scene。

228 Real Texture Decode / GPU Texture Upload / Sprite Textured Present v1
  已完成：复杂打飞机 sample 的真实 PNG 已 cook 进 RuntimePackage，并能通过 Sprite2D real texture binding / RealWgpuBackend texture upload / project_e2e_gate real_texture_present 证明。

232 Editor Build And Run Productization v1
  已完成：编辑器 Build 面板已有 Build & Run，复用 DesktopExportPipeline 导出 Windows dev package，并启动 staged Game.exe；EditorBuildAndRunReport / Report Panel / project_e2e_gate 已闭环。

233 Rule Graph / Card Authoring Productization v1
  已完成：Rule Cards 成为可编辑产品面，Generated read-only Rule Graph Preview 可从 Gameplay Rule Asset 派生，card edit 仍降低到 RuleAuthoringService / RuleAuthoringEditCommand。
```

仍然成立的核心判断：

```text
目标不是继续堆新概念，而是把“编辑器自由编辑 -> RuntimePackage -> Windows exe -> 真实窗口可玩”这条竖切打穿，并用强制验收证明。
```

## 2. 总优先级

### P0：可玩闭环

缺任意一项，打包出来都还不能算真正可玩。

| 优先级 | 系统 | 一句话作用 | 当前建议 |
|---|---|---|---|
| P0-1 | Real Texture Decode / GPU Texture Upload / Sprite Textured Present v1 | 让导出窗口里看到真实飞机、子弹、背景贴图，而不是色块或测试几何 | 已完成，见 228 阶段记录 |
| P0-2 | Complex Shooter Gameplay Rule Runtime Execution v1 | 让敌机移动、玩家射击、子弹生命周期、碰撞、扣血、计分真实运行 | 已完成，见 229 阶段记录 |
| P0-3 | Project Rule Driven UiStateSnapshot Producer v1 | 让 HUD 分数、血量、波次来自真实规则/ECS 状态，而不是样例写死 producer | 已完成，见 230 阶段记录 |
| P0-4 | Exported Windows Playable Golden Gate v1 | 用真实窗口、真实像素、输入、碰撞、计分证明导出的 exe 真能玩 | 已完成，见 231 阶段记录 |

### P1：自由编辑体验

这些系统决定用户是否能“不写 JSON”完成主要编辑动作。它们可以穿插讨论，但不应抢在 P0 可玩闭环之前。

| 优先级 | 系统 | 一句话作用 | 当前建议 |
|---|---|---|---|
| P1-1 | Editor Build And Run Productization v1 | 编辑器里一键导出并启动 Windows exe | 已完成，见 232 阶段记录 |
| P1-2 | Rule Graph / Card Authoring Productization v1 | 把规则从结构化服务层推进到用户可视化编辑 | 已完成，见 233 阶段记录 |
| P1-3 | Input Mapping Visual Authoring Panel v1 | 用户可视化改键鼠/手柄映射 | 已完成，见 234 阶段记录 |
| P1-4 | Asset Browser Native Productization v1 | 真实浏览、替换、引用项目资产 | 已完成，见 235 阶段记录 |

### P2：可信发布

这些系统让“能跑”升级成“可信地能跑、可复现、可发布”。

| 优先级 | 系统 | 一句话作用 | 当前建议 |
|---|---|---|---|
| P2-1 | Save / Reload / Rebuild Consistency Gate v1 | 编辑、保存、重开、重建结果一致 | 已完成，见 236 阶段记录 |
| P2-2 | Release Package Polish / Metadata / Icon / Layout v1 | Windows 产物目录、图标、元数据、发布结构收尾 | 已完成，见 237 阶段记录 |

### P3：AI 编辑增强

这些不是“用户手动自由编辑 + Windows 可玩”的硬前置，但会显著提升 AI 深度参与体验。

| 优先级 | 系统 | 一句话作用 | 当前建议 |
|---|---|---|---|
| P3-1 | Real LLM Provider / Minimal Repair Loop v1 | 真实 HTTP provider + 最小修复循环，让 AI patch 不只停留在 mock/stub | 已完成，见 238 阶段记录 |

## 3. 下一轮推荐讨论顺序

### 当前有效排序（2026-07-11 更新）

```text
P0-1 已完成：Real Texture Decode / GPU Texture Upload / Sprite Textured Present v1。
P0-2 已完成：Complex Shooter Gameplay Rule Runtime Execution v1。
P0-3 已完成：Project Rule Driven UiStateSnapshot Producer v1。
P0-4 已完成：Exported Windows Playable Golden Gate v1。
P1-1 已完成：Editor Build And Run Productization v1。
P1-2 已完成：Rule Graph / Card Authoring Productization v1。
P1-3 已完成：Input Mapping Visual Authoring Panel v1。
P1-4 已完成：Asset Browser Native Productization v1。
P2-1 已完成：236 Gate A-F、整体回归、阶段记录与归档均已完成。
P2-2 已完成：237 B-min+ 的 BuildProfile v2 + AssetRef icon + stamped direct Runtime entrypoint + manifest-driven entrypoint/file roles + portable relative layout + staging process gate + ReleasePackageReport 已通过 Gate A-F 与整体回归并归档。
P3-1 已完成：238 B-min+ 的真实后台 HTTP + strict ProjectPatch schema + stale/cancel guard + 最多一次 diagnostics-driven repair 已通过 Gate A-G、整体回归并归档。
239 B-min+ Critical Correctness and Safety Convergence Gate 已完成 Gate A-E、整体回归、完成记录与归档；当前没有可直接施工文档。后续审查讨论顺序以 `240-5.6审查剩余问题讨论与施工优先级.md` 为准，下一步默认先讨论 CQ-04。
```

### 已完成：P0-1 Real Texture Decode / GPU Texture Upload / Sprite Textured Present v1

当前状态：

```text
228 已完成 C-min 施工。
真实 PNG -> RuntimePackage cooked texture metadata + rgba8 payload -> Runtime load -> Sprite2D real texture binding -> RealWgpuBackend texture upload / sampler bind group -> real_texture_present report 已打通。
下一轮不再重复讨论 P0-1，除非后续测试证明贴图链路回归。
```

为什么当时第一：

```text
复杂打飞机是强视觉项目。
Prefab bake 已完成后，Scene 对象可以进 RuntimeScene。
但如果真实纹理没有进入 wgpu 贴图采样链路，导出窗口仍然无法证明“看到真实飞机/敌人/背景”。
这也是后续真实窗口 golden 的前置。
```

讨论时必须重点确认：

```text
PNG/RGBA 解码是否已有 metadata-only 基线。
RuntimePackage 中 texture asset 如何进入 RuntimeAssetIndex。
RealWgpuBackend 当前是否仍用硬编码颜色/测试几何。
SpriteRenderer2D 如何从 SpriteRef/TextureRef 解析到 GPU texture bind group。
Report 如何区分 real_texture_present / fallback / missing / decode_failed / upload_failed。
```

预期方案方向：

```text
Build/Package:
  Texture asset -> decoded RGBA or cooked texture payload -> RuntimePackage / RuntimeAssetIndex

Runtime:
  RuntimePackage load -> texture decode/load -> wgpu::Texture -> bind group

Render:
  Sprite2D draw item -> textured quad -> RealWgpuBackend present

Validation:
  complex shooter exported/window screenshot 能证明 player/enemy/background 使用真实像素。
```

### 已完成：P0-2 Complex Shooter Gameplay Rule Runtime Execution v1

为什么现在第一：

```text
看得见之后，下一步必须能玩。
225 已补 Rule authoring assets，226 已让 PrefabInstance 进入 RuntimeScene。
现在需要把 sample 的移动、射击、碰撞、计分、死亡这些项目规则变成真实运行时行为。
```

讨论时必须重点确认：

```text
当前 .rule.json 能覆盖哪些规则。
generated Rust / RustAOT 当前是内存产物、validation-only，还是已经能进入 runtime registry。
cargo build 是否仍被跳过。
Runtime Spawn/Despawn Request 是否足够承载子弹生成与生命周期。
碰撞/命中事件如何进入 Project Rule，而不把打飞机玩法写进 engine core。
```

预期方案方向：

```text
Rule authoring assets
  -> validated Project Rule artifacts
  -> generated Rust/project module or controlled runtime rule backend
  -> RuntimePackage/Registry
  -> EngineHostLoop tick
  -> movement/fire/collision/score events
  -> structured report
```

### 已完成：P0-3 Project Rule Driven UiStateSnapshot Producer v1

为什么第二：

```text
HUD 链路已经存在，但 ComplexShooterSampleUiStateProducer 仍偏 C-min 样例。
真实可玩后，HUD 必须显示真实分数、血量、波次、敌人数。
```

讨论时必须重点确认：

```text
ProjectUiStateSnapshot 只能读项目状态快照，不让 AUI Binding 直接读 ECS 或调用 Rule。
Producer 应由项目侧 Rust framework/module 提供，不能把复杂打飞机字段写进引擎通用 API。
Report 要能证明每个 binding path 的来源、类型、缺失和 fallback。
Producer 只能按 active AUI binding path 生产数据，后续所有复杂 UI state 默认走 dirty / cached。
P0-3 可以让小 HUD 每帧轻量刷新，但不能设计成每帧全量生成全项目 UI 镜像。
```

预期方案方向：

```text
Project runtime state
  -> project-owned UiStateSnapshotProducer(active_binding_paths, dirty_domains, cache)
  -> ProjectUiStateSnapshot
  -> AUI Binding Resolve
  -> AuiRuntimePresentReport
```

### 已完成：P0-4 Exported Windows Playable Golden Gate v1

为什么第三：

```text
它是验收系统，不是补功能系统。
必须等真实贴图、真实玩法、真实 HUD 基本成立后再做，否则只能验证“会启动”，不能验证“可玩”。
```

讨论时必须重点确认：

```text
真实窗口/GPU 测试哪些默认强制、哪些保持 local-only。
pixel evidence 如何避免机器差异导致误报。
输入脚本如何验证射击、碰撞、计分。
Report 如何进入 Report Panel，并区分 runtime/editor、Off/Summary/Trace。
```

## 4. 不建议下一轮优先讨论的系统

这些系统有价值，但当前不应排在 P0 前面。

```text
完整字体导入与复杂文本排版
  重要，但复杂打飞机第一目标的硬阻塞低于真实贴图和真实玩法。

真实 LLM provider / repair loop
  对 AI 编辑重要，但不直接让导出 exe 变得可玩。

Prefab Variant / Nested Prefab
  对大型项目复用重要，但 226 已满足当前 sample 的 PrefabInstance bake 主缺口。

复杂 UI 高级控件继续扩展
  当前 AUI 已完成多轮 C-min/v1；除非装备/菜单成为明确阻塞，否则先让主游戏可玩。

Release packaging polish
  必须做，但应在可玩和可信 golden 后做。
```

## 5. 每次“讨论下一个系统”的选择规则

之后用户说“讨论下一个系统”时，默认按以下规则选题：

1. 如果 P0 仍有未讨论/未施工项，优先讨论 P0。
2. P0 已全部完成；P1 内部默认选择尚未完成的最高优先级系统。
3. 如果用户明确想优化“编辑体验”，可以从 P1 选择，但必须说明它对 P0 是否有直接帮助。
4. 如果用户明确想优化“AI 自动改项目”，可以提前讨论 P3-1，但必须说明它不会替代可玩闭环。
5. 不重复讨论已完成系统，除非实现证据证明方案走偏，或用户明确要求推翻。

## 6. 文档与施工关系

本文只作为讨论入口，不直接允许施工。

当选中某个系统后，必须新建或更新对应正式方案文档，例如：

```text
228-Real-Texture-Decode-GPU-Texture-Upload-Sprite-Textured-Present-v1方案.md
229-Complex-Shooter-Gameplay-Rule-Runtime-Execution-v1方案.md
230-Project-Rule-Driven-UiStateSnapshot-Producer-v1方案.md
231-Exported-Windows-Playable-Golden-Gate-v1方案.md
```

方案文档通过审查后，才能生成：

```text
施工文档/当前/<编号>-当前可自动化施工文档-xxx.md
```

## 7. 自审

```text
是否采纳 4.8 路线图：
  是。保留其“可见/可运行/可玩/可编辑/可信/可发布”的判断框架。

是否按当前状态修正：
  是。225、226、224 已完成，不再列为待讨论系统。

是否符合项目 skill：
  是。本文只定义讨论优先级，不越过正式方案、审查、施工文档和测试流程。

是否避免扩引擎 Core：
  是。复杂打飞机玩法仍应落在项目侧 Rule / Prefab / AUI / Project Module，不写进通用 engine API。

是否服务用户目标：
  是。优先级围绕“编辑器自由编辑 + Windows 打包运行 + 真实可玩证据”排序。
```

## 8. 结论

当前最应该推进的是：

```text
239 已完成；按 240 先讨论 CQ-04 SafeProjectPath / Project Write Containment v1，确认方案后再生成施工文档
```

原因：

```text
P0-1/P0-2/P0-3/P0-4 已完成，复杂打飞机已经具备导出包级 golden gate。
P1-1 已完成，用户已经可以从编辑器 Build & Run 导出的 Windows dev package。
P1-2 已完成，复杂打飞机规则已经具备 Rule Cards 可编辑产品面和只读 Rule Graph Preview。
P1-3 已完成，用户已经可以不写 JSON 地修改、预览、保存键鼠/手柄目录映射，并让保存结果进入 RuntimePackage。
P1-4 已完成，用户已经可以通过原生 Asset Browser 浏览、查询、打开、拖拽和替换项目资产，并让结构化 AssetRef 进入 RuntimePackage。
P2-1 已完成：236 已证明复杂项目经过真实编辑、原子保存、独立进程重开、双 clean rebuild 和正式 Runtime loader 后语义一致，并已完成整体回归与归档。
P2-2 已完成：237 Gate A-F、整体回归、阶段记录与归档均已完成，Windows portable release 合同闭环。
P3-1 已完成：238 Gate A-G、整体回归、阶段完成记录与归档均已闭环。
5.6 审查确认的 CQ-03、CQ-05、INC-01、INC-03 四个关键缺口已由 239 B-min+ 收敛并完成默认/all-features workspace 回归；239 未扩展为全面代码质量治理。
```

随后按顺序推进：

```text
239 已完成并归档，施工文档/当前/ 为空。
下一步按 240 正式优先级先讨论 CQ-04；用户确认前不直接施工。
```
