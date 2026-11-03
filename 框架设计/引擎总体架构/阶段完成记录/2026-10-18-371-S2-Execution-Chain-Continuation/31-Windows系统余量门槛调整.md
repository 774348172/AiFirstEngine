# 371 §0.32 Windows系统可用内存门槛调整

2026-10-21用户明确将Windows验收期间系统可用物理内存最低门槛改为**4.5 GiB（4,831,838,208 bytes）**，预算数据371-budget-v2。代码默认值与项目JSON已同步，3项内存判定及1项配置一致性测试通过。其它预算和原规模保持；历史6 GiB结果不改写，单点满足不授全程/正式资格。此次调整已完成，S1剩余实现/资格仍按30记录推进，未安装。

用户直接授权只调整system.available最低余量；沿既有GiB口径，不把GB/GiB混为两套字节值。当前A路09:56:34 UTC开始，实际结束及耗时见证据04-closeout.json，原dirty和B路保持。

## 实际修改

- [预算默认值](../../../../rust/crates/project_authoring_execution/src/platform_3d_benchmark/budget.rs)：minimum_system_available由6 * GIB改为4 * GIB + GIB / 2，判定仍为低于最低值拒绝。
- [项目预算](../../../../samples/platform_3d_benchmark_project/Benchmark/budget-v1.json)：minimumSystemAvailable=4831838208，revision=371-budget-v2；稳定文件名保留budget-v1.json。旧版本证据保留自己的内容/hash，不将新revision写回历史产物。
- [既有integration](../../../../rust/crates/project_authoring_execution/tests/platform_3d_assets.rs)：配置/default一致性；恰4.5 GiB、其上1byte、5 GiB通过，4.5 GiB减1byte失败；总量、冲突、缺仪器unqualified逻辑保持。

CPU tracked aggregate4 GiB、process PrivateBytes peak6 GiB、GPU tracked allocated4 GiB、最低GPU余量2 GiB，所有role/capacity/上传预算、13.3/16.67ms及负载保持。没有改宿主、安装、SDK、Physics或游戏。调整允许后续按新版本评估，不等于证明引擎占用下降，也不自动重判旧正式失败。

## 验证与经济性

固定I盘SSD target、Rust1.96.0、locked/-j1、dev-fast，project_authoring_execution真实platform_3d_assets integration。只编译一次，measurement_memory_三项通过，再用同一exe运行recipe_target_layouts_match_static_sources_and_world_coverage_without_future_cook，一项通过；最终两文件rustfmt --check退出0。无需新建runner、扩大suite、Compiler/Cook、Release、Player或完整基准。

证据[01-memory](../../../../.artifacts/371/system-reserve-20261021-095634/01-memory/result.json)、[02-config](../../../../.artifacts/371/system-reserve-20261021-095634/02-config.json)。构建82.205秒，内存测试0.551秒、配置测试0.524秒分计；无失败返工。不是原规模内存测量，没有新增运行峰值或CPU/GPU性能结论。

同步371-A、S0-B数值表、371-G、S1-EF当前说明、总施工§0.32及49/54/导航。31仅完成用户数值修订；30的5.46–5.74 GiB是原门槛盘点，数值高于新最低值但仍仅单点。S1/371保持partial，Android延期。
