# 326 开发构建与交付验收解耦施工

状态：已完成并归档（source_integration_passed）。用户授权修正夹具并归档；本轮收尾补丁未更新安装Provider。下列进度和缺口段为修复前历史，终态以文末为准。

必要性：真实重复 build 约16秒，其中RuntimePackage 3.3秒、Player启动验收2.6秒，其余尚未分段测量，不能全部归因于复制/写入。最小改动是已有RunOptions/BuildRequest透传 `verify_player`，Provider的build设false，run默认true；保留delivery_verify显式验证。不新增schema或工具。Provider已按本轮用户授权替换，config未修改。

Gate A：DesktopExportRequest/RunOptions owner测试默认值与透传；cargo test -p project_authoring_execution --lib desktop_export --locked。
Gate B：provider编译与MCP build回归，确认build结果仍有deliveryRef、package和artifact身份，且不隐式启动Player；delivery_verify显式通过。定向engine_tool_provider测试。
Gate C：真实宿主重复构建计时，对比326前16秒；记录RuntimePackage与交付剩余成本，更新入口并归档。

当前进度：真实宿主构建与显式验收已通过，缓存重复计时完成；发现证据引用合同缺口，326保持当前施工，暂不归档。既有project_authoring_execution 60项与engine_tool_provider 25项测试通过；此前没有新增专门覆盖默认值/透传的回归测试，不把既有suite通过等同于该新增需求已有定向覆盖。

## 2026-09-11 真实宿主验收

实际安装Provider SHA256：`446D944EAECFE9A08F9F9C33B5025A416CEFC5BE036D44C1FB5C8232B1D065AB`。宿主会话PID6356，已可调用typed工具，不再等待重启。

| 调用 | 耗时 | 结果 |
| --- | ---: | --- |
| 完整项目首次build，6356-2 | 84000ms | completed；Player重编译151项；not_started |
| 无修改重复build，6356-4 | 13693ms | completed；缓存hit，0 rebuilt；准备10/10复用 |
| 重复交付显式verify，6356-5 | 约2秒 | passed；1帧完成，进程/Player退出码均0 |

重复build的RuntimePackage阶段3314ms；其余10379ms尚无完整阶段归因。相比325宿主重复16025ms，本次减少2332ms（14.55%），仅为单次对照，不是统计性能保证。首次84秒与此前不同缓存/源码条件不可直接当回退或提速结论。

build操作：`engine-op-mcp-process-6356-4`；verify操作：`engine-op-mcp-process-6356-5`。
deliveryRef：`engine-delivery:sha256:918ae09ecf77c5d16bd52e3fb4b9f96d11bbce7a2a43db841f4fe6f8b9316e73`。
artifactIdentity：`sha256:154bdb4cfb17a34fd33887e123a76f8487d2e6da4ff18c82ff55c828dbe314ef`。
projectRevision：`sha256:08d54716f1418d6cea93917de592093dd1ad662132acca8aa7809c76262c32ff`。

原始build回执和当次缓存报告已归入阶段完成记录`2026-09-11-326-Development-Build-Delivery-Decoupling-v1/evidence/`，避免后续构建覆盖缓存报告。交付报告根目录：`samples/complex_shooter_project/Library/EngineTools/Deliveries/engine-op-mcp-process-6356-4/Build/Windows/dev/reports/`。显式verify前该目录只有desktop-export-report.json；其中playerExecutableStatus=copied、playerExitReason=not_started、playerExitCode=null。verify后才产生运行报告，不能用verify后的文件存在反证build隐式运行。

## 最终验收

Gate A/B/C完成。准确修正观测夹具，补serde与低层策略2项测试，扩充既有集成用例验证不运行/证据存在/显式verify/复制失败。最终62项owner lib、25项Provider lib、3项desktop集成全部通过，rustfmt与diff检查通过。此前真实宿主计时仅绑定当时Provider，收尾补丁安装态未更新；未重新宣称宿主资格。详见[326完成记录](../../阶段完成记录/2026-09-11-326-Development-Build-Delivery-Decoupling-v1/00-总览.md)。当前槽释放，不自动激活新的Player编译方案。

## 已发现的收尾缺口

1. Compiler构造DeliveryRef时无条件加入player_report_path；普通build尚未生成该文件，evidenceRefs因此悬空，显式verify还会重复追加。应只发布实际生成的证据，复用已有接口，不新增工具。
2. DesktopExport在verify_player=false时直接放行player_gate；源码中复制失败仍是warning，需要定向验证缺少Player不会被误报为完整成功交付。当前实际样本copied且显式verify通过，不受此失败分支影响。
3. 补默认策略与失败分支的廉价owner回归；serde跳过字段的默认false与构造器true也需核对调用边界。完成前不宣称Gate A全覆盖或326全部完成。

历史失败：独立MCP探测出现LNK1181，不作为产品逻辑测试失败；首次解耦实现仍以Player退出0判断最终成功，导致未启动被判失败，已经修复并由上述真实宿主build证明。首次失败和修复事实保留，不以最终成功覆盖历史。

自审：不移除验收能力，只改变开发build默认策略；用户可通过既有delivery_verify完成同一验收。预计30分钟，失败先回滚默认策略，不扩大到RuntimePackage缓存重构。

## 本轮收尾验证范围

用户授权修正夹具并归档。旧夹具在字符串匹配阶段即失败，不能解释为Player默认策略失败；`values: output.values`属于UI，实际应注入`ProjectRuntimeObservationOutput`。更新既有集成用例，覆盖普通build未运行且证据存在、同delivery显式verify、既有语义与防篡改验收、缓存命中及增量、复制失败时关闭验收仍失败。复用同一次Player构建，避免新增大型runner。低层默认策略/serde用2个lib测试覆盖，Provider消费者跑lib suite。估计真实编译2–5分钟，超过10分钟检查首因。整个workspace/Local CI/安装替换不属于本次源码收尾；旧宿主13.693秒与显式verify证据只证明当时安装身份，不证明本次收尾代码已安装。
