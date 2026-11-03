# post-301 Production Editor consistency：recent store BOM harness failure

日期：2026-08-19

状态：失败并已完整回滚；301 仍未完成。

## 授权与范围

本次获得新的单次 post-301 production consistency 授权：使用 fresh run root 构建包含 allocation owner
repair 的新 Editor candidate，备份并仅替换 production Editor，只重建 Tower Preview cache，并执行 Tower
Open / Play / 60 秒内存平台化 / 一次真实点击 / Stop。禁止 Local CI、真实配置修改以及 Player、MCP 或其它
安装态二进制替换；任一失败立即回滚。

run root：`G:\AifeProductionRuns\post-301-consistency-20260819-103722`

## Candidate

`cargo build -p editor_host --locked --features real-window,real-wgpu-surface` 在独立 run-owned target 中完成，
耗时约 193 秒。candidate 无参 readiness exit 0：

```text
SHA-256: 4AED3F98A899F2785283E50AA3F612AD46452D9F2BEB6B70AEF069CF7F94B090
length: 126673920 bytes
source HEAD: acaffa8cb95bbb54d093bbf45835a59315c82da5
repair source: E532217E208DB1798BF035DB64967E20A3DF4110313AE43BBC464D45BBFED53F
```

首次 transaction wrapper preflight 因 Windows PowerShell 子进程未解析 `Get-FileHash` 而停止，
`mutationStarted=false`。harness 改用 .NET SHA-256 后重新通过 preflight；该非 mutation 失败保留在 run-owned
evidence 中。

## 失败首因

production Editor 与原 cache 已 byte-exact 备份，candidate replacement 与 production readiness 均通过。
修正后的 PID + 精确窗口标题枚举选中了真实 `AI First Engine Editor` 顶层窗口，但 launcher 显示“没有最近项目”，
所以 Tower 没有打开。

首因属于 run-owned harness：Windows PowerShell 5 的 `Set-Content -Encoding utf8` 给
`editor_recent_projects.json` 写入了 `EF BB BF` BOM；Rust `serde_json` 拒绝该前缀，recent store load 失败。
截图 `stage-2-project-open.png` 与 `launcher-recent-store-root-cause.json` 固化了证据。harness 已改为
`File.WriteAllText(..., UTF8Encoding(false))`，但没有在本次单次 mutation 授权内再次执行。

## 回滚与未执行项

确认 Tower 未打开后，主动终止本次 owned Editor，使 transaction 进入既定 failure/rollback 分支：

```text
production Editor: E27EE515D365E8E778D5166249CB804B6A3665B67889E4DBD4FC15B7435BF163
Tower cache: 93 files
cache manifest: 6710E2372C4F1FD5FC5670A8186C89C8DBD4539409FE5CA2BBAE7CC8DB4755D8
Editor/Cargo/rustc residual processes: 0
rollback: passed
```

Tower Preview cache 未形成新 rebuild；Play、60 秒内存平台化、真实 AUI 点击与 Stop 均未执行。Local CI、真实配置、
Player、MCP 和其它二进制均未触碰。

## 下一步

301 继续占用当前施工槽。新的尝试需要新的单次 production consistency 授权、fresh run root 与新 candidate；
使用已确认的 BOM-free recent store writer，不复用本次 candidate 或 mutation attempt。
