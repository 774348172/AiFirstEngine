# 327 Player 冷编译分层缓存与失效边界收敛施工文档

状态：已激活。327方案已确认，A分解完成；用户授权继续A+B组合。

## A 终态证据

真实 staging 报告：`G:/gameEngin/.artifacts/a-a7573c98c243025deb0b42de/project-runtime-player-build-report.json`。最大阶段为 Host 编译 42103ms，其次项目 tests compile 22282ms；格式 2831ms、锁定 539ms、描述 905ms。报告为 tower 项目同一 Player 生成链，作为阶段结构证据，不替代 complex shooter 冷构建资格。

## B Gate

当前进度：Gate 1 完成，已将 `compile_tests` 加入既有 Player build request；默认 true，Provider 普通开发 build 透传 false。`project_authoring_execution` lib 62 项通过，rustfmt/diff 检查通过。完整集成测试需继续确认 skip 分支的实际 report，尚未宣称节省的冷编译时间。

Gate 2 结果：停止独立 Host 二进制缓存实现。依赖图证明生成 Host 的 `main.rs` 静态链接 Project Runtime，Project Runtime 直接依赖 `runtime_cli`/`runtime_player_winit`，后者又直接依赖 `engine_runtime`/`engine_input`；因此 Host 不能在项目逻辑变化时作为稳定独立二进制复用。强行拆分会冻结错误的 Project Module 或引入未授权动态 ABI。保留 Cargo CompileWorkspace 增量复用作为安全分层边界。

Gate 3：普通 build 跳过测试编译的源码实现与owner测试已通过；完整集成单独运行3/3通过，耗时107.09秒。由于 Gate 2 的安全终止条件成立，327 不伪称“Host/Project Module 分层完成”，当前施工保留为部分完成，后续需另行设计可接受的静态链接缓存或明确放弃该拆分。

1. 先在 `project_player_artifact` owner 内定义稳定 Host/依赖层与项目模块层的失效矩阵，禁止先改公共工具面。
2. 让普通 development build 可跳过 `cargo test --no-run`，将测试编译保留给显式测试/verify路径；保留失败诊断。
3. 验证冷构建、逻辑修改、资源修改、SDK/toolchain变化四类 fresh/rebuilt 与 artifact identity。
4. 运行 project_authoring_execution 与 engine_tool_provider 受影响测试，随后做一次真实 complex shooter build/verify 对照。

## 终止条件

若稳定 Host 与项目模块无法在当前 Cargo/运行时 ABI 下安全分层，停止实现并保留 A 证据；不以新增动态库、脚本运行时或工具数量换取时间。
