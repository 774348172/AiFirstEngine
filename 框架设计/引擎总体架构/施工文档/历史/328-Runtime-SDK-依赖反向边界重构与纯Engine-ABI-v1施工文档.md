# 328 Runtime SDK 依赖反向边界重构与纯 Engine ABI v1 施工文档

状态：已激活。用户授权按照施工文档开始施工 Gate 0-4。

Gate 0 终态：完成。metadata 证据保存于 `.artifacts/328-cargo-metadata.json`；直接反向依赖为 `engine_runtime -> project_runtime_abi/project_runtime_sdk`、`runtime_player_winit -> project_game_sdk`。

Gate 1 终态：停止。现有 `project_runtime_abi` 已提供 C ABI，但 `engine_runtime/src/project_runtime_native_adapter.rs` 仍直接消费 `project_runtime_sdk` 的高层 JSON/业务类型（规则描述、AUI、观测、Deferred Mutation、SDK Error 等）。这些类型不是可直接迁移的纯 Engine ABI；继续改动会把 Project 语义塞进通用层或改变现有 ProjectRuntimeAbi。Gate 2-4未执行。

## Gate 0

用 `cargo metadata --manifest-path rust/Cargo.toml --format-version 1 --locked` 和 `rg` 列出 Engine→Project SDK 类型引用及消费者，不先修改 manifest。

## Gate 1

定义项目无关 ABI，迁移 descriptor、value、frame、handle、diagnostic、session 合同；每个类型有 owner、版本和 fail-closed 测试。

## Gate 2

更新 `engine_runtime`、`runtime_player_winit`、`runtime_cli` manifest 和源码，切断 Project SDK 直接依赖；禁止 feature、trait 泛型或 re-export 重新引入反向依赖。

## Gate 3

运行受影响 crate、默认/all-features、metadata 反向依赖断言、complex shooter/tower defense 编译和 RuntimePackage/descriptor 检查。

## Gate 4

源码集成通过后执行 complex shooter build、delivery verify、playtest/observe，记录 artifact/revision/delivery identity 与阶段耗时；不替换安装 Provider 或修改真实配置。

若 ABI 无法抽出、需要项目玩法类型、出现隐式反向依赖或样例语义改变，停止施工并记录原因。
