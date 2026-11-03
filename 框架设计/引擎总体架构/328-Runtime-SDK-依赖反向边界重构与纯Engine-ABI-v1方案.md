# 328 Runtime SDK 依赖反向边界重构与纯 Engine ABI v1

状态：正式方案已确认，施工文档待执行。

目标：解除 `engine_runtime`、`runtime_player_winit` 对 Project SDK 的反向 Cargo 依赖，使 Engine Runtime 能形成稳定静态层。保持单一静态 `Game.exe`、RuntimePackage、ProjectRuntimeAbi 语义和公共 Engine Tool 面不变。

```text
engine_runtime_abi
        ↑
engine_runtime ← runtime_player_winit ← runtime_cli
        ↑
Project Runtime / Generated Glue → project_runtime_sdk / project_game_sdk
```

Engine ABI 只承载通用帧、Runtime Value、Component 访问、descriptor/capability、handle、diagnostics 和 session 合同。项目玩法和 AUI 语义不进入 Engine ABI。

范围：盘点并迁移 Engine Runtime 对 Project SDK 的直接类型引用；让 Player 运行时只依赖通用 ABI；保持 Generated Runtime Glue 为 Compiler 生成的 Adapter；更新 Cargo manifest、消费者和测试。

不做：DLL、动态加载、脚本 VM、新 Provider/Tool、玩法迁移、RuntimePackage schema 修改、项目逻辑拥有权改变。

约束：Engine ABI 不得依赖 Project SDK，跨层类型必须为通用 ABI 类型。成功标准是 metadata 依赖闭包不再包含 Project SDK，受影响 workspace、样例和 complex shooter build/verify 通过。若需要玩法进入 Core、动态 ABI 或破坏 ProjectRuntimeAbi，立即停止并回退。
