# post-301 Production Editor consistency attempt：harness failure 与回滚

日期：2026-08-19

## 授权与范围

用户单独授权新的 post-301 production consistency：fresh run root 构建包含 allocation owner repair 的
Editor candidate，备份并替换 production Editor，只重建 Tower Preview cache，并执行普通 Editor Tower
Open / Play / 60 秒内存平台化 / 一次点击 / Stop。失败立即回滚；不运行 Local CI，不修改真实配置，不替换
Player、MCP 或其它二进制。

run root：

`G:\AifeProductionRuns\post-301-consistency-20260819-100416`

## Candidate

- source HEAD：`acaffa8cb95bbb54d093bbf45835a59315c82da5`
- repair source SHA-256：`E532217E208DB1798BF035DB64967E20A3DF4110313AE43BBC464D45BBFED53F`
- candidate SHA-256：`0F96CD3B1C4AF228F7637489FEBC3B2FCE5980262815D966A80B47AD2F58AC36`
- build：通过，约 233.5 秒
- candidate readiness：exit 0
- replacement readiness：exit 0

production Editor 与 93-file Tower cache 在 mutation 前均已 byte-exact 备份。candidate、pending 与 replacement
hash 一致；真实配置没有修改，普通 Editor 的 APPDATA、LOCALAPPDATA、trust 与 native-module cache 均使用
run-owned state。

## 首个失败

失败码：`post301.play_report_timeout`

失败发生在 Open/Play 自动化前置，尚未进入 60 秒内存平台化、AUI 点击或 Stop 验收。截图确认 harness 读取了
`editor_host` 的 console/terminal 主窗口句柄，`GetWindowRect` 因而返回整个 2560x1440 桌面；run-owned 坐标
`(850,307)` 没有落入实际 Editor 的 Tower recent row。Tower 没有打开，Play report 在 90 秒内未生成。

这不是 allocation owner repair 的运行时复发，也不是 Tower Play 的产品失败；本次 attempt 没有形成相应产品
判定。run-owned harness 已改为按 process id 与精确标题 `AI First Engine Editor` 枚举真实顶层窗口，但没有自动
重跑 production mutation。

## 回滚

- production Editor 已恢复：`E27EE515D365E8E778D5166249CB804B6A3665B67889E4DBD4FC15B7435BF163`
- Tower cache 已恢复：93 files
- cache manifest SHA-256：`6710E2372C4F1FD5FC5670A8186C89C8DBD4539409FE5CA2BBAE7CC8DB4755D8`
- Editor/Cargo/rustc 残留：0
- Player、MCP 与其它二进制：未替换
- Local CI：未运行
- 真实配置：未修改

结构化证据：

- `evidence/candidate.json`
- `evidence/backup.json`
- `evidence/editor-replacement.json`
- `evidence/failure.json`
- `evidence/rollback.json`
- `evidence/stage-1-launcher.png`
- `evidence/stage-2-project-open.png`

301 仍未完成并继续占用当前施工槽。下一步必须取得新的单次 post-301 production consistency 授权，使用已修正
窗口选择的 fresh run root；不得复用本次 mutation attempt。
