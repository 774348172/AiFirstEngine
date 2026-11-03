# 301 post-300 Production Editor / Tower Preview Consistency Minimal v1

状态：已完成。allocation pressure one-owner repair、run-owned harness 首次 trust Ready 修复、新 qualification seal
与一次 production transaction 均通过；production Editor 已更新为 qualified candidate，真实 Tower Preview cache
已由普通 Editor 重建并通过 Open / Play / 60 秒内存平台化 / 点击 / Stop smoke。

正式方案：`../../301-post-300-Production-Editor-Tower-Preview-Consistency-Minimal-v1方案.md`

## 唯一 Gate

1. 在 `G:\AifeProductionRuns\post-300-consistency-20260819-005252` 记录 source、production Editor、Tower
   cache 与进程 preflight。
2. 使用独立短 `CARGO_TARGET_DIR` 构建标准 debug `editor_host.exe`：
   `cargo build -p editor_host --locked --features real-window,real-wgpu-surface`；candidate 无参 readiness
   必须 exit 0。
3. byte-exact 备份 production Editor 与 Tower `scene-main` cache；pending copy 替换后 candidate/after
   SHA-256 必须一致。
4. 只移出 Tower `scene-main` cache；用替换后的普通 production Editor 和 run-owned state 打开真实 Tower，
   Play 触发正式 Preview owner 重建。
5. 在同一普通 Editor 会话执行一次真实 Tower AUI 点击并观察业务 action，然后 Stop；记录 cache manifest、
   runtime/frame、进程清理与 rollback 状态。
6. 成功后写一次完成记录，同步 `49`、`54`、两份 README 并归档本文件。失败则立即恢复 Editor/cache，
   保存首个失败证据并停止。

## 必要性与验证经济性

```text
requested claim: 普通 production Editor 已包含 300，且 Tower Open/Play/click/Stop 可用
minimum change: one standard Editor build + one binary/cache transaction + one ordinary Editor smoke
excluded: product source changes, Player/export, Local CI, full E2E, visual matrix, real config
Gate -> risk: wrong binary/cache or unusable installed flow
cheapest red-capable evidence: candidate/after hash + rebuilt manifest + one real action + Stop
reused evidence: 300 ABI/SDK/Tower/Editor/Player directed tests and 13/13 source-linked smoke
invalidation: any product-source change after candidate build, hash mismatch, or smoke failure
estimated cost: 5-25 minutes; stop/review if candidate build exceeds 30 minutes or first mutation fails
```

激活前复核：当前无其它活动施工文档或待执行文档；production 仍为 post-298
`E27EE515...BF163`；300 source closure 已归档；用户授权与本 Gate 完全一致。自审通过，没有过量施工。

## 本次执行结果

```text
run root: G:\AifeProductionRuns\post-300-consistency-20260819-005252
candidate build: passed, 320453 ms
candidate SHA-256: FB6DA0C90514E06B47712604173DF04F5B05A9594D42DDF3193CDD6E7B77EB12
candidate / replacement readiness: passed
production replacement: candidate/after hash exact
Tower cache rebuild: passed in 10093 ms, cacheKey=98064b423c9c8000
ordinary Editor: Open passed, trust approval observed, Play rendered Tower
terminal failure: memory allocation of 1048576 bytes failed
failed stage: after Play, before the authorized AUI click and Stop
```

按 Gate 合同立即回滚。production Editor 已恢复为
`E27EE515D365E8E778D5166249CB804B6A3665B67889E4DBD4FC15B7435BF163`；Tower cache 已逐文件恢复为
93 files、manifest SHA-256 `6710E2372C4F1FD5FC5670A8186C89C8DBD4539409FE5CA2BBAE7CC8DB4755D8`；
production trust store 恢复为批准前 2 files；Editor/Cargo/rustc 残留为 0。candidate、重建 cache、批准后
trust store、stderr、截图和 receipts 均保留在 run root。当前未完成点击/Stop，不得声明 301 完成。

## Allocation owner 诊断补充

2026-08-19 的单独有界诊断确认 owner 为
`engine_runtime::render_thread_worker::RenderCommandDispatcher::inline_reports`。inline submit 在返回 immediate
完整 output 的同时，把 `output.clone()` 保留进 cache；cache 只由 ticket poll 删除，而 EngineHostLoop 的
immediate 路径直接返回。487-item AUI A/B harness 中，不 poll 连续两次分别以约 203,878 和 204,595
bytes/frame 线性增长；每帧 poll 对照均平台化。1 MiB glyph atlas 可能是内存压力耗尽后的失败申请者，
但不是 pressure owner。

证据见阶段记录 `01-allocation-failure-owner-诊断.md` 和 run-owned
`allocation-owner-diagnostic.v1.json`。诊断阶段未修改产品源码或外部 production 状态；后续 source repair
已单独完成，但不自动包含 production replacement 或 301 重试。

## One-owner source repair

授权范围：只修复 `RenderCommandDispatcher` inline-immediate duplicate retention；不修改 EngineHostLoop、schema、
renderer、production Editor、真实 Tower Preview cache、真实配置或 trust store，不运行 Local CI，也不重试 301。

```text
confirmed failure: inline submit 返回 immediate output，同时向 inline_reports 保留完整 clone
minimum causal change: inline-immediate 成功路径不再缓存第二份 output/report
files: render_thread_worker.rs + 本文件/失败记录的单次证据同步
excluded: dedicated worker fallback、公共 API/schema、GPU/render 重构、production mutation
red test: inline submit 后 immediate 可用且 inline_reports 始终为空
direct consumer: existing EngineHostLoop inline render submission test
terminal evidence: owner test + consumer test + focused engine_runtime fmt/check
stop condition: test 证明现有 caller 依赖 inline immediate 后再次 poll，或修复需要跨 owner
```

激活前复核：owner 已由重复 A/B 确认；现有 inline test 只要求 immediate report，不要求随后 poll；最小反事实
是在产生 immediate 时不创建 duplicate cache entry。无需新 schema、helper、runner 或 production 验证。

### Repair 结果

`RenderWorkerMode::InlineDeterministic` 已移除向 `inline_reports` 写入完整 cloned output/report 的四行重复缓存；
dedicated worker fallback 未变。修复前 300 帧 no-poll 净增 61,534,208 bytes；修复后 dispatcher no-poll、
poll 与 EngineHostLoop consumer 分别仅增 643,072、479,232、729,088 bytes，均低于 5 MiB 上限并平台化。
`cargo check -p engine_runtime --locked` 与 `cargo fmt -p engine_runtime -- --check` 通过。

仓库 owner test 已添加，但其运行被既有 `render_thread.rs:552` test fixture 的 E0560 编译错误阻断；未跨 owner
修改该问题。替代绿证据由 run-owned public-API dispatcher + EngineHostLoop consumer harness 提供。详见
阶段记录 `02-one-owner-source-repair.md` 与 `allocation-owner-repair.v1.json`。

当前 source repair 已完成，但 production Editor、真实 Tower cache 与 301 主 Gate 均未更新；下一步需要新的
production consistency 授权，不能沿用本次 source repair 权限。

## Post-301 Production consistency

新授权 run root：`G:\AifeProductionRuns\post-301-consistency-20260819-100416`。

```text
source identity: HEAD acaffa8c... + dirty repair source E532217E...D53F
old production: E27EE515...BF163
old cache: 93 files / manifest 6710E237...755D8
new candidate: must be built in fresh run-owned target; old FB6DA0C9...7EB12 is ineligible
transaction: backup Editor/cache -> exact replace -> only Tower cache rebuild
acceptance: ordinary Tower Open / Play / >=60s non-linear memory / one click / Stop
rollback: any failure restores old Editor and 93-file cache before reporting
excluded: Local CI, real config/trust-store mutation, Player/MCP/other binaries, full visual/E2E matrix
```

激活前复核：fresh root 原先不存在；production/cache 与 301 rollback identity 一致；repair source identity 未漂移；
Editor/Cargo/rustc 为 0；G 盘可用 89,603,489,792 bytes。本次是新的 source-bound transaction，不继承旧
candidate 或旧失败 attempt。

## post-301 production consistency attempt

2026-08-19 使用 fresh root
`G:\AifeProductionRuns\post-301-consistency-20260819-100416` 构建新 candidate；SHA-256 为
`0F96CD3B1C4AF228F7637489FEBC3B2FCE5980262815D966A80B47AD2F58AC36`，candidate 与 replacement readiness
均通过。production Editor 与原 93-file Tower cache 已完成 byte-exact 备份和 replacement。

真实 attempt 在 `post301.play_report_timeout` 停止。首因是 run-owned 自动化误取 `editor_host` 的
console/terminal 主窗口句柄，坐标没有落入实际 Editor 的 Tower recent row；Tower 未打开，因此没有形成
allocation repair 的 60 秒内存判定，也没有执行 AUI 点击或 Stop。按 Gate 合同立即回滚：production Editor
恢复为 `E27EE515...BF163`，Tower cache 恢复为 93 files / manifest `6710E237...755D8`，进程残留为 0；
Local CI、真实配置和其它二进制均未触碰。

run-owned harness 已修正为按 PID + 精确标题枚举真实 Editor 顶层窗口，但本次授权不自动重试 mutation。
阶段记录见 `../../阶段完成记录/2026-08-19-post-300-Production-Editor-Tower-Preview-Consistency-Minimal-v1/03-post-301-consistency-attempt-harness-failure.md`。
301 仍未完成；后续需要新的单次 production consistency 授权与 fresh run root。

## post-301 production consistency attempt 2

2026-08-19 使用 fresh root
`G:\AifeProductionRuns\post-301-consistency-20260819-103722` 冷构建新 candidate；SHA-256 为
`4AED3F98A899F2785283E50AA3F612AD46452D9F2BEB6B70AEF069CF7F94B090`，构建约 193 秒，candidate 与
replacement readiness 均通过。production Editor 与原 93-file Tower cache 已 byte-exact 备份和 replacement。

修正后的 PID + 精确标题窗口选择成功取得真实 Editor，但 launcher 没有加载 run-owned Tower recent row。首因是
Windows PowerShell 5 的 `Set-Content -Encoding utf8` 写入 `EF BB BF` BOM，Rust `serde_json` 拒绝 recent store。
确认 Tower 未打开后主动结束 owned Editor，使 transaction 按合同回滚。production Editor 恢复为
`E27EE515...BF163`，Tower cache 恢复为 93 files / manifest `6710E237...755D8`，进程残留为 0。

harness 已改为 `.NET File.WriteAllText + UTF8Encoding(false)`，但没有再次 mutation。Play、60 秒内存、点击与 Stop
均未形成产品判定；Local CI、真实配置、Player/MCP/其它二进制均未触碰。阶段记录见
`../../阶段完成记录/2026-08-19-post-300-Production-Editor-Tower-Preview-Consistency-Minimal-v1/04-post-301-consistency-attempt-recent-store-bom-failure.md`。
后续需要新的单次授权、fresh run root 与新 candidate；不得复用本次 candidate 或 mutation attempt。

## post-301 qualified production consistency attempt 3

用户授权顺序：

```text
freeze production replacement
-> fresh run root + new candidate
-> run-owned Tower project/cache/state qualification
-> prove BOM-free recent store is consumed by the real Rust Editor
-> prove Tower Open / Play / real click / Stop
-> freeze candidate SHA-256 + harness SHA-256
-> one production Editor/cache transaction + the same smoke
```

硬边界：qualification 任一失败时不得备份、替换或移动 production Editor/真实 Tower cache；不得在 qualification
后修改已冻结 harness 或 candidate。production transaction 任一失败立即恢复旧 Editor 与 93-file cache。继续禁止
Local CI、真实配置修改、Player/MCP/其它二进制替换、完整视觉/E2E matrix。

qualification 使用 Tower 的 run-owned 副本并排除真实 `.aife` 与 `RuntimeModule/target` 派生产物；
`Library` 默认不复制，但 Preview assembler 真实消费的 3 个 `Library/AssetPipeline` cooked 索引文件例外，必须
byte-exact 复制到 run-owned 副本，否则资格 fixture 会在 `FontAssetDatabaseMissing` /
`TextureDescriptorReadFailed` 处失败。运行时依赖只读当前 engine Rust source。此副本只证明同一
candidate/harness 的启动、项目读取和输入链路，不代替最终真实 Tower production smoke。

第一次 run-owned qualification 已证明 BOM-free recent store 被 Rust 读取、Tower recent row 出现并可打开；旧
`mouse_event` harness 未可靠更新 winit client cursor，改为显式 client `WM_MOUSEMOVE/DOWN/UP` 后 trust prompt
出现。随后 Play 暴露上述缺失 cooked 索引的 fixture 错误；production Editor 与真实 Tower cache 始终未变。
补齐索引后，同一 candidate 已生成 `Success/Rebuilt` 的 run-owned Preview cache；该 cache 只消费未变化的
candidate/project/AssetPipeline inputs，因此后续最终 qualification 复用它，不重复 103 秒 producer。最终点击使用
从 `start-button` 1080x1920 AUI 合同计算出的中心比例，不沿用旧的下缘经验坐标。
可见 AUI 的中心点击最初被旧 harness 误判为 `inputBridgeStatus=not_requested`。首个观察合同错误是把 AUI 业务
action `td.start-round` 当成 Input Mapping 的 `gameplayActionIds`；第二个错误是 294 后普通帧 report 不再逐帧写盘，
旧 `Wait-ForAction` 只能反复读取启动 report。harness 改为点击后通过既有 Pause publication 读取
`projectObservationState.snapshot.values["tower.phase"]`，以 `organizing -> combat` 作为真实业务判定。

该判定同时证明前台 `SendInput` 在当前桌面没有进入 winit/AUI；屏幕点的 leaf/root HWND 均属于目标 Editor，讯飞
浮层没有覆盖按钮，但桌面级输入 hook 仍可能丢弃 injected event。此前所谓“posted GameView 失败”使用了上述无效
observer，不能作为排除证据。最终 harness 使用显式 client `WM_MOUSEMOVE`，并将 posted `WM_LBUTTONDOWN` / 
`WM_LBUTTONUP` 分开 120 ms；launcher、Editor chrome 与 GameView 均使用该可审查的 client route。

最终 run-owned qualification 通过：candidate
`7FDF496ED411480BAA041D26D2C700D9FB895027ADF3C62C0DB643C01D306A9C`，harness
`D5A2CB85DB38C783A8F6F4F905A3FD8EE99DF74C0847B7B94BA751F03A35AC25`；BOM-free prefix 为 `7B0D0A`，
Open/Play/click/Stop 全部通过，点击后 phase 为 `combat`，8 秒观察 private bytes 净增 720,896 bytes，
Cargo/rustc spawn 为 0。qualification seal 已生成。

随后按授权仅执行一次 production transaction。Editor 与旧 93-file cache 完成 byte-exact 备份并替换/移出后，
production smoke 在 `post301.play_report_timeout` 失败。`stage-3-project-open.png` 证明固定等待 12 秒后仍停留在
production 项目 trust prompt；harness 随即把 Play 坐标发送给被 modal 覆盖的窗口，因此没有 running report。
transaction 已立即回滚：production Editor 恢复为 `E27EE515...BF163`，cache 恢复为 93 files / manifest
`6710E237...755D8`，Editor/Cargo/rustc 残留为 0。301 仍未完成，不得重试本次 production mutation；详见阶段记录
`05-post-301-qualified-production-transaction-trust-prompt-failure.md`。

## 301 harness minimal repair

2026-08-19 用户仅授权修复 run-owned harness。没有修改引擎源码、重建 candidate、替换 production Editor、
移动真实 Tower cache、运行 Local CI 或执行新的 production transaction。

最小因果修复位于
`G:\AifeProductionRuns\post-301-qualified-consistency-20260819-111211\post-301-smoke-harness.ps1`：删除
Production 分支固定 12 秒等待，新增 Qualification / Production 共用的有界 project-ready owner。该 owner 在 240 秒
上限内幂等尝试 trust Approve 与 Play，并且只接受本轮新产生、`status=success` 且
`controlState=running` 的 report；结果记录 `playAttempts` 与 `elapsedMs`。PowerShell parser 为 0 errors。

修复后的 harness SHA-256 为
`08C146369C9FC9E9B7362FB67813DBBD2DFF041DFA54D7CF4D3D953D901033AE`。未变化的 candidate 仍为
`7FDF496ED411480BAA041D26D2C700D9FB895027ADF3C62C0DB643C01D306A9C`。

为覆盖首次信任路径，既有 run-owned receipt 被保留移动到
`retained-qualification-trust-before-harness-rerun`；`qualification-composition\project-runtime-native-modules`
的 568 files 保持原位。新 `stage-2-trust.png` 明确捕获“信任项目运行模块”modal，随后自动 Approve；同一
qualification 以 1 次 Play attempt、2,087 ms 进入 running，Open / Play / click / Stop 全部通过，点击后
`tower.phase=combat`，8 秒 private bytes 净增 1,421,312 bytes，allocation failure=false，Cargo/rustc spawn=0。
新 smoke SHA-256 为
`ABB97F9FF69665ED66D090209A4A156469BFEECA81A0D8BD8DB8E6CAAF715183`，qualification seal 已重新生成并绑定
上述 candidate / harness / smoke identities。

资格化前后 production Editor 均为
`E27EE515D365E8E778D5166249CB804B6A3665B67889E4DBD4FC15B7435BF163`；真实 Tower cache 仍为 93 files，
manifest 为 `6710E2372C4F1FD5FC5670A8186C89C8DBD4539409FE5CA2BBAE7CC8DB4755D8`；Editor/Cargo/rustc 残留为 0。
301 仍未完成且不归档。`post-301-production-transaction.ps1` 仍绑定旧 harness seal，本轮禁止修改或执行；只有取得
新的 production transaction 授权后，才可将它绑定到新 seal 并执行一次 transaction。

## Final production transaction

2026-08-19 用户单独授权一次新的 production transaction。transaction script 只更新到新 harness seal，并为本次
transaction 使用独立的 run-owned backup / evidence / retained / failed 路径；旧 570-file production composition
被保留移动到同一 run root，未覆盖或删除。script SHA-256 为
`646D0597C5B385DD304F6B8F7D837BCE773B8E1E8B25E6FF56C166176BDD158C`，PowerShell parser 为 0 errors。

transaction 一次通过，未重试：

```text
candidate / production after:
7FDF496ED411480BAA041D26D2C700D9FB895027ADF3C62C0DB643C01D306A9C
harness:
08C146369C9FC9E9B7362FB67813DBBD2DFF041DFA54D7CF4D3D953D901033AE
qualification seal:
310CF68C138A9AC911A63B68C0FD0283295C87F33C39D954A1FF329A40252FF5
production before / backup:
E27EE515D365E8E778D5166249CB804B6A3665B67889E4DBD4FC15B7435BF163
rebuilt cache: 93 files
rebuilt cache manifest:
9EF380D96FD75412A5E47EF5737FE97FCF62DDB971268B5DAEF80C6747C2FA97
old cache backup: 93 files
old cache manifest:
6710E2372C4F1FD5FC5670A8186C89C8DBD4539409FE5CA2BBAE7CC8DB4755D8
```

普通 production smoke 以 3 次 Play attempt、8,544 ms 进入 running；60 秒 private bytes 尾段净变化
`-98,304` bytes，allocation failure=false；点击后 `tower.phase=combat`，Stop passed，Cargo/rustc spawn=0，
owned Editor/Cargo/rustc residue=0。transaction result 为 passed，`rollbackRequired=false`，没有 failure 或 rollback
receipt。Local CI、完整视觉/E2E matrix、真实配置、Player/MCP/其它安装态二进制均未修改。

301 唯一 Gate 已完成，可以写完成记录、同步入口并归档本文件。
