# 371 Windows功能施工完成

2026-10-30终态：**371 Windows功能施工完成并归档，完整资格独立待验**（`windows_functional_construction_completed_with_deferred_qualifications`）。本轮修复native阴影参数在混合计划readiness→draw重复上传、导致准备成功后实际绘制容量拒绝的确定性缺陷；176byte原额度反例先红，修复后10项阴影直接消费者及scoped Clippy通过。正常457交付身份已核对，原480tick短场景6/6断言、1张非零捕获、自然exit0，实际159.265秒。原完整K2未重跑：455历史22618tick失败/2图/9终断言未产生保持，没有原失败帧packet/字节快照，故不宣称该历史点已逐帧重放、重复上传是其唯一根因或完整路线通过。本轮确定性机制已修复，历史问题保留为完整路线回归待验。深层内存、OS显示、正式性能/压力/额外生命周期、满载故障配方和精确75%可选视觉降级继续未验证；其中视觉降级缺完整实现/消费证据，是交付限制，安全拒绝不等于降级实现。Android延期。A功能施工关闭并释放，不自动消耗余下授权；B/374、C安装、冻结游戏V0.0.5及Git索引保持。

## 本轮新增完成与证据

1. 在既有真实GPU阴影夹具中加入一个mixed plan负例。旧实现readiness已用176bytes且返回true，draw仍重复上传176bytes，确切报`wgpu.render_surface_failed: geometry.visual_upload_capacity`。见[02确定性首红](../../../../.artifacts/371/functional-finish-20261030-003316/02-shadow-red/result.json)。
2. 只修改`engine_render_native/src/wgpu_backend/real/geometry/shadows.rs`这个产品owner：完整参数words与当前实际native depth TextureView逐slot相同才复用绑定；新增/变更/离开active集合仍走原准入和真实完成退休。未提高预算、删除拒绝或改变ACK寿命；新增words backing沿原opt-in内存观察接入。
3. 加强版负例证明readiness实际只上传一次、draw不重复扣费、后续同内容总uploaded_bytes不增、像素与原参考相同，retiring/staging归零。既有阴影像素/mask/atlas校验/实际在途寿命消费者共10项通过；最终构建及执行共4.698秒。见[05最终消费者](../../../../.artifacts/371/functional-finish-20261030-003316/05-shadow-terminal-consumers/result.json)、[06 scoped Clippy](../../../../.artifacts/371/functional-finish-20261030-003316/06-native-clippy/result.json)。01是测试初稿编译问题，04是无新上传时snapshot仍保留前帧frame_uploaded_bytes的oracle误读；均保留首错，未为过验放宽产品合同。
4. 通过原Compiler/正常Provider交付457，无手工替换DLL。首次456在353.875秒因系统available<4.5GiB守护中止，未到Player；现场外部ComfyUI任务保持运行。复核4.5GiB是Player/运行资格门槛后，仅将编译阶段的run-owned守护分离为1GiB系统余量、原6GiB单进程/单job及660秒上界，沿相同缓存续建（Player4.5GiB及原预算不变），457实际496.594秒，其中准备19.201秒，38/38项Cook复用。07失败保留，不以旧455冒称本轮新包。见[07b正常发布](../../../../.artifacts/371/functional-finish-20261030-003316/07b-publication-process.json)、[08身份核对](../../../../.artifacts/371/functional-finish-20261030-003316/08-publication-identity-audit.json)。
5. 新457执行原`bm10-k2-smoke.scenario.json`，480模拟tick、6/6断言、1张非零捕获，九cell/原人口、200ray和50sweep每tick、原预算及正常pacing保持，自然exit0，159.265秒。600秒场景上界/720秒外守护均低于15分钟；不是拆段完整长测。场景和三二进制前后SHA未漂移。见[09生产短验](../../../../.artifacts/371/functional-finish-20261030-003316/09-bm10-k2-smoke-process.json)、[原result](../../../../.artifacts/371/functional-finish-20261030-003316/09-bm10-k2-smoke/result.json)。

短验前的共享资源处理：用户明确授权只在ComfyUI队列完全空闲时卸载模型内存。重新核对running/pending均为空后，仅调用一次既有`POST /free`卸载模型并释放缓存，返回200；没有取消任务、删除文件或重启服务。系统可用内存从2.770GiB回升到44.634GiB，保持原Player资源门槛后启动。见[授权空闲释放记录](../../../../.artifacts/371/functional-finish-20261030-003316/08b-authorized-idle-model-unload.json)。

正常交付：[D:/GameEngine/EngineMigration/EngineMigration/01-Engine/main/samples/platform_3d_bm10_k2_project/Library/EngineTools/Deliveries/engine-op-371-bm10-k2-457-1/Build/Windows/release](D:/GameEngine/EngineMigration/EngineMigration/01-Engine/main/samples/platform_3d_bm10_k2_project/Library/EngineTools/Deliveries/engine-op-371-bm10-k2-457-1/Build/Windows/release)。这是可直接消费的新Windows样包，没有更新用户安装或冻结游戏。

| 产物 | SHA256（各自匹配457 manifest） |
| --- | --- |
| Game.exe | `8c4e94bb98abcd0d82848bcdc72aca89bb5804e60663aa34dfae10fb8b29a1f0` |
| engine_runtime.dll | `1bf0135dc8e63ca62e7eac93b3bd82bd320da42b72c998f7d2d4594ecef9ffe7` |
| data/bin/sample_371_bm10_k2_runtime.dll | `f8b2ba3bf1013053df5b14713d864e892bd412b33270cdb441e8a4ff9eba0a6c` |

## 既有成果复用与真实故障边界

导航修复、原九cell/56701实体实际draw后tracked清退、模拟三频、BM10 K3、BM13 K2、原Cook和基本安全owner按原身份/精确证明范围复用；本轮阴影改动不使未消费该owner的CPU取消/代次、队列有界、required Physics/Nav拒绝/rollback等证据失效。阴影binding的实际退休由本轮10直接consumer覆盖；不把旧55直接外推为新包完整长路线或driver全堆资格。

371-WIN-K2-22618的可确定重现机制已修复，状态为`reproduced_owner_defect_fixed; historical_full_route_regression_pending`。当前无其它本轮已复现而未修复的正常运行故障；这不是无故障穷尽证明。原455失败帧没有packet/具体调用点/当次bytes快照，因此不能证明该历史22618tick仅由此机制引起，也没有重新经过该点。该不确定性在交付限制与独立综合回归中保留，禁止写成原完整K2已修复验证通过。

原455完整路线保留失败：9763.875秒，22618模拟tick/22617呈现帧，2/5图，9终断言未产生，exit1。原场景48000模拟/36000路线tick、9断言/5捕获及SHA保持；本轮未启动完整K2。用户15分钟限制下，已在开工时把完整路线从本次功能施工关闭前置移交独立资格；没有缩短原正式场景或删除断言。

## 出口、经验与后续边界

本轮按已同步的最小修复→owner/直接consumer→正常交付→必要短生产消费出口结束，371总施工移入已完成，A槽释放。原371全部资格未完成，精确75%可选视觉降级缺完整实现/消费证据；参见[独立资格与限制](01-独立资格待验与交付限制.md)。实际回归仍回原owner处理，不因资格表未清空恢复功能施工。

施工经验：同名预算错误先检查readiness与draw是否重复收费，在既有consumer用精确剩余额度建立反例，比重跑数小时路线更便宜。复用必须比实际参数及native资源身份，不能只按逻辑atlas名。历史首错与本轮确定性因果分开，不凭相同错误码声称唯一根因。snapshot字段可能是最近有上传帧的历史值，是否重复上传应核对总uploaded_bytes变化，不能为错误oracle改产品。外部任务造成内存守护时保留任务和首错；先区分编译保障与Player运行资格，不能将正式Player余量门槛无差别套在编译阶段。本轮仅改编译外守护，Player4.5GiB与产品预算保持，不重做缓存/发布框架。

本轮开始北京时间2026-10-30 00:33:16，本记录写入时实际经过6517.785秒；最终结束时刻/耗时以[终态审计](../../../../.artifacts/371/functional-finish-20261030-003316/11-functional-completion-audit.json)为准。10小时是上限，不为耗满继续加任务。所有本轮测试均小于900秒；编译、实际测试、准备和等待不混记。精确本轮Token无法从现有usage接口取得，历史累计Token/工时未重新核算，不估造。
