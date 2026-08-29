# 272-Editor Declarative Control Style / Pseudo-State v1 方案

## 1. 状态与结论

```text
讨论结论：用户确认方案 C
方案状态：正式方案已确认并完成施工
施工状态：已完成；Gate A-F 与 273 后 input/authority closure 已闭环
施工归档：施工文档/已完成/272-当前可自动化施工文档-Editor-Declarative-Control-Style-Pseudo-State-v1.md
适用范围：Native Rust Editor UI
不适用范围：Project Runtime AUI、项目玩法 UI、Web/CSS 兼容层
```

本方案为 Native Editor 建立声明式控件样式与伪状态系统。按钮、图标按钮、页签、开关等控件不再各自
判断 hover/pressed/selected 后手写颜色，而是提交稳定的控件身份和状态，由一个深
`EditorControlStyleModule` 完成规则匹配、确定性级联、视觉解析和绘制配方输出。

用户截图中的 `Game` 页签必须具备清楚的普通、悬停、按住和选中反馈；相同能力必须自动适用于其它
同角色控件，不能只给 `Game` 或 Dock Tab 增加特例。

## 2. 问题定义

现有 Editor 已经有：

```text
EditorFocusInputSystem.hovered_hit_id
EditorFocusInputSystem.pressed_hit_id
UiRendererConfig.hovered_hit_id / pressed_hit_id
WidgetRole / WidgetId / enabled / HitRegion
ImageTextureSlot 与 WGPU 图片绘制通道
```

但消费方式是分散的：

```text
Toolbar：手写 disabled -> pressed -> hovered -> normal
Workspace Tab：只手写 active/inactive
其它 Button/Tab/IconButton：各 panel 自行决定，状态覆盖不一致
EditorInputRouter：大部分命令在 PointerDown 直接触发
```

因此当前缺口不是鼠标事件，而是缺少统一的交互状态、样式规则和激活语义。继续在 panel 绘制函数中
追加 `if hovered` 会复制规则、制造状态组合遗漏，并让以后更换主题或状态图片需要修改大量 Rust 调用点。

## 3. 目标

v1 必须达到：

1. `Button / IconButton / Tab / Toggle` 共享同一套声明式视觉状态规则。
2. 支持 `normal / hover / active / selected / checked / disabled / focus-visible` 及必要组合。
3. 支持纯色、边框、文字/图标颜色、透明度、内容偏移和可选状态图片。
4. 状态图片支持 Nine-Slice，按钮尺寸变化时不要求准备多套位图。
5. 普通按钮默认在控件内部释放时激活；菜单等少数控件可显式选择 press 激活。
6. 页签/开关的持续选中态来自 model，不用定时器伪造“点击后仍按下”。
7. 样式匹配、优先级、fallback、diagnostic 和视觉证据可被 AI 稳定解释和测试。
8. 伪状态变化只失效受影响控件的 paint，不重建项目 RuntimePackage，不推进 Game Runtime。

## 4. 非目标

v1 不做：

```text
完整浏览器 CSS/USS 语法解析器
后代、兄弟、属性、通配符等任意复杂选择器
脚本求值、表达式、反射式属性访问
伪状态驱动布局尺寸变化
transition/keyframe 动画
Project Runtime AUI 样式迁移
允许未受信项目向 Editor 注入任意图片或样式
为 Game 页签、塔防项目或某个 panel 写专用引擎规则
```

“完整声明式”在本方案中指完整覆盖控件交互状态到视觉输出的链路，不代表复制 Web 平台全部能力。

## 5. 成熟引擎源码结论

### 5.1 Unity UI Toolkit

参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\UIElements\Core\Clickable.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\UIElements\Core\Controls\Button.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\UIElements\Core\FocusController.cs
```

`Clickable` 负责 pointer capture、active pseudo state 和 click，`VisualElement` 的 pseudo state 再由样式系统
消费；`Button` 可以使用 Texture、Sprite 或 Vector Image。可学习点是交互状态与视觉声明分离、状态图片
不进入业务命令。不可照搬点是 USS/CSS 选择器和完整 style property 面过大，不适合当前 Rust Editor v1。

### 5.2 Unreal Slate

参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Slate\Public\Widgets\Input\SButton.h
```

`SButton` 明确区分 `IsPressed`、hover/pressed sound、normal/pressed padding 与
`EButtonClickMethod`，默认 `DownAndUp`，也允许特例调整触发方式。可学习点是视觉 pressed 与命令触发
策略分离。不可照搬点是每种 Slate 控件持有大型 C++ style struct，会让本项目继续形成浅接口和重复字段。

### 5.3 Godot

参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\scene\gui\base_button.cpp
<GODOT_SOURCE>\godot-master\godot-master\scene\gui\button.cpp
```

Godot `BaseButton` 集中处理 hover、pressed、toggle、button mask 与 action mode，`Button` 再从 Theme
取得对应 StyleBox/Icon/Color。可学习点是基础按钮状态机和主题资源的稳定分工。不可照搬点是按控件类型
枚举大量 theme property 名称，不利于 AI 统一审查和跨角色复用。

### 5.4 Bevy UI Widgets

参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_ui\src\focus.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_ui_widgets\src\button.rs
```

Bevy 使用 `Pressed` / `InteractionDisabled` 等数据状态；Button 默认在解除按下时发出 `Activate`，菜单可用
`ActivateOnPress`。可学习点是小而明确的状态数据和显式激活策略。不可照搬点是 ECS marker 不是本项目
retained Editor widget tree 的最佳外部接口。

## 6. 核心架构决定

新增一个深模块：

```text
Editor input + model state
  -> ControlPseudoStateSet
  -> EditorControlStyleModule.resolve(ControlStyleQuery)
  -> ResolvedControlStyle
  -> existing ordered UiDrawList
  -> WGPU painter batches
```

外部接口只暴露两个稳定对象：

```rust
pub struct ControlStyleQuery {
    pub role: WidgetRole,
    pub classes: ControlClassSet,
    pub pseudo_states: ControlPseudoStateSet,
}

pub struct ResolvedControlStyle {
    pub background: ControlBrush,
    pub border: ControlBorder,
    pub foreground: UiColor,
    pub icon_tint: UiColor,
    pub opacity: f32,
    pub content_offset: UiOffset,
}
```

调用方不接触规则排序、token 解析、fallback、图片注册或 cache。删除该模块后，这些复杂性会重新散落到
所有 panel renderer，因此该模块具有真实深度和 locality。

## 7. EditorStyleSheet.v1

样式必须来自版本化、可校验的数据，不把每个状态颜色和图片路径硬编码在 panel Rust 中。

```yaml
schemaVersion: editor-style-sheet.v1
sheetId: aife-dark-neutral-v1
tokens:
  color.controlBg: "#2f3136ff"
  color.controlHover: "#41454cff"
rules:
  - selector:
      role: Button
      classes: [toolbar-control]
      pseudo: []
    declarations:
      background: { colorToken: color.controlBg }
      border: { colorToken: color.controlBorder, width: 1 }
  - selector:
      role: Button
      classes: [toolbar-control]
      pseudo: [hover]
    declarations:
      background: { colorToken: color.controlHover }
  - selector:
      role: Tab
      classes: [workspace-tab]
      pseudo: [selected, hover]
    declarations:
      background: { texture: tab-selected-hover, slice: [4, 4, 4, 4] }
```

正式资源可以使用 repo 惯例选择 JSON；上面的 YAML 仅用于解释。施工时 schema、canonical serialization
和版本升级规则必须唯一，不能同时维护两种格式真相。

## 8. 受控选择器

v1 选择器只允许：

```text
role：Button / IconButton / Tab / Toggle
classes：稳定语义 class 的 all-of 集合
pseudo：必须存在的伪状态集合
pseudoNot：必须不存在的伪状态集合
```

禁止 path、文本内容、HitTarget payload、panel_id 和任意祖先遍历成为选择条件。`Game` 页签使用
`role=Tab + class=workspace-tab`，不得使用 `panel_id=game_view` 特例。

规则匹配只针对一个 widget，复杂度有稳定上界。不存在递归 selector、运行时正则或字符串脚本。

## 9. 确定性级联

规则优先级从低到高：

```text
engine base sheet
engine active theme sheet
trusted editor extension sheet（v1 可保留 schema，默认不开启加载）
widget declaration override（仅测试/迁移，产品调用点禁止常态使用）
```

同一 origin 内按以下 tuple 排序：

```text
(pseudo 条件数量, class 条件数量, role 是否指定, canonical rule index)
```

后者只覆盖自己显式声明的 property。相同输入、sheet 和资源 registry 必须产生 byte-identical
`ResolvedControlStyle`。所有冲突和 fallback 都能在 Trace 中报告 winning rule，不依赖 HashMap 迭代顺序。

## 10. 伪状态合同

`ControlPseudoStateSet` 是 flags，不是互斥枚举：

```text
hover          指针当前位于有效控件内
active         主指针/键盘正在按住该控件，且当前仍满足激活候选条件
selected       Tab/List choice 等单选模型的持续状态
checked        Toggle/Checkbox 等布尔模型的持续状态
disabled       控件不可执行
focus          控件拥有键盘焦点
focus-visible  焦点来自键盘导航或需要可见焦点提示
```

合法组合例如 `selected + hover`、`checked + focus-visible`。Resolver 不使用一条硬编码优先级把组合压扁，
而是按规则级联组合 property。`disabled` 必须禁止 active 和业务激活；主题仍可声明
`disabled + selected` 的可辨识视觉。

## 11. 指针与激活语义

控件声明增加显式策略：

```text
ReleaseInside（默认）：按下时 capture，内部释放时 Activate
Press：按下时立即 Activate，仅用于菜单等经审查特例
```

默认状态机：

```text
PointerMove into       -> hover
PointerDown inside     -> capture + active
PointerMove outside    -> 保留 capture，清除 active visual
PointerMove back       -> 恢复 active visual
PointerUp inside       -> Activate exactly once，清除 capture/active
PointerUp outside      -> Cancel，不 Activate
FocusLost/Cancel       -> 清除 capture/active，不 Activate
```

当前 `EditorInputRouter` 的 PointerDown 激活不能继续作为普通按钮默认真相。施工必须迁移为 release-inside，
同时给确实依赖 down 的控件显式 `Press`，并用 consumer 矩阵防止命令双发或丢失。

键盘语义：Space/Enter 应产生可见 active/focus-visible，并 exactly-once Activate；disabled 不激活。

## 12. 持续选中态与瞬时按下态

```text
active：瞬时输入状态，松开或取消后消失
selected/checked：业务或 workspace model 状态，直到模型改变
```

普通 Button 点击后不保留 active，也不增加 100ms 定时闪烁。Tab 点击后由
`WorkspaceTopology.active_panel_id` 产生 selected，因此 `Game` 页签能够持续显示为当前页签。Toggle 由其
布尔 model 产生 checked。Renderer 不自行修改 selected/checked。

## 13. 视觉声明与状态图片

`ControlBrush` 至少支持：

```text
None
Solid(color token)
Texture(textureId, tint, fit)
NineSlice(textureId, left/top/right/bottom, tint)
```

状态图片属于主题视觉资源，不属于输入状态。不同伪状态规则可以引用不同 textureId，也可以只改变颜色、
边框或图标 tint。默认 DarkNeutral 主题优先使用轻量颜色/边框反馈；只有主题确实提供图片时才走图片，
不能要求每个按钮都复制 normal/hover/pressed 三张位图。

`content_offset` 可用于按下时 1px 视觉位移，但不得改变布局 rect、hit rect 或相邻控件位置。伪状态规则
不得修改 width/height/margin/font size 等布局属性，避免 hover 引起抖动。

## 14. 图片资源与安全

Editor theme texture 使用 engine-owned、hash-pinned 资源 registry：

```text
style sheet textureId
  -> EditorThemeTextureRegistry
  -> decoded/uploaded texture identity
  -> DrawCommand image brush
  -> ordered ImageTextures painter batch
```

不得把任意文件路径放进样式表，也不得让 Native Editor 运行时扫描项目目录寻找按钮皮肤。资源缺失或
digest 错误时输出 typed diagnostic，并回退到规则中明确声明的 fallback color；UI 必须保持可操作，不能
因为装饰图片失败而白屏。

## 15. Retained Widget Tree 集成

`EditorWidgetDeclaration` 增加稳定的 `control_classes`、model pseudo state 和 activation policy。
hover/active/focus 由现有 `EditorFocusInputSystem` 产生，selected/checked/disabled 由 model/declaration
产生。Reconcile 后形成单一 effective state，再交给 style module。

panel renderer 只声明语义：

```text
WidgetRole::Tab
class = workspace-tab
selected = panel_id == active_panel_id
```

panel renderer 禁止读取 `hovered_hit_id`、`pressed_hit_id` 决定颜色。现有 Toolbar 手写分支在迁移完成后
删除，不能保留两套视觉真相。

## 16. 缓存、失效与性能

Resolver cache key：

```text
style_sheet_generation
role
canonical class set
pseudo-state bits
```

WidgetId、label、panel_id 和绝对 rect 不进入样式 cache。hover/active 改变只标记旧、新目标的 paint dirty；
selected/checked 改变只标记对应 model widget。样式表或主题 generation 改变才整体失效。

v1 性能资格必须记录：

```text
规则数量
unique cache key 数量
cache hit/miss
resolve duration
hover frame redraw/present duration
texture upload 是否只发生一次
```

不得在每帧为每个 widget 解析 JSON、分配字符串集合或遍历无限规则。

## 17. Report 与 diagnostics

遵守 Off / Summary / Trace：

```text
Off：不构造样式解释字符串
Summary：sheet id/generation、rule count、cache metrics、fallback count
Trace：指定 widget 的 query、matched rules、winning declarations、resolved style
```

建议 diagnostic code：

```text
editor_style.schema_unsupported
editor_style.selector_invalid
editor_style.token_missing
editor_style.texture_missing
editor_style.texture_digest_mismatch
editor_style.nine_slice_invalid
editor_style.property_not_allowed_in_pseudo_state
editor_style.no_matching_base_rule
editor_style.activation_policy_invalid
```

AI 可以通过 WidgetId 请求 Trace，但样式规则本身不能以 WidgetId/path 做选择，从而避免调试接口反向成为
特例入口。

## 18. 迁移范围

v1 正式 consumer：

```text
Toolbar Button / IconButton / overflow item
Workspace Dock Tab（包含 Game）
Workspace panel chrome buttons
Project Launcher primary/secondary buttons
常用确认/取消/信任决策按钮
Toggle（若现有正式 Native Editor consumer 可达）
```

迁移必须先建立统一 resolver 与输入状态机，再按 consumer 矩阵删除手写状态绘制。未迁移控件必须在
报告中显式列出，不得宣称通用化完成。

## 19. 验收矩阵

### 19.1 Resolver

```text
normal / hover / active / disabled
selected / selected+hover / selected+active
checked / checked+focus-visible
规则级联与 property merge 确定
规则顺序变化仅在同 specificity 冲突时按 canonical index 生效
缺 token/texture/Nine-Slice 非法产生稳定 diagnostic 与 fallback
```

### 19.2 输入状态机

```text
down -> active 可见，尚未 Activate
up inside -> exactly-once Activate
drag outside -> active 清除；up outside -> Cancel
drag out/back -> active 恢复；up inside -> exactly-once Activate
FocusLost/capture lost/Escape -> 无 stuck pressed
Press policy -> down exactly-once Activate，up 不重复
disabled -> 不 active、不 Activate，保留既有 disabled feedback
键盘 Space/Enter -> focus-visible + exactly-once Activate
```

### 19.3 视觉

```text
Game Tab：normal、hover、active、selected、selected+hover 五态截图可辨
普通 Button：normal、hover、active、disabled 四态截图可辨
配置状态图片时实际 texture pixels 可辨，不用 fallback rect 冒充
无图片主题时颜色/边框 fallback 可辨
100% / 125% / 150% / 200% DPI 无布局位移、裁切和文字遮挡
```

### 19.4 通用性

```text
同一 workspace-tab 规则自动应用 Scene/Game/Timeline/Animator 等页签
至少三个不同 panel 的 Button 使用同一 resolver
源码不存在 panel_id == game_view 的视觉分支
Toolbar 旧手写 hover/pressed 逻辑删除
```

### 19.5 回归

```text
Dock tab 激活、拖拽和跨窗口 docking 不回归
Toolbar Play/Pause/Step/Stop 不双发
Trust/Confirm/Cancel 不因 release 语义丢命令
disabled feedback 与中文本地化不回归
ordered painter batch 不破坏文字、背景和状态图片顺序
真实 Windows WGPU production composition 通过
```

## 20. 建议施工窗口

本文不是施工文档。后续独立施工文档建议拆为：

```text
Window 1 / Gate A：schema、selector、pseudo-state、deterministic cascade 与 resolver
Window 2 / Gate B：theme tokens、brush/Nine-Slice、texture registry 与 ordered painter integration
Window 3 / Gate C：pointer/keyboard state machine、capture、ReleaseInside/Press exactly-once
Window 4 / Gate D：Toolbar、Workspace Tab、panel chrome consumer migration
Window 5 / Gate E：Launcher/decision buttons、fallback/diagnostic、未迁移矩阵
Window 6 / Gate F：受影响回归、multi-DPI production visual matrix、文档闭环
```

施工不得直接从 Gate D 的 `Game` 页签开始。Gate A-C 是通用能力前置，否则仍会退化成局部补丁。

## 21. 风险与控制

### 风险 1：长期方案演变成 CSS 引擎

控制：选择器仅 role/classes/pseudo/pseudoNot；无树关系、任意属性、脚本和表达式；v1 只覆盖 control
paint，不接管 layout。

### 风险 2：PointerDown 到 ReleaseInside 迁移造成行为回归

控制：先建立 activation policy 和 exactly-once 状态机，再做 consumer 清单；需要 down 的菜单显式 Press，
不得依靠历史偶然行为。

### 风险 3：伪状态组合爆炸

控制：flags + property cascade，不建立所有组合的巨大枚举；主题只声明需要覆盖的差异。

### 风险 4：状态图片增加 GPU 和资源复杂度

控制：颜色/边框是默认路径；图片可选且 hash-pinned，Nine-Slice 复用，单次上传，缺失有可操作 fallback。

### 风险 5：样式改变布局导致跳动

控制：伪状态 declaration 禁止布局属性；content offset 只影响内容绘制，不影响 layout/hit rect。

### 风险 6：新旧绘制真相并存

控制：consumer 迁移完成即删除局部状态分支；以源码 guard 和未迁移矩阵阻止重新引入。

## 22. 方案自审

### 22.1 是否满足用户确认的方案 C

是。方案建立声明式 Style/Pseudo-State、受控 selector、确定性 cascade、状态图片、统一输入状态机和
多 consumer 迁移，不是只修 Dock Tab。

### 22.2 是否过度设计

没有复制完整 CSS/USS。v1 明确删除树选择器、脚本、动画和 layout style，只保留解决 Editor 控件交互
视觉所需的最小长期结构。

### 22.3 是否为深模块

是。调用方只提交 `ControlStyleQuery` 并取得 `ResolvedControlStyle`；匹配、级联、fallback、cache 和
diagnostic 全部收在一个实现内，测试通过同一 interface 验证。

### 22.4 是否项目无关

是。规则只识别 WidgetRole、语义 class 和通用 pseudo state，不识别塔防、Game 页签 id、panel payload
或项目路径。

### 22.5 是否保持现有架构真相

是。复用 retained widget tree、EditorFocusInputSystem、UiDrawList、ordered painter batches 和 WGPU
图片通道；不修改 Project Runtime AUI、RuntimePackage 或游戏引擎玩法层。

### 22.6 AI 适配性

高。schema、selector、specificity、state bits、winning rule、resolved style 和 diagnostic 都是结构化、
确定且可追踪的；AI 修改主题时无需扫描各 panel Rust 分支，也能用 Trace 解释某个控件为何呈现当前状态。

### 22.7 自审结论

```text
方案结论：通过
与 127 / 257 / 258 / 266 冲突：无；272 深化 127 未通用化的视觉状态合同
需要修改塔防项目：否
施工文档：已完成并归档
当前允许施工：否；272 已结束，不得从归档继续施工
```
