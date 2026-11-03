# 361：AUI 与 Godot / Unity / UE 界面架构对比

日期：2026-09-24。配套：[AUI 正式方案](361-AUI表现逻辑分离与常驻节点运行时-v1方案.md)。

R2结论：项目开发体验以Unity UGUI的引用、事件、模板和生命周期为主要参照，运行内核以UI Toolkit的常驻树、分类失效与绘制缓存为主要参照；Godot/Slate交叉验证交互与缓存机制。AI友好放在作者资产和工具接口，不成为每帧负担。361是设计目标，不宣称Unity兼容或达到其它引擎成熟度。

## 1. 用户写游戏时的区别

| 问题 | Godot Control | Unity UGUI | Unity UI Toolkit | UE UMG / Slate | 361 AUI 目标 |
| --- | --- | --- | --- | --- | --- |
| 界面结构 | Control 场景树、Container | GameObject、RectTransform、组件 | VisualElement 树、UXML/USS | Widget 树、UMG/Slate | AUI 资产、组合 View、常驻实例 |
| 逻辑放哪里 | 脚本/信号，可自行分层 | MonoBehaviour/事件，可自行分层 | C# 控制器/事件，可选绑定 | Blueprint/C++，可选 MVVM | Rust Controller；Model 不依赖 UI |
| 修改文字/位置 | 控件属性/setter | 组件属性/RectTransform | 属性/style/transform | Widget API / Slate 属性 | typed 节点 setter |
| 绑定是否强制 | 否 | 否 | 否 | 否，MVVM 为可选插件 | 否，兼容已有绑定源 |
| Grid 是什么 | 布局容器 | 布局组件 | 可组合布局结构，不能等同 Godot GridContainer | Grid/UniformGrid 等 Panel | 子节点排列策略 |
| UI 根是不是按钮 | 否 | 否 | 否 | 否 | UIBattle 是组合 View，Button 是具体控件 |
| 每帧提交整个 UI 状态 | 不要求 | 不要求 | 不要求 | 不要求 | 不要求 |

Unity 并不强制 MVC；我们借鉴的是“外观资产与项目逻辑可分离，逻辑能直接使用控件”的开发方式。UGUI 与 UI Toolkit 是不同系统，不能把后者的增量渲染实现直接说成前者已有的同一机制。

## 2. 引擎内部对比

| 维度 | Godot | Unity | UE | 当前 AUI 与 361 的差距 |
| --- | --- | --- | --- | --- |
| 常驻状态 | Control/CanvasItem 常驻 | 两套系统均保留运行对象 | SWidget/UMG 常驻 | 已有布局缓存，但完整文档仍频繁参与处理；361 要消除正常热路径全文复制 |
| 更新合并 | queue_redraw、queue_sort 合并 | UGUI 布局/图形队列；UITK 分类版本与更新器 | Invalidation reason 与 fast-update 队列 | 需端到端合并，不只顶层 cache hit |
| 位置变化 | 纯位置可走 CanvasItem transform | UITK 区分 transform 等变化；UGUI 受 Canvas/布局边界影响 | render transform 仍可带 Layout 失效，取决于机制 | 设计独立 post-layout transform，明确裁剪/命中依赖 |
| 文本 | Label 局部 dirty 与 shaping | UITK 文本设置/缓存；UGUI 字体组件另有路径 | 文本/布局与绘制缓存结合 | 已有真实 glyph；需避免无关变更重做 shaping/几何 |
| 渲染缓存 | 画布命令与更新调度 | UITK 保留渲染数据；UGUI Canvas 批处理 | cached draw elements；可选 Retainer | Renderer 仍有重建成本，361 必须连后端一起改 |
| 交互 | GUI 路由、捕获、拖放预览 | EventSystem 或 UITK 事件/捕获 | Slate 路由、capture、hit grid | 已有交互能力；需与新实例和局部命中更新统一 |
| 长列表 | 可按具体控件/项目组织 | UITK 有虚拟化控制器 | 列表与条目回收体系 | 361 明确可见区和稳定 key；不能只裁剪后照常处理全部节点 |
| 绑定更新 | 信号/脚本按项目组织 | UITK 多种更新触发策略 | MVVM 通知；Slate 某些属性仍轮询 | 可选按字段更新，禁止“一切绑定必然逐帧全量” |

成熟引擎也会发生大范围重排、Canvas 重建、属性轮询和缓存失效。学习的是可控制、可解释的更新边界，不是声称它们所有操作都是 O(1)。

## 3. 源码依据与采纳决定

下面是本次调查的本机源码快照。Godot/Unity 目录名不证明它们与用户交付游戏的版本完全一致；UE 本地 Build.version 标记 5.8.0、Changelist 0，也不据此认定正式发行身份。文件与符号是依据，行号只供本次定位。未进行四引擎同硬件、同 UI、同功能的受控跑分。

### Godot

根目录：`I:/godotcode/godot-master/godot-master`。

| 文件/位置 | 观察 | 采纳与边界 |
| --- | --- | --- |
| scene/gui/control.cpp:2264–2290 | 区分 position/size 变化；位置更新 canvas transform | 分开布局位置和运行变换，不为拖动反复重建文档 |
| scene/main/canvas_item.cpp:540 | queue_redraw 合并待处理刷新 | 同帧多次 setter 合并 |
| scene/gui/container.cpp:156 | queue_sort 合并容器重排 | 脏布局根队列去重 |
| scene/gui/label.cpp:190、219、440、1131 | 分层文字 dirty、按需 shaping、setter 同值检查 | 文字状态缓存，不因颜色变化重排字形 |
| scene/main/viewport.cpp:1748、2055、2445、2518 | GUI 传播、拖动阈值、预览跟随与取消清理 | 捕获源/drop target/预览分离 |

不照搬：Godot 整套 SceneTree、资源编辑器与 GDScript 对象模型。AUI 保留 RuntimePackage、Rust 项目模块和现有工具体系。

### Unity UGUI

根目录：`I:/com.unity.ugui/com.unity.ugui`。

| 文件（Runtime/UGUI/UI 下） | 观察 | 采纳与边界 |
| --- | --- | --- |
| Core/CanvasUpdateRegistry.cs:86、279、317 | 分开的布局/图形更新队列及去重 | 按成本类别组织更新 |
| Core/Layout/LayoutRebuilder.cs:170 | 向必要祖先查找布局根 | 局部更新仍需遵守父子布局依赖 |
| Core/Graphic.cs:628、644 | 绘制重建与剔除条件 | 隐藏/不可见数据不做无效图形重建 |

项目体验借鉴组件引用、事件与直接属性设置，不引入完整 GameObject/MonoBehaviour 克隆。C# 包源码不包含完整原生 Canvas 实现，因此不对其全部底层合批算法作源码已核验声明。

### Unity UI Toolkit

根目录：`I:/UnityCode/UnityCsReference-master/UnityCsReference-master`；下表相对 `Modules/UIElements/Core/`。

| 文件/位置 | 观察 | 采纳与边界 |
| --- | --- | --- |
| Panel.cs:36；Renderer/UIRRepaintUpdater.cs:98 | VersionChangeType，区分 transform/size/clip/opacity/color/repaint | typed invalidation 与分阶段处理 |
| Style/Generated/ComputedStyle.gen.cs:1586、4211 | translate 触发对应变换变化 | setter 自动分类 |
| Renderer/UIRRenderEvents.cs:790–845 | 变换数据可局部更新；裁剪等会扩大成本 | 不承诺所有变换恒定成本 |
| VisualTreeStyleUpdater.cs:109、334、350 | 标记变化及依赖，跳过干净子树 | 主题/样式依赖索引 |
| Text/UITKTextHandle.cs:130 | 文字设置 identity 与缓存 | 复用稳定文字结果 |
| Events/EventDispatchUtilities.cs:151；PointerCaptureController.cs:30 | 事件传播与捕获状态提交 | 明确事件边界和命中一致性 |
| Collections/Virtualization/FixedHeightVirtualizationController.cs:108；VerticalVirtualizationController.cs:174 | 可见区、回收、解绑重绑与 item identity | 真正虚拟化，避免条目身份串用 |
| Bindings/Binding.cs:13 | WhenDirty/OnSourceChanged/EveryUpdate 等触发策略 | 绑定可选、触发策略显式 |

不照搬：USS/CSS 全语义和 Unity 编辑器生态。保留现有 AUI 主题、布局资产，用成熟失效模型重构内部。

### UE UMG / Slate / MVVM

根目录：`I:/UECode/UnrealEngine-release/UnrealEngine-release/Engine`；核心文件相对 `Source/Runtime/`。

| 文件/位置 | 观察 | 采纳与边界 |
| --- | --- | --- |
| SlateCore/Public/Widgets/SWidget.h:774、1771、1780 | 测量、排列、绘制职责 | 独立阶段与缓存边界 |
| SlateCore/Public/Types/Attributes/SlateAttributeBase.inl:257 | 属性比较后失效 | 同值 setter 零失效 |
| SlateCore/Public/Widgets/InvalidateWidgetReason.h:13；Private/Widgets/SWidget.cpp:1306 | 分类失效 | 引擎承担依赖传播 |
| SlateCore/Private/Widgets/SWidget.cpp:191 | RenderTransform 注册 Layout 与 RenderTransform 原因 | 不能宣称 UE 所有位移都跳过布局 |
| SlateCore/Private/FastUpdate/SlateInvalidationRoot.cpp:301、358、1092 | 去重队列、快速绘制、属性轮询路径 | 学习增量机制，不声称全部绑定事件驱动 |
| SlateCore/Public/Rendering/DrawElements.h:139；Private/Input/HittestGrid.cpp:859、932 | 缓存绘制元素、命中成员更新 | 绘制与命中一起增量化 |
| Slate/Private/Framework/Application/SlateApplication.cpp:6385、5742、5841 | 路由与捕获 | 控件级交互职责 |
| UMG/Public/Components/RetainerBox.h:15；Private/Slate/SRetainerWidget.cpp:403、574、667、687 | 离屏纹理缓存、刷新阶段与命中处理 | Retainer 是可选优化，存在纹理、pass 和更新延迟成本 |

MVVM 位于 `Plugins/Runtime/ModelViewViewModel/Source/ModelViewViewModel/`：`Public/MVVMViewModelBase.h:81` 比较并通知；`Private/View/MVVMView.cpp:507、743、760` 订阅/更新；`Private/MVVMBindingSubsystem.cpp:144` 去重。该插件支持 Immediate/Delayed/Tick 等方式，不是所有 UMG 项目的必经层。

不照搬：UObject/Blueprint 体系、完整 Slate 声明式 DSL、强制 MVVM、默认离屏缓存。AUI Controller setters 与可选 Binding 足以表达当前需求。

官方资料交叉验证：

- [Unity：UI Toolkit 运行时性能建议](https://docs.unity3d.com/6000.0/Documentation/Manual/UIE-performance-consideration-runtime.html)：usage hints、几何更新与 atlas/batching。
- [UE：Slate 与 UMG 的 Invalidation](https://dev.epicgames.com/documentation/en-us/unreal-engine/invalidation-in-slate-and-umg-for-unreal-engine)：缓存和失效边界。
- [UE：UMG 优化建议](https://dev.epicgames.com/documentation/en-us/unreal-engine/optimization-guidelines-for-umg-in-unreal-engine)：事件更新、轮询成本与 Retainer 取舍。

## 4. 我们保留的差异与仍然落后的部分

| 方面 | 361 的取舍 | 成熟度判断 |
| --- | --- | --- |
| AI 修改 | schema、稳定 ID、typed refs、可定位报告 | 适合本项目工具链，不意味着其它引擎无法由 AI 开发 |
| 开发模型 | 熟悉的节点/控制器/数据，绑定可选 | 应减少现有快照/revision 心智负担；API 尚未实现 |
| 运行效率 | 常驻对象、局部失效、后端缓存 | 方向接近成熟引擎，必须用真实项目证明 |
| 控件与平台 | 继承已有布局/字体/控件，再补实际缺口 | IME、焦点、DPI、无障碍与平台覆盖仍不能宣称同等成熟 |
| 工具生态 | 无编辑器主线，九工具与源码资产 | 可视化调试、性能分析和第三方控件生态仍明显较小 |
| 复杂渲染 | 先满足正确布局、文本、裁剪、交互与局部更新 | 不把完整材质 UI、离屏特效、世界空间 UI 全部扩入本轮 |

## 5. 如何证明改造有效

第一层对比旧 AUI 与新 AUI：同一塔防场景、相同节点/视觉、相同玩法输入、相同机器和 profile，测分段耗时、帧间隔、内存和工作量计数；不删真实 UI 凑成绩。

第二层如需对比 Godot/Unity/UE：制作功能和素材相当的 UI 样本，分别记录构建模式、字体、分辨率、渲染后端、垂直同步、节点/可见项、输入和采样条件。数值可分别报告，条件不等同的结果不得写成引擎性能倍率。

最终判断要同时看“写一个功能是否直接”“交互是否正确”“动态场景是否流畅”“错误是否容易定位”。361 的重点是补齐更新架构与编程接口，不能只把旧流程重命名为 MVC。

## 6. R2审核采纳：Unity相似度、扩展性与效率

本轮沿用原有主方案，在用户确认后补齐接口和执行合同，不再另开一套竞争方案。

| 审核发现 | Unity源码依据（相对前文根目录） | R2采纳位置 |
| --- | --- | --- |
| setter之外缺乏完整日常操作语义 | UGUI Runtime/UGUI/UI/Core/Button.cs的onClick、OnPointerClick、OnSubmit；Runtime/UGUI/EventSystem/UIBehaviour.cs的启停销毁回调 | 主方案5.1、9.1：事件、模板、层级、启停与通知 |
| Rect操作只有抽象描述 | UnityCsReference Runtime/Transform/ScriptBindings/RectTransform.bindings.cs:143：anchorMin/Max、anchoredPosition、sizeDelta、pivot | 5.2：映射至已有布局内核，明确坐标差异和父布局所有权 |
| 项目自定义绘制/布局入口未成合同 | UGUI Graphic.cs的OnPopulateMesh；Core/Layout/ILayoutElement.cs的ILayoutElement/ILayoutController | 5.3：组合、行为、测量/布局/绘制扩展，统一投影与缓存 |
| 自动dirty可能被误读成禁止作者局部重绘 | UnityCsReference Modules/UIElements/Core/VisualElement.cs:2434–2480：内置属性自动失效，MarkDirtyRepaint支持自定义generateVisualContent | 1、5.3：普通用户自动，控件作者可按需申请 |
| 仅有局部更新原则，调度合同不足 | UGUI CanvasUpdateRegistry.cs的PerformUpdate；UITK Renderer/UIRRepaintUpdater.cs的OnVersionChanged/Update | 7.1–7.3：分类owner、队列、执行阶段、重入限制及成本计数 |
| DLL扩展成本和生命周期未明确 | 本引擎Project Module ABI约束；Unity参考源码不代表本引擎DLL合同 | 5.4：生成分发、有界数据、稀疏写集合及卸载清理 |

上表Graphic.cs、CanvasUpdateRegistry.cs相对UGUI的Runtime/UGUI/UI/Core；UITK路径相对Modules/UIElements/Core。源码快照与版本适用范围仍按第3节。

### 接近Unity的部分与有意保留的差异

接近UGUI：直接控件引用、注册事件、模板实例化、层级与矩形操作、明确生命周期；不要求项目提供整UI快照。接近UI Toolkit：常驻树、类型化失效、按需自定义绘制、局部几何缓存和虚拟列表。

保留差异：Rust组合与生成Glue替代C#继承/UnityEvent序列化；结构化AUI资产替代Unity场景/Prefab格式；UI左上坐标与明确布局owner不宣称RectTransform完全兼容；绑定属性冲突明确报错；跨DLL扩展使用版本化数据交换。并不要求用户学习四套引擎模型，也不增加第二套UI运行时。

“不修改Core就能增加控件”是R2必须实现和验收的能力，现有Custom枚举或测量trait不足以证明它已交付。设置面板、塔防拖放、自定义绘制/布局与规模列表三组样本共同检验易用性和效率。R2补齐的是设计缺口，实际性能仍须施工后测量。
