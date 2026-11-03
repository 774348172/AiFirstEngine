# 371 阶段46：纹理CPU像素安全回收与同源再上传

2026-10-23，用户授权按45推荐继续施工。本块完成包纹理上传后的CPU像素回收。相同原1.5×资源、Host、720帧参数下，Windows PrivateUsage采样峰 **5.286903→4.568188 GiB**，减少 **0.718716 GiB（13.59%）**；loader稳态驻留减少 **1.500 GiB**。仅本块源码与这一次规模对照成立，完整371/S1/S2仍partial。

证据根：[texture-cpu-residency-20261023-133204](../../../../.artifacts/371/texture-cpu-residency-20261023-133204)。13:32:04 UTC开工，原16:17:04停止新增、16:32:04硬截止；实际提前收尾时间及Git/进程保护见21-closeout，成本见20-cost-ledger。UE实测按用户决定跳过；本轮没有新增UE/Godot数值。

## 实现与寿命合同

沿原资源准备和上传owner修改，没有增加全局管理器、包schema或设备恢复协议：

- `ProductionGeometry`只对具有完整原包receipt的纹理创建可再生描述；原`VerifiedTexturePayload`保留元数据和完整内容身份，CPU像素由共享source持有。既有弱memo按逻辑纹理句柄复用source，不为每个材质复制像素，不复制整个资源索引。
- loader在该批准备完成后放弃自身纹理像素引用，保留句柄、Ready状态、依赖及引用计数。准备缓存、旧snapshot和上传队列各自转换或持有租约，避免只删loader却仍被其它Arc常驻的问题。
- 原分段上传队列持有明确payload Arc。仅原真实GPU completion回收source的强引用，再清理队列payload；外部CPU读者或另一上传持有的Arc保持有效。待上传、取消、准入拒绝不会使其它租约失效；尚未成功上传的source可保持到重试或缓存退出。
- GPU资源存在时仅检查描述/内容身份，不重读源包、不重传。需要CPU访问或新backend上传时，从捕获的原包路径/record/cooked读取，核对metadata+base像素SHA256、大小和bundle，再按原采样/色彩算法重建mip，核对完整最终identity。损坏、缺失、换版失败不会替换成空白或新内容。
- 源码复查额外复现了loader冷读取可接受同长度坏像素的问题。`runtime_texture`内增加共享的checked读取函数，loader两种getter与upload source共同消费，消除这条回读旁路。旧API/wire不变；不完整legacy receipt保持原语义，且不转换为可再生source。

`PreparedTextureAsset.payload`在内部ProductionGeometry包纹理路径只保留元数据；需要实际像素的消费者使用`acquire_upload_payload()`并持有返回Arc。普通`prepare_cooked_from_package`、手工/动态纹理仍保留原CPU载荷。旧直接静态注册路径可取得真实像素，但不伪称已在其提交完成后回收；本次完整Player走分段上传。显式冷读取会进行同步磁盘读取与hash/mip工作，原生产准备缓存避免每帧重读。

实际变更为9个产品文件与2个直接测试文件，详见12-source-review和各owned.diff；原stage45修改保留，asset_memory.rs未改。未改SDK/schema、Physics、预算、安装及游戏V0.0.5。B路374仍沿独立目录施工；本轮只更新A当前摘要，保留写入时B登记。

## 验证与失败记录

| 证据 | 结论 |
| --- | --- |
| 01-red | 原实现真实GPU完成后，cache/snapshot仍pin住像素；首个失败完整保留 |
| 03–05 | 寿命断言已通过，但测试先后受过小绘图上传额度、修改sealed plan identity影响；属于测试装配失败，没有放宽产品预算或graph校验 |
| 06、17-lease-gpu | 分段真实GPU完成后Weak消失，CPU读者保活；同GPU缺文件仍Ready且无新增上传；缺失/同长度损坏明确失败；恢复后新backend成功重传、完成释放且真实图像含红色像素；17为最终源码终验 |
| 07、14、15、16 | 4×4→3级mip/84字节及sampler可重建；不同材质共享source；descriptor改动被拒绝。14保留cold-loader坏像素首红，15定向转绿，16终态suite涵盖该用例 |
| 16-foundation | 61通过、0失败、9忽略；包含原mesh raw、普通CPU消费者和新增mip/cold读取测试；忽略不算通过 |
| 09-receipt-gpu（复用） | 手工纹理同句柄换像素仍拒绝，等内容Arc接受；后续只改包冷读取，未改变该测试/消费路径的输入，复用其通过证据 |
| 格式 / 18-clippy | 11个修改文件fmt通过；core149、native17及input2既有warning保留，修改行没有新增primary diagnostic |
| 02、11、19 Release | 02为优化前基线；11初版成功构建后被cold读取修复取代，未用于终态对照；19为最终Release |

61项基础测试+2项独立GPU用例为63项去重终验。07/15等定向开发反馈不重复计数。环境为本机Rust1.96.0、便携MSVC/SDK、371独立target，Cargo locked/j1、dev-fast或Release。未跑完整workspace、历史全GPU矩阵或新增Cook/export，因为直接消费者和实际原包Player已覆盖本块影响面。真实新backend上传证明来源再生能力，不代表已经实现自动device-loss恢复编排。

## 原1.5×完整负载对照

两轮均使用原15000独立mesh、384材质、96张2048² RGBA8、15M三角形资源包和相同Game.exe；run-owned delivery/data联接指向同一原Compiler完整包。均1765×993、120帧warmup+600样本、Immediate、Summary且allocationSummary关闭，外部0.1秒采样。没有重新导出、降低负载或增加预算。

| 指标 | 修改前 | 修改后 |
| --- | ---: | ---: |
| PrivateUsage采样峰 | 5,676,769,280 B / 5.286903 GiB | 4,905,054,208 B / 4.568188 GiB |
| OS PeakPagefileUsage | 5.295555 GiB | 4.571430 GiB |
| loader驻留计量 | 1,627,877,103 B | 17,264,367 B |
| GPU mesh / texture | 15000 / 96 | 15000 / 96 |
| 完成帧 / exit | 720 / 0 | 720 / 0 |
| CPU P95 / P99 | 6.3281 / 8.7400 ms | 4.5812 / 4.9252 ms |
| Player墙钟 | 22.203 s | 20.922 s |

GPU物理mesh/texture字节及总上传量一致，截图逐像素完全一致。loader减少1,610,612,736 B，确定性Weak测试另行证明真实backing释放；逻辑驻留计量和整个进程峰值属于不同口径，不要求收益数值相同。最终系统available最低36.780GiB；进程6GiB、图形4GiB及系统余量4.5GiB合同保持。

基线DLL SHA256 `d477f511accb04e312042459e091e24987501e0f06dd3fba2dab60eecb257249`；最终DLL `7e076894ff0d736c0af898fed7ae6196f99b2584cc7db5d734fad785bdc30c65`；相同Host `422a951cbc474da9f8478d8936aaa61b349fb60da752e3ff5a3eac0d11a7cbd3`。源/测试/产物身份分列在baseline/final-identity、12-source-review及release-source-start；最终Release前后Rust输入核对一致，另一路未向本测量源接入改动。

这是一对本机dirty-source Release的短窗诊断组合，不能据此声明持续性能、完整BM03/BM11矩阵或所有平台资格。历史Godot对照属于42/43的产物与资源条件，本轮没有重新测Godot，不将本轮引擎数值冒称新的同场同步对照。

## 出口和下一块

本块Gate A–D完成；49/54、README、阶段/施工入口及本目录总览同步。371总施工保持当前，因为整体仍partial；本窗提前收尾，不继续在后台施工。完整371/S1/S2、持续/旧机及平台资格继续按41固定退出清单推进，Android等原延期保持。

下一优先处理**CPU几何源backing**：43原规模约0.436GiB，先核对GPU上传、重传、CPU访问和旧快照消费者，再在原owner移交/回收；该载荷量不是承诺可实现的进程峰值收益。余下allocator/driver/启动临时峰值继续按实测归因，避免用提高预算掩盖驻留问题。下一块尚未开工。

实际收尾：2026-10-23 14:23:50 UTC，总墙钟51.78分钟；所有Cargo构建/Clippy累计16.03分钟，测试执行累计62.67秒，两个Player分别单列，Cook/export均0。文档同步helper首次因CRLF分段失败、首文件未写入，修正后同步完成；部分backup和诊断原因保留于21-closeout，未触发产品重测。HEAD/index/原空锁保持，最终Rust输入身份一致，无本路后台进程。
