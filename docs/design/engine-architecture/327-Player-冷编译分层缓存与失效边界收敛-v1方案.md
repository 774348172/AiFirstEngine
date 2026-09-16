# 327 Player 冷编译分层缓存与失效边界收敛 v1

状态：正式方案已确认（用户选择 A+B；先完成 A 耗时分解，再进入 B 施工）。

## 目标

降低项目首次 Player 构建中稳定引擎依赖和无关测试编译造成的等待，同时保持 Rust Project Runtime、RuntimePackage、结构化交付和显式验收合同不变。不引入脚本 VM、新工具或第二个 Player。

## A 阶段基线结论

已有真实 Project Player 构建报告显示：`build_project_runtime_dev_host` 42.103 秒，`validate_project_runtime_tests_compile` 22.282 秒，格式检查 2.831 秒，依赖锁定 0.539 秒，模块描述 0.905 秒。最大成本是 Host 编译，其次是每次开发构建都执行项目 tests compile。该报告来自同一生产 staging/Host 链的真实 tower 项目，不冒充 complex shooter 的独立冷编译统计；complex shooter 当前宿主仅有缓存命中证据。

## B 推荐边界

采用稳定 Host/引擎依赖与项目 Runtime Module 的分层缓存，并将开发构建的测试编译从 Player 产物构建中解耦。兼容键继续区分 project manifest/lock、toolchain、target、feature/environment；最终 artifact 仍绑定完整项目与 SDK 身份。项目逻辑、Runtime Glue 或依赖变化只失效必要层；显式 playtest/verify 继续按需运行测试和 Player。

## 不做

不把项目逻辑移入 Engine Core，不删除 Rust AOT，不修改 RuntimePackage schema，不增加 Engine tool，不承诺所有 SDK 修改零编译，不以小 fixture 时间宣称完整游戏性能。

## 风险与验证

必须证明：冷构建仍能生成正确 Player；只改项目逻辑时稳定 Host 不重编；只改资源时不触发 Rust 编译；测试目标不再阻塞普通开发 build；toolchain/feature/SDK ABI 变化会正确失效；显式 verify 与语义试玩仍通过。先用 owner 级报告和 Cargo fresh/rebuilt 计数，再做一次完整 shooter 对照。
