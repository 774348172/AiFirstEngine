# 371 阶段69：原规模动态CPU资格完成

2026-10-25 02:08 UTC：371阶段69本机原1×BM03 K1 CPU资格已完成。331源/332无诊断探针Release、333–338六个独立进程按固定顺序完成三组Summary/Off；各600预热+14000采样、至少120秒及7200tick，预热后原轨迹复位。三轮Summary CPU P95/P99为12.3555/13.3755、12.2467/13.2489、12.3917/13.4619ms，均满足13.3/16.67ms；统计P95增量0.3064/0.1004/0.3336ms全部满足原3%或0.2ms取大上限。最终增量125项去重验证（120新执行、5按不变输入复用）、fmt/scoped Clippy通过，1397引擎输入及10396项目源/资源指纹无漂移。仅授本机Windows RTX4090/Vulkan原1×K1 CPU资格；其它配方/放大档、完整S1/X1–X6、OS显示及Android保持原状态，不授全驱动显存峰值。未安装，游戏V0.0.5保持；CPU单目标达标后停止新增实现，无本路构建/Game后台。

## 通过范围与原门槛

本记录关闭用户指定的CPU优化目标，状态`pc_bm03_1x_k1_cpu_qualified_local`。只归属于本次实际dirty源码、引擎DLL、原Game/资源包和本机设备配置，不是clean exact-commit或安装资格。原10000独立mesh、1000万三角形、256材质、64张2048²纹理/1GiB decoded、运动相机与选择阶段均保持；Vulkan、1765×993、Immediate、target-fps0。CPU统计排除显式Present/pacing等待，门槛P95≤13.3ms/P99≤16.67ms。

每窗600预热后恢复同一配方/相机起点，14000正式样本/14600项目帧，独立满足≥120秒wall与≥7200tick。三轮Summary与三轮Off固定顺序S0/O0/O1/S1/S2/O2，同一二进制和包；两种模式均显式cpuProbe=true、nativeEvents=false、allocationSummary=false。Off保留必要CPU计时，不称无探针。实际模拟tick/wall差在审计容差内，无慢放；保留所有异常帧。

## 完整窗口结果

| 窗口 | PID | CPU均值ms | P95 ms | P99 ms | 正式采样秒 | tick跨度 |
| --- | --- | --- | --- | --- | --- | --- |
| 333-direct-summary-0 | 56588 | 10.0107 | 12.3555 | 13.3755 | 144.929 | 8693 |
| 334-direct-off-0 | 27740 | 9.6799 | 12.0491 | 12.9836 | 139.883 | 8390 |
| 335-direct-off-1 | 28916 | 9.7717 | 12.1463 | 13.0850 | 141.168 | 8468 |
| 336-direct-summary-1 | 2600 | 9.9562 | 12.2467 | 13.2489 | 144.095 | 8643 |
| 337-direct-summary-2 | 50576 | 10.0502 | 12.3917 | 13.4619 | 145.414 | 8722 |
| 338-direct-off-2 | 35092 | 9.6808 | 12.0581 | 13.0885 | 139.867 | 8389 |

| 配对 | Summary P95增量ms | 原上限ms | 结果 |
| --- | --- | --- | --- |
| 1 | 0.3064 | 0.361473 | 通过 |
| 2 | 0.1004 | 0.364389 | 通过 |
| 3 | 0.3336 | 0.361743 | 通过 |

六窗各自正常退出、呈现不少于14600帧、截图captured且文件字节数匹配；无runtime error、无诊断guard截断，外部所筛选cargo/rustc/link/其它Game竞争快照均为空。外部0.1秒采样private峰最大3477557248 bytes（3.2387GiB）<6GiB，system available最低34995916800 bytes（32.5925GiB）≥4.5GiB；该private峰是外部观测采样峰，不冒称连续精确峰。内部采样期private峰也逐轮通过。单帧CPU最大62.1524ms完整保留，达标声明针对原P95/P99合同，不表示所有帧都低于13.3ms。

原图形物理分配准入保持4GiB，actual census及预算拒绝消费者仍在；本轮Summary不导出完整物理高水位，因此不虚构全驱动/OS显存峰值或完整GPU内存资格。GPU timestamp仅作为实际Summary消费证据；OS显示timing unavailable保持原未资格，不属于本CPU目标。

## 保留的实现

阶段69相对01-baseline-source共19个Rust输入变化，其中新增geometry/streaming/source_tests.rs；不是只有最后三个文件。完整清单见终审`stage69ChangedInputs`。最终相对196基线仅geometry.rs、geometry/template_backing.rs及native_template.rs变化，最后一项只补原owner测试。

- 同次准备共享不可变事实和已验证源闭包；大选择复用既有最多四worker只读准备，原GPU变更和提交保持原线程。
- 全单实例视图直接发布既有scalar命令，避免生成随后丢弃的GeometryBatch包装；实际批/混合选择、完整校验和最终命令语义保持。
- 已准入视觉mesh前缀的CPU读取/校验/解码复用既有有界worker；错误和资源发布顺序保持。
- 有序源Vec直接移动到成功计划，moving骨架复用既有选择遍历，减少重复reconcile/capture。
- 物理census使用原生对象身份hash去重；成功计划保存原先已计算的确切recording keys，仅在两输入集合标记有效、实际完整键逐项匹配且所有Recipe ready时免扫已被真实Recipe覆盖的两组模板依赖。错误key、部分准备、失效ready或替换集合均回完整路径；Trace仍逐原角色观察，新增Vec backing纳入CPU观测。
- 测量来源分类直接匹配Geometry/GeometryBatch，免重复跨crate解头；批量所有选中行仍检查，mixed来源仍返回None，不新增cache/资源owner/API。

普通测试项目仅为600预热后的轨迹时钟复位修正RuntimeModule/src/lib.rs，普通Compiler169正式生成新AOT绑定包；原相机/phase数学与资产保持。159原module四项测试验证全部原轨迹tick/重复、预热/首采样合法起点、完整计划和四阶段非前缀选择。

## 终态身份与验证

- 引擎源码：[331全1397输入](../../../../.artifacts/371/cpu-selection-publish-20261024-2143/331-direct-source-source.json)，[源码归档](../../../../.artifacts/371/cpu-selection-publish-20261024-2143/331-direct-source-source.zip)。最终哈希无漂移，保护的Player/Spine/wgpu_backend.rs没有本路变化。
- Release：[332构建结果](../../../../.artifacts/371/cpu-selection-publish-20261024-2143/332-direct-source-release/result.json)，一次成功141.430秒；saved player为`333-direct-source-player`。实际执行DLL SHA256 `6d6bbce2d965f652a6ed742fedfd29f88530c2ec20c659f152e30e3b667e1f80`；Game SHA256 `a1b1bc8ee3ae37bda68edb80f6ebd05accce0bbfe0cd3dbf35d45c38c7ea13c6`，Game保持原198副本身份。
- 原包manifest SHA256 `ceac80e3274060f4f1fbaa2ff82dacc01690799bb0056bf9dbc2dff77fe43bc7`；AOT `sha256:04f6cad033976a489cd73341041ccd66c9c6167f48d47591eddea1b527f34be9`。每窗实际module receipt passed/ready，session advanced至14600且无terminalFault。export中旧engine DLL仅是包旁历史产物，不冒称实际执行DLL；执行用argv指定saved Game所在的新DLL并强制AIFE_REQUIRE_ENGINE_DLL=1。
- 最后增量125不同测试：327来源分类oracle1项、328几何/内存直接消费者118项、329实际Spine/ordinary交错时间戳与像素consumer1项；305物理覆盖owner5项按未变化执行输入复用。不要将历史重跑次数相加为新测试数。早期core/任务/真实package消费者的原结果和身份保留scope.md，最新修改不影响其执行输入，不重跑workspace。
- 330 scoped Clippy通过，原15项native告警保留，修改行无新告警；三文件fmt通过。见[331自审](../../../../.artifacts/371/cpu-selection-publish-20261024-2143/331-audit.json)。
- [最终逐项审计](../../../../.artifacts/371/cpu-selection-publish-20261024-2143/340-final-audit.json)：原6.1/6.3窗口/成本、六个独立PID/runId、固定顺序、二进制、包/module/复位源、CPU/内存/竞争/guard/错误及实际截图全部通过。[六窗输入](../../../../.artifacts/371/cpu-selection-publish-20261024-2143/333-direct-inputs.json)绑定原始request、report、memory和pair审计。

## 失败保留与边界

308上一候选第二组成本0.5279ms超限，314/315未运行；295数量覆盖存在证明缺口，虽两对通过也不授资格。共享material/Locations、弱覆盖、纹理描述、位置重映射和冷capture并行等无整帧收益实验均已逐字撤回；不得使用其二进制或诊断旁路授资格。316/320等临时诊断均恢复，最终331只含保留生产改动。

325/326测试fixture违反空选择/首选中行合同的首错保留，327只修测试而未放松validator。最终审计脚本首次错误地读取Off不存在的GPU字段，已修正为Summary专属检查；原CPU/窗口/成本条件保持、未重跑任何性能窗口，见[脚本首错](../../../../.artifacts/371/cpu-selection-publish-20261024-2143/340-first-audit-error.txt)。Compiler161构建超时和普通169基于已完成cache的成功续编分别保存，不伪造首轮成功。

CPU达标后停止本路新增实现，不继续Godot/历史20ms研究或其它S1施工。完整S1、其余X1–X6、其他recipe/放大档、OS显示、Android和工具交付资格按原登记保留；本记录不关闭总施工、不自动安装，不升级游戏V0.0.5。用户12小时授权自2026-10-24 17:47:34 UTC起，当前达标提前收尾，未触及05:32:34停止新增/05:47:34硬截止。

完整过程和首失败见[施工范围流水](../../../../.artifacts/371/cpu-selection-publish-20261024-2143/scope.md)。本记录写入后仅同步49/54及当前施工/两入口；源码和运行证据不改写。
