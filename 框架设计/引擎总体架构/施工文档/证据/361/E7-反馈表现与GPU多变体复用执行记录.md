# 361 Gate E E7 反馈表现与GPU多变体复用执行记录

日期：2026-09-29。终态：`source_and_windows_release_performance_qualified`。本块消除反馈层与WGPU在theme/DPI双态间的单槽抖动；最终400节点Windows Release三类冻结负载全部满足AUI P95不超过2 ms、完整CPU P95不超过16.67 ms，Gate E关闭。

## 1. 实现与边界

- `aui_control_feedback.rs`、`aui_feedback_cache.rs`、`aui.rs`：style与presentation由单槽改为最多4个近期变体；只缓存不超过4096节点的文档。A-B-A第三次可直接命中，动画/交互/override身份变化只保留仍相容的变体，直接feedback advance仍清空presentation变体。
- `wgpu_backend.rs`：当前UI vertex buffer之外最多保留3个inactive epoch，可覆盖4个近期表现集合；变体准入为不超过4096 items且不超过8 MiB vertex bytes，全局1024 buffers/32 MiB限制不变。空plan立即清空，容量不足先淘汰最旧inactive buffer；大文档连续帧仍可局部更新，但切换后不保留备用。
- ABI、公开wire、序列化frame、项目SDK、RuntimePackage与默认报告分档未变化；4097/10k不通过多份缓存换取性能数字。

源码SHA-256：

| 文件 | SHA-256 |
| --- | --- |
| `aui.rs` | `715DBF0B0813E34277ABDCE2239EF2EF757188B931499CBAD63BDDD80E7862D1` |
| `aui_control_feedback.rs` | `D6549502696141C2CC31254BD4ACAD439F4D203BB5706E5379E96B0FBC65A170` |
| `aui_feedback_cache.rs` | `58B236F0E3AF0161A3E181AD2C39E4CAE1E1793149A869DB4BCF943D433AC984` |
| `wgpu_backend.rs` | `F75DBBC70066513871860D568B7DD87EC75AE79FBB24F3F28A249E7812900C55` |

## 2. 红态与源码验证

反馈红态稳定复现A-B-A第三次未命中及style rebuild为3而非2。修复后：

| 验证 | 结果 |
| --- | --- |
| 3项E7反馈变体测试 | 3/3通过 |
| `aui_control_feedback::tests::` | 9/9通过 |
| E7真实Release WGPU测试 | 3/3通过；含bounded交替复用和大stream拒绝 |
| 既有atlas COW、B3容量/空帧释放、B18局部上传/回退 | 通过 |
| `cargo check -p engine_runtime -p runtime_player_winit --locked` | 通过 |
| `cargo check -p engine_runtime --features real-wgpu --locked` | 通过 |
| 受影响Clippy correctness | 返回0；仅仓库既有普通warning |
| `engine_runtime`全lib | 712/712通过；首次唯一失败为C盘Temp空间不足，TEMP/TMP切至G盘后原测试通过 |
| scoped `git diff --check` | 通过；仅LF/CRLF提示 |

Debug真实WGPU链接曾因LLVM内存/分页不足中止；同一源码的Release profile随后完成并执行3项真实GPU测试，因此记录为环境构建成本，不是产品断言失败。未运行全workspace或Local CI。

## 3. Release身份与性能结果

使用正式`ReleasePackageBuilder`生成并验证400节点候选：manifest/inventory、formal load、资源回读、原子发布和3帧进程验证均通过。

| 身份 | 值 |
| --- | --- |
| Release payload | `sha256:909585a1c465dd543b275a985b0e3ce109837ce877427bc3633c1dfe27b031f1` |
| Runtime content | `sha256:068010600ea25e5b3a76a785d23030e58b8f0939b210b8aec8cd5f78f2322362` |
| `engine_runtime.dll` | `1B776917A66F7C4D024B49A3C2FCFF9BF9C0590E6823202AE1A7199BAF0B5304` |

固定口径为Windows Release、1280x720、RTX 3070/Vulkan、120帧预热和600帧采样。三次运行均退出码0、presented且无error diagnostics。

| 样本 | AUI P95 | 完整CPU P95 | update P95 | render P95 | 判定 |
| --- | ---: | ---: | ---: | ---: | --- |
| 400 single | 1.1443 ms | 10.4911 ms | 1.7344 ms | 8.9301 ms | 双预算通过 |
| 400 10% | 1.3828 ms | 10.6800 ms | 2.1275 ms | 8.8275 ms | 双预算通过 |
| 400 theme/DPI | 0.2875 ms | 5.2027 ms | 0.8465 ms | 4.3408 ms | 双预算通过 |

theme/DPI采样期为present hit 600/rebuild 0、GPU upload hit 600、full/partial upload均0、uploaded bytes 0、theme与DPI event各600；resident high water为4,537,440 bytes。相对E6，AUI P95由3.4029降至0.2875 ms，完整CPU P95由8.9989降至5.2027 ms，600次full upload和1,361,232,000上传字节全部消除。

报告：

- `samples/aui_static_tree_400_project/Evidence/E7/static-400-release-single-720.json`，SHA-256 `7C83D57D0B43F45F59DA56CC55E3BB8B4E99420A83AE9BE4C058856E852C70C7`
- `samples/aui_static_tree_400_project/Evidence/E7/static-400-release-ten-percent-720.json`，SHA-256 `59BD999060127AC4CE1A084FBB03C4F9E8EB1A1DF4A16077E41499BA33A58431`
- `samples/aui_static_tree_400_project/Evidence/E7/static-400-release-theme-dpi-720.json`，SHA-256 `BF05AFB8225EF350C2C1DFB28E66E77C5A8EE3D1797903379D5649E96FE0F26D`

E5虚拟列表materialized/recycled/disposed 142/117/137直接计数继续有效：E7未修改虚拟列表slot owner、Player生命周期聚合或普通项目输入。其失效条件为这些owner、10k逻辑项目或对应计数合同变化；本轮不为标签重复运行。10k静态树仍是压力样本，不属于400节点2/16.67 ms关闭预算。

## 4. 构建恢复、外部状态与结论

首次构建仍写入C盘共享Player BuildCache并因空间不足失败；未删除共享缓存。设置`AIFE_PROJECT_RUNTIME_PLAYER_BUILD_ROOT=G:\aife-e7-build-cache\player-build`后，首次G盘冷编译命中构建器固定300秒超时，缓存预热后的同输入重试成功。首次theme/DPI调用又因传入相对RuntimePackage路径而得到`native_host.engine_runtime.project_module_load_failed`，改用同一候选的绝对RuntimePackage路径后成功完成720帧；这是调用路径错误，不是候选断言失败，失败报告的关键诊断已记录于此。构建阶段两次失败均无产品断言失败。

本块未更新默认安装、未重建或覆盖旧塔防包、未修改真实配置。全部400冻结负载通过，生命周期直接计数合同闭合，Gate E关闭；361可生成总完成记录并归档。独占`G:\aife-e7-build-cache`在证据保存后按施工清理规则整体清除。
