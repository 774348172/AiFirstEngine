# 374 Gate F原owner分配修复与并行交接

Gate A–E已关闭；F本轮按6文件串行合入Spine默认输出容量随实际几何调整，以及Player有界测量history启动前预留/普通Off零分配。272项不同测试通过，其中主线76项去重组合；6文件fmt和scoped Clippy通过，增改行无新增告警。371当前源码与A行保留。未在371活动窗口启动GPU/正式Release复测，上一版single CPU与增长失败保留为旧源码结果；新源码预算未测，F/374仍partial，未安装，游戏固定V0.0.5。

## 授权、范围与实际结束

用户明确授权在371施工时开始可并行下一步。2026-10-24 09:13:57 UTC开工，11:58:57停止新增、12:13:57硬截止；实际于2026-10-24T09:40:13.515857+00:00结束，累计1576.5秒。证据根`.artifacts/374/gate-f-owner-20261024-091357`。本记录关闭本轮窗口，F和374均不关闭、不归档，无B自有后台施工。

主线371通用geometry、runtime_renderer/render_thread及native准备/模板接缝归A所有，本轮不写这些文件。B保留原374目录与target，先登记54/总施工§33，Player范围补登记后才修改。`source-scope-audit.json`证明W仅6源文件变化；主线发布逐文件比较原SHA、保留before-publication.zip及handoff.diff，未同步旧W通用源码覆盖371。A的原target构建和09:27左右probe运行只读观察，未取消或终止。

## 首红与最小修复

1. `backend-red-01.log`：真实3顶点/3索引输出预留1,441,792 bytes，16KiB上界失败；大小皮肤切换后的暖输出也仍保留最大空间。原`geometry()`每次按32768顶点/98304索引初始化，属于已确认分配，不把它直接等同整帧预算失败的全部根因。
2. 在原Instance增加两个usize容量提示（64位本机16 bytes），默认先64顶点/192索引；成功后按实际长度上取整并限制在原上限。只保存容量，不保存姿态/skin/geometry/原生引用。若skin、attachment或clipping增长使提示不足，同一只读事务提取最多回退一次原完整上限；显式`geometry_with_capacity`拒绝语义保持。小三角默认顶点/索引存储从1,441,792变为2,816 bytes；首次增长回退仍可能暂时使用原最大容量，不能说所有帧都只分配2,816 bytes。新几何与完整容量提取逐值一致，降回小skin后容量收回，实例之间提示不共享。
3. `player-red-01.log`：真实窗口原6个CPU测量history使用Vec::new，4200帧固定backing要求在启动容量0处失败。新增helper先保留这一原行为形成首红，再统一三条原Player路径的初始化。启动时依原有MAX_CAPTURE_FRAMES=262144和已验证frame_limit预留；普通Off容量0、closure不求值，显式Off CPU probe/Summary/Trace仍保留全部4200帧。
4. 原AuiFrameWorkCounters每项1,696 bytes，4200项backing为7,123,200 bytes，五个u64时长history另168,000 bytes，总7,291,200 bytes在启动前预留。测试证明直到帧4200容量及指针不变、首末帧和计数完整。预留不等于全部页面已触碰、实际private稳态或全部嵌套分配消失；GPU样本、提交回执、driver和功能路径可能仍增长。无删样本、截尾、改预算或从旧增长中扣数。

原C ABI、159个官方vendor源、导入边界、冻结切换点/规模及官方update/apply/vertices/clipping计算保持；本轮未修改projection编码。上一版Control/Official/Vertices/Projection各自P95不能相加，新版整帧/四段时长未实际测量。

## 验证与证据身份

| 位置 | 实际验证 | 通过数 |
|---|---|---:|
| 独立374后端 | gate_a 11、gate_b 22、gate_c 10 | 43 |
| 独立374项目消费者 | spine374_project常规项；6个显式GPU/Release诊断ignored | 21 |
| 独立374 Player | 原lib单测含三档策略、raw样本、headless组合及本轮固定backing；6个真实OS/GPU诊断ignored | 208 |
| 主线后端 | 同三组，发布后执行 | 43 |
| 主线Player | measurement过滤，含原Off probe、固定backing、wire兼容及raw过滤 | 12 |
| 主线项目 | 同21个实际项目/HostLoop消费者 | 21 |

不同测试名并集272；主线76是其中重复验证，不加到总数。Player整套是本轮原owner一次终验，3.39秒实际测试，不称272个全新测试或全workspace回归。backend与Player首红及完整终验输出均保留，不用测试编译成功代替运行。backend和Player scoped tests Clippy终态成功，增改行诊断空；W 6源fmt通过，主线6源与交接SHA一致。已有main native `with_dense_capacity`及core告警属于另一范围，保留，不宣称全workspace零warning。

独立W最终1325文件（当前扫描后缀集合，不含前窗额外两个sample文件）seal `7a06466cd87dedae8cb40d1201604802acb6d65039d68bd0b8cc18b9774c73a6`。主线组合前1305文件seal `b83930014714426bd652fdaa63956beb05fb273b2960e6bea17b8dbaf33ea5a4`，组合后seal `6a949d283ecd51fd3eaefa4056de0aef3b7bb106eeef2a8134af8bfeb19b99b6`。主线组合期间A另外改动4文件，详见validation-summary.json；6个B文件始终匹配交接SHA。已运行测试只证明其实际构建的组合，不给后续A源码冒领相同资格。 本轮是dirty内容身份，不是新Git提交。Git index及原零字节index.lock字节保持。源码、测试、产物与外部状态分别记录；未构建新Release二进制，不复用旧Game/DLL冒称新源码产物。

W项目测试未设置SPINE374_EVIDENCE_ROOT，按原fixture默认保存于374独立目录`.artifacts/374/gate-b-integration/*-10952-*`；这些属于本轮PID的真实证据并保留，不改写历史其它fixture。主线项目测试显式使用本轮`main-fixtures`。日志、逐文件发布SHA、并行源码差异与进程见[本轮closeout](../../../../.artifacts/374/gate-f-owner-20261024-091357/closeout.json)。

## 未关闭的出口与下一步

371仍在构建/CPU优化窗口，正式GPU/Release性能必须避开机器竞争，本轮未运行。前窗旧源码single CPU P95 0.8018ms及tail1200 private 4545.32 bytes/frame超限继续保留，不改成新源码已达标或仍是同一数值。当前最小下一步是在无A构建/Player竞争的窗口，用本轮新源码执行原single WindowsRelease及同二进制空场景预算；若仍失败，按新四段与增长证据回到原owner，不能凭分配量降幅外推CPU时间降幅。

普通Off增长尚未定位；本轮固定CPU测量backing不证明Off或driver增长修复。projection/Prepare剩余成本仍须实际分段反馈。single通过后才能继续三配方各三轮、Off/Summary/Trace成本、最终无测量产物与目标硬件资格。原预算不降；F/374 partial，施工文档保留当前/B槽，无安装、SDK/schema/全局环境或游戏V0.0.5升级。
