# 301 harness minimal repair

日期：2026-08-19

## Scope

本次只修复 run-owned smoke harness 对首次 trust prompt 的处理。不修改引擎源码，不重建 candidate，不替换
production Editor，不移动真实 Tower Preview cache，不运行 Local CI，不修改真实配置，也不执行 production
transaction。

## Minimal repair

- harness：`G:\AifeProductionRuns\post-301-qualified-consistency-20260819-111211\post-301-smoke-harness.ps1`
- 删除 Production 模式固定 `Start-Sleep 12`。
- 新增 Qualification / Production 共用的 `Start-PlayWhenProjectReady`。
- 240 秒内幂等尝试 trust Approve 与 Play，只以本轮新的 `running` report 判定 Ready。
- smoke result 新增 `projectReady.playAttempts` 与 `projectReady.elapsedMs`。
- PowerShell parser：0 errors。

## First-trust qualification

既有 run-owned trust receipt 保留在
`G:\AifeProductionRuns\post-301-qualified-consistency-20260819-111211\retained-qualification-trust-before-harness-rerun`，
未删除。`qualification-composition\project-runtime-native-modules` 的 568 files 未移动，因此本轮没有重新编译
RuntimeModule。

`evidence\qualification\stage-2-trust.png` 明确显示“信任项目运行模块”modal；旧的无 modal 证据保留在
`evidence\qualification\pre-first-trust-harness-rerun`。自动 Approve 后：

- Ready：1 次 Play attempt，2,087 ms
- Tower Open / Play / click / Stop：passed
- click 业务证据：`tower.phase=combat`
- 8 秒 private bytes 净增：1,421,312 bytes
- allocation failure：false
- Cargo/rustc spawn：0
- owned Editor/Cargo/rustc residue：0

## Bound identities

- candidate SHA-256：`7FDF496ED411480BAA041D26D2C700D9FB895027ADF3C62C0DB643C01D306A9C`
- harness SHA-256：`08C146369C9FC9E9B7362FB67813DBBD2DFF041DFA54D7CF4D3D953D901033AE`
- smoke SHA-256：`ABB97F9FF69665ED66D090209A4A156469BFEECA81A0D8BD8DB8E6CAAF715183`
- production Editor SHA-256：`E27EE515D365E8E778D5166249CB804B6A3665B67889E4DBD4FC15B7435BF163`
- real cache：93 files
- real cache manifest SHA-256：`6710E2372C4F1FD5FC5670A8186C89C8DBD4539409FE5CA2BBAE7CC8DB4755D8`

qualification seal 已重新生成并绑定新 harness。production Editor 与真实 cache 身份未变化。

## Status

301 仍未完成且不归档。本轮没有执行 production transaction；现有 transaction script 仍绑定旧 harness hash，
必须在取得新的 production transaction 授权后才可更新并执行。
