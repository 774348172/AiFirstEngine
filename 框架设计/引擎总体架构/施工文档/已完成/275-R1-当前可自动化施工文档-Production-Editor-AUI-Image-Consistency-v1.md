# 275-R1 Production Editor AUI Image Consistency v1 施工文档

## 0. 状态与授权

```text
状态：已完成并归档
授权：2026-08-07 用户明确要求先完成当前 UI 在普通 production Editor 中的一致性闭环
前置：275 与 Tower UI-V1 Gate E 已完成并归档
时限：本次连续施工不超过 3 小时
```

## 1. 问题证据

普通 production Editor `G:/gameEngin/rust/target/debug/editor_host.exe` 的时间为
2026-08-04，SHA256 为
`fef9b0a571bd953ed0f848b03e68cc6eb40835bc0e5c9c60fa2b30536fbb5cfa`，早于 275 完成。
用户从普通入口观察到纯色背景和线框；275 Gate E 合格 composition
`83a4da3019e732df4b12d984a90eddff8b9a5c7dc7ea2e818e74d5f8bae49daf`
已证明水墨背景、格框、卷轴、竹简和印章正常呈现。

## 2. 目标

1. 使普通 production `editor_host.exe` 包含 275 后的 AUI Image cooked texture / GPU present 实现。
2. 在仓库外隔离 target 构建和验证候选，不以 production 目标作调试循环。
3. 事务化替换 production Editor，保留 before/after hash 与可回滚副本。
4. 重开普通 Editor 和 Tower，确认新 composition 不再回退到纯色 UI。

## 3. 边界

```text
不修改 Animator2D，不恢复 P1-2。
不运行 Local CI。
不修改真实 recent/layout/trust/config 文件。
不重跑 Tower Gate A-D/F/G/H。
不删除 Gate E 或失败 fresh roots。
不替换 MCP 或其它安装态二进制。
不修改产品源码，除非候选验证暴露新的 owner 缺陷。
```

## 4. Gate A：基线与证据复用

- 记录 production Editor 路径、hash、时间和运行进程。
- 复用 275 owner/consumer tests 和 Gate E 报告；因产品源输入未改变，不重跑相同测试。
- 确认当前唯一施工文档为本文。

## 5. Gate B：隔离候选构建与验证

```powershell
cargo build -p editor_host --locked --features real-window,real-wgpu-surface
```

命令在仓库外专用 `CARGO_TARGET_DIR` 中运行。候选必须：

- build exit 0；
- 无参数 headless readiness exit 0；
- `--real-window` 创建可响应的真实窗口；
- 后续 source-built Tower composition 能消费同一 275 源身份。

## 6. Gate C：production 事务替换

1. 再次确认没有运行中的 production `editor_host.exe`。
2. 将旧 EXE 备份到本次仓库外 run root。
3. 将候选复制到同目录临时文件，核对 hash 后原子替换目标。
4. 核对目标 after hash 与候选精确一致。
5. 任何步骤失败时从备份回滚，并验证 before hash 恢复。

## 7. Gate D：普通入口一致性

- 从替换后的 `rust/target/debug/editor_host.exe --real-window` 打开普通 Editor。
- 打开 Tower 后记录实际 specialized composition 路径与身份。
- 实际 GameView 必须出现水墨背景、格框、卷轴、竹简与印章，无纯色 fallback、黑块或缺失纹理。
- 最终可视结果需用户确认；确认前本文保持当前施工状态。

## 8. 完成与归档

全部 Gate 通过后：

1. 生成 `阶段完成记录/<date>-Production-Editor-AUI-Image-Consistency-v1/00-总览.md`。
2. 更新 49、54、施工文档 README 和阶段完成记录 README。
3. 将本文移到 `施工文档/已完成/`。
4. 保持 P1-2 和 Animator2D 未激活。

## 8.1 实际结果

```text
首次 run root：G:/AifeProductionRuns/275-r1-production-ui-20260807-145334
首次结果：G 盘空间不足，os error 112；失败 evidence/root 保留
失败缓存清理：仅删除上述 root/cargo-target，strict-child/reparse/process proof 通过
最终 fresh root：C:/AifeProductionRuns/275-r1-production-ui-20260807-145908
候选/production SHA256：159eecc2ef90d9234566afb5a8456c9b5beae1f2d83f8443fe47b37df3ae1c8b
旧 production SHA256：fef9b0a571bd953ed0f848b03e68cc6eb40835bc0e5c9c60fa2b30536fbb5cfa
旧版备份：C:/AifeProductionRuns/275-r1-production-ui-20260807-145908/editor_host.before.exe
Tower composition identity：9aec57ec56de09fc8760e0a02ccc1b497555b1b8924c7309825bfd2d950c5b8f
Tower composition EXE SHA256：77ea3234ab026eb0ddf0706c97d206877da6f4bbf80ad7d2e4e883b4d1fdc9d6
用户视觉确认：通过，2026-08-07
```

候选 build exit 0、headless readiness exit 0；真实窗口获得非零句柄且 Responding=True。
production 替换后的 hash 与候选一致。普通入口生成的 Tower descriptor 将
`editorBuildIdentity` 精确绑定到新 production hash，用户确认水墨背景、格框、卷轴、竹简和印章正常。

普通 Editor 打开 Tower 时按产品流程生成了新 composition cache、handoff state，并刷新
project-runtime trust receipt；这些是用户在真实普通入口触发的产品状态。Local CI、其它 Tower Gate、
MCP/其它安装态二进制均未触碰。

## 9. 自审

```text
是否扩大 275 能力范围：否，只激活已通过能力到普通 production Editor
是否修改 Tower gameplay：否
是否需要新正式方案：否，这是 275/UI-V1 的发布身份 remediation
是否要求 Local CI：否
是否覆盖 owner、consumer 与真实可见声明：是
是否保护外部状态与回滚：是
是否符合 3 小时限制：是
```
