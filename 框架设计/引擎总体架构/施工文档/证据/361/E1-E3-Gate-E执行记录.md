# 361 Gate E E1-E3 执行记录

日期：2026-09-29。终态：`measurement_completed_with_failures`。E1 Release owner 和 E2 有界测量合同已进入真实 Windows Release 消费；E3 第一轮塔防、400/1k/10k 静态树及 10k 逻辑虚拟列表矩阵已完成。400 节点 10% 更新和全树失效代理未达目标，真实主题/DPI负载与可直接观察实例回收的生命周期证据尚未闭合，Gate E 不关闭，361 不归档。

## 1. 范围与禁止项

- 只验证 361 正式方案第 13/13.1 节的 Windows Release、1280x720、120 帧预热和 600 帧采样口径。
- 默认安装仍为 360；没有替换安装 Provider、真实宿主配置或旧塔防包。
- Release Package 继续使用既有 `ReleasePackageBuilder`、`ProjectPlayerArtifact` 和 RuntimePackage assembler，没有建立第二套构建系统。
- 所有性能报告来自发布目录中的 Host EXE、`engine_runtime.dll` 和项目 DLL；GPU 为 RTX 3070 / Vulkan，不能用 CPU 耗时倒数声称显示 FPS。

## 2. 冻结机器与合同

| 项目 | 冻结值 |
| --- | --- |
| OS | Windows 11 Pro 10.0.26200 |
| CPU | Intel Core i5-12490F，6 核 12 线程 |
| 物理内存 | 16,980,672,512 bytes |
| GPU | NVIDIA GeForce RTX 3070 |
| 驱动 | 32.0.15.9186；WGPU 报告 NVIDIA 591.86 |
| 共存显示设备 | Todesk Virtual Display Adapter，仅记录，不作为实际 WGPU adapter |
| 窗口与采样 | 1280x720；120 warm-up + 600 samples |
| profile | Windows `release`；Host、Engine DLL、Project DLL 均由 Cargo release 构建 |
| 400 节点目标 | AUI CPU P95 <= 2 ms；完整 CPU 帧 P95 <= 16.67 ms |

E1 修复了 release 包过去仍消费 debug Project Player artifact 的问题：profile 进入请求、缓存身份、构建报告、Cargo `--release` 命令和 `target/release` 查找目录。默认 Project Player、Desktop Export 和 Editor Preview 保持 dev。Release staging 同时携带 `engine_runtime.dll` 和 `data/bin/<module-id>.dll`，且生产 staging 不再复制项目 `Library`。

E2 在既有 Player Summary/Trace 路径增加有界采样：AUI CPU、完整 CPU、update、render submit、present wait、GPU timestamp，及 layout/geometry/drawing/paragraph、Project DLL 扩展字节、UI upload、驻留和缓存高水位。正式 runtime 默认仍不写 Trace。

## 3. Release 包身份

| 普通项目 | 节点/数据口径 | Runtime content hash | Release payload hash |
| --- | ---: | --- | --- |
| 塔防 | 1,298 个真实 AUI 节点 | `sha256:5b3bfc0ad13ed3e1de3da97bc8f0e4401d4e58ce526c88a9d25653fcf29b5b95` | `sha256:531439ef6a7ccd1f75fd5bdd8c98246a26ea38c04d6b5b5acf9e9e36344ce465` |
| 虚拟列表 | 26 个静态节点，2 x 10,000 逻辑项 | `sha256:c856d95fdb976c51c5cb720eab0c5828ca4d70c037ee391f423bba29da0c2853` | `sha256:0a000d75e2a6c37eb13dc005d123dfa489971ccb47234b21fe67886c1ba62ae3` |
| 静态树 400 | 400 | `sha256:068010600ea25e5b3a76a785d23030e58b8f0939b210b8aec8cd5f78f2322362` | `sha256:6924013773bc4cc5e94fda740e8302e1a10cabe6ea8f482eca09df87342e9333` |
| 静态树 1,000 | 1,000 | `sha256:eefcb72c95ac8a175bb78a9cdd6a5b2bdcf4cc3b2ad406a523a87ed52a01d071` | `sha256:a6b4b11fd9424114ec186b82d573782744412a8e74b8ce25c84745fe4834c667` |
| 静态树 10,000 | 10,000 | `sha256:8aa1a1e1d3709717ebeb5d237261faa80d44a7207d116def866d44a899ad982f` | `sha256:a0d3efcfd97985ef1d0180bb68ba124d7f06d94793630eb3b5b926e6fbe9b2a7` |

五个包均通过 manifest、inventory、RuntimePackage formal load、Windows 资源回读、publish validation 和 3/3 子进程帧验证。塔防实际是 1,298 节点，已纠正 E1 的“约 415 节点”旧估算；它不能替代精确 400 节点样本，也不能把 1,298 节点结果伪称为 400 节点预算。

## 4. 真实窗口结果

下表均为 Release 600 个样本帧的 P95，单位 ms。`global` 是项目 SDK 对根节点 translation 的全树失效代理，不是真实主题/DPI切换。

| 样本 | AUI P95 | CPU P95 | GPU P95 | Present P95 | 结论 |
| --- | ---: | ---: | ---: | ---: | --- |
| 塔防 1,298 clean/真实玩法 | 1.8206 | 7.7186 | 0.4649 | 0.1028 | P95 通过；AUI/CPU P99 为 22.4581/27.3414，保留峰值风险 |
| 静态 400 clean | 0.0983 | 14.0151 | 0.3389 | 0.0953 | 通过 |
| 静态 400 single | 1.7880 | 15.5459 | 0.3400 | 0.0979 | 通过 |
| 静态 400 10% | 2.0558 | 16.2666 | 0.3021 | 0.1056 | AUI 超标 0.0558 ms，未达 |
| 静态 400 global | 3.1832 | 18.1115 | 1.9610 | 0.0993 | AUI 与完整帧均未达 |
| 静态 1,000 clean | 0.2389 | 32.5509 | 0.6492 | 0.1068 | 规模数据，不套 400 节点预算 |
| 静态 1,000 single | 4.5477 | 37.5802 | 0.6697 | 0.0989 | 单点仍比较全树 |
| 静态 1,000 10% | 4.8742 | 37.4493 | 0.6205 | 0.1012 | 规模数据 |
| 静态 1,000 global | 7.6430 | 43.9568 | 0.8643 | 0.1078 | 规模数据 |
| 静态 10,000 clean | 1.4781 | 122.1591 | 0.6492 | 0.1018 | clean cache 命中，但整帧不具备实时预算 |
| 静态 10,000 single | 54.3907 | 174.7645 | 0.6748 | 0.1008 | 单点仍比较全树 |
| 静态 10,000 10% | 61.1207 | 189.6705 | 0.7946 | 0.1048 | 规模数据 |
| 静态 10,000 global | 90.7176 | 216.2855 | 0.7076 | 0.1050 | 规模数据 |
| 10k 逻辑列表 clean | 0.1182 | 3.7264 | 0.1792 | 0.1016 | 单列通过，不冒充静态 10k |
| 10k 逻辑列表重复滚动脚本 | 0.1087 | 3.7484 | 0.1925 | 0.0926 | 输入脚本已消费；缓存高水位稳定，实例回收计数仍不可直接观察 |

静态树 clean 的完整帧成本主要位于 CPU update/render submit，而非 GPU 或 Present：400 节点 P95 为 7.3315/6.9505 ms，1,000 节点为 16.9210/16.0779 ms，10,000 节点为 67.5840/55.5277 ms。

## 5. 工作量与内存事实

- 400 single：600 帧比较 240,000 节点、只重建 600；10% 同样比较 240,000，重建 24,000。
- 1,000 single：比较 600,000、只重建 600；10% 重建 60,000。
- 10,000 single：比较 6,000,000、只重建 600；10% 重建 600,000。局部 payload 复用成立，但变化帧仍有 O(总节点) 比较。
- 400/1k/10k clean 均为 600 cache hit、0 rebuild、0 tracked allocation event、0 UI upload。
- 静态树 UI resident high-water 分别为 2,526,912 / 6,053,568 / 6,053,568 bytes；10k 超出视口的节点没有继续扩大可见 GPU resident，但仍扩大 CPU update/render 成本。
- 10k 逻辑列表 clean 与重复滚动脚本的 UI resident high-water 均为 205,152 bytes，paragraph cache high-water 均为 120,940 bytes，tracked allocation event 为 0。报告未暴露逐次 materialize/recycle 数，故这里只能证明本次 600 帧没有高水位增长迹象。
- 塔防 RuntimePackage/AUI 装配曾达到约 4.25 GB private/working set，并一度因约 36 MB 分配失败；最终同一有界复验成功。1298 节点的构建侧峰值仍是未解决效率风险，不能因最终成功而消失。

## 6. 验证与首因修复

已通过的 owner/consumer 证据：

- `project_rust_release_build_request_requires_release_profile`
- `project_rust_release_reuses_existing_verified_player_candidate`
- `sealed_project_player_artifact_root_requires_all_runtime_sidecars`
- `project_runtime_player_production_staging_normalizes_trusted_and_crates_io_dependencies`
- `frame_performance_reports_update_render_submit_and_present_wait`
- 三个静态项目各 1 项 Project Module owner 测试、`cargo fmt --check` 和 `cargo clippy --locked -- -D warnings` 均通过。
- `editor_core` 三项 Release owner、`project_authoring_execution` staging owner、`runtime_player_winit` 分段统计 owner 测试通过；`editor_core`、`project_authoring_execution`、`runtime_player_winit`、`runtime_cli` 的 `cargo check --locked` 通过。
- 六个直接修改的 Rust 文件通过 `rustfmt --check` 和 `git diff --check`。四个受影响 crate 的 `cargo clippy --locked --no-deps` 返回 0，但输出已有全 crate warnings，因此不把它记录成 `-D warnings` 资格。

施工中修复了三个实际首因：Release consumer 不应编译项目 tests；生产 staging 不应复制 `Library`；Release 包必须携带 Engine/Project DLL sidecar，否则运行会退回 `engine.empty.runtime` 并报 module ID mismatch。测试断言没有为过验而削弱。

## 7. 未关闭项与下一步

Gate E 维持打开，原因如下：

1. 400 节点 10% 更新 AUI P95 超过 2 ms；全树代理同时超过 AUI 和完整帧预算。
2. 变化帧仍按总节点数比较，10k 单点更新 AUI P95 为 54.3907 ms，是明确的运行时首因。
3. 真实主题/DPI变化尚无 Release 窗口负载入口；根 translation 只能证明全树依赖传播成本，不能冒充主题/DPI。
4. 生命周期报告缺少 materialize/recycle/dispose 的直接计数，重复滚动只能证明输入被接受及缓存高水位未增长。
5. 塔防 1298 节点报告存在约 22–27 ms 的 AUI/CPU P99 周期峰值；构建侧大 AUI 装配峰值约 4.25 GB。

下一施工块应先把“变化帧全树比较”收敛到可信 changed-slot/fragment 索引，再补真实主题/DPI驱动和生命周期直接计数。不得以调整阈值、删除节点、关闭玩法或只保留 clean 样本关闭 Gate E。
