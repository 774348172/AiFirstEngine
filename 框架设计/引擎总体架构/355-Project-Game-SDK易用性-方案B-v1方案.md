# 355 Project Game SDK 易用性：方案 B v1

日期：2026-09-19。状态：用户已选择方案 B，并授权方案自审、施工文档自审及施工。

实施状态：源码集成完成，218项去重测试及可执行SDK示例通过；见[355完成记录](阶段完成记录/2026-09-19-355-SDK-Usability-B/00-总览.md)。默认安装仍为354，本方案不声明新版已接入。

## 1. 目标与边界

让 SDK 的合法用法容易发现、容易首次写对，并使 SDK、generated glue、Runtime 对同一能力有相同解释。继承顶层产品/架构 v1.1、310 SDK/glue、331 知识产物及353输入验收；不新增工具、Agent、DSL、Runtime owner，不改变 RuntimePackage 真相或 deferred mutation 模型。

这是中等范围的接口与跨模块契约修复，不是全 SDK 重构。施工只声明源码集成完成；不替换当前354安装版，不借此执行发布资格化、全workspace回归或安装接入。

## 2. 调查依据

- Godot 本地4.8-dev：`I:/godotcode/godot-master/godot-master/scene/main/node.cpp` 的回调检测与处理启用；`core/input/input.cpp` 的 pressed/just_pressed及物理帧语义；`scene/gui/base_button.cpp` 的鼠标/ui_accept汇流。
- Unity：本地4.3.1历史 `I:/Unity4.3.1f1/Runtime/Mono/MonoBehaviour.cpp` 仅将存在的方法加入更新管理器；不据此宣称现代native实现一致。uGUI2.5 `I:/com.unity.ugui/com.unity.ugui/Runtime/UGUI/UI/Core/Button.cs` 的 OnPointerClick/OnSubmit -> Press -> onClick。
- UE 本地Build.version标识5.8.0：`I:/UECode/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Engine/Private/Actor.cpp` 的tick能力注册；Slate SButton与UMG Button的输入汇流；EnhancedInput `InputTriggers.cpp` 的Down持续、Pressed按下沿。
- 联网复核 Godot Input、Unity Input System Actions官方文档；GitHub请求超时、Epic文档403，本轮UE结论依据本地源码。
- 本项目349 E05、353缺FixedUpdate失败证据；SDK capabilities / generated glue均允许缺省回调，而native adapter将其视为required call。Rule上下文可访问媒体writer，但Runtime拒绝该阶段媒体命令。输入Down/Pressed语义与UE不同；AUI业务分别接Click/Submit。

只学习能力驱动调度、清晰语义与完整行为封装；不照搬反射、继承式对象模型或编辑器依赖。

## 3. 设计

### 3.1 可选回调

Native adapter在会话未故障且可选回调缺省时返回NoOp；固定更新、AUI、lifecycle保持一致。终态故障不得被缺省回调掩盖。capability和函数指针不匹配仍由现有加载检查拒绝。observe维持空观察语义。UI state是有具体binding需求的消费端，不能用缺省空值掩盖缺少producer；维持现有需求失败语义并在使用说明明确。

不注册FixedUpdate的合法最小项目应可运行，不再要求空函数。兼容已有显式空函数。

### 3.2 阶段受限接口

新增推荐的 `.rule_typed(id, handler)` 与 `RuleContext`。RuleContext只暴露WorldRead、诊断、Rule合法的transform/component/prefab/despawn命令；不提供媒体writer、HostControl、原始writer或Deref逃逸。使用独立的受限writer view，不复制提交队列。仍经同一erased adapter与generated glue提交。

原 `.rule` / GameCallContext 保留兼容，Session回调仍使用现有GameCallContext执行音频、动画、粒子和正常退出；不强迁移所有项目。typed与legacy规则共享id唯一性、capability、descriptor和dispatch。原始RuleOutput仍属于底层兼容出口，故Runtime校验继续保留，类型限制不是安全沙箱。

不自动跨帧搬运媒体、不改变Applied/NoOp/Rejected提交语义。通过正向示例与compile_fail验证限制，不引入全套泛型阶段框架。

### 3.3 输入明确语义

InputTriggerPreset新增明确的Rust构造辅助方法与JSON反序列化别名：JustPressed -> Down、WhilePressed -> Pressed、JustReleased -> Released。旧序列化名称和含义保持，默认仍Pressed；不增加wire enum变体。

InputAction增加类型化phase查询和just_pressed/just_released辅助，保留is_pressed。未知/缺省phase返回None而非猜测。它们只解释收到的action，不伪装全局持续状态快照，不从Down事件合成held。

### 3.4 AUI统一激活

新增AuiActionEvent::Activate及构造辅助，用一个业务绑定消费Click或Submit命令，低层输入、焦点、禁用、消费规则保持。AuiActionMapper按每条实际command处理：同一actionId若有对应的显式Click/Submit绑定，则该显式绑定优先于Activate，避免迁移时双触发；不同actionId可并存；独立两次操作不能被全帧去重吞掉。Activate输出事件为Activate，旧绑定输出保持不变。

同步所有穷尽match、资产校验/映射和直接consumer。新事件仅能由包含355的工具链使用；老项目不强迁移，旧安装不宣称支持新资产。只增加必要事件，不升级整个RuntimePackage协议。

施工复核：新增禁用按钮验收发现既有键盘Submit会向仍持焦点的disabled节点提交。修复限定在键盘/手柄Submit生成处，复用现有control_node_status检查可见、interactable及modal scope；不改变焦点保留策略，不由mapper重新实现输入规则。该修复是“禁用按钮不能激活”的直接必要条件。

### 3.5 可执行知识

SDK README、符号元数据和可运行Rust示例共同说明：最小注册、typed rule、持续移动/一次触发、合法媒体提交、Applied提交，以及AUI Activate资产用法。示例随Cargo验证，沿用既有知识产物入口，不另建Wiki或服务。

## 4. 验收与兼容

- Native真实适配器边界：缺省回调NoOp、之后仍可执行合法回调；终态故障保持；能力与入口不匹配拒绝。
- SDK：typed规则登记/重复id/能力/擦除分发/实际mutation；媒体接口compile_fail；legacy兼容。
- 输入：新别名与旧值相同，Down不产生held、WhilePressed持续、release正确；未知phase不误判。
- AUI：鼠标与键盘分别激活一次、显式旧绑定优先、禁用/焦点/消费保持、不同操作不被吞掉。
- 使用353既有回放与UI消费测试覆盖输入边界，执行受影响直接消费者及generated glue测试；不声称测试证明真实OS设备、试听或安装版。
- 原项目通过受影响编译/测试作兼容检查，不修改玩法，不要求重新人工验收349。

## 5. 方案自审

2026-09-19 自审通过。已复核SDK注册、glue能力位、native适配器、AUI mapper、输入预设与直接消费者。

- 修正潜在过度承诺：缺省UI producer不能掩盖真实binding需求；只把合法可选会话调用视为NoOp。
- typed入口是增量兼容接口，不改变旧handler签名；限制避免误用，原始output仍由Runtime验证。
- Activate新增事件需同步editor_core模板的穷尽match；旧事件不改义，不对整帧去重。
- 输入采用serde别名而非新增运行时预设，保留旧输出；不新增输入状态owner。
- 测试为源码集成证据，不声称354安装版已更新；353真实交付证据只能证明原输入回放能力，新增行为须跑当前源码测试。
- 用户未提供本编号其它AI审查文档；已读取现有310/331边界与本轮源码调查，无需扩展正式架构。下一步可生成施工文档。
