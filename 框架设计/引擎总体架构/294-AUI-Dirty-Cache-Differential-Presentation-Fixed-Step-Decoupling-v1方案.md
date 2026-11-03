# 294 AUI Dirty Cache / Differential Presentation / Fixed-Step Decoupling v1 方案

## 1. 文档状态

```text
系统编号：294
方案版本：v1
建立日期：2026-08-17
问题来源：Tower 普通 Editor Play 在简单战斗规模下仍只有约 21.85 FPS，怪物移动和按钮响应明显卡顿
选定方案：AUI dirty/cache + 普通帧轻量诊断 + revision identity + Tower 差量 mutation + fixed simulation / presentation 分离
用户确认：已确认
当前状态：Gate A-C、定向回归与 post-292 ABI normal-loader 真实 Tower smoke 已完成；294 已归档
```

本文档只固化已确认方案，不构成代码修改、测试、构建、production Editor 更新、Tower Preview
缓存重建、真实配置修改或 Local CI 授权。

## 2. 一句话目的

让固定步长只推进必须确定执行的战斗、物理和动画模拟；让 AUI、项目表现 mutation、诊断数据和
render presentation 按真实 dirty 状态执行，并保证一次普通主循环最多只构建和提交一次可见帧。

## 3. 已确认基线

2026-08-17 的真实 Tower 普通 Editor Play 证据：

```text
运行时间：约 66.395 s
frameCount：1451
平均帧率：约 21.85 FPS
平均单帧：约 45.8 ms
目标 tick 间隔：16 ms
```

对应报告：

```text
samples/tower_defense_project/.aife/editor-preview/windows-dev/scene-main/
  runtime_package/reports/editor-play-preview-package-report.json

samples/tower_defense_project/.aife/editor-preview/windows-dev/scene-main/
  reports/editor-gameview-present-report.json
```

Tower 当前可见规模并不大：

```text
runtime renderable：41
Animator2D observation：40
每 fixed update 最后一帧 mutation：49
```

但 AUI 热路径规模为：

```text
AUI document：约 2.2 MB
node：1296
binding ref：767
UI draw item：449
font glyph metadata：608
```

因此首因不是战斗规则复杂，而是每个普通 tick 都重复承担了全量 AUI resolve/layout/glyph、完整
presentation identity、观察详单、项目表现投影和 render presentation。

## 4. 日志结论

本问题必须区分两件事：

```text
磁盘日志写入：不是普通帧首因。
报告/诊断数据构造：是普通帧成本的一部分。
```

现有 `editor-gameview-present-report.json` 只在 Start、Stop、Pause、Resume、Step 等生命周期边界落盘；
普通连续 tick 没有每帧 JSON 写盘，也没有常驻 `println! / eprintln! / tracing` 输出。

但普通帧仍会构造或克隆：

```text
完整 GameViewRuntimeFrame
40 条 Animator2D play observation
project runtime session report
project observation state
AUI action targets
AUI composition / glyph plan identity 输入
```

294 不把“禁止写盘”重复施工为主修复，而是关闭普通帧不需要的详单生成，让 Summary 真正轻量，
Trace 只在显式诊断时开启。

## 5. 既有方案继承

294 不推翻现有系统：

```text
199：ProjectUiStateSnapshotProducer 必须 active-binding-driven、dirty、cached。
260：项目 action、fixed update、deferred mutation 和 observation 的生命周期边界。
265：AUI effective visibility、稳定 GameView surface 和 frame publication。
275：GPU texture residency 与 fixed tick 有界 catch-up 基线。
277：按钮反馈是 present-only，不改变 layout / hit rect。
283：普通连续 tick 不再每帧写 GameView report。
284：AUI 输入和业务 action 在普通帧即时处理，不等待 fixed tick。
286-R1：GameView last-good texture/presentation 规则。
```

294 只补齐这些方案之间仍缺失的运行时性能合同：

```text
dirty snapshot 不应继续触发 clean frame 的全量 present 重建。
Summary 不应生成 Trace 级数据后再丢弃。
presentation identity 不应通过每帧序列化完整 composition 得到。
项目表现 mutation 不应每帧重写未变化组件。
fixed-step catch-up 不应重复执行 AUI/render presentation。
```

## 6. 正式运行顺序

```text
Native ordinary frame
  1. Pump platform events
  2. 284 immediate AUI input / feedback / action dispatch
  3. Resolve fixed steps due
  4. Run 0..N fixed simulation steps
     - project fixed update
     - battle rules
     - physics
     - Animator2D fixed progression
     - authoritative World commit
  5. Aggregate dirty facts and project presentation delta
  6. Build or reuse AUI presentation once
  7. RenderExtract / render once when required
  8. Present once when required
  9. Publish compact Summary counters
```

关键红线：

```text
一次 ordinary frame 可以补多个 fixed simulation step。
一次 ordinary frame 最多只能构建一次 AUI presentation。
一次 ordinary frame 最多只能提交一次 runtime viewport presentation。
```

## 7. 子方案 A：AUI dirty/cache

### 7.1 v1 缓存边界

由 AUI runtime/present owner 持有 session-scoped cache：

```text
resolved document
layout result
draw list / composition
glyph plan
action targets
last visible presentation
```

cache key 只由已有事实组成：

```text
AUI document revision
ProjectUiStateSnapshot visible revision / dirty domains
presentation target revision
font / texture resource generation
AUI feedback visual revision
```

v1 不新增通用响应式 UI 框架，不给每个 AuiNode 增加复杂依赖图。第一阶段采用保守失效：只要影响
可见 AUI 的 revision 未变化，完整复用；发生相关变化时允许重建一次完整 AUI present。

### 7.2 clean frame 合同

当以下事实都未变化时：

```text
document
binding-visible values
target extent / scale policy
font/texture generation
feedback visual state
```

普通帧必须：

```text
不 clone 完整 AuiDocument
不重新扫描全部 767 bindings
不重新 layout 1296 nodes
不重新生成 glyph plan
不重建 action targets
直接复用 last valid present
```

### 7.3 dirty 分类

最小分类：

```text
DataDirty          binding value 变化
VisibilityDirty    canvas/node effective visibility 变化
LayoutDirty        rect、clip、target、scale policy 变化
TextDirty          可见文本或字体选择变化
ResourceDirty      font/texture generation 变化
FeedbackDirty      hover/pressed/activated present-only 变化
```

v1 可以保守地把 `DataDirty` 提升为一次完整 AUI rebuild，但不得把 World/Animator/Transform 普通变化
误标成 AUI dirty。后续细粒度 node patch 不属于 v1 前置条件。

### 7.4 FPS HUD

Tower FPS 文本保持项目侧 250 ms 采样周期；它只在文本采样值变化时标记 `TextDirty`。FPS HUD 不得
使 60 Hz 普通帧全部变成 AUI cache miss。

## 8. 子方案 B：普通帧关闭 Animator/report 详单

复用已有 `Off / Summary / Trace` 概念，不增加新的报告系统。

```text
Off
  不生成可选观察详单和长字符串。

Summary（普通 Editor Play 默认）
  只保留 frame/revision、计数、状态、cache hit/miss 和 compact diagnostic code。

Trace
  仅在 Inspector、Report Panel、测试或用户显式诊断时生成 Animator observation、完整 paths、
  action trace 和详细 stage report。
```

必须让现有 `ProjectUiStateReportMode` 真正控制数据生产，而不是先构造 Trace 数据再在输出阶段省略。

普通帧 Summary 禁止：

```text
遍历所有 Animator2D entity 生成 observation vector
克隆完整 project runtime session report
克隆完整 project observation state
构造完整 action/path trace
```

生命周期边界、失败现场和显式 Trace 仍可生成完整报告；报告失败不得影响游戏 authoritative state。

## 9. 子方案 C：presentation identity 改为 revision

现有每帧把 composition stages、glyph plan、canvas references 序列化为 JSON，再计算 SHA-256 的路径退役为
普通帧 identity 来源。

正式 identity 由 owner 维护的 revision 组成：

```text
AuiPresentationIdentity
  session_generation
  document_revision
  visible_state_revision
  layout_revision
  resource_generation
  feedback_revision
```

规则：

```text
只有可见输出变化时，visible presentation revision 才增加。
fixed simulation step 增加不自动增加 AUI revision。
Animator observation、report timestamp 和 frameCount 不参与可见 identity。
GameView target/surface identity 继续与 AUI visible identity 分开。
```

SHA-256 仍可在 Trace、持久化证据或跨进程 artifact seal 中使用，但只在 revision 变化或显式请求时计算，
不进入 clean frame 普通热路径。

该 identity 必须继续满足：

```text
284 immediate AUI-only publication 能识别可见反馈变化。
265/286-R1 last-good publication 不会把相同 revision 误当成新帧。
输入 hit-test 使用的 layout revision 与显示 presentation revision 可核对。
```

## 10. 子方案 D：Tower mutation 差量化

Tower 是第一个 consumer，不改变引擎公共接口中的项目无关性。

### 10.1 `tower.matchView`

`tower.matchView` 只在 AUI 实际使用的投影字段变化时 replace。项目 session 保留上一份 UI projection
fingerprint 或 revision；怪物连续位置变化不得强制重建整份 UI read model。

### 10.2 怪物表现槽

每个表现槽保留上一帧：

```text
assigned enemy identity
visible
sprite/animation frame identity
transform
```

仅在字段实际变化时发 mutation：

```text
Transform：位置变化时写入。
SpriteRenderer2D.visible：spawn/despawn 或槽位复用时写入。
sprite_ref：Animator2D frame 或资源身份变化时写入。
未变化槽位：零 mutation。
```

禁止继续每个 fixed tick 对全部 40 个槽位无条件 replace `SpriteRenderer2D`。

### 10.3 所有权

```text
Tower RuntimeModule：决定 tower.matchView 和怪物槽位的项目侧 dirty/delta。
engine runtime：验证并提交通用 component mutation。
AUI runtime：只消费 ProjectUiStateSnapshot，不读取 tower.matchView。
```

## 11. 子方案 E：fixed simulation catch-up 与 presentation 分离

### 11.1 当前问题

现有 `about_to_wait` 最多计算 8 个 overdue tick，并对每个 tick 调用完整 GameView descriptor frame。
当单帧约 45.8 ms 时，最坏可能连续占用主事件线程约 366 ms，期间输入和 redraw 都被延迟。

### 11.2 新合同

catch-up 只重复 simulation，不重复 presentation：

```text
fixed_steps_due = bounded_accumulator(now)

for each fixed step:
  advance deterministic simulation
  apply authoritative mutations
  merge dirty facts

after all due steps:
  build/reuse AUI once
  extract/render once
  present once
```

### 11.3 模式语义

```text
Editor Play：0..N simulation step + 最多一次 presentation。
Editor Pause：0 simulation step；AUI feedback/input dirty 时仍可 presentation。
Editor Step：恰好 1 simulation step + 最多一次 presentation。
Exported Player：复用相同阶段合同，不要求与 Editor 共用窗口实现。
Headless：只运行 simulation，不创建 presentation。
```

### 11.4 spiral-of-death 边界

继续保留有界 fixed-step debt，防止无限追帧；但达到上限时必须输出 compact counter，不能在一次事件回调中
无界运行。v1 不通过丢弃业务 action 或重复 action 来追帧，也不引入怪物插值作为首修复。

## 12. 内部接口建议

优先深化现有 owner，不新增公共大 schema：

```text
AuiPresentCache             editor/runtime session-owned internal cache
AuiDirtySet                 internal bitset / enum set
AuiPresentationRevision    compact internal revision tuple
GameViewDiagnosticsLevel   复用 Off / Summary / Trace 语义
Project mutation delta     复用现有 MutationBuffer，只改变 producer 写入策略
```

如果施工发现现有 public report schema 必须扩展，只允许增加紧凑、可选、向后兼容字段：

```text
auiCacheStatus
auiRebuildReasons[]
auiPresentationRevision
fixedStepCount
presentationCount
stagedMutationCount
```

不得把缓存内部节点图、完整 glyph quad 或 Tower 项目字段公开成通用 schema。

## 13. 实施顺序

按已确认顺序分五个可独立验证的纵切：

```text
1. AUI dirty/cache
   clean frame 直接复用 last valid present。

2. 普通帧诊断轻量化
   Animator/report 详单只在 Trace/Inspector 生成。

3. revision identity
   普通帧移除 composition JSON + SHA-256 identity。

4. Tower mutation delta
   matchView、SpriteRenderer2D、Transform 只提交真实变化。

5. fixed simulation / presentation 分离
   catch-up 可补多个 simulation step，但一轮最多 presentation 一次。
```

每一步都必须先建立窄性能计数再改 owner；不得用完整视觉矩阵或 production replacement 作为日常调试循环。

## 14. 验收合同

### 14.1 正确性

```text
clean AUI frame 不执行 document clone / resolve / layout / glyph rebuild。
UI 值、可见性、target、资源或 feedback 变化后下一次普通 presentation 正确失效。
284 AUI action 仍在普通帧即时执行且 exactly once。
fixed catch-up 不重复 action，不跳过 authoritative simulation step。
Editor Pause/Step 语义不回退。
last-good texture/presentation 不回退。
```

### 14.2 性能与观测

以同一 Tower 720x1280 Contain、普通 Editor Play、至少 30 秒为定向基线：

```text
平均 FPS：>= 55
普通连续帧磁盘 report write：0
无 UI 变化区间 AUI cache hit：>= 90%
Summary 下 Animator observation vector：0
一轮 ordinary frame 的 presentationCount：<= 1
catch-up fixedStepCount > 1 时 presentationCount 仍为 1
未变化怪物槽位 SpriteRenderer2D mutation：0
按钮普通输入不等待 fixed tick，且没有 replay
```

如果环境噪声使绝对 FPS 不稳定，仍必须同时证明相同场景下：

```text
平均 frame time 相对基线显著下降
AUI rebuild count、Trace detail count、mutation count 和 presentation count 符合结构合同
```

## 15. 非目标

294 v1 不做：

```text
重写整个渲染器或引入独立输入 GPU Present
新建通用 reactive UI / virtual DOM / signal framework
逐节点增量 layout engine
字体 shaping、atlas recook 或 GPU text pipeline 重做
Render Thread / Runtime Thread 全面并行化
怪物移动插值
修改 Tower 战斗规则、10 轮内容、兵种或敌人设计
自动更新 production/installed Editor
自动重建真实 Tower Preview cache
自动运行 Local CI 或完整视觉矩阵
```

## 16. 风险与失败关闭

```text
缓存失效遗漏
  -> revision/dirty matrix 定向测试；identity 不一致时重建，不复用可疑缓存。

delta mutation 漏写
  -> 同输入下对比 full projection 与 delta projection 的最终 World snapshot。

fixed/present 分离改变时序
  -> Play/Pause/Step、AUI action-before-fixed、no-replay 和 catch-up sequence 测试。

Summary 关闭详单后失去诊断
  -> 显式 Trace 可重开；失败边界仍保留 compact diagnostic code 和生命周期报告。

性能数字被旧 composition 或旧 production Editor 污染
  -> 施工验证必须记录 source/candidate identity；production 更新另行授权。
```

任何 cache identity 不完整、revision 回退或 delta equivalence 失败都必须 fail closed 到“本轮重建”，不得复用
可能过期的可见帧。

## 17. 方案自审

是否过量施工：

```text
否。方案复用现有 AUI presenter、report mode、MutationBuffer、fixed tick accumulator 和 284 输入阶段；
不新增通用工作流、响应式框架或项目专用引擎 API。
```

是否混淆引擎和 Tower 所有权：

```text
否。dirty AUI、revision identity、诊断分档和 fixed/present 调度属于通用引擎；
Tower 只负责自己 matchView 和怪物表现槽的差量 producer。
```

是否只靠关闭日志：

```text
否。磁盘写盘已经不是普通帧首因；本方案直接移除 clean frame 全量工作和 Trace 级数据构造。
```

是否保持 284 即时输入：

```text
是。输入/AUI action 先于 fixed catch-up；presentation 分离只减少重复工作，不把按钮重新推迟到 fixed tick。
```

是否需要立即施工：

```text
否。下一步必须单独生成并自审 294 施工文档，再由用户明确激活和授权施工窗口。
```

## 18. 结论

正式采用：

```text
AUI Dirty Cache / Differential Presentation / Fixed-Step Decoupling v1

= clean frame AUI cache hit
+ Off/Summary/Trace 真正控制数据生产
+ revision-based presentation identity
+ Tower project-owned differential mutations
+ N fixed simulation steps : 1 presentation
```

该方案先消除确定存在的 CPU/分配/主线程重复工作，再判断是否仍需要怪物插值。不得在 294 完成前用插值
掩盖低帧率，也不得把 production 更新、完整矩阵或 Local CI 混入源码调试循环。
