# 329 Engine Runtime 最小通用 Adapter 类型抽取 v1

状态：正式方案已确认，施工中。采用 328 讨论方案 B。

## 目标

解除 `engine_runtime`、`runtime_player_winit` 对 Project SDK 高层类型的直接 Cargo 依赖，只在 Engine 内建立少量 Native Adapter 类型。保持现有 `project_runtime_abi` 公共合同、RuntimePackage、ProjectRuntimeAbi、单一静态 Player 和项目逻辑拥有权不变。

## 方案边界

Engine 内部仅抽取 `NativeModuleDescriptor`、`NativeRuleDescriptor`、`NativeCallStatus`、`NativeAdapterError` 四类适配类型。复杂规则请求、AUI、观测、UI 状态和 JSON 编解码继续由 `project_runtime_sdk`/Project Runtime 持有，通过已有 ABI 字节合同传递。

不新增 crate，不修改 Engine Tool，不引入 DLL、动态 ABI、脚本 VM，不迁移玩法语义到 Core，不整体迁移 `project_runtime_sdk`。

## 实现原则

- Adapter 类型是 `engine_runtime` 内部类型，不是新的公共 Project SDK。
- ABI layout、schema digest、descriptor version 仍以 `project_runtime_abi` 为准。
- SDK 高层类型只在 Project Runtime/Generated Glue 和测试 fixture 中出现。
- 任何解析失败返回结构化 Engine diagnostics，不 panic、不弱化为无类型成功。

## 成功标准

`engine_runtime` 与 `runtime_player_winit` 的生产依赖闭包不再包含 `project_runtime_sdk`/`project_game_sdk`；受影响 workspace、两个样例和 complex shooter build/verify/playtest 通过；descriptor、status、error 和 artifact identity 不变。

## 回退

若四类类型不足以表达现有合同，或需要把规则/AUI/观测语义移入 Engine，立即停止并保留已完成盘点，不扩展 Adapter 面。
