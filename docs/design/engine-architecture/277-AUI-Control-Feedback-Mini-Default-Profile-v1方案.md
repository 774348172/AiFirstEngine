# 277-AUI Control Feedback Mini / Default Profile v1 方案

> 状态：已完成；Window A-C / Gate A-F 已于 2026-08-09 全部通过并归档。
> 日期：2026-08-08。
> 定位：通用 Project Runtime AUI 控件交互反馈能力，不是 Tower 专用动画，也不是 Animator2D 或完整 AUI Timeline。

## 1. 问题与结论

当前 AUI 已经能够命中 Button、消费输入并产生 `PointerDown / PointerUp / PointerMove / Hover / Click / Submit`
等命令，但按钮视觉不会随输入变化。问题不在业务 Action，而在以下链路缺少中间层：

```text
RuntimeInputEvent
  -> AuiInteractionSystem
  -> AuiInteractionState / AuiCommand / AuiAction
  -> [缺失：AUI Control Feedback]
  -> AuiDrawList Rect / Image / Text
  -> UiProjection / RuntimeRenderer / Present
```

本方案选择方案 B：建立一个深 `AuiControlFeedbackModule`，为普通 `AuiNodeKind::Button` 提供零配置的
Hover、Pressed、Release/Activated、Disabled、键盘/手柄 Submit 视觉反馈；项目可通过紧凑、版本化 Profile
统一覆写手感，但不能为每个按钮创建任意关键帧或第二套 Animator。

首版必须满足：

1. 普通 Button 不增加项目配置即可获得可见反馈；
2. 触摸按下在当前 present 周期立即反馈，不依赖 Hover 或 gameplay fixed tick；
3. 鼠标、触摸、键盘和手柄遵守同一激活真相，视觉反馈不重复派发 Action；
4. 动画只改变最终 draw visual，不改变 Layout、Hit Rect、Navigation 或 Clip 所有权；
5. 项目可以按 class 覆写少量缩放、位移、颜色和时长，并可显式关闭；
6. PointerUp outside、Cancel、失焦、隐藏、禁用、Modal/Screen 切换不得遗留 stuck pressed；
7. Editor GameView 与导出 Player 共享同一个 evaluator、时间输入和诊断合同。

## 2. Context Scan：当前真实基线

### 2.1 已存在的 AUI 输入与状态

`rust/crates/engine_runtime/src/aui.rs` 当前已经拥有：

- `AuiInteractionEventKind::PointerDown / PointerUp / PointerMove`；
- `AuiCommandKind::Hover / Click / Submit`；
- `AuiInteractionState.pressed_node`、focus、modal、screen stack、scroll 与 input field 状态；
- down/up 命中同一节点时产生 Click；
- 节点 effectively hidden 时清理 pressed/focus/drag 等状态；
- `AuiLayoutEngine::layout_with_interaction_state`，但当前只消费 scroll 和 canvas visibility；
- `AuiInteractionProductizationReport.control_style_deferred = true`。

因此 277 不应重写 Hit Test 或 Action Mapper。Interaction 继续拥有输入语义，Feedback 只消费稳定的只读
interaction snapshot 和本帧 command，输出 present-only visual override。

### 2.2 当前缺口

当前实现存在以下直接缺口：

- `Hover` 是瞬时 Command，没有持久 `hovered_node`；
- `pressed_node` 是私有交互状态，没有给视觉系统的稳定只读快照；
- 没有 `Activated` 瞬态，极快 Down+Up 可能没有任何可见周期；
- 没有 pointer device/capability，无法严格区分 mouse hover 与 touch；
- 没有统一 PointerLeave、PointerCancel、window focus lost 协调输入；
- PointerUp outside 的 pressed 清理必须补成明确 invariant；
- 当前 Player/GameView 链路先消费已有 present，再处理本帧 interaction，不能保证按下当帧进入 DrawList；
- AUI Style 只包含基础颜色、文字和字体，没有交互反馈 Profile。

### 2.3 与已有系统的边界

- 272 只负责 Native Editor 自身控件样式，不适用于 Project Runtime AUI；277 可复用“交互状态与视觉声明分离”
  的原则，但不得直接依赖 `EditorControlStyleModule`。
- 275 已经打通 AUI Image cooked texture、GPU upload、textured UI batch 与 alpha present；277 只装饰既有 draw item，
  不新增 texture resolve 或 GPU pipeline。
- 276 Animator2D Mini 属于 world `SpriteRenderer2D` fixed-tick 表现；277 属于 AUI presentation-time 输入瞬态，
  两者时钟、owner 和 consumer 均不同。
- 273 已统一 GameView target 与 AUI input coordinate；277 必须复用其 resolved coordinate，不建立第二次缩放或 hit test。

## 3. 成熟引擎参考结论

### 3.1 Unity UI Toolkit

参考：

- `https://docs.unity3d.com/6000.0/Documentation/Manual/UIE-Transitions.html`
- `https://docs.unity3d.com/6000.0/Documentation/Manual/UIE-USS-Selectors-Pseudo-Classes.html`
- `<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\UIElements\Core\Clickable.cs`
- `<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\UIElements\Core\Controls\Button.cs`

可学习点：`Clickable` 拥有 capture/active/click，VisualElement pseudo-state 由 style/transition 消费；输入命令和视觉
过渡分离。不可照搬点：完整 USS/CSS property、selector 与 transition 面对当前 AUI Mini 过宽。

### 3.2 Unreal Slate

参考：

- `https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/SlateCore/Styling/FButtonStyle`
- `<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Slate\Public\Widgets\Input\SButton.h`

可学习点：Normal/Hovered/Pressed/Disabled visual、PressedPadding 与 click method 分开表达。不可照搬点：每种控件
持有大型 style struct 会造成浅接口和逐控件重复配置。

### 3.3 Godot

参考：

- `https://docs.godotengine.org/en/stable/classes/class_basebutton.html`
- `<GODOT_SOURCE>\godot-master\godot-master\scene\gui\base_button.cpp`
- `<GODOT_SOURCE>\godot-master\godot-master\scene\gui\button.cpp`

可学习点：BaseButton 集中拥有 hover/pressed/action mode，Button 从 Theme 解析视觉。不可照搬点：277 不引入完整
Theme 继承树，也不把 Toggle、Shortcut、ButtonGroup 一次并入。

### 3.4 Bevy UI

参考：

- `https://docs.rs/bevy/latest/bevy/ui/enum.Interaction.html`

可学习点：`Pressed / Hovered / None` 是小而稳定的交互数据，视觉系统可以独立查询。不可照搬点：当前 AUI 已有
命令、capture、modal、focus 与 action mapping，不能退回只靠 ECS enum 猜 click 语义。

## 4. Ownership 与深模块接口

### 4.1 Owner 划分

```text
engine_input / platform adapters
  规范化 pointer device、cancel、leave 与 focus-lost 输入

AuiInteractionSystem
  拥有 hit test、capture、hover/pressed/focus、Click/Submit 与 input consumption

AuiControlFeedbackModule
  拥有 visual state、短过渡、Profile resolve、reconcile 与 visual override

AuiDrawList extraction
  将 override 应用到既有 Rect / Image / Text

UiProjection / RuntimeRenderer
  只消费最终 draw/composition，不读取 interaction 或 Profile
```

### 4.2 公共入口

公共行为面保持 1 个主入口，reconcile 作为模块内部阶段：

```rust
pub struct AuiControlFeedbackModule;

impl AuiControlFeedbackModule {
    pub fn advance(
        state: &mut AuiControlFeedbackState,
        input: AuiControlFeedbackFrameInput<'_>,
    ) -> AuiControlFeedbackFrame;
}
```

概念输入：

```rust
pub struct AuiControlFeedbackFrameInput<'a> {
    pub document: &'a AuiDocument,
    pub layout: &'a AuiLayoutResult,
    pub interaction: &'a AuiControlInteractionSnapshot,
    pub commands: &'a [AuiCommand],
    pub presentation_delta_us: u32,
    pub motion_scale_permille: u16,
}
```

概念输出：

```rust
pub struct AuiControlFeedbackFrame {
    pub visual_overrides: AuiVisualOverrideSet,
    pub report: AuiControlFeedbackReport,
}
```

调用方不接触动画游标、插值起点、Profile fallback、pulse retention 或 state cleanup。

### 4.3 只读交互快照

Interaction 必须提供只读快照，Feedback 不得重复 Hit Test：

```rust
pub struct AuiControlInteractionSnapshot {
    pub hovered_node: Option<String>,
    pub pressed_node: Option<String>,
    pub pressed_inside: bool,
    pub focused_node: Option<String>,
    pub pointer_device: Option<AuiPointerDeviceKind>,
}
```

`AuiInteractionState` 的可写 owner 仍是 InteractionSystem；Feedback 只能维护自己的 transition/pulse memory。

## 5. Schema：零配置默认值与紧凑覆写

### 5.1 版本

277 采用 `aui-document.v2` 的 additive authoring contract。Cooker 必须：

- 接受既有 `aui-document.v1`；
- 将缺失 feedback 字段确定性升级为 `Auto`；
- RuntimePackage 只消费规范化后的 v2；
- migration/report 明确记录 source version、normalized version 与 fallback。

### 5.2 Document 级 Profile

```yaml
schemaVersion: aui-document.v2
interactionFeedback:
  motionScalePermille: 1000
  profiles:
    - profileId: ink.button
      hoverScalePermille: 1010
      hoverBrightnessPermille: 40
      pressedScalePermille: 970
      pressedBrightnessPermille: -80
      pressedOffset: [0, 1]
      hoverInMs: 70
      pressInMs: 45
      releaseMs: 80
      activatedMs: 120
      hoverEasing: easeOutCubic
      pressEasing: easeOutCubic
      activatedEasing: easeOutBack
```

### 5.3 Node 级选择

Button 默认 `Auto`，无需声明：

```yaml
kind: Button
```

需要项目风格时：

```yaml
feedback: ink.button
```

显式关闭：

```yaml
feedback: none
```

合法值只有：

```text
Auto
None
Profile(profile_id)
```

解析顺序固定为：

```text
node Profile
  -> document defaultButtonProfile（若声明）
  -> engine built-in button.default.v1
```

Profile v1 只允许固定字段：uniform scale、translation、brightness/tint、opacity、duration 与有限 easing。
禁止关键帧数组、任意属性路径、脚本 callback、循环、状态图、资源换图和多层继承。

## 6. 状态模型与优先级

基础状态：

```rust
enum AuiControlVisualState {
    Normal,
    Hovered,
    Pressed,
    Disabled,
}
```

`Activated` 是正交短 pulse，不是业务状态；Focus 也为正交视觉层。组合优先级固定：

```text
Disabled > Pressed > Hovered > Normal
                         + FocusVisible layer
                         + Activated pulse
```

进入 Disabled 必须立即清理 hover、pressed eligibility 与 activated pulse。v1 对
`AuiNodeKind::Button && interactable == false` 解析 Disabled；是否增加独立 enabled binding target，留给施工文档
按现有 binding contract 做影响分析，不允许用项目私有 binding path 硬编码。

## 7. 输入与取消语义

### 7.1 鼠标

```text
Normal -> PointerEnter -> Hovered
Hovered -> PointerDown -> Pressed
Pressed -> PointerUp inside + Click -> Activated pulse -> Hovered
Pressed -> PointerUp outside/cancel -> recovery -> Normal/Hovered
```

按下后移出：capture owner 可保留，但 `pressed_inside=false`，Pressed 视觉退出；移回原按钮可重新进入 Pressed。
只有 Interaction 产生 Click 时才启动 Activated。

### 7.2 触摸

```text
Normal -> TouchDown -> Pressed
Pressed -> TouchUp inside + Click -> Activated pulse -> Normal
Pressed -> TouchUp outside / TouchCancel -> recovery -> Normal
```

触摸不生成或遗留 Hover。Normalized pointer input 必须提供 `Mouse / Touch / Pen` 或至少 `hover_capable`，不能
通过事件频率或是否移动猜设备类型。

### 7.3 键盘与手柄

Focused Button 收到 Submit Down 时显示 Pressed；Submit Up/最终 Submit Command 触发 Activated。视觉层不得
产生第二个 Submit，也不得改变现有 Action dispatch 时机。

### 7.4 强制 reconcile

以下情况必须终止 Pressed，且除真实 Click/Submit 外不得产生 Activated：

- PointerUp outside、PointerCancel、capture lost、window focus lost；
- node hidden、removed、disabled 或 document re-hydration 后 ID 不存在；
- canvas hidden、Modal root 改变、Screen Stack 切换；
- active input mode 阻止该控件；
- Stop Play、session replacement 或 viewport identity 改变。

## 8. Present 集成与 Visual Override

推荐正式顺序：

```text
AUI binding resolve / hydration
  -> stable layout + visibility reconcile
  -> current-frame interaction / action mapping
  -> AuiControlInteractionSnapshot
  -> AuiControlFeedbackModule::advance
  -> draw extraction with visual overrides
  -> existing Rect / Image / Text DrawList
  -> UiProjection / RuntimeRenderer UI pass
  -> Present
```

概念 override：

```rust
pub struct AuiVisualOverride {
    pub owner_node_id: String,
    pub scale_permille: u16,
    pub translation: AuiVec2,
    pub brightness_permille: i16,
    pub opacity_permille: u16,
}
```

Button 根节点和其可视子树围绕 Button computed rect 中心一起变换；背景 Image、装饰 Rect、Label Text 与图标
必须保持相对关系。owner/subtree 通过稳定树关系解析，不依靠 `:label` 字符串命名约定。

必须保持不变：

- `AuiComputedRect` 与 Hit Test Rect；
- navigation 与 focus traversal rect；
- clip rect 与 clip owner；
- painter order、draw item kind 与 texture handle；
- AuiCommand、AuiAction、payload 与 input consumption；
- ProjectUiStateSnapshot 与 gameplay fixed tick。

Renderer 不新增 `DrawButton`、`AnimatedButton` 或反馈专用 GPU pipeline。

## 9. 时间、确定性与 reduced motion

- 模块只接受显式 `presentation_delta_us`，禁止内部读取 wall clock；
- 不使用 Animator2D/gameplay fixed tick；Pause gameplay 时 AUI 仍可反馈；
- 相同 document、layout、interaction、commands 和 delta 序列必须产生相同 override；
- 单帧 delta 可在模块内有界 clamp，并在 Summary/Trace 报告 clamp count；
- 同帧 Down+Up+Click 的 Activated 至少保留一个 present 周期；
- pulse 到期必须回收，重复点击从当前视觉值平滑 retarget，不无限堆积；
- `motionScalePermille=0` 时取消过渡插值和弹性缩放，但保留即时颜色/透明度状态反馈。

## 10. 引擎默认 Profile

`button.default.v1` 建议值：

| 状态 | 视觉 | 时长 |
|---|---|---:|
| Hover in | scale 1.01，brightness +4% | 70 ms |
| Hover out | 恢复 normal | 80 ms |
| Pressed | scale 0.97，brightness -8%，Y +1 logical px | 45 ms |
| Activated | 轻微 1.02 -> 1.0 回弹，brightness +6% | 120 ms |
| Cancel release | 无 pulse，平滑恢复 | 80 ms |
| Disabled | opacity 55%，不响应 hover/press | 即时 |

默认值必须是引擎版本化常量，并在 report 中输出 resolved profile identity。Tower 水墨 UI 可以使用
`ink.button` class 降低 scale、主要使用墨色加深和轻微下压，但该 Profile 是项目资产，不进入引擎硬编码。

## 11. Diagnostics 与 Report

诊断至少覆盖：

```text
aui.feedback.profile_missing
aui.feedback.profile_invalid
aui.feedback.pointer_device_missing
aui.feedback.time_reversed
aui.feedback.delta_clamped
aui.feedback.pressed_reconciled
aui.feedback.owner_subtree_missing
```

报告遵守 Off / Summary / Trace：

- Off：正式 runtime 默认，不生成字符串 trace；
- Summary：active transition count、resolved profile、reconcile/clamp/missing count；
- Trace：测试或显式诊断记录 node、输入状态、from/to、elapsed、override 与 cleanup reason。

Runtime 热路径只为 hovered、pressed、focused、activated 或仍在 transition 的节点保留状态，不为整份 Document
每帧创建动画对象。

## 12. 验证合同

未来施工文档必须按 274 v2 的 owner/consumer closure 选择最小充分证据，不机械叠加完整测试菜单。

### 12.1 Owner contract

1. 无 feedback schema 的 Button 自动获得默认反馈；
2. PointerDown 当帧 visual override 已为 Pressed；
3. hover、press、outside release、click pulse、disabled、focus/submit 状态表通过；
4. 同帧 Down+Up 仍保留一次 Activated present；
5. cancel、失焦、hidden/removed/disabled、Modal/Screen/session replacement 无 stuck state；
6. 相同 delta 序列输出确定，time reversed 与大 delta 有结构化诊断；
7. v1 -> v2 migration、Profile fallback、`feedback:none` 与非法 Profile 诊断通过。

### 12.2 Consumer contract

1. decorated draw list 仍只含既有 Rect/Image/Text primitive；
2. Button 根与 Image/Text 子树同步变换；
3. 动画前后 layout、hit-test、navigation、clip 与 Action 结果完全相同；
4. runtime_player_winit 与 Editor GameView 对同一输入序列产生一致状态和 override；
5. 至少一个非 Tower fixture 证明不存在项目硬编码；
6. Tower 只作为真实水墨 Button consumer visual smoke，不替代引擎 owner tests。

真实 OS/GPU 视觉只需要覆盖能证明 present consumer 的最小矩阵；Local CI、production binary replacement、真实配置
或完整 Tower Gate 必须由未来施工文档和用户授权单独决定。

## 13. 明确延期

v1 不包含：

- Toggle、Slider、InputField、ScrollView 的自动反馈；
- 多 pointer 同时按压多个控件；
- 状态图片切换、Nine-Slice skin swap、material/shader property；
- 任意关键帧、动画事件、循环、Timeline、Animator Controller；
- 音效、震动、粒子与平台 haptics；
- loading、cooldown、selected、checked 等业务长生命周期状态；
- 完整 CSS/USS selector、继承和通用 AUI Transition 系统。

后续若多个真实项目证明需要状态图片或更多控件，不得把 v1 Profile 字段无限扩宽，应单独讨论 AUI Theme/Control
Skin 或通用 Transition 的新系统边界。

## 14. Red Lines

- 不在 Tower RuntimeModule 中逐按钮计时或修改 draw item；
- 不把 Hover/Pressed/Activated 写入 ProjectUiStateSnapshot；
- 不让 Feedback 生成 Click/Submit 或改变 Action exactly-once；
- 不复用 Animator2D fixed tick、Clip、Controller 或 SpriteRenderer2D；
- 不改变 Layout/Hit Rect 来制造按压缩放；
- 不新增 Button 专用 renderer primitive 或 GPU pipeline；
- 不在引擎中硬编码水墨、兵种、招募、出怪等项目语义；
- 不通过 wall clock、HashMap 顺序或 render FPS 产生不可重放状态；
- 本方案确认不等于施工授权，不授权 Local CI、production/安装态替换或真实配置修改。

## 15. 建议施工窗口

本文不是施工文档。277 施工文档在激活前自审中根据 dirty baseline、跨 input/AUI/session 影响面与三小时上限，
将原两窗口建议收紧为三个独立授权窗口；功能范围不变：

```text
Window A / Gate A-B
  input capability/cancel contract、schema/migration、interaction snapshot、PointerUp outside 修复

Window B / Gate C-D
  AuiControlFeedbackModule deterministic evaluator、draw subtree override

Window C / Gate E-F
  Player/GameView present integration、affected consumer regression、documentation closure
```

每个窗口必须遵守三小时上限。不得从 Tower 按钮视觉特例开始；通用 owner contract 是项目 consumer 的前置。

## 16. 方案自审

### 16.1 是否符合用户选择的方案 B

是。所有 Button 零配置获得引擎默认反馈，同时保留紧凑 Profile 与 `none` 覆写；没有退化为只能写死参数的
方案 A，也没有扩张为完整状态皮肤/Timeline 的方案 C。

### 16.2 是否为深模块

是。调用方只提交一帧 interaction/time 输入并取得 visual override；状态组合、短过渡、fallback、reconcile、
subtree 传播和诊断全部隐藏在 `AuiControlFeedbackModule` 内。

### 16.3 是否保持 AUI 架构边界

是。Interaction 拥有输入与 Action，Feedback 拥有 present 瞬态，Layout/Hit Test 保持稳定，Renderer 只消费最终
draw/composition，ProjectUiStateSnapshot 不承载按钮视觉状态。

### 16.4 是否满足 Rust 底层规则 + schema 上层规则

是。Rust 深模块拥有状态机、时间推进、取消和合成顺序；AUI v2 schema 只声明有限 Profile 参数和节点选择，
项目不能脚本化 evaluator，也不需要逐按钮写运行时代码。

### 16.5 是否过度设计

没有。v1 只覆盖 Button、固定属性和短过渡；状态图片、多控件、任意属性轨道、声音/震动和完整 Theme/Transition
均明确延期。

### 16.6 自审结论

```text
方案结论：通过
方案编号：277
用户选择：方案 B
需要修改引擎：是；仅未来施工授权后允许
需要修改 Tower 项目：否；Tower 只作为未来可选 consumer
施工文档：施工文档/当前/277-当前可自动化施工文档-AUI-Control-Feedback-Mini-Default-Profile-v1.md
当前允许施工：仅 Window A / Gate A-B
当前施工槽：277 active
下一步：按已激活范围开始 Gate A；不得进入 Window B-C
```
