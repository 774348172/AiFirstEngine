# Rule IR SDK 最小接入与像素游戏试玩

状态：2026-10-15有界接入与独立试玩交付完成，Gate A/B/C通过，见[完成记录](阶段完成记录/2026-10-15-RuleIR-SDK-Game-Delivery/00-总览.md)。不改变371主线或安装状态。

2026-10-15，用户已确认“按最小方案接入并生成试玩包”。本项为游戏接入所需的有界例外，不改变371主线目标。

## 范围与设计

项目只依赖 Project Game SDK。Compiler 从租约内的 Rules/rule-slots.json 读取显式输入组件/输出字段合同和 IR 路径，调用现有 RuleCompiler 生成 Rust AOT，纳入现有 Generated Runtime Glue。Glue 在模块初始化时安装不可变规则分发表；SDK 提供 facts → output 的纯数据接口，不传 ECS/引擎指针，不新增 ABI 回调或扩展 IR 语言。

执行器使用调用期的有界事实上下文复用已有 World/LogicContext/AOT 执行语义；它不是第二个游戏场景，不保存跨帧角色状态。v1只允许单逻辑实体、组件查询和白名单字段的标量赋值。明确拒绝事件、spawn/despawn、固定实体写入、未知组件和未声明输出；没有解释器或 Rust 策略回退。规则/合同/生成源码均进入现有 Glue generation digest，使 IR 修改能使编译缓存失效。没有规则槽的项目保持原依赖与行为。

游戏 Session 明确绑定 IR 策略，缺少分发表时创建失败。原 Game 的 Rust 默认策略仅保留为独立纯逻辑测试基线；交付 Session 不使用它。碰撞、pending、射击、动画保持原实现。新增观察项证明运行的包已启用规则。

## 必要性及验证经济性

确认缺口：正式 staging 禁止项目 build.rs 和 engine_runtime 直接依赖，实验无法原样发布。最小改动涉及 SDK rule_slots、引擎有界规则执行器、Compiler Glue 生成、像素游戏规则合同和策略绑定。保留 staging 政策，不放宽可信依赖或手工拼接发布包。延期通用事件队列、热更、复杂表达式与全面 ECS 改造。

Gate A：SDK/执行器/合同生成 owner 测试，覆盖拒绝非法输出、隔离性、生成与hash失效；只跑相关测试，不跑整workspace。

Gate B：原游戏34项回归、原IR实验、格式检查；验证缺失槽无静默回退。

Gate C：正式Compiler独立Windows Release导出，真实窗口810tick两次蘑菇拾取/开火/规则启用断言及截图；另做240FPS启动短窗。新包放F盘，不替换旧包或安装Provider。

源码和测试改动后预计owner反馈数分钟；发布首次重编译单列成本。遇到与本项无关的371并发修改或构建失败，保留首因，只修本项造成的错误。当前dirty worktree不是clean commit资格。

## 自审

已读取项目规则、195既有RuleSlot边界和正式staging实现。方案保持SDK/Compiler/Runtime职责及内部AOT生成，未将通用脚本、游戏术语或直接ECS权限引入项目API；未采用实验中的项目build.rs。用户已明确选择最小接入，测试与交付范围匹配，未重开371其它资格矩阵。
