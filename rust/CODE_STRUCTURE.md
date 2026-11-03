# Rust 代码结构与放置规则

2026-10-23 整理。构建入口见 [BUILD_FEEDBACK.md](BUILD_FEEDBACK.md)，产品状态和施工归属仍由 [49](../框架设计/引擎总体架构/49-当前状态与下一步入口.md) / [54](../框架设计/引擎总体架构/54-当前可自动化施工总入口.md) 定义。

## 现有 crate 分工

成员清单以workspace Cargo.toml为准；下表按职责导航，不另建目录层级或新的依赖规则。实际依赖以各 Cargo.toml 为准。

| 职责 | 成员 | 代码放置位置 |
| --- | --- | --- |
| 基础输入与资源 | engine_input、engine_content、engine_text、engine_render_resources | 输入、内容发布/摘要、字体文字、渲染资源的原 owner |
| 项目公开合同 | project_runtime_abi、engine_public_api、project_runtime_sdk、project_game_sdk | ABI、公开接口、项目运行及游戏 SDK；变化先遵循现有版本合同 |
| 内容制作与执行 | authoring_project_context、project_authoring_execution | 项目上下文、Compiler/Cook/Assembler 等原制作链 |
| 运行核心 | engine_runtime_core | World、FrameLoop、AUI、运行包与资源加载、渲染协调及共享合同 |
| 原生导航 | engine_navigation_native | 私有Recast/Detour1.6.0 FFI、64位poly ref、离线bake及运行tile/query；vendor与来源记录在crate内 |
| 原生渲染 | engine_render_native | wgpu/native 执行、GPU 资源与完成/退休生命周期 |
| 兼容装配与宿主 | engine_runtime、engine_runtime_host | 保持原公开导入路径、生产装配及动态宿主 |
| Player 与运行入口 | runtime_player_winit、runtime_player_android、runtime_cli、project_players/empty | Windows/Android 平台适配、命令行及最小项目 Player |
| 编辑器 | editor_ui_model、editor_core、editor_ui_backend_egui、editor_ui_renderer、editor_wgpu_renderer、editor_input、editor_window_winit、editor_host | 分别维护原模型、核心、UI 后端/呈现、输入、窗口与宿主 |
| 工具服务 | engine_tool_provider | 原引擎工具与 Provider 接口 |
| 开发验证 | code_hygiene、quality_gate、architecture_review、project_e2e_gate | 源码卫生、质量/架构与项目端到端验证；不混入运行时 owner |

生产运行链已有 `engine_runtime → engine_render_native → engine_runtime_core`，兼容装配也直接消费 core。新 native 实现放 native，CPU/World/AUI 逻辑放 core，平台事件与窗口放 Player；先寻找原 owner，避免把实现重新堆入兼容装配层。

## 本轮整理的模块

```text
crates/engine_runtime_core/src/
  aui.rs                         公共类型、AUI 核心及原导出入口
  aui/interaction.rs             AuiInteractionSystem 实现
  aui/layout.rs                  AuiLayoutEngine 实现
  aui/tests.rs                   原 aui::tests
  runtime_renderer.rs            渲染协调、共享 sprite/矩阵等逻辑
  runtime_renderer/ui_projection.rs  UI 投影、批次、缓存与统计
  runtime_renderer/tests.rs       原 runtime_renderer::tests
crates/runtime_player_winit/src/
  lib.rs                         Player 合同、运行协调、公开入口
  real_window.rs                 原 real_window，仍受 real-window feature 控制
  tests/mod.rs                   原 tests
  tests/session_exit.rs          原 session_exit_tests
  tests/foundation_cpu_probe.rs   原 foundation_cpu_probe_tests
```

原公共导入路径保持。渲染器原公开 UI 投影类型与函数由根模块显式重新导出；子模块仅向父作用域开放需要的内部成员。迁移测试时保持 Rust 模块名，并更新 include_str!/include_bytes! 相对于源文件的位置。

`engine_runtime/src/unit_composition.rs` 仍在测试配置下按源路径装配 core/native 私有测试。上述子模块使用明确的源路径，兼容生产 core 与测试装配。此次没有拆出独立测试 crate，也没有改变这套重型测试装配或证明增量编译提速。

## S3角色与模拟模块（2026-10-26）

| 位置 | 职责 |
| --- | --- |
| engine_runtime_core/src/character_animation.rs及同名目录 | 角色资产、采样/graph、IK/retarget/morph、模式/根运动和模拟/呈现pose；按asset、sampling、controller、solvers、session、presentation等职责拆分 |
| engine_runtime_core/src/physics_3d/ | 合同、持久Jolt session、typed控制/只读结果与cell prepare；native solver句柄不外泄到项目SDK |
| engine_runtime_core/src/navigation_3d/ | 合同、异步session/control、cell prepare及motor意图，真实bake/query委托engine_navigation_native |
| engine_render_native/src/wgpu_backend/real/geometry/skin.rs、skin.wgsl | morph后GPU蒙皮、current/previous不可变流及实际资源owner |
| project_authoring_execution/src/model_import/character.rs、navigation_cook.rs及原collision Cook | 沿普通Importer/Cook/Assembler发布character/collision/nav资产，不新增第二资产库 |
| runtime_player_winit/src/device_audio.rs | Windows/Android共用Rodio/CPAL PCM游标、gain及设备生命周期适配；旧windows_audio.rs已被替代 |
| engine_runtime/tests/platform_3d_simulation.rs及platform_3d_visual/下S3测试 | 真实模拟consumer、GPU参考与兼容闭包；普通样例在samples/platform_3d_simulation_project |

S3功能与独立Windows交付已完成，完整规模/压力及Android设备资格见371-G。Jolt本机已核对前缀在工作区05-Build/371-native，配置见07-Tools/native-build/README.md；构建输出不混入源码。

## 后续放置规则

1. 同一 owner 的实现按职责拆子模块；根文件保留合同、装配和必要导出。不要按日期或“新版本”复制第二套实现。
2. 私有单测靠近被测模块；跨 crate 消费者使用现有 tests/ 集成入口。fixtures 放原 fixtures 目录，构建产物使用指定 target，运行证据使用 .artifacts。
3. 新子模块默认私有；确有父模块消费才使用 pub(super)，公共导出按原合同显式维护。不要为拆文件扩大 SDK 或运行时权威。
4. 移动含资源引用的文件时同时核对 include_*、模块路径、feature 和 cfg；定向编译及测试名称比较后再收口。
5. 374 保持独立后端包；按 [接入边界](../框架设计/引擎总体架构/阶段完成记录/2026-10-18-371-S2-Execution-Chain-Continuation/40-本机源码基线与代码结构整理.md) 串行接原制作与呈现链，不合并它的旧外层 workspace。

本轮源码整理完成不代表新的 DLL 已构建、发布或安装，也不改变 V0.0.5 游戏消费基线。
