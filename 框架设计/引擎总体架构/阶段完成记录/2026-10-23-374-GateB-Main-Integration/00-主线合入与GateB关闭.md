# 374 Gate B：主线合入与关闭

状态：`main_source_integration_passed`，仅关闭Gate B。Gate A/B已通过，Gate B已串行合入主线并关闭；本窗260项组合测试通过，31项后端证据按186文件身份复用，16套真实GPU像素通过，scoped Clippy增改行无告警、25文件fmt通过。371阶段42的4GiB图形上限及其它既有成果保留。C–F未启动，374整体仍partial，未安装、游戏固定V0.0.5。

## 授权、必要性与串行交接

用户直接授权“登记共享文件交接→按改动合入并保留371成果→补受影响组合验证→关闭B”。本窗2026-10-23 11:23:54 UTC开始；14:08:54停止新增、14:23:54硬截止，实际收尾见closeout.json。

最小改动是前窗封存的36个外层增改与186个后端，合计222文件，不复制旧外层workspace。54登记本窗B负责这次串行发布，其余371 owner归A。371阶段42在途Release期间守卫拒绝发布；A Release于11:29:50 UTC完成，11:30:21 UTC实际发布，发布前进程记录无Cargo/rustc源码消费者，未停止A进程。脚本路径/行尾比较的预检错误在产品写入前修正；没有产品测试失败或降低断言。

35个B外层文件的主线SHA与B基线一致；唯一重叠是geometry.rs。以A42 before-source.zip核验确切基线，保留A的2→4GiB常量和对应说明，只加入B的PMA shader/blend改动。实际三方差异见geometry-three-way.diff。其余956个主线文件逐一验证未覆盖，包含loader/streaming、Windows路径修复、Player和预算配置。前态在before-main.zip，逐项前后SHA在publication.json。没有写Git索引、重置、提交、安装或配置更改。

## 出口、验证与成本

原Compiler.check/唯一Assembler→原纹理Cook派生linear PMA页→原缓存/AssetGraph→密封运行包及依赖闭包→无源目录实际headless Player→原Projection/native中性几何链路已在真实主线组合验证。覆盖旧Animator2D/341共存、增量Cook、包/摘要/路径篡改及缺能力拒绝。

runtime组合为233+7项，额外覆盖原loader生命周期与7个asset_memory直接消费者；native组合包含5项Spine与1项4GiB准入/超限/零值拒绝。默认配置一次覆盖缺Spine拒绝和预算总量/process限制。实际features/profile、输入源摘要、命令、stdout/stderr与首错完整保留在各*-inputs/result.json和日志；所有终态退出0、失败0、ignored0、源码漂移0。

| 验证 | 通过项数 | 总调用秒 | 测试执行秒 | 证据 |
| --- | ---: | ---: | ---: | --- |
| project | 12 | 227.266 | 63.570 | `project-result.json` |
| runtime | 240 | 201.047 | 10.600 | `runtime-result.json` |
| gpu | 6 | 107.719 | 9.690 | `gpu-result.json` |
| default | 2 | 122.375 | 0.020 | `default-result.json` |
| clippy-feature | check | 34.485 | 0.000 | `clippy-feature-result.json` |
| clippy-gpu | check | 22.500 | 0.000 | `clippy-gpu-result.json` |
| fmt | check | 0.594 | 0.000 | `fmt-result.json` |

总调用秒包括编译与启动，不冒称全部为编译或Player耗时。Cargo原始日志保留独立编译时长。没有重复运行已被同配置suite覆盖的case，没有完整workspace、Release性能、Android或安装验证。

16套1280×720 GPU像素：非边缘最大误差1/255，非边缘超差0，最大总超差比例0.01953125%；原2/255、1px边缘及0.1%阈值保持。覆盖四blend×straight/PMA、半透明底/tint、多页透明顺序、旋转atlas、PPU/TRS、skin/attachment、clipping和weighted linked deform/bounds。图与逐case JSON在gpu-evidence，汇总gpu-summary.json。

Provider+Host真实窗口+Compiler/spine项目，以及native real-wgpu scoped Clippy均退出0；原告警保留，增改行告警0、新suppression0（lint-audit.json）。25文件rustfmt通过。官方后端186文件完全同身份，复用31项CPU测试和既有FFI限制，不重新授予sanitizer资格。

## 身份、复用与剩余范围

当前主线1183文件源seal：`ce380de101d3095d24828e3aad6cf9db1ed533ecc5e2092e49cf5484594958da`；逐文件身份和seal算法见source-identities.json。此前交接ZIP SHA256 `0fe35aded93fe4b5fa9bede6cc598753ed1028d1940157e5225c43cf00584967`保持。当前是dirty主线源码集成证据，没有新commit或已安装能力声明。后端31项仅在这186文件及其输入不变时复用；后续共享源码变化只刷新实际受影响消费者。

Gate B达到总施工§7出口并关闭。总施工保留当前目录；C的控制/tick/混合/mask/事件、D作者/SDK、E独立交付和F正式配方尚未启动。374旧独立目录保留隔离历史快照；后续C激活前须从当前主线重新核对基线及371共享归属，不能直接以旧B快照覆盖。371阶段42及其资格由A路维护，本记录不关闭或改写A。游戏V0.0.5与安装态保持。

本窗所有自有验证进程已结束；closeout记录实际耗时及进程状态，未取消共享服务或其它路进程。测试证据保留在`.artifacts/374/gate-b-main-integration-20261023-112354`。

## 实际命令

每次在同一shell加载07-Tools/Enter-EngineEnvironment.ps1 -Lane 374，以主线真实rust目录为cwd，使用374独立target。SPINE374_EVIDENCE_ROOT分别指向本窗独立子目录；Rust1.96.0/MSVC14.44.35207。命令如下：

```text
cargo test --offline --locked -j1 -p project_authoring_execution --features spine --lib --test spine374_project -- animator2d runtime_package spine374_b_ project_player_artifact_cache_identity --test-threads=1
```

```text
cargo test --offline --locked -j1 -p engine_runtime --features spine --lib --test platform_3d_foundation -- animator2d runtime_package render_geometry render_contract runtime_asset_loader asset_memory --test-threads=1
```

```text
cargo test --offline --locked -j1 -p engine_render_native --features real-wgpu --test spine374_pixels --test native_boundary -- spine374_b_ windows_four_gib_visual_resident_limit --include-ignored --test-threads=1
```

```text
cargo test --offline --locked -j1 -p project_authoring_execution --test spine374_project --test platform_3d_assets -- spine374_b_default_compiler_and_player_reject_required_backend measurement_memory_four_gib_resident --test-threads=1
```

```text
cargo clippy --offline --locked -j1 -p engine_tool_provider -p engine_runtime_host -p project_authoring_execution --features engine_tool_provider/spine,engine_runtime_host/spine,project_authoring_execution/spine --lib --test spine374_project --message-format=json
```

```text
cargo clippy --offline --locked -j1 -p engine_render_native --features real-wgpu --test spine374_pixels --test native_boundary --message-format=json
```

```text
rustfmt --check --edition 2021 --config skip_children=true D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\engine_render_native\src\wgpu_backend\real\geometry.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\engine_render_native\tests\spine374_pixels.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\engine_runtime\src\unit_composition.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\engine_runtime_core\src\aui_present_cache.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\engine_runtime_core\src\lib.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\engine_runtime_core\src\project_logic.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\engine_runtime_core\src\render_contract\cooked.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\engine_runtime_core\src\render_geometry.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\engine_runtime_core\src\render_geometry\build_inputs.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\engine_runtime_core\src\render_geometry\commands.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\engine_runtime_core\src\render_geometry\shadow\planning.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\engine_runtime_core\src\render_projection_adapter.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\engine_runtime_core\src\render_projection_adapter\spine.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\engine_runtime_core\src\runtime_audio_tests.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\engine_runtime_core\src\runtime_instance_loader.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\engine_runtime_core\src\runtime_package.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\engine_runtime_core\src\runtime_package_builder.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\engine_runtime_core\src\runtime_scene_hydration.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\engine_runtime_core\src\spine_animation.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\project_authoring_execution\src\game_project_compiler\check.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\project_authoring_execution\src\neutral_assembler.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\project_authoring_execution\src\project_player_artifact.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\project_authoring_execution\src\project_player_artifact\engine_player.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\project_authoring_execution\src\spine_import.rs D:\GameEngine\EngineMigration\EngineMigration\01-Engine\main\rust\crates\project_authoring_execution\tests\spine374_project.rs
```
