# 371 阶段47：网格CPU源安全回收与同源重传

2026-10-24北京时间。371阶段47网格CPU源回收完成：静态/分段上传、取消退休和设备销毁后释放原字节，快照保留布局及完整内容身份，原包校验后可重取。202项去重源码/GPU测试通过，定向fmt/Clippy通过且修改文件无新告警；同原1.5×包720帧/exit0/截图像素一致。运行采样窗private峰3.771660→3.461681GiB，减少317.418MiB；全程峰4.566162→4.566578GiB，未改善启动峰。阶段48续补原S1故障与可访问backing，S1/371仍partial。未安装，游戏V0.0.5与预算保持。

证据根：`.artifacts/371/s1-completion-20261023-163338`。本连续窗口16:33:38 UTC开始，19:18:38停止新增、19:33:38硬截止；本块完成后继续阶段48，未重置计时，未声称S1完全完成。

## 实现与边界

原 `GeometryBuffers` / ProductionGeometry 弱共享表保留布局、逻辑字节数、原u64 hash和完整字节SHA256身份；只有具有完整record/cooked/hash/size/bundle来源的生产包几何转换为描述。普通/手工及不完整legacy来源保留完整CPU字节。没有第二资源管理器、SDK/schema改动或新设备恢复协议。

原静态visual上传batch和分段Progress持有明确完整源Arc。真实GPU完成才释放共享字节；外部CPU读者和其它上传继续持有各自Arc。最后票据取消时，已提交源转入同一完成队列；未提交源可直接退出。显式device.destroy后释放旧epoch源，不增加completed计数。

实际上传必须acquire完整源；准入、页大小和GPU统计均按不可变完整逻辑长度，不能因描述内字节为空而少算。已常驻GPU资源不读取包；新物理backend/CPU读取从捕获原路径重读并检验原hash/size/bundle和完整布局/字节身份。序列化显式重取完整wire，缺失/损坏返回错误，deserialize仍走完整validator，绝不把空数组作为mesh写出。

9个Rust文件（7产品、2测试；其中1个私有包源模块新增）及精确diff/最终快照见15-stage47-source。原dirty/index/空index.lock保持。当前基线包含374 Gate D，B登记保留。

## 验证与首失败

| 证据 | 实际结论 |
| --- | --- |
| 02-mesh-red | 实际GPU完成后，live snapshot/cache/native仍持顶点源，Weak无法退出；首红保留 |
| 05、06 | 静态完成释放、外部读者、缺失/坏包拒绝、恢复后完整wire往返/新backend重传、分段共享票据通过 |
| 09b-mesh-cancel-red | 显式共享源退出后取消票据，CPU上传源在真实completion前消失；实际失败保留 |
| 09c、10 | 取消源交给原完成队列后通过；device.destroy释放并不伪造completed；完整7项foundation GPU终验通过 |
| 07 / 09 / 08-test0 / 10 / 11d | 61 foundation +126 geometry owner +7 GPU lifetime +7 foundation GPU +1双32MiB取消代际 =202个不同测试通过；重复定向不重复计数，忽略不算通过 |
| 08-test1失败 / 08b / 10 | 旧夹具删包后又新准备，与46冷读取完整性要求冲突；保留拒绝断言，在新准备前恢复来源，原上传期间不读盘的断言仍成立；最终通过 |
| 11a–11e | 三crate定向Clippy通过；既有warning保留，修改文件无新告警；9文件fmt --check通过 |

F10原三消费者配方另在14-f10-owner-proof核对源码并执行2个既有精确owner测试；旧09测试binary消费的loader/audio源与当前一致，后续native取消修复不在这两个测试的消费路径。仅核销该故障子例，不等于BM11全表。

## 实际1.5×对照

相同原RuntimePackage/15000独立mesh、384材质、96张2048² RGBA8、15M三角形、同Game.exe/相同项目模块、720帧(120+600)、Immediate、allocationSummary=false。两侧实际截图像素一致，逻辑/物理mesh各468000000字节、纹理1610612736字节及总上传2113218912字节一致。原报告与逐0.1秒外部PROCESS_MEMORY_COUNTERS_EX记录完整保留。

| 指标 | 优化前当前基线 | 优化后 |
| --- | ---: | ---: |
| 全程PrivateUsage采样峰GiB | 4.566162 | 4.566578 |
| Windows PeakPagefileUsage GiB | 4.569401 | 4.569641 |
| 600帧窗口private峰GiB（Player既有窗口计数） | 3.771660 | 3.461681 |
| CPU mean/P95 ms | 4.1354/4.5367 | 4.9756/6.7660 |
| 全程最低系统available GiB | 35.001 | 32.851 |
| Player wall秒 | 159.625 | 29.828 |

运行窗峰减少332836864字节（317.418MiB）；全程峰反而增加446464字节，不能声称降低加载峰。最终峰仍约4.57GiB，低于原6GiB预算。两次磁盘缓存/机器背景状态不同，首扫wall和CPU分位数不是该改动的因果性能证据；不授正式持续性能资格。观测脚本phase标签没有获得实时Player采样窗事件，全部标startup；不能据此把全程峰归作稳态。窗口值单独取Player既有采样窗口字段，不根据外部时间线猜窗口。

## 构建身份与成本

基线Release 297.836秒；最终Release 469.773秒，两个Player分别159.625/29.828秒。完整源码/测试和每次失败输出、独立构建与测试耗时见各result。第一次Player在场景启动前因旧项目模块SDK digest拒绝，0帧，内存结果不参与对照；没有放宽ABI校验。复用原Compiler生成的glue/source，以当前SDK局部重编译34.07秒，输出仅放run-owned comparison-data/bin；原包/原module和安装未改。初次shell bootstrap路径错误被保留，使用绝对既有环境脚本修复；对原junction删除曾被宿主策略拒绝，未重试删除，改用新的完整对照目录，原链接保留。

最终DLL SHA256 `b1df5e75a4afa145f76ace97cdcb2cee7d247e891d8c1b9dee8182b53e72309a`；Game `422a951cbc474da9f8478d8936aaa61b349fb60da752e3ff5a3eac0d11a7cbd3`。两侧项目模块、源/产物身份绑定13/15和各identity；没有把旧SDK模块或旧DLL冒充新产物。

## 后续原S1出口

固定X1–X6逐项保持，完整BM03输入/压力/上限矩阵、F03/04/05/11/12未匹配组合、可访问backing、当前BM01预算、真实工具/显示与三轮正式持续/流式/全程4.5GiB仍须直接证据。网格CPU源机制与单次对照已完成，整体S1仍partial。接续阶段48；没有更新安装或游戏冻结版本。
