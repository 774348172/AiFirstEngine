# 293 post-292 Production Editor + Tower Native Module Cache Consistency Mini v1 方案

## 1. 状态

```text
建立日期：2026-08-16
正式选择：方案 B
方案状态：已确认并自审
施工状态：唯一 Gate 已完成并归档
```

## 2. 目标

把 292 已完成的 source availability 转为普通 production Editor 的 installed availability：生成正确的
`editor_host.exe`，事务替换现有 production Editor，复用 Gate H 已验证的 Tower native module exact
artifact，并用一次普通 Editor Open / trust approve / Play / Stop 证明首次使用不启动 Cargo。

这不是新功能开发，不重新资格化 292，也不拿 `editor_ui_authority.exe` 代替 production `editor_host.exe`。

## 3. 已确认基线

```text
production Editor:
  <repository-root>\rust\target\debug\editor_host.exe
  sha256:F8856CE9A507E7A2E6D4DCA6E8D20B2957ECB507AACB414917281240D7420EA8

Gate H retained target:
  <RUN_292_ROOT>\20260816-143623\cargo-target
  editor_host.exe 当前不存在，可复用依赖做一次增量 build

Tower exact module identity:
  sha256:65029b9650706621ebc15ba5c02f4245e040a5bf617df01feb3438abecf81303

production native-module cache:
  当前不存在

Tower trust receipt:
  存在，但绑定 pre-292 Editor/project identity；post-292 首次需显式重新批准
```

## 4. 最小事务

```text
preflight + run-owned evidence root
-> 复用 Gate H CARGO_TARGET_DIR 增量构建 editor_host
-> candidate copy/hash
-> byte-exact 备份 production Editor 与 Tower trust receipt
-> pending hash + exact Editor replacement
-> staging copy + descriptor/seal/hash 校验 + exact cache publish
-> 普通 production Editor Open / approve / Play / Stop
-> Cargo spawn = 0 + process cleanup + receipt/cache identity
-> 完成记录与归档
```

任一步失败：终止本轮 owned 进程，恢复旧 Editor 和旧 trust receipt，移除本轮新增 exact cache entry，
复核 production hash/inventory 后停止。

## 5. 边界

- 不修改产品源码、Tower 项目源码或真实项目配置。
- 不重建 Tower DLL，不重建 Tower Preview cache；Preview owner 如判 stale 只允许正常自动处理。
- 不运行完整 E2E、Local CI、视觉矩阵或 292 受影响回归。
- 不替换 Player、MCP 或其它安装态二进制。
- 只允许修改唯一 production `editor_host.exe`、Tower trust receipt、本轮 exact native-module cache entry
  与 run-owned evidence。

## 6. 自审

```text
必要性：production Editor 仍为 post-290，且真实 native-module cache 不存在。
最小因果动作：正确 editor_host replacement + 已验证 exact artifact seed + 一次普通 Editor smoke。
复用证据：292 Gate H source suites、artifact descriptor/seal/hash、source-built 8/8 scenario。
新增证据：installed binary/hash、real cache/receipt、ordinary Editor Open/Play/Stop、Cargo spawn=0。
排除：源码修复、冷 DLL build、完整 suite、Local CI、视觉矩阵、Preview 强制重建。
结论：通过；没有过量施工。
```

## 7. 完成结果

production Editor 已从 `F8856CE9...EA8` 更新为 `84729797...1682`；Gate H exact Tower artifact
以 identity `65029b96...1303` 发布到 production cache。普通 Editor Open / Approve / Play / Stop 通过，
GameView 最终 `status=stopped`、`frameCount=1131`，Preview `cacheStatus=Hit`，Cargo spawn=0，进程清理为 0。
完整证据见 `阶段完成记录/2026-08-16-post-292-Production-Editor-Tower-Native-Module-Cache-Consistency-Mini-v1/00-总览.md`。
