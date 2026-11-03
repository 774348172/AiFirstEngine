# 371 阶段42：图形常驻预算调整与 Godot 内存对照

日期：2026-10-23。用户批准 Windows 图形常驻预算2→4GiB，并询问合理进程峰值及Godot同源负载内存。本阶段任务完成；371/S1/S2整体仍partial。证据根：`.artifacts/371/budget-godot-20261023-111515`。原dirty、HEAD/index/空index.lock保留，不提交、不覆盖374在途工作，不更新安装或游戏V0.0.5。

## 1. 实际修改与验证

现行配置为`371-budget-v3`：native visual ceiling、Windows Player resident预算、MemoryBudget `gpu.resident`及示例JSON均为4,294,967,296 bytes。GPU aggregate仍4GiB，staging/retiring与resident共同受总量限制；CPU cache2GiB、CPU tracked4GiB、进程PrivateBytes6GiB、系统available4.5GiB、GPU available2GiB、上传16MiB/frame保持。

共7个源码/配置文件：`engine_render_native`的geometry.rs/native_boundary.rs、`runtime_player_winit`的lib.rs/tests/mod.rs、`project_authoring_execution`的budget.rs/platform_3d_assets.rs及示例Benchmark/budget-v1.json。S0-B和S2-D当前合同同步，历史2GiB失败保留。

- 首红：旧ceiling拒绝4GiB，01-native-red保留具体断言及输出。
- 终验：native真实GPU边界3项、预算/配置/直接测量13项、Player profile1项，共17项去重通过。4GiB接受、4GiB+1拒绝、0拒绝；process6GiB+1及GPU总量超标继续拒绝。边界测试没有分配4GiB显存。
- 六Rust文件定向rustfmt及受影响Clippy通过，既有告警保留。06/07记录Clippy耗时16.003/5.548秒。
- 必要一次Release DLL构建87.801秒；SHA256 `94ca6123b942770ce6984454bda592559dd0b745bc0be0b3f4f1259ea3fe96a0`。固定Host SHA `422a951cbc474da9f8478d8936aaa61b349fb60da752e3ff5a3eac0d11a7cbd3`。原1.5×完整Compiler产物中的RuntimePackage数据未重Cook。

## 2. 同源资源实跑

RTX4090/Vulkan，本机64GiB；实际1765×993、关闭vsync，720帧（120预热+600样本）。负载直接取阶段41有效BM03 1.5×K0：15000个独立mesh/instance、900万vertices、4500万indices、1500万三角形、384材质、96张2048² RGBA8纹理（无压缩/无mip，解码合计1.5GiB）。未减面、合并网格或使用MultiMesh。两边GPU运行串行；374独立施工可能影响机器状态，本轮不授受控性能比。

统一外部采样器使用Windows `GetProcessMemoryInfo/PROCESS_MEMORY_COUNTERS_EX.PrivateUsage`，每0.1秒一次，同时记录working set和OS peak pagefile。该私有提交量不是物理working set或独立显存计数，不能将三者当互斥分配直接相加。采样可能漏掉更短峰，OS peak pagefile另列于20-comparison.json。

| 本轮程序 | 私有提交采样峰值 | 稳定段事实 | 帧/截图/退出 |
| --- | --- | --- | --- |
| 本引擎4GiB DLL，15-engine-1p5 | 6,898,630,656 bytes = **6.424850GiB** | 原逐帧600样本窗口的private峰同为6.424850GiB；外部连续约3秒维持6.423GiB，不能只归为瞬间加载峰 | 720 / captured，1765×993 / 0 |
| Godot Release第一次，16-godot-1p5-a | 3,318,116,352 bytes = **3.090237GiB** | 预热完成至采样完成：3.089344–3.089600GiB | 720 / OK / 0 |
| Godot Release第二次，17-godot-1p5-b | 3,106,385,920 bytes = **2.893047GiB** | 预热完成至采样完成：2.892925GiB | 720 / OK / 0 |

本引擎功能实跑通过，但private仍超过6GiB **456,179,712 bytes =435.046875MiB**；阶段41旧值6.425617GiB/约436MiB保持旧身份。调整图形预算解决截图拒绝，没有降低进程内存，也未使压力内存资格通过。本轮外部system available最低约24.27GiB，不能倒授旧16GiB机器的余量资格。

Godot版本为本机**4.7.1.stable.official.a13da4feb**，不是编辑器进程；两份报告`editorFeature=false`、`debugFeature=false`，渲染方法Forward+，driver Vulkan。实际每帧15000 objects/draw calls、1500万primitives，96纹理与384材质数检查通过。两次截图均非空、尺寸正确；18-visual-check为并排检查图，背景/颜色处理有差异，未声明逐像素一致。

Godot两轮loaded及steady内部video计数均2,038,410,752 bytes（约1.898GiB），texture1,648,524,800、buffer368,455,260 bytes；卸载后video85,748,864 bytes。此计数含其内部资源，并不与本引擎账本口径相同。本引擎mesh468,000,000、texture1,610,612,736 bytes，合计约1.936GiB，截图临时资源导致旧2GiB上界拒绝的历史结果保留；不把内部计数称为系统物理显存A/B。

## 3. 比较边界与合理目标

这是**同源、同资源数量的资源加载/静态渲染对照**，不是完整游戏或完整渲染器等价评测：Godot将相同顶点/索引转为ArrayMesh，保留独立资源，交换三角形绕序适配正面；内部GPU顶点打包允许不同。Godot运行时创建ArrayMesh，本引擎读RuntimePackage；CPU更新、HUD、渲染图、材质和缓存/卸载策略不同。没有正式三轮长窗、OS实际显示或泄漏资格。Godot两轮相差约202MiB，未隔离驱动/缓存原因，不将任一轮称为全冷态。

在这些限制下，本引擎峰值为Godot的**2.08–2.22倍**，说明6.43GiB对该资源负载仍有明显优化空间；不能把两边全部差额直接归因于某一个clone、缓存或驱动。

建议（工程判断，**不是新验收标准或已达成结果**）：同一冻结负载优先争取稳定private **3–4GiB**、含加载/截图的峰值 **4–5GiB**，保留现行 **6GiB硬上限**。原目标16GiB RAM/8GiB VRAM仍要给OS、工具及其他运行分配留空间；本机64GiB可运行并不能证明目标机合理。若未来明确改变支持的设备档和场景，应另行按实测制定预算；不能仅为当前6.43GiB通过而直接改到8GiB。

下一步优先沿原loader/cache/prepared/snapshot/native上传与驱动实际存储寿命定位额外常驻，不做全量逐帧深扫，不凭本次对照断言泄漏。当前查询未实施这轮额外优化，process6GiB保持。

## 4. 启动问题与证据身份

12-engine启动exit1/约1.19秒：把旧Compiler manifest复制进新DLL诊断组合，触发完整发布报告校验而缺少reports；未加载资源，不能当内存基线。仅将本窗复制的manifest移为15-original-manifest-reference.json，按已有raw RuntimePackage诊断入口启动Game+新DLL+原data；不伪造新Compiler清单、不降低产品校验。15是独立诊断组合，不是完整新Compiler导出或安装。

13-Godot启动exit1/约0.44秒：现有Release template禁用`--main-pack`路径覆盖；未加载资源，不能拿其低内存作比较。改用本机官方导出器，以只读复制的Release template正式导出嵌入benchmark项目的新exe，14-export exit0/3.382秒；16/17直接运行该exe。原游戏/引擎安装及导出模板配置均未修改。

本轮run-owned转换/采样脚本和Godot工程只在证据根，不扩为产品运行时架构。19-source-audit记录7项源码与实际构建输入SHA一致；11保存DLL/Host/source/data来源。后续374串行接入如改共享文件，证据仍按这里的已构建身份使用，不自动冒称已验证后续合并源。

## 5. 成本和收尾

构建/测试（含首红）外部wall合计193.289秒；其中必要Release87.801秒，不重复叠加嵌套build/test。Godot输入转换163.375秒、正式导出3.382秒；实际本引擎118.515秒（原采样窗口2.449秒）、Godot19.969/16.953秒。转换、导出、加载与运行不混为稳态性能；启动失败成本保留。完整细分见21-cost-ledger.json。

Gate A预算变更/定向验证、B新DLL实跑、C Godot两新进程对照、D比较与入口同步已完成；这不授371整体验收完成。原完整BM11/BM03矩阵、backing、持续和正式资格及旧机余量缺口按41固定出口继续保留。总施工仍在当前；只关闭本轮阶段42任务，无本路后台施工。HEAD/index/空锁及owned进程最终复核见22-closeout.json；374 B行按收尾当时内容保留。
