# 361 Gate E E6 单一渲染owner与全局变体有界复用执行记录

日期：2026-09-29。终态：`source_and_release_requalified_theme_feedback_gpu_followup_required`。本块消除了Player同帧重复构建render submission，并为Presenter与Renderer增加有界的A/B表现变体复用。400节点single、10%更新和theme/DPI的完整CPU P95均已进入16.67 ms预算；但theme/DPI的AUI P95仍为3.4029 ms，超过2 ms冻结预算，因此Gate E继续打开，361不归档。

## 1. 实现范围

- `engine_host_loop.rs`：`EngineFrameInput`的`build_render_submission`现在真正控制Host是否构建render submission；默认仍为`true`，不改变Editor和默认ExportedGame行为。
- `runtime_player_winit/src/lib.rs`：三个随后明确向真实surface提交的Player路径使用deferred render submission，移除同一帧Host与Player的重复渲染owner。
- `aui_present_cache.rs`：Presenter最多保留4个静态表现变体，key为根document theme identity与viewport identity；仅缓存不超过4096节点的文档。snapshot Replace、retained/content/virtual interaction变化立即清空，命中变体后重置并重新observe当前retained instance，避免下一帧误判变化。
- `runtime_renderer.rs`：每个UI stage保留一个备用`CachedUiStage`，精确命中复用items/glyph Arc、presentation mapping和texture storage；备用只接纳不超过4096 items且不超过8 MiB vertex bytes的stage。删除对整个`UiVertexStream`的深度`PartialEq`比较，改为先执行局部`continue_from`，仅在顶点变化为零且非顶点批次元数据一致时标记整批复用。
- 4097节点与10k压力样本均不会保留Presenter多变体，10k也不会保留Renderer双stage，避免用无界内存换取基准数字。ABI、公开wire、默认报告分档和项目SDK合同未变化。

源码SHA-256：

| 文件 | SHA-256 |
| --- | --- |
| `engine_host_loop.rs` | `D6F1E29923CFC8B923CE4DBF467C79A8B19E3ACFC7F1593DBE7811BDC326BB07` |
| `aui_present_cache.rs` | `3C2BF2C2D4C530F19DF1762995094F52DF75FF38C36166C8DA97EC995AF3A1F7` |
| `runtime_renderer.rs` | `E78335AC95DC96AFF7653F968F17360FA5A9DC634C2238DCD2FA9D57543A8BA3` |
| `runtime_player_winit/src/lib.rs` | `214BAFE17D4CB7DE0477AC48245B39CE0BC218A34E02C3669AE8DC459CFE066D` |

## 2. 源码验证

| 验证 | 结果 |
| --- | --- |
| `engine_host_loop::tests::` | 29/29通过 |
| `runtime_renderer::tests::` | 35/35通过 |
| `runtime_player_winit --lib aui_present_cache_` | 10/10通过 |
| 三条原始红测 | 全部通过：外部target owner延迟提交、viewport/theme A-B-A复用、Renderer有界交替表现复用 |
| 大文档排除测试 | 4096可缓存；4097与10k不缓存 |
| `cargo check -p engine_runtime -p runtime_player_winit --locked` | 通过 |
| 受影响crate Clippy correctness | 返回0；仅保留仓库既有普通warning |
| scoped `git diff --check` | 通过；仅有既有LF/CRLF提示 |

Presenter和Renderer的A-B-A测试均使用不同的真实内容或mapping，并在连续交替后验证retained baseline没有被错误失效。未运行全workspace或Local CI。

## 3. Release身份与生产验证

| 项目 | Release payload hash | Runtime content hash |
| --- | --- | --- |
| 400静态树 | `sha256:d16c01d0ca9aa74bd229d84e4f3d8f106a2e15744eb2d557e46aa76b913c5c95` | `sha256:068010600ea25e5b3a76a785d23030e58b8f0939b210b8aec8cd5f78f2322362` |
| 10k静态树 | `sha256:2f287b49a9906033a8f3853ed396441f256fb0504b7c37fa837c4bde5bcc4e2c` | `sha256:8aa1a1e1d3709717ebeb5d237261faa80d44a7207d116def866d44a899ad982f` |

两个候选的`engine_runtime.dll` SHA-256均为`7D8A2A9C0F6D50CDBD3A8E0B0C53B5EB5E506368E4E85F2213FADAE6FBF6CFB7`。manifest/inventory、formal load、资源回读、原子发布及3帧进程验证全部通过，没有手工替换DLL。

首次经`cargo run`和首次直接运行各遇到一次构建工具内部固定300秒超时。诊断确认环境中的`CARGO`身份不同导致compile workspace分裂；在独占缓存完成稳定冷编译后，直接运行通过。两次超时没有产生产品断言失败，不作为候选不合格记录。

## 4. 最终性能结果

固定口径为Windows Release、1280x720、RTX 3070/Vulkan、120帧预热和600帧采样。四次运行均退出码0、成功present且无error diagnostics。

| 样本 | AUI P95 | 完整CPU P95 | update P95 | render P95 | 判定 |
| --- | ---: | ---: | ---: | ---: | --- |
| 400 single | 1.1666 ms | 10.6759 ms | 1.7631 ms | 9.2464 ms | AUI与完整CPU通过 |
| 400 10% | 1.3543 ms | 10.7599 ms | 2.0996 ms | 8.9587 ms | AUI与完整CPU通过 |
| 400 theme/DPI | 3.4029 ms | 8.9989 ms | 3.9096 ms | 5.6560 ms | 完整CPU通过；AUI超过2 ms |
| 10k single | 26.9979 ms | 109.2425 ms | 40.4353 ms | 71.2766 ms | 压力样本，未达 |

工作量直接计数：

| 样本 | 关键计数 |
| --- | --- |
| 400 single | present rebuild 600/hit 0；drawing compared 600；partial upload 600；upload 4,723,200 bytes |
| 400 10% | present rebuild 600/hit 0；drawing compared 24,000；partial upload 600；upload 151,142,400 bytes |
| 400 theme/DPI | present rebuild 0/hit 600；drawing compared/built 0；theme与DPI event各600；full upload 600；upload 1,361,232,000 bytes |
| 10k single | drawing compared 600；partial upload 600；resident high water 6,053,568 bytes；未进入备用双份缓存 |

相对E5：

- 400 single完整CPU P95由16.5781 ms降至10.6759 ms。
- 400 10%完整CPU P95由17.0787 ms降至10.7599 ms，已进入预算。
- theme/DPI完整CPU P95由23.3106 ms降至8.9989 ms，AUI P95由8.0699 ms降至3.4029 ms；Presenter由中间诊断的400 rebuild/200 hit收敛为最终0 rebuild/600 hit。
- 10k完整CPU P95由153.3961 ms降至109.2425 ms，update由96.9486 ms降至40.4353 ms；render由59.8024 ms变为71.2766 ms，仍为主导成本且存在样本波动，不能判为通过。

报告位置：

- `samples/aui_static_tree_400_project/Evidence/E6/static-400-release-single-720.json`
- `samples/aui_static_tree_400_project/Evidence/E6/static-400-release-ten-percent-720.json`
- `samples/aui_static_tree_400_project/Evidence/E6/static-400-release-theme-dpi-720.json`
- `samples/aui_static_tree_10000_project/Evidence/E6/static-10000-release-single-720.json`

## 5. 停止条件与下一首因

E6的停止条件已经触发：Presenter基础theme/viewport变体达到600/600命中，但theme/DPI AUI仍超过2 ms，并由另一个全树owner继续主导。源码追踪定位到`aui.rs`中的`apply_control_feedback_with_fonts_for_presentation_and_themes`：`AuiControlFeedbackState.presentation_cache`只保存最后一个表现变体，A-B-A切换每帧重建反馈后的composition并改变提交身份；WGPU侧同时只保留一个resident UI buffer，因此每帧发生full upload。

下一整块应正式处理反馈层的有界基础/表现变体复用，以及GPU UI buffer多变体驻留或等价的可靠复用。该工作不在E6中临时扩张；需要继续以4096节点/8 MiB等有界策略和大文档拒绝合同约束内存。

## 6. 外部状态与Gate判定

本块只使用独占构建根`G:\aife-e6-build-cache`，没有删除C盘共享缓存。证据保存后确认目标解析为字面路径、不是workspace/home/盘符根，根及全部子项无reparse point，且无cargo/rustc/link/project_e2e_gate或外部进程引用。原生`Remove-Item`在PowerShell启动前被宿主策略拒绝；重新完成同一安全证明后使用允许的`.NET Directory.Delete`回退，最终状态为`removed`且路径不存在。未更新默认安装，未重建或覆盖旧塔防包，未运行全workspace或Local CI。

400 single和10%已同时满足AUI不超过2 ms、完整CPU不超过16.67 ms；400 theme/DPI仅完整CPU通过，AUI P95 3.4029 ms未通过。Gate E保持打开，361继续激活；只有下一整块修复反馈表现缓存与GPU buffer复用，并刷新受影响Release样本达到全部冻结预算后，才允许关闭Gate E。
