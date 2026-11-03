# 合并Release与统计成本首错

2026-10-25 07:55 UTC。S1仍partial。此记录纠正旧入口“F04待补、修复尚未进入Release”的过期状态。

266必要合并Release用时501.378秒；269冻结DLL SHA256为3082f423a5d7176c47f17a39d22ee8541025327b010792419804e00e5fc745e3。自前一a914 DLL以来，生产变化仅为AssetLoadDiagnostics有界历史；F04补充只有测试变化。273强制新DLL加载的16帧、module/session、21 HUD、capture和正常退出通过。六个未变Compiler导出的Game/data按271复制并绑定新DLL，没有重复Cook/export，也不是新的Compiler交付或安装。

280 K0/0%按原正式顺序已执行Summary0、Off0、Off1、Summary1，各600预热+4200样本，实际采样wall约70.12–70.15秒；身份、轨迹/时钟、资源稳定、内存及采样competition检查通过。CPU P95分别3.5810/3.4508/3.2686/3.4749ms，均满足13.3ms；P99均满足16.67ms。第一对统计增量0.1302ms通过；第二对0.2063ms>0.2ms，超6.3微秒，runner首错exit1。Summary2/Off2和281–285均未启动。280原输出保留，不降低阈值、不删除异常帧、不自动重跑挑绿。

现有stage means只能缩小范围：Off1→Summary1 update均值增加0.1110992ms、renderSubmit增加0.0367091ms，不能将嵌套阶段或P95相加。两个Off窗口自身P95相差0.1822ms，跨进程变化可能显著，尚不能确认代码首因。两档allocationSummary均false，CPU probe均开启；全payload内存census不在本次路径。虽请求runtime-report-level不同，但NativePlayerRuntimeComposition::from_bound当前对两档均启用ProjectRuntimeSession Summary，因此不能把update差异直接归因于session报告档位。Summary另有AUI计时/工作计数、GPU timestamp及bounded资源聚合，需分离其实际成本。

下一仅在无编译/GPU竞争时进行一次有界档位分离诊断；该诊断只回答原因，不授正式资格。无确认缺陷不改生产实现。等待测量条件时核销原BM11/BM12确切缺项，已通过F03/F04/F05/F10/F11/F12子例按各自身份复用，不能笼统重复全部故障矩阵。371-G的全内存、完整流式、OS显示及Android缺项仍保留。

证据：280-bm01-k0-zero-audit.json、286-bm01-k0-zero-first-cause.json、267-f04-scaled-audit.json、273-merged-release-smoke-audit.json，均位于.artifacts/371/s1-exits-20261025-033155。旧资格和失败各保留自己的产物身份。
