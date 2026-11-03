# 341 AI 友好 Animator2D 动画描述与显式播放 v1 施工文档

状态：2026-09-14 Gate A/B/C 全部完成，`source_and_windows_dev_delivery_passed`，已归档。338 保持暂停、未完成、未归档。本次 Provider 未安装，宿主配置未改。

## 施工目标

验证 AI 只创建一个动画描述文件即可生成现有 Animator2D Clip/Controller，并让项目逻辑通过公开 `play("name")` 做显式播放。使用激光小屋完成 idle、walk、jump_up、jump_down 的真实 Fixed Tick 帧切换。

## 必要性与验证经济性自审

- 最小原因：现有多个 Clip/Controller 文件和引用关系对 AI 生成成本过高；需要一个 Compiler 入口，不需要新 Runtime ABI。
- 最小文件：`project_authoring_execution` 的 Animator2D 装配与测试、项目层显式播放适配、激光小屋动画描述和 Sprite 资源。
- 排除：Blend Tree、多层、骨骼、Root Motion、动画编辑器、新工具、MCP 聚合、完整项目迁移。
- 复用：现有 276 Clip/Controller、Fixed Tick、RuntimePackage、Renderer、observe 和九项独立工具。
- 估计：Gate A/B 为源码定向测试，约 5–15 分钟；Gate C 为一次激光小屋 dev 构建和短窗口截图，约 2–5 分钟。超过估算先回到 owner，不追加动画基础设施。

## Gates

### Gate A：描述文件 Compiler 派生

实现可选 `*.animator-description-2d.json` 解析，派生现有 Clip/Controller；覆盖正常 idle/walk、缺帧、未知条件、未知播放名、重复动画名和缺少 default。定向命令：

```powershell
cargo test --manifest-path rust/Cargo.toml -p project_authoring_execution animator2d --locked
```

终态证据：测试输出、派生 registry digest、结构化 `animator2d.description_*` 错误。失败时保留首个诊断。

### Gate B：显式播放与 Fixed Tick

为项目层增加最小 `play(name)` 适配，命令只能引用已声明动画并在 Fixed Tick 生效；验证显式播放优先于规则、循环帧序列确定、一次性 Clip 正确结束。复用现有 `engine_runtime` Animator2D 定向测试，不重复无关 workspace suite。

终态证据：同输入两次帧序列相同，`state/clip/frame/tick` Summary/Trace 可选字段一致。

### Gate C：激光小屋真实消费

将机器人 Transform 摆动替换为描述文件驱动的 idle/walk/jump 动画，保留项目自由逻辑和程序化视觉效果。运行既有项目测试、一次 dev 构建和窗口截图，确认实际 Sprite 帧变化；失败则区分 Compiler、Runtime Animator2D、RenderProjection owner。

终态证据：项目 RuntimePackage、帧序列断言、窗口截图和运行报告。Provider 安装只有在另行授权真实宿主验收时执行。

## 自审结论

- 范围与 341 一致，没有新增工具、C ABI 布局或强制 Skill 菜谱；已有 SDK wire enum 和 contract digest 增量，旧 consumer 需要重新构建。
- 旧 Clip/Controller 输入保持兼容，描述文件是可选便捷入口。
- 每个 Gate 都对应明确失败风险并有最小测试；不把截图当作源码通过证明。
- 施工必须在当前槽为空时重新检查代码基线、方案和测试命令，移动本文到 `当前/` 后才可执行。

## 2026-09-14 Gate A 首段施工结果

- 已在 `neutral_assembler::cook_animator2d_registry` 增加可选 `*.animator-description-2d.json` 核心派生：动画帧、fps 到 Fixed Tick duration、loop、default 和现有 Clip/Controller registry。
- 缺少 animations、空帧、非法 fps、缺少或未知 default 均返回结构化 `animator2d.description_*` 错误；旧 `.sprite-animation-clip-2d.json` 与 `.animator-controller-2d.json` 路径保持不变。
- 既有 `game_project_compiler_assembler_cooks_animator2d_from_lease_sources` 回归通过（1/1，执行 1.88s）；rustfmt 对本次文件通过。全 workspace format check 仍受既有其它文件漂移影响，未扩大格式化范围。
- 尚未完成：规则条件派生、显式项目层 `play(name)`、激光小屋真实消费和 Windows 截图。当前不宣称 341 完成。

以上是 Gate A 首段的历史进度，不再定义终态。

## 2026-09-14 终态与自审

- Gate A passed：独立 `animator_description.rs` 在唯一 Assembler 入口内存派生，3 项 owner 与 2 项集成测试通过；覆盖有限布尔首匹配、稳定身份、实体绑定、错误输入和旧 Clip/Controller 兼容。
- Gate B passed：SDK typed intent / native adapter / Fixed Tick 使用现有命令提交路径；Runtime 25 项与 Game SDK 1 项测试通过。回调提交必须返回 Applied，rule callback 明确拒绝 session intent；C ABI 和 package schema 保持。
- Gate C passed：项目 7 项测试通过；新 Provider 构建的 Windows 交付 1020 tick / 18 断言 / 8 捕获 / observe 通过。复用同一交付的受伤分支 380 tick / 8 断言 / 2 捕获通过，313 tick 为 hurt 第二帧、340 tick 恢复 idle。总计 38 项去重源码测试。
- 修复了 scenario 排序/数值类型、项目 NoOp 丢弃动画提交、测试 include_str 越出模块目录的问题；有首错及 owner 回归证据。没有放宽原有玩法/动画断言，也未扩展 staging 或 Animator2D 架构。
- 定向格式检查通过；复用已有效的 Compiler/SDK 测试，不追加全 workspace、安装态、release 或 Android 验证。新 Provider、源码文件、package/registry 身份及截图已落盘。
- 自审：符合 341 范围。测试快照只供隔离单测，源码侧与真实场景比较；运行时继续消费 RuntimePackage。截图 80 tick 已落地，下落姿态由 70 tick 实际 Sprite 断言证明，避免过度解读截图名。
- 已同步 49 / 54 / 施工与阶段记录索引，本文移至已完成。338 不自动恢复；不宣称当前宿主加载了 341。

完整结果、命令及证据见 [341 完成记录](../../阶段完成记录/2026-09-14-341-AI-Friendly-Animator2D-v1/00-总览.md)。
