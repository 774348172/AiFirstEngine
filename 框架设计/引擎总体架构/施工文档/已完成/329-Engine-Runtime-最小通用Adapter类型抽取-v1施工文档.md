# 329 Engine Runtime 最小通用 Adapter 类型抽取 v1 施工文档

状态：已激活。用户授权生成后直接开始施工。

Gate 1 进度：已完成引用扫描与 Engine 回归。扫描证据：`G:/gameEngin/.artifacts/329-gate1-reference-scan.txt`；`cargo test -p engine_runtime --lib --locked`：577 passed。现有 Engine 自有 `ProjectRuntimeModuleDescriptor` 可复用，但规则描述、Session/UI/Observation 输出和 SDK Error 仍是 Project SDK 高层类型，尚未替换。

## Gate 1：最小类型与引用盘点

在 `engine_runtime/src/project_runtime_native_adapter.rs` 盘点四类类型的字段、构造位置和错误映射，确认复杂 Project SDK 类型仍只作为 ABI 字节输入输出。先建立 owner 类型和定向测试，不改玩法。

## Gate 2：内部 Adapter 实现

在 `engine_runtime` 内定义并替换 `NativeModuleDescriptor`、`NativeRuleDescriptor`、`NativeCallStatus`、`NativeAdapterError`；保持 `project_runtime_abi` layout 与 descriptor digest。清理生产代码对 Project SDK 的直接类型引用。

## Gate 3：依赖与消费者回归

运行 `cargo metadata --locked` 反向依赖断言、受影响 crate 默认/all-features 测试、两个样例编译和 ABI/descriptor 负例测试。任何项目语义变化立即停止。

## Gate 4：真实项目验收

执行 complex shooter build、delivery verify、playtest/observe，记录 artifact/revision/delivery identity 和阶段耗时；不自动替换安装 Provider 或修改真实配置。

## 停止条件

需要扩大通用类型、迁移规则/AUI/观测、改变公开 ABI 或引入动态加载时停止并记录原因。

## Gate 3 基线结果（2026-09-12）

Gate 2 已完成：模块描述、规则描述、调用状态和适配器错误均由 Engine 内部类型承载；`cargo test -p engine_runtime --lib --locked` 为 578 passed。

依赖扫描证据：`G:/gameEngin/.artifacts/329-gate3-reference-scan.txt`。生产代码剩余 `project_runtime_sdk` 引用集中在 JSON/ABI 边界的 Session、Observation、UI、Value 与统一 SDK 错误返回；继续抽取将迁移 Project 语义，属于停止条件。`runtime_player_winit` 的 `project_game_sdk` 引用仅位于测试模块。
