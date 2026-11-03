# 365：通用二维绘制与 AUI 表现接口——方案 B v1

日期：2026-10-08。状态：用户已授权再次自审、生成施工文档、自审后直接施工。正式方案复审完成；施工进度与唯一执行槽见49/54，设计确认不代表实现资格。

顶层 authority：[产品需求 v1.1](00-AI-First-Game-Engine-权威产品需求-v1.1.md)与[架构设计 v1.1](00-AI-First-Game-Engine-权威架构设计-v1.1.md)。实现和安装基线只见[49](49-当前状态与下一步入口.md)，唯一执行槽只见[54](54-当前可自动化施工总入口.md)。本方案深化既有 SDK、AUI、Asset 与 UiProjection，不另建 UI 框架或渲染器。

2026-10-10新增消费端设计衔接：[372二维局部GPU几何与增量呈现方案B](372-二维局部GPU几何与增量呈现-方案B-v1方案.md)已按用户确认生成。它沿原owner补稳定Graphics2D的GPU变换／矩形裁剪快通路和必要在途生命周期，继承本文精度、颜色、命中与复杂裁剪语义；本文14.3的CPU实现及既有完成证据保留历史身份。372当前仅design_documented，不表示本文旧交付或安装已经采用新GPU路径。

## 1. 决策与目标

用户要求先确认成熟引擎是否有对应能力，以及这些能力应由引擎还是项目提供；确认属于通用引擎能力后，采用方案 B 生成正式文档。本次核对支持这个条件。

方案 B 是“通用绘制创作接口 + 完整表现属性 + 可组合补间”，由引擎与其配套 SDK 提供。项目用普通 Rust 代码、AUI 文档与版本化资源使用它们，无需自行维护三角剖分、UI 变换链、裁剪与命中同步、动画生命周期和 GPU 缓存。

目标是让同样的视觉需求可以通过稳定公开接口完成，缩短 AI 制作、修改和修复游戏表现的成本；保持 361/363 的常驻节点和局部更新优势。它提供画图和驱动表现的通用工具，游戏美术内容与玩法编排仍由项目决定。

不将本方案解读为“当前渲染器无法达到 Godot 的画面”，也不以新增接口自动保证画面或性能超过其它引擎。当前证据支持接口完整性与易用性缺口；最终效果必须以真实项目和相同视觉负载验收。

本方案完整包含路径/曲线绘制、表现属性、组合补间、资源消费、兼容合同、真实窗口/GPU与普通项目交付。不能仅完成几个项目 helper 就宣称方案 B 完成。

## 2. 成熟引擎核对：能力相同，组织方式不同

| 能力 | Godot | Unity | Unreal Engine | 归属结论 |
| --- | --- | --- | --- | --- |
| 通用二维图形 | CanvasItem 绘制线、圆、弧、多边形；RenderingServer 保留命令 | UI Toolkit Mesh API / Painter2D 路径、描边和填充 | UMG 绘制接口进入 Slate DrawElement / ElementBatcher | 公开创作接口及绘制消费由引擎/SDK提供 |
| 表现变换与颜色 | CanvasItem/Control 变换、调制等 | VisualElement 的 translate/rotate/scale、opacity/color | UWidget 的 RenderTransform/RenderScale/Angle/Opacity | 属性与绘制、坐标、命中、裁剪必须统一 |
| 通用动画 | Tween 的属性、串并行、时钟与节点绑定 | 样式过渡及元素动画；复杂补间也可用插件/框架 | Slate CurveSequence 与 UMG SequencePlayer/MovieScene | 通用调度由引擎/配套框架提供；无需照搬一个 Tween 类 |
| 游戏表现内容 | 游戏脚本、场景和资源 | 项目代码、资源、动画内容 | 项目 C++/Blueprint、资源和序列 | 由项目提供 |

源码核对使用本地 Godot 4.8-dev、Unity C# reference 6000.6.0a7、UE 5.8 与 Bevy main。它们不是用户 Godot 4.7.1 成品的精确构建源码；用于确认通用能力与分层，不能充当同版本性能比较。

### 2.1 可借鉴的调用链

- Godot：`CanvasItem::queue_redraw` 合并重复重绘请求；`_redraw_callback` 更新保留绘制命令。`draw_line/draw_arc/draw_circle` 调用 RenderingServer，绘制 API 与消费端分开。Tween 的 `bind_node/kill/step/set_parallel` 处理生命周期和组合。
- Unity：`Painter2D` 记录路径，`Stroke/Fill` 输出 mesh；`MeshGenerationContext` 的绘制数据有明确调用期。`UIRRepaintUpdater` 区分 Transform/Size/Clipping、Opacity、Color、Repaint；样式动画按元素/属性管理，并非项目每帧重新创建整棵 UI。
- UE：`UWidgetBlueprintLibrary::DrawLine/DrawSpline` 进入 `FSlateDrawElement`，再由 `ElementBatcher` 消费。`UWidget::SetRenderTransform/SetRenderScale/SetRenderTransformAngle` 下传缓存 SWidget。CurveSequence 与 UMGSequencePlayer 处理不同粒度动画。
- Bevy：`UiTransform` 提供平移、旋转和缩放；`UiGlobalTransform::try_inverse` 对不可逆变换返回失败。渲染提取与布局、游戏状态分开；Fixed/Virtual/Real 时钟可区分模拟和呈现。
- PixiJS：GraphicsContext 保留并可共享图形描述，几何与实例表现分开。Rust lyon 可承担路径填充/描边三角剖分，是算法候选而非完整 UI 或抗锯齿方案。

### 2.2 不照搬的部分

不强制 Unity 的编辑器工作流或 Canvas 层级；不复制 UE 的完整 MovieScene；不引入 Godot 脚本语言；不以 Bevy ECS 作为项目 UI 创作 API。UE 的 RenderTransform 变更实际也会触发 Layout 相关失效，不能据此声称所有成熟引擎变换都零布局开销。

Unity 不内置 DOTween；Bevy 核心也不等于 Godot 的 Tween。我们采纳的是通用动画能力及生命周期管理，不宣称其它引擎有与本方案一模一样的 API。

## 3. 责任边界

| 内容 | owner | 理由 |
| --- | --- | --- |
| 路径构建、绘制样式、缓动函数、typed 创作对象 | 引擎分发的 SDK/框架库 | 可复用纯函数不必全部塞进 Native Runtime |
| 三角剖分、几何缓存、裁剪、抗锯齿、合批/上传 | Runtime / UiProjection / Renderer | 依赖真实绘制及资源生命周期，项目不接触内部 GPU 对象 |
| UI 仿射变换、逆变换、命中、父子传播 | AUI Runtime | 仅项目旋转顶点会造成视觉与交互分离 |
| 补间实例、时钟接入、取消/释放、属性冲突和安全回调 | 既有 AUI Runtime 调度与 SDK | 多项目共享一致语义；不增加平行的反馈/动画系统 |
| 图形与动画描述资源、校验、打包与版本 | 既有 Asset / Compiler / RuntimePackage | authoring 真相与运行输入延续既有链路 |
| 地图、布阵点、兵种外观、弓箭飞行规则、伤害和组合 | 游戏项目 | 带有具体玩法语义 |
| 骑兵冲刺、受击闪色、合字弹出、飘字的时间和内容 | 游戏项目调用通用能力 | 引擎提供参数、曲线和播放工具，项目选择效果 |
| 贴图、配色、音效、美术风格与节奏 | 项目资源/项目制作 | 工具齐全不等于内容自动完善 |

项目今天也能用 mesh 与 lerp 做部分效果，这说明存在低层通路，不说明必须由每个游戏重写基础库。纯补间数学不是 Native Runtime 的必需项；本方案选择统一提供的是完整、可复用且与 UI 生命周期一致的合同。

## 4. 现有基线：复用与真实缺口

| 现有位置 | 已有能力 | 本方案增量 |
| --- | --- | --- |
| `project_runtime_sdk/src/aui.rs` | generation handle、typed setter、Binding/Controller owner；公开属性包含 Translation | 补齐旋转、缩放、pivot、opacity/tint 等 typed 表现能力 |
| `project_runtime_sdk/src/aui_extension.rs` | describe/measure/layout/paint；Rect/Image/Text 及自定义顶点/索引 | 在已有扩展链提供路径/图元创作和保留图形消费，避免项目重复组 mesh |
| `engine_runtime/src/aui_instance.rs` | 常驻实例、同值消除、原子更新、局部失效；坐标转换累积平移 | 引入共用仿射变换及可逆坐标查询，贯通画面/命中/裁剪 |
| `engine_runtime/src/aui_theme.rs` | Solid/Image/NineSlice brush、边框、圆角、阴影、透明度 | 公开可安全更新的表现属性/局部样式覆盖，复用已有样式消费 |
| `engine_runtime/src/aui_control_feedback.rs` | scale/translation/brightness/opacity 反馈覆盖、缓动与呈现 delta | 复用数学和时钟入口，定义通用动画与内部反馈的组合 |
| `engine_runtime/src/aui.rs` | DrawRect/Image/Text/Mesh、Presenter、更新分段计时 | 消费稳定图形与最终表现属性，不重写全部命令体系 |
| geometry/hit/drawing/feedback cache、`aui_session.rs`、`wgpu_backend.rs` | 361/363 局部更新与缓存、UiProjection、GPU 复用 | 增加真实依赖，保持清洁帧及局部上传 |
| Project Game SDK | Animator2D、AudioSource、ParticleEffect Writer | 维持既有接口，不以本方案替换骨骼/帧动画、粒子或音频 |

现有扩展合同限制包括每输出 4096 顶点、12288 索引、4096 primitives，以及既有 batch/wire 上限。新路径接口必须在预算内输出，或在施工合同中明确新的有界传输与缓存预算；不能悄悄取消限制。

当前 `UiProperty` 没有通用 Rotation/Scale 等属性，`local_to_screen` 使用矩形位置加祖先平移，`screen_to_local` 只做减法。反馈内部能缩放不等于项目已有统一仿射变换 API。这是需要引擎接入的具体缺口。

## 5. 接口组织与消费链

```text
项目 Rust / 版本化图形和动画资源
  -> Project SDK typed 创作与请求
  -> Generated Runtime Glue（有界数据，无 trait/closure 跨 DLL）
  -> 常驻 AUI 实例 / 扩展绘制 / 既有呈现调度
  -> 局部布局、最终变换、裁剪与命中
  -> 保留绘制片段 / UiProjection
  -> 既有 Renderer UI Pass / RHI / Present
```

这是现有 owner 内的深化，不新增 Provider、Compiler、Host、运行时玩法解释器或第二份 UI authority。Renderer 只收解析后的 Projection 数据，不读取项目代码或 Binding。

图形描述采用局部坐标，可以被未来 World RenderProjection adapter 复用。本期完整消费端为 AUI/自定义绘制；不承诺已经完成 world vector renderer，也不把实体运动包装成 UI。现有 world Sprite/Animator/Particle 通路保持其各自 owner。

## 6. 通用二维绘制合同

### 6.1 创作能力

提供 typed `Draw2D` / `PathBuilder` 概念接口，最终命名在施工合同冻结，包含：

- 线段、折线、矩形、圆角矩形、圆/椭圆、圆弧、多边形。
- 路径 Begin/Move/Line/Quadratic/Cubic/Arc/Close，开放路径描边与闭合区域填充。
- stroke 宽度、颜色、butt/round/square cap、bevel/round/miter join 与有限 miter limit。
- fill 的 NonZero/EvenOdd 规则；明确自交、洞、反向轮廓与退化路径语义。
- Image/NineSlice 与 Text 复用既有资源和字体接口；图形路径不重新实现文本整形。
- 既有自定义 mesh 通路保留为高级扩展，普通形状不要求项目手写顶点。

v1 不承诺完整 SVG/CSS 导入、任意 shader graph、路径布尔运算、复杂滤镜或全新 2D 渲染器。上述通用图元、路径与曲线属于本期完整范围，不能将所有曲线都留给项目。

### 6.2 描述、缓存与资源

SDK 构建有界、typed 命令描述；稳定描述发布后不可变，运行实例持有 handle + generation。替换内容是一次显式更新，变换/颜色动画不逐帧重传整个 mesh 或图形 JSON。

缓存 key 至少包括实际几何内容、stroke/fill 几何参数、剖分精度、影响精度的有效 scale/DPI bucket 与算法版本。实例 tint/opacity/translation 等可分离数据不无故进入几何 key。缓存有 owner、容量、驱逐和资源释放策略；复用现有绘制缓存，不另建无界全局图库。

可选图形资源经 GUID/meta/AssetRef、schema、Asset Graph、Compiler 与 RuntimePackage 发布；普通代码也能构建图形，不强制所有效果先写新 DSL。同一资源多实例可共享几何，各自拥有表现状态。实例释放与共享资源释放分开。

### 6.3 几何质量与失败

lyon 为优先评估候选：其 FillTessellator/StrokeTessellator 与本项目 Rust 底座相合。施工需用洞、自交、尖角、小尺寸和复杂路径验证质量、许可、成本后冻结依赖；其三角剖分本身不保证抗锯齿。

抗锯齿需与既有 UI Pass 一起选定并验证 coverage/fringe 或适用采样方案。明确直线/曲线在 DPI、缩放和半像素位置的边缘覆盖、预乘 alpha 以及线性颜色计算。精度误差以屏幕像素衡量，不能固定低分段数使大尺寸曲线出现折线。

有限数值、非负宽度/半径、容量和合法索引在进入 Runtime 前检查；零长度线、重复点、极尖 join、空路径有确定结果。失败提供首因及对象身份，不 panic、不截断成表面成功，不提交半成品或污染原缓存。

## 7. 完整表现属性与统一几何

### 7.1 属性和坐标

新增稳定 typed 属性覆盖 Translation、Rotation、Scale、Pivot、Opacity、Tint，以及图像/图形资源替换与受限局部样式覆盖。复用 Translation 与既有 owner，不开平行 setter。Rotation 使用明确角度单位，v1 创作约定为弧度；Pivot 为布局矩形内归一化位置，允许有限的越界值以实现外部旋转中心。

设节点布局原点为 `L`、局部 pivot 点为 `p`、表现位移为 `t`，基础矩阵约定：

`M_node = T(L) · T(t) · T(p) · R(rotation) · S(scale) · T(-p)`。

所有节点矩阵与祖先矩阵顺序统一；已有屏幕布局矩形必须转换为一致父级坐标，不能同时重复加祖先布局偏移。布局 rect 与最终呈现 rect 分离：旋转/缩放不改变兄弟节点的排版占位。布局、父变换和 pivot 所依赖尺寸变化时更新实际后代。

变换提供正向/逆向坐标查询。负缩放允许镜像；零或数值上不可逆的变换返回明确坐标错误，节点及其不可逆后代不参与命中，不能产生 NaN。绘制按同一矩阵处理退化几何。逆矩阵阈值与策略由施工合同冻结并测试。

### 7.2 命中、裁剪与颜色

画面、命中窄阶段、指针 local 坐标、拖拽换算使用同一最终变换。空间索引可用屏幕 AABB 作宽阶段，但旋转节点命中不能只用 AABB。普通节点仍按自身局部命中区域判断，自定义形状命中由既有扩展合同明确，不自动将每个路径当可点区域。

旋转裁剪必须按变换后的真实裁剪形状及祖先交集处理，不能只扩大为 AABB 后漏画。scissor 只用于可精确表达的轴对齐情况；必要的精确 clip geometry/mask 接入同一 UI Pass 并有有限嵌套与成本预算。

v1 节点 opacity/tint 在父子链逐项相乘，作用于绘制 primitive；这不等于把重叠子树先合成再整体透明。不默认每节点创建离屏纹理；真正的 group opacity 是独立成本的后续能力。opacity=0 本身不关闭交互；项目须显式设置 Interactable/Visibility/Active。

颜色输入空间、范围、alpha clamp 与资源 sRGB 解码沿用既有 Renderer 合同并明确暴露。局部样式覆盖只失效真实消费者；Image/Graphics 替换遵循资源 identity，不用路径字符串绕开 AssetRef。

### 7.3 局部更新原则

同值 setter 不产生变化工作；纯 transform/tint/opacity 更新不重新做文本整形或路径剖分（有效精度变化除外）。父变换影响必要后代矩阵/命中/裁剪，不全量重建静态 UI。节点不会因拥有变换就自动变成 Canvas 或 offscreen layer。

沿用 363 的节点、布局依赖、绘制缓存域和 batch 分工；后端确实需要拆 batch 的变化应记录原因。复杂 clip 等成本必须如实测量，不能把新增开销藏进其它计时段。

## 8. 可组合补间与调度

### 8.1 创作与实例

动画描述不可变，可反复播放；每次播放生成独立 `AnimationHandle(instance, generation)`。typed 目标绑定节点及属性，支持 duration、delay、easing、串行、并行、有限 loop、reverse/yoyo，以及 pause/resume/cancel/completion。

无限 loop 必须绑定存活 owner/scope；零时长步骤可立即求终值，但禁止无耗时无限循环造成忙循环。完成、取消、target disposed 和错误有不同结果，事件只通知一次。动画数量、步骤/目标数量与描述大小有界。

v1 动画目标为 AUI 可插值表现属性。Visibility/Active/Asset 切换等离散状态可由项目收到完成事件后处理；不伪装成数值插值。路径运动可在项目曲线上求位置后用 setter，通用路径采样可以在 SDK 提供，不将骑兵路线或弹道规则进入引擎。

### 8.2 时钟与帧阶段

明确 UI 呈现时钟和项目可控游戏表现时钟。项目显式设置游戏时钟的暂停/速度；UI 时钟可继续播放暂停菜单。引擎不通过“战斗阶段”等项目字段推断暂停。

既有 Runtime frame time 是时间输入 owner。一个呈现帧内多个 Fixed Tick 不能重复累计同一 presentation delta；UI 更新/补间推进后发布最终几何，再按既有输入时序保证命中和可见状态一致。精确阶段及输入使用哪次已发布状态由施工合同冻结，禁止绘制用新矩阵而命中用未经声明的旧矩阵。

时间截断、长帧、暂停恢复和回放策略明确配置并与现有反馈 delta 约束协调。完成事件经现有安全事件队列交给项目，不在 paint 期间重入项目逻辑；跨 DLL 仅传 callback ID/结构化事件，不传 Rust closure/trait。

### 8.3 属性 ownership 与组合

Binding/Controller 继续拥有基础值。动画是临时表现覆盖，不静默夺取基础 owner；同一属性同一层并发默认拒绝，显式 Replace 才能取消已有动画。多属性并行动画允许；组合描述内部同目标冲突也必须校验。

默认完成/取消/释放动画覆盖后，回到“当时最新基础值”，不是播放开始时的旧快照。保持终值使用显式 Hold；写回终值使用显式 CommitFinal，且仅在预期基础 owner/revision 仍一致时成功，否则报告冲突。无限保持也必须绑定生命周期。

组合次序固定为基础样式与节点属性 → 项目动画覆盖 → 控件内部交互反馈。反馈变换以其局部中心应用，颜色/opacity 使用明确乘法；共享最终矩阵与颜色消费端，不在绘制侧偷偷再乘一遍。拖拽起始先按显式策略取消冲突的位移动画，指针跟随直接写入，不增加追鼠标补间延迟。

迁移内部反馈应保持 361 点击/hover/pressed/capture/cancel 的资格；若统一矩阵改变实际命中行为，必须更新可审查合同并验证，不以“复用”忽略兼容影响。

### 8.4 生命周期与项目运动

节点 dispose、实例关闭、generation 变化取消所属动画，旧 handle 不得修改复用 slot。Active=false 暂停 active-scope 动画；恢复不补跑停用期间时间。Visible/Hidden/Collapsed 遵循 361 生命周期语义：Hidden 不自动等于 disposed，时钟是否运行由 scope 显式规定。

场景重开、项目卸载、设备恢复不会保留悬空回调或持有已释放 GPU 数据。常规单体运动需要平滑时，项目提供上一次/本次模拟姿态和插值策略；SDK 可提供插值数学，项目仍拥有路线、状态和模拟。固定步插值可能带来一个模拟步延迟，不能把它强制应用于直接指针拖拽。

## 9. 项目使用和 AI 友好性

AI 应通过命名 View/typed node refs、图形构建器、typed 属性和动画描述完成效果。普通项目不手写 wire、dirty/revision、三角剖分、GPU handle 或 Generated Glue。

以下是拟议调用意图，不是已发布或可编译 API：

```text
创建弧线图形：PathBuilder -> Stroke -> 发布 GraphicsHandle
修改表现：node.set_rotation / set_scale / set_tint
播放合字反馈：sequence(parallel(scale, opacity), settle).play(node, ui_clock)
拖拽字牌：screen_to_local(parent, pointer) -> set_translation
结束/重开：cancel(handle) / dispose(scope)
```

提供至少两个独立普通项目消费示例：图形/嵌套变换/输入验证 UI 示例，和塔防项目中的小范围视觉消费切片。示例只通过公开 SDK，塔防效果编排不进 engine crate。此处第二项目用于证明通用能力被真实游戏使用，不扩大为整个塔防美术重制。

SDK 文档应包含可编译最小示例、默认单位、属性 owner、生命周期、性能代价和失败处理。已有 inspect/check/playtest/observe 消费版本化合同与证据，不为每个绘制命令增加工具。

Diagnostics 区分 Off/Summary/Trace。Release 默认 Off 或功能必需 compact result；不能为了“AI 友好”常驻生成长 JSON、日志或文件。结构化错误至少定位对象、属性/图形、stage、原因与可执行修复建议。

## 10. 兼容、资源和 Glue 同步

当前 `UI_UPDATE_VERSION=6`，`UI_UPDATE_SCHEMA` 包含现有属性集合并参与 SDK 协议指纹；扩展绘制也有独立 schema。新增属性、图形/动画请求或查询改变 wire 时，必须同步 SDK 编解码、Runtime reader、协议指纹和 Generated Runtime Glue。

ABI 当前为 1.6。施工先冻结确切数据与函数表变化，再决定 ABI 版本和兼容矩阵；不能预先随意命名下一 ABI，也不能仅改 Rust trait 让旧 DLL 静默读取新布局。旧 reader 对不支持的新请求明确拒绝，旧项目的迁移路径须可构建、可交付、可回退。

涉及资产时同步 authoring schema、依赖收集、Compiler/Assembler、RuntimePackage validator 与 Runtime consumer。不得靠 Runtime 扫描源目录加载图形/动画描述。资源缺失、schema 错误和预算超限在 check/build 或加载阶段给出首因。

范围包括 `project_runtime_sdk`、`project_game_sdk` 按既有 re-export 边界、`project_runtime_abi`、`project_authoring_execution` 的 glue/host，以及实际 AUI/Asset/Renderer owner。接口 owner 与外部 consumers 同步验证，避免再次出现生成 Glue 的 ABI 不匹配。

## 11. 完整交付验收

| 验收面 | 必须证明的结果 |
| --- | --- |
| 基础绘制 | 图元、曲线、开放/闭合路径、洞/自交、caps/joins、退化与超限有确定输出/错误 |
| 真实 GPU | 不是只有命令计数：像素证明颜色、曲线、纹理、clip、alpha 和抗锯齿被真实消费 |
| 变换与交互 | 嵌套旋转/缩放/pivot、镜像、零缩放、DPI、parent local 转换、拖拽、裁剪内外命中一致 |
| 局部更新 | 静止帧不重复剖分/传 mesh；纯位移复用几何；改变路径只失效实际依赖；父变换更新必要后代 |
| 动画 | 串并行、循环/反向、长帧/暂停/时钟、冲突、cancel/complete/hold/commit、释放和重开可回放 |
| ownership | Binding 基础变化、Controller 转移、动画覆盖与内部反馈有确定顺序；失败不会半提交 |
| ABI/资源 | 新 SDK/Host/Glue/DLL一致；旧 reader 拒绝新协议；缺失资源和缓存失效能正确恢复 |
| 普通项目 | 两项目公开 API 实际消费；真实窗口可见、输入可操作；不依赖 engine private symbols |
| 独立交付 | RuntimePackage 原样副本脱离项目源/Library可运行；设备恢复后图形和动画生命周期正确 |

真实 GPU/窗口证据在 Windows 已支持后端上冻结资格；其它平台不能未经实测就宣称支持。CI 可使用确定性 headless owner 测试，真实设备验证在最终本地集成与交付阶段完成，不把窗口拉进每次开发反馈循环。

自定义绘制质量需有可审查的截图/像素阈值及参考图。制作体验记录两普通项目的 create/change/repair：需要修改多少业务代码、是否接触底层、失败首因是否可定位。单次制作耗时不直接换算为跨引擎速度比。

## 12. 性能与比较边界

363 实际塔防的已资格基线为 415 节点，同包双 28000 帧三关；保守 AUI P95 最大 1.5741 ms，CPU P95 最大 4.8544 ms。旧场景预算 AUI P95≤2 ms、CPU P95≤16.67 ms，在可比硬件/profile/分辨率/输入和采样条件下作为回归底线。

新路径和动画工作负载单独冻结节点数、路径复杂度、活动动画数、clip 深度、DPI、像素质量和环境，再规定绝对预算与增长限制；不能把旧 415 节点成绩套给任意复杂路径。该冻结在施工开始实质实现前完成，预算未冻结不得关闭性能验收。

记录 P50/P95/P99/max、完整 CPU frame、AUI 分段、GPU/Present 可得计时，以及剖分/几何重建数、上传 bytes、活动动画、失效节点和 cache 占用。计数在受控 Trace 采样启用；正式默认不开昂贵采集。

静止图形、只变换图形、改变路径、密集补间、精确嵌套裁剪分开测量。长期播放/反复重开验证资源收敛；短时 PrivateBytes 小幅增长不能证明无泄漏。不得用降低动画帧率、线段精度或关闭真实效果通过性能验收。

与 Godot/Unity/UE 比较采用相同画面与交互需求、相近负载及相同统计口径。取不到 UI 专项时比较每帧开销；测不到的阶段明确留空，不混用 CPU工作、VSync等待与显示FPS。当前性能预算通过不代表成熟引擎等价。

## 13. 实现顺序与后续施工约束

完整模块顺序为：

1. 公共数据/owner/时钟/版本与验收负载冻结。
2. Draw2D/PathBuilder 到既有扩展绘制、资产与真实 GPU 的完整链路。
3. 表现属性到仿射几何、裁剪、命中与局部缓存的完整链路。
4. 可组合补间到生命周期、ownership、反馈组合及 SDK/Glue 的完整链路。
5. 两普通项目、真实窗口/Release性能、独立交付与兼容验收。

这些是依赖顺序，实际任务和测试见对应总施工文档；避免把零散 setter 单独宣称一阶段交付。用户已授权生成施工文档、自审后按54激活并开始实现。

源码完成、指定平台独立交付、安装可用、重启后实际宿主接入分别记录。新增安装包不覆盖已有回退基线；仅设计确认不改变默认 Provider。

## 14. 正式文档自审

审查日期：2026-10-08。审查对象：365 方案 B，不是施工实现或测试通过声明。

| 审查项 | 结论与约束 |
| --- | --- |
| 用户授权 | 用户条件选择B；成熟引擎与当前源码核对支持通用能力归属，允许生成正式方案，不授权本轮施工 |
| 引擎/项目边界 | 图形、属性及生命周期通用；兵种、地图、弹道和效果编排留项目；SDK纯算法无需强制塞Runtime |
| 成熟引擎对标 | 有官方文档与具体源码链；区别Tween/transition/sequence/plugin，不声称相同API或性能 |
| 重复实现 | 延续主题/字体/反馈/Animator/Particle、361/363缓存与Projection；不建第二渲染器或每节点Canvas |
| 完整方案B | 图形/曲线、变换/命中/clip、组合动画、资源/Glue与交付均纳入，没有用项目mesh helper替代 |
| 易遗漏语义 | 已明确奇异变换、真实旋转clip、group alpha区别、owner冲突、恢复最新base及跨DLL事件 |
| 性能与证据 | 旧预算只作可比回归；新负载需先冻结；不以计数代替像素，不承诺自动超过Godot |
| 外部审查 | 当前查阅材料中未定位到针对365的外部审查；旧361/363约束继承，不复活历史施工队列 |
| 尚待施工冻结 | 最终API/序列化字段、clip/AA技术、容量、逆矩阵阈值、时间阶段、版本号和新负载预算须在首模块冻结 |

初版自审结论：方案边界成立，可以据此生成完整施工文档。上述待冻结项受结果合同约束，不代表相关能力可以省略；该初版时尚未生成施工文档，后续授权与状态见14.1和49/54。

### 14.1 施工前复审与合同补充（2026-10-08）

用户已明确授权本次施工，替代上方初版自审中的“本轮不授权施工”。再次对照当前SDK、扩展绘制、几何缓存和反馈时钟，发现以下必须补充项，已在此修正；没有改变方案B的边界或减少能力。

1. 路径坐标为局部逻辑像素，椭圆弧采用中心、两轴半径、起始角和有符号扫角（弧度）；屏幕Y向下时正角视觉顺时针。建造器有当前子路径，曲线必须有起点，Close后继续绘制须显式Move。空路径合法并绘制为空，退化线不制造非法几何。
2. 共用精度输入以最大有效scale与DPI映射屏幕误差，默认0.25像素。几何剖分预算超限明确拒绝，不能静默降低精度。初版单描述上限4096路径命令、256绘制项，输出mesh继续沿用现有4096/12288限制；扩大容量须用实际负载证明。
3. 仿射乘法定义为`parent * local`；共用数学使用较高精度中间计算后检查有限f32结果。逆变换采用归一化线性部分的相对行列式阈值`1e-8`，同时拒绝逆矩阵不可表示或非有限结果，避免固定绝对epsilon错误拒绝正常小尺度变换。
4. Opacity/Tint输入为有限0..1，越界报错，遵循既有颜色转换后在Renderer线性空间计算，不将srgb数值直接相乘。数学合同提供可插值数值；最终输出只在消费边界按颜色合同处理。
5. 同属性冲突指时间重叠的活动写入；串行步骤可以复用属性。并行冲突由区间分析校验，默认Reject，显式Replace在播放实例owner处处理。描述最多256叶步骤、32嵌套深度，时长以整数微秒表示并检测溢出；无限循环体必须有正时长。
6. 每个呈现frame index只推进一次UI/游戏表现时钟，默认最大delta沿用250000微秒；游戏clock的pause/speed由项目控制。倒序或重复frame不重新推进，恢复不追补暂停时间。输入处理读取上次已发布几何，项目更新与补间求值后发布本帧几何给绘制及下一次输入；同帧重新查询必须使用明确的已提交状态，不能混用半提交矩阵。
7. 公共创作描述和数学先以SDK纯数据/函数落地，不先改ABI函数表或宣告wire能力。真实接入阶段才更新相应wire/schema及SDK协议指纹；C函数表未改时不自动升级ABI 1.6。每阶段公开“已可创作”和“已被Runtime消费”的区别。
8. 新验收负载固定为400节点、64共享路径实例（单路径最多128命令）、128活动属性轨道、8层精确clip；Windows Release、1280×720、DPI1/1.5/2，300暖机+3600采样帧。静止、只变换、每帧改一条路径、密集补间、嵌套clip分项，目标AUI P95≤2ms、CPU P95≤16.67ms；GPU计时另报。算法初始化成本独立记录。不得靠降质量通过；如目标实测不可达，保留失败并审查owner，不直接放宽预算。

复审结论：必须补充的合同已修正，可生成施工文档并执行；路径抗锯齿与精确clip的后端技术在对应模块原型中冻结，必须满足上述像素/性能合同，不得用文档先行声称其通过。

### 14.2 Gate B实际绘制合同（2026-10-08）

代码创作入口为`PathBuilder/Path2D`和`Draw2D`；公开`Path2D::line/rectangle/rounded_rectangle/circle/ellipse/polygon`及`UiExtensionPaintOutput::from_graphics/from_graphics_asset`，通过既有Custom节点paint回调发布。不可变Graphics2D可重复使用，运行时发布生命周期由现有`UiNodeHandle(instance,slot,generation)`及Custom output owner承担；几何缓存持有私有Arc，不新增一套项目GPU handle或旁路Renderer。路径内容替换属于paint失效，清洁回调沿用既有identity缓存；独立图形播放或表现属性留C/D。

冻结`lyon_tessellation = 1.0.16`（MIT OR Apache-2.0；锁文件包含lyon_path/lyon_geom、euclid、float_next_after）。复用FillTessellator/StrokeTessellator和有界GeometryBuilder处理填充洞、自交、caps/joins；不将lyon当作完整UI/AA系统。曲线先有界展平，圆弧按最大半轴和精确弦高公式划分；最多16384展平事件，整份图形仍不得超过4096顶点/12288索引（含AA）。失败不提交半份图形、不降低精度，保留item与原因。

AA采用既有UI Pass的逐顶点coverage fringe：焊接相同位置并移除内部边，外边/洞边一像素宽度从原alpha线性衰减至0，接角扩展有界。该边缘覆盖并非解析面积积分或MSAA；颜色是现有UI顶点的归一化RGBA，使用straight alpha及`ALPHA_BLENDING`，不提前预乘，也不对RGB重复做sRGB解码。纹理仍由既有纹理格式承担sRGB解码。裁剪交点必须同时插值position/UV/RGBA，再交给原UI投影。GPU像素资格针对已声明Unorm离屏目标；后续实际窗口/Release与其它目标格式留E，不能据此声称全色彩链等价。

有效像素scale当前取Canvas参考坐标到真实target的最大轴倍率，包含实际DPI/viewport mapping；精度档位向上取四分之一倍频程，默认误差0.25像素。C接入节点仿射后再乘节点有效scale。几何cache以不可变描述与精度档位为键，FIFO最多64项/8MiB；此预算只统计该cache，不冒充全部UI/GPU内存预算。平移和清洁帧复用几何，DPI恢复复用旧档；资源registry替换或mapping变化使绘制产品失效，未改内容仍命中几何。关闭实例释放所属cache，超限淘汰不持有GPU对象。

新增`graphics2d-asset.v1`，cooked字段为`schemaVersion/assetId/assetGuid/graphics`。作者可沿用标准`displayName`等资源metadata，Assembler提取并校验typed payload，再由原Asset Graph/RuntimePackageBuilder写包、生成hash与GUID索引。Runtime仅从包内安全路径读有界1MiB cooked payload，校验hash、schema、ID/GUID，向加载的AUI Document共享只读registry；源目录不是运行依赖。

扩展协议升为`UI_EXTENSION_VERSION=2`、`aui-extension.v2`，增加`Graphics { graphics }`与`GraphicsAsset { asset_id,guid }`；SDK协议指纹同步包含v2。新Host明确拒绝v1 batch，旧Host通过SDK协议指纹拒绝新模块；旧项目保留代码并用新SDK/Generated Glue重建。C函数表、ABI1.6及UI_UPDATE_VERSION6保持不变。Generated Glue的实际DLL消费者须验证v2图形paint，不能只证明Describe或模板字符串存在。

具体命令、像素与缓存证据见Gate B执行记录；本合同不提前授予C/D/E、安装或完整性能资格。

### 14.3 Gate C实际表现与裁剪合同（2026-10-08）

2026-10-09局部修正：AUI十六进制样式RGB按sRGB输入，Renderer消费时解码一次，alpha保持线性；数值Draw2D、Custom mesh/primitive、UiLocalStyle和Tint保持线性RGBA。数值颜色借用旧字符串桥接时先编码为sRGB，亮度反馈在线性空间计算，不重复解码纹理或数值顶点。ProgressBar填充消费现有解析前景色/内联text_color，底条仍消费background/color，未配置填充色时保留绿色默认值。不增加schema或ABI版本；源修复及实际GPU资格见[局部完成记录](阶段完成记录/2026-10-09-AUI-Color-Progress-Repair/00-总览.md)，既有安装/Release不会自动更新。

节点新增可选`presentation`，默认单位变换、opacity=1、线性RGBA tint=白色。公开typed Rotation/Scale/Pivot/Opacity/Tint/Resource/LocalStyle及Binding消费共享校验。Pivot为相对布局尺寸，可超出0..1；旋转为弧度，镜像合法，零scale合法但不可逆节点不命中。布局占位、glyph shaping与路径描述保持独立；最终`local_to_canvas/vertex_transform/bounds/modulation`由原几何owner求值并沿原UiProjection供绘制、命中及SDK坐标查询使用。

正式实现使用CPU顶点仿射与凸多边形交裁剪后进入既有UI Pass；不增加独立Canvas、离屏纹理、shader/stencil或GPU资源owner。AABB只用于宽阶段，矩形/圆角的命中采用局部逆变换；镜像clip按轮廓方向处理。支持8层祖先clip，图片自身圆角另占一个内部边界。圆角绘制采用自适应角分段，最大屏幕弦高0.25像素，每角最多256段，超限报diagnostic并拒绝对应图元，不静默降质。圆角输入使用解析边界，绘制离散边界存在≤0.25像素误差；这不是无限精度或完整矢量抗锯齿资格。最终多边形在三角化/坐标投影前裁到Canvas参考边界，交点保留插值UV/RGBA，避免越界顶点导致可见三角形被丢弃；完全在界内的多边形沿用借用数据。

父变换只重算必要后代；纯表现setter保留原prepared document、路径与文字排版。Controller覆盖为逐属性稀疏mask，不能覆盖同节点其它Binding；释放后下次Binding发布恢复。成功呈现后Session接收最终几何，清洁Arc发布O(1)，有journal时仅同步变化闭包。同帧原子提交之后坐标查询基于已提交基础属性；输入读取最后成功发布（含反馈）的几何，不读取半接受回调。KeepScreen重挂用新父矩阵逆及节点pivot/RS原点修正；不可逆父拒绝。

控件反馈以基础结果为起点组合，子节点继承最终矩阵与颜色，绘制、命中和SDK发布同一几何；保留原capture/cancel与最多4个反馈变体。反馈几何目前仍复制一份不可变layout向量，矩阵重算局限变化闭包；不宣称零全量拷贝，实际性能资格在E。Legacy直接layout与Presenter共用几何owner，Presenter从原始布局求值一次，避免重复变换。

Resource必须同时匹配已挂载RuntimePackage的ID、GUID及texture/Graphics2D类型，Binding与Controller一致；不接受源路径。LocalStyle仅局部paint字段（背景/前景/边框颜色、圆角），不修改布局；边框颜色沿用已有宽度。原UI_UPDATE升为7（aui-update.v7），UI_HOST schema为ui-host.v5，协议指纹随实际schema更新；C函数表未变，ABI1.6与扩展v2保持。Generated DLL需实际接受和回滚新属性；旧项目用新SDK/Glue重建。

本合同冻结技术与边界；源码、GPU与消费者验收状态只以C执行记录为准，不提前授予D/E、安装或Release性能资格。

### 14.4 Gate D实际播放接入合同（2026-10-08）

复用A的Animation2D/PresentationClock和C的逐属性journal/最终几何，AuiSession拥有有界播放实例、活动属性覆盖与结构化结束队列；不增加第二套layout/paint/cache。每播放返回session/slot/generation，拒绝回调曾返回的句柄不会被新播放复用；最大4096播放、总4096轨道及4096待结束事件。描述256叶、32深度、整数微秒与溢出校验不变。

满额拒绝新增播放，但暂停/取消/游戏时钟控制仍可使用已预留结束事件容量。公开UiWriteBuffer的播放请求不强制中途flush：基础setter可在播放前后调用，统一随原callback接受；新播放CommitFinal的基础stamp在全部基础写入接受后捕获。属性读取始终读base，最终几何与绘制才消费临时覆盖；Binding中途更新及取消恢复最新值由生产缓存消费者验证。

UI_HOST schema为ui-host.v6：既有query通道增加typed play/pause/cancel/game-clock请求，最多1MiB，4096字节回复容量预检，状态型调用不重试；基础读取仍16KiB。UI_UPDATE7、扩展v2、ABI1.6不变，SDK协议指纹随实际schema改变，旧项目需重建。播放操作和游戏时钟变化在原callback接受边界提交，失败/NoOp回滚；完成事件通过既有ui_event及callback ID派发，不跨DLL传闭包，不在paint重入。

活动区间覆盖基础Binding/Controller但不修改其owner/value。串行间隙释放临时属性，Hold才保持终值；Release/取消读取当前base。完成的Hold只通知一次，之后取消/释放不再次报告完成。CommitFinal全目标校验预期逐属性owner/revision，仅对仍为Controller的基础值原子写回；Binding保持原owner并报告CommitConflict。多个播放的活动重叠默认Reject；延迟轨道首次实际重叠时收到Conflict并终止，显式Replace取消旧实例。多个属性可并行；逆向/Yoyo的端点和有限/无限循环由同一编译轨道求值。

每frame index仅推进一次，重复/倒序无推进；UI clock及项目pause/speed控制的Game clock独立，默认delta截断250000微秒。Player先让输入读取前次发布动画几何，再取消DragStart冲突translation、推进动画/按journal必要重建、组合既有内部反馈、发布本帧最终几何；固定回放用固定delta，真实窗口用呈现开始间隔并与反馈共享。Active scope由owner及所有目标有效Active暂停，恢复无追补；Instance scope可在Inactive期间运行；Hidden不等于dispose。owner/目标删除或代际变化取消播放，项目shutdown清空回调/实例，设备不承载动画owner。

此节冻结实际接口细节，D完成以执行记录为准；OS窗口、两普通项目、指定Release性能/交付留E，安装364不变。

### 14.5 Gate E 采集与比较实际边界（2026-10-08）

2026-10-08用户明确要求缩短28000帧验收：塔防改为三关各2168帧（120暖机+2048样本）、同包各两遍的真实窗口战斗切片；20轮重开压缩到2000帧。保持2/16.67ms预算及每关至少600有效战斗样本，完整30波语义复用同包既有headless结果，窗口切片不宣称完整通关或长时间稳定性。仅显式项目验收捕获时可选择原始起始关卡；普通游戏流程与关卡内容不变。新展示15组3600样本矩阵已经完成，不因塔防时长调整重跑。

新普通展示项目固定为`samples/aui_presentation_showcase_project`，另一个消费者为塔防普通项目`Projects/hezi_shoucheng`。现有Player诊断入口追加固定DPI1/1.5/2，输入实际AUI viewport而不修改OS设置；这是冻结DPI算法负载，不能冒称Windows多显示器迁移验证。CPU完整3600样本、GPU独立有界2048样本，缺失GPU时间不填0。

几何cache原有builds/hits/resident_bytes通过现有显式性能summary只读透传，普通默认不采集；记录新增剖分、命中和占用高水位。路径修改分项在暖机期暂停密集补间，并每帧产生1000个循环中的不同控制点描述，超出64项cache，不能用两条已暖机形状冒充新剖分负载。静止/只变换/路径/八层clip各保留400节点与64路径实例；密集补间分项保持128活动轨道。

冻结预算的`auiCpuNs`是既有authoring/layout/presentation区间；renderer UiProjection、draw-buffer准备、GPU及完整CPU帧分别报告。不得把此区间冒称全部UI CPU，或把不同分段P95相加当同一分布。冷首帧CPU/AUI独立报告；进程到第一帧截图的时间包含设备/字体/readback/编码，仅作为可见启动时间上界，不冒称纯剖分初始化。

## 15. 参考来源与源码定位

正式依据以当前源码和本方案合同为准；外部材料仅解释能力与可借鉴点。

| 来源 | 本地核对位置/符号 | 官方资料 |
| --- | --- | --- |
| Godot | `I:/godotAllCode/godot/scene/main/canvas_item.cpp`：`queue_redraw/draw_line/draw_arc/draw_circle`；`servers/rendering/renderer_canvas_cull.cpp`；`scene/animation/tween.cpp`：`bind_node/kill/step/set_parallel` | [自定义绘制](https://docs.godotengine.org/en/stable/tutorials/2d/custom_drawing_in_2d.html)、[Tween](https://docs.godotengine.org/en/stable/classes/class_tween.html) |
| Unity | `I:/UnityCode/UnityCsReference-master/UnityCsReference-master/Modules/UIElements/Core/Renderer/UIRPainter2D.cs`、`UIRMeshGenerationContext.cs`、`UIRRepaintUpdater.cs`；Core下`StylePropertyAnimationSystem.cs`、`ValueAnimation.cs` | [2D绘制](https://docs.unity3d.com/6000.0/Documentation/Manual/UIE-generate-2d-visual-content.html)、[Transitions](https://docs.unity3d.com/Manual/UIE-Transitions.html) |
| UE | `I:/UECode/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/UMG/Private/WidgetBlueprintLibrary.cpp`、`Components/Widget.cpp`、`Animation/UMGSequencePlayer.cpp`；SlateCore下`Rendering/ElementBatcher.cpp`、`Widgets/SWidget.cpp`、`Animation/CurveSequence.cpp` | [UMG动画](https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-in-umg-for-unreal-engine) |
| Bevy | `I:/BevyCode/bevy-main/bevy-main/crates/bevy_ui/src/ui_transform.rs`；`bevy_ui_render/src/lib.rs`；`bevy_time/src/fixed.rs` | [固定步与呈现插值](https://bevy.org/examples/movement/physics-in-fixed-timestep/) |
| PixiJS / lyon | GraphicsContext 文档；FillTessellator/StrokeTessellator 源码文档，作为分层/算法候选 | [Graphics](https://pixijs.com/8.x/guides/components/scene-objects/graphics)、[lyon](https://docs.rs/lyon/latest/lyon/)、[剖分源码](https://docs.rs/lyon_tessellation/latest/src/lyon_tessellation/lib.rs.html) |

项目内继承材料：[361正式方案](361-AUI表现逻辑分离与常驻节点运行时-v1方案.md)、[361引擎对比](361-AUI与Godot-Unity-UE界面架构对比.md)、[363正式方案](363-AUI常驻节点与端到端增量更新-v1方案.md)、[363总完成记录](阶段完成记录/2026-10-07-363-AUI-Incremental-Complete/00-总览.md)。历史源码参考目录的 Unity 旧版本渲染总结不能代替本次现代 UI 源码核对。
