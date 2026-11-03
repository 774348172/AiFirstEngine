# post-301 qualified production transaction trust prompt failure

日期：2026-08-19

## Qualification

- run root：`G:\AifeProductionRuns\post-301-qualified-consistency-20260819-111211`
- candidate SHA-256：`7FDF496ED411480BAA041D26D2C700D9FB895027ADF3C62C0DB643C01D306A9C`
- harness SHA-256：`D5A2CB85DB38C783A8F6F4F905A3FD8EE99DF74C0847B7B94BA751F03A35AC25`
- BOM-free recent prefix：`7B0D0A`
- Tower Open / Play / click / Stop：passed
- click 业务证据：`tower.phase=organizing -> combat`
- 8 秒 private bytes 净增：720,896 bytes；allocation failure：false
- Cargo/rustc spawn：0

旧 harness 曾错误读取低频持久化 report 的 `gameplayActionIds` 来等待 AUI action。最终 observer 改为通过 Pause
publication 读取 `tower.phase`；当前桌面的 injected `SendInput` 未进入 AUI，分离 120 ms 的 client posted
down/up route 通过。

## Production transaction

按用户授权仅执行一次 transaction。备份、candidate exact replacement 与旧 cache detach 均成功；production
smoke 在 `post301.play_report_timeout` 失败。截图
`evidence\production\stage-3-project-open.png` 显示固定等待 12 秒后仍存在 production 项目 trust prompt，Play
坐标没有进入 toolbar。真实配置未修改，不能把 qualification 项目路径的 run-owned trust 直接视为 production
项目路径的 approval。

## Rollback

- rollback receipt：passed
- production Editor：`E27EE515D365E8E778D5166249CB804B6A3665B67889E4DBD4FC15B7435BF163`
- Tower cache：93 files
- cache manifest：`6710E2372C4F1FD5FC5670A8186C89C8DBD4539409FE5CA2BBAE7CC8DB4755D8`
- owned Editor/Cargo/rustc residue：0
- Local CI / Player / MCP / 真实配置：未触碰

301 保持未完成。若后续继续，需新的授权；最小 repair 是让 production-mode run-owned harness 显式处理 trust
prompt 并等待项目真实 ready，而不是固定 sleep。harness 身份变化只失效 harness-bound qualification，不要求重建
未变化的 candidate，但必须重新形成新 harness seal 后才可申请新的单次 production transaction。
