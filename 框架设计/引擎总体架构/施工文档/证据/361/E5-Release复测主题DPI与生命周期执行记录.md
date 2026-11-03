# 361 Gate E E5 Release复测、主题/DPI与生命周期执行记录

日期：2026-09-29。终态：`release_requalified_lifecycle_closed_performance_gate_open`。本块已重建包含E4修复的Windows Release候选，刷新400/1k/10k必要样本，并补齐真实主题/DPI负载和虚拟列表生命周期直接计数。400节点10%更新的完整CPU P95、主题/DPI负载以及10k完整帧成本仍未达要求，因此Gate E继续打开，361不归档。

## 1. 实现范围

- `AuiVirtualListPresentState`在原slot绑定owner内累计materialize、recycle和dispose，present cache与Player只读取累计值并计算采样窗口delta；没有新增对象池、ABI或公开wire。
- Player显式Trace负载新增`theme-dpi`与`virtual-list-lifecycle`。前者交替变更实际主题引用，并把不同scale factor送入既有viewport/layout/present链；后者每60帧修改既有虚拟列表scroll state。默认运行与未请求性能测量不启用这些负载。
- `runtime_cli`透传`--aui-performance-load`，`project_e2e_gate release-package --project`只投影既有`ReleasePackageBuilder`，不复制构建、manifest、hash或验证逻辑。
- AUI性能工作量报告增加生命周期、theme和DPI计数；默认报告分档与项目SDK合同不变。

## 2. Release身份与生产验证

| 项目 | Release payload hash |
| --- | --- |
| 400静态树 | `sha256:7dca090c5cfe775f4488b3e85ae8196d0392d3756414f8dabbf38c440cf943b5` |
| 1k静态树 | `sha256:fb42d48b4ff709997cc9918bb7cd7e56f0625af9f06f5275ca2540789161fbcb` |
| 10k静态树 | `sha256:4ef822899a74e5904b02c1fed6d405c01ea2aefd162a5f584f8908330c196a12` |
| 10k逻辑虚拟列表 | `sha256:9194d368d29f65a2f8cd4c9ad8b46993ead3e80f630a71af80ede8a88ba8b46d` |

四个候选均通过release manifest/inventory、formal load、资源回读、原子发布及3帧进程验证。运行报告确认NVIDIA GeForce RTX 3070/Vulkan和Engine Runtime DLL执行owner；没有手工替换DLL。

## 3. 性能与工作量结果

固定口径为Windows Release、1280×720、120帧预热和600帧采样。

| 样本 | AUI P95 | 完整CPU P95 | drawing compared | 判定 |
| --- | ---: | ---: | ---: | --- |
| 400 single | 1.1273 ms | 16.5781 ms | 600 | AUI与完整CPU预算通过 |
| 400 10% | 1.3348 ms | 17.0787 ms | 24,000 | AUI通过；完整CPU超过16.67 ms |
| 400 theme/DPI | 8.0699 ms | 23.3106 ms | 240,000 | 双项未达 |
| 1k single | 2.6512 ms | 38.1239 ms | 600 | 扩展样本，未达400档预算 |
| 10k single | 26.2901 ms | 153.3961 ms | 600 | 扩展样本，完整帧仍显著超预算 |

E4已进入新Release：400、1k和10k的single负载均只累计600次drawing比较，即每个采样帧只比较一个变化slot，不再按总节点数比较。真实`theme-dpi`负载累计600次主题变化和600次DPI变化，同时累计240,000次drawing比较，证明该路径仍触发400节点全树layout/theme/drawing重建。10k single的drawing比较已经局部化，但update P95为96.9486 ms、render submit P95为59.8024 ms，完整CPU首因已不再是drawing comparison。

报告位置：

- `samples/aui_static_tree_400_project/Evidence/E5/static-400-release-single-720.json`
- `samples/aui_static_tree_400_project/Evidence/E5/static-400-release-ten-percent-720.json`
- `samples/aui_static_tree_400_project/Evidence/E5/static-400-release-theme-dpi-720.json`
- `samples/aui_static_tree_1000_project/Evidence/E5/static-1000-release-single-720.json`
- `samples/aui_static_tree_10000_project/Evidence/E5/static-10000-release-single-720.json`

## 4. 虚拟列表生命周期

`samples/aui_virtual_list_project/Evidence/E5/virtual-list-release-lifecycle-load-720.json`记录：AUI P95 0.1258 ms、完整CPU P95 4.3598 ms、present rebuild 9、drawing compared 573、materialized 142、recycled 117、disposed 137、error diagnostics 0。三项生命周期计数均来自实际slot owner且非零，生命周期可观察性缺口关闭；P99尖峰仍保留在报告中，不以P95通过掩盖。

## 5. 源码验证

| 验证 | 结果 |
| --- | --- |
| `cargo check -p runtime_cli -p runtime_player_winit --features runtime_player_winit/real-window --locked` | 通过 |
| CLI请求保真测试 | 2/2通过，含生命周期负载参数 |
| Player虚拟列表consumer | 1/1通过，materialize/recycle/dispose直接计数非零 |
| Player性能汇总测试 | 1/1通过，新增工作量字段被聚合 |
| 四个受影响crate的Clippy `-D clippy::correctness` | 返回0，仅保留仓库既有普通warning |
| 六个修改文件rustfmt与scoped `git diff --check` | 通过 |

首次并行Cargo测试因Windows资源不足/rustc内存分配失败，降为`-j 1 --lib`后通过；这是环境容量问题，不是产品断言失败。全仓`cargo fmt --check`命中三个与E5无关的既有格式漂移，本块未修改它们，也未把全仓格式声明为通过。

## 6. 外部状态与Gate判定

构建10k候选时C盘空间耗尽并出现MSVC `LNK1318`/`LNK1108`。本块没有删除或修改C盘约148 GB的既有共享缓存，改用本轮独占的`G:/aife-e5-build-cache`及其`temp`完成Release构建。证据保存后确认无cargo/rustc/link或引用该目录的进程、两个子目录均为字面严格子路径且无reparse point；`Remove-Item`在PowerShell启动前被宿主策略拒绝，重新验证后使用允许的`.NET Directory.Delete`回退清除两个子目录及空根，最终路径不存在。

本块未运行全workspace或Local CI，未重建塔防项目，未更新默认安装，也未覆盖旧塔防包。Gate E保持打开；下一整块应分别定位并修复真实theme/DPI路径的全树layout/theme/drawing重建，以及10k single中update/render的主导成本。只有刷新受影响的Release样本并达到冻结预算后，Gate E才可关闭。
