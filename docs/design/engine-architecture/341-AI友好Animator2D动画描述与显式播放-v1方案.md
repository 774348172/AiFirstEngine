# 341 AI 友好 Animator2D 动画描述与显式播放 v1

状态：用户已确认采用方案 A，并吸收方案 C 的显式 `play` 特点；2026-09-14 Gate A/B/C 已完成归档，见 [完成记录](阶段完成记录/2026-09-14-341-AI-Friendly-Animator2D-v1/00-总览.md)。源码与新 Windows dev 交付通过，本轮未安装 Provider；338 保持暂停。

## 目标

降低 AI 创建角色动画时需要同时理解的资源和引用数量，同时保留现有 Animator2D 的 Clip、Controller、Fixed Tick 和 RuntimePackage 运行链。AI 以一个项目侧动画描述文件表达常见动画；Compiler 将其派生为现有 Clip/Controller。项目代码可在特殊时机显式请求播放某个已声明动画。

## 对现有实现的最小增量

新增项目侧 `*.animator-description-2d.json`（暂不改变 Runtime ABI）。描述包含实体绑定、`default`、动画名、Sprite 帧引用、fps 或 duration、loop，以及有限的条件规则。Compiler 在装配阶段校验并派生现有 `CookedSpriteAnimationClip2D`、`CookedAnimatorController2D` 和组件引用；派生物不成为 AI 手写输入。

规则条件只允许已声明的布尔语义参数、`!` 与 `&&`；枚举和速度方向由项目代码计算成 grounded/rising 等布尔值，不引入表达式执行器、任意函数、循环或 ECS 访问。规则选择为显式播放优先、随后首条匹配规则、最后 default。Compiler 将有界布尔组合派生为现有互斥条件过渡（最多8个参与规则的参数），不产生每帧自跳转。帧时长采用60Hz整数 tick；fps 必须可精确映射为整数 durationTicks，否则给出改用 durationTicks 的诊断，不静默降速。

实体绑定使用可选 `entity` 稳定实体 ID；动画描述必须有独立 `assetId`，派生 Controller 使用同 ID、Clip 使用其子身份。`parameters` 声明布尔初值。显式 play 对当前同名动画保持播放进度，切换名称则从首帧播放；once 完成后恢复规则，loop 持续至另一 play 或 resume。暂停冻结帧推进。运行时动态名称须在已加载 Controller 中校验，不能承诺由 Compiler 静态分析任意 Rust 字符串。

实现复核：既有 RuntimeMutationBuffer 已有动画延迟命令，但项目 SDK 尚未暴露。新增 SDK typed 动画意图及 native adapter 映射，更新已有 SDK contract digest；C ABI 布局、DLL入口、RuntimePackage registry schema 不变。这里的“不改变 Runtime ABI”指不新建 C ABI 或并行加载边界，不代表已安装旧 SDK consumer 自动具备新增命令。首次项目接入从 FixedUpdate/session mutation 通道提交，rule 回调不支持的命令必须明确报错。

## AI-facing 体验

AI 通常只需写：

```json
{"schema":"animator-description-2d.v1","assetId":"robot-animation","entity":"robot-body","default":"idle","parameters":{"moving":false,"grounded":true},"animations":{"idle":{"frames":["robot-0","robot-1"],"fps":4,"loop":true},"walk":{"frames":["robot-2","robot-3"],"fps":10,"loop":true}},"rules":[{"when":"moving && grounded","play":"walk"}]}
```

项目逻辑可调用公开项目层能力 `play("attack")` 作为一次性覆盖；该调用仍通过已编译 Animator2D 命令进入 Fixed Tick，不直接接触 Renderer、GPU 或内部实体索引。正常项目状态继续通过语义状态驱动，Skill 只给出轻量提示，不规定固定调用菜谱。

实际 SDK 入口为 `context.mutations().animator2d(&entity).play("attack")`；该名称须先在描述中声明。写入 intent 的 session callback 返回 `HandlerStatus::Applied` 才提交，`NoOp` 按既有合同丢弃；未初始化时可以返回 NoOp。帧引用须对应现有 Texture Asset ID，示例名称需按项目替换。

## 诊断与验证

Compiler 对缺少帧、重复动画名、未知条件、未知播放名、无效 fps/duration 和缺少 default 返回结构化 `animator2d.description_*` 诊断，并包含 source path、实体和字段。测试/observe 在 Summary/Trace 中可选报告 `entity/controller/state/clip/frame/tick`；默认 Runtime 不写长报告。

最小验收使用激光小屋：idle 两帧、walk 两帧、jump_up/jump_down 单帧；速度和 grounded 状态切换动画；显式 `play` 覆盖一次攻击或失败动画；60Hz Fixed Tick 重复运行得到相同帧序列；Windows 截图确认真实 Sprite 替换。现有 276 Animator2D、Renderer、Runtime ABI、九项工具和项目自由调用保持不变。

## 边界与延期

不加入 Blend Tree、多层状态机、骨骼动画、Root Motion、编辑器时间轴、新 AI 工具或统一 MCP 大工具。不把所有代码驱动动画强制迁移；粒子、抖动和连续 Transform 仍可由项目代码实现。描述文件是便捷生成入口，现有显式 Clip/Controller 仍兼容。

## 方案自审

- 必要性：已确认 AI 直接写代码比手工创建多个动画资源更简单；本方案减少输入文件和引用数量。
- 最小改变：只新增 Compiler 描述解析与项目层显式播放适配，复用现有 Runtime Animator2D。
- 不扩大边界：没有新工具、ABI、daemon、强制 Skill 流程或完整动画编辑器。
- 失败可定位：所有派生引用和规则错误在 Compiler 阶段报告；播放状态在现有 Summary/Trace 可观察。
- 兼容性：旧 Clip/Controller 输入继续有效；描述文件只是新增可选来源。
- 风险：条件语法若扩展为任意表达式会重新变成脚本系统；施工必须保持有限条件白名单。

施工前置：先生成独立施工文档并自审；用户确认施工后才能修改 Compiler、项目 SDK 或激光小屋。
