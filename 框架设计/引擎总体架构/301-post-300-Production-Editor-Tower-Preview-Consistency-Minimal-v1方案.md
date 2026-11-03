# 301 post-300 Production Editor / Tower Preview Consistency Minimal v1 方案

状态：用户已于 2026-08-19 明确确认并授权执行；本方案为 300 源码闭环后的安装态一致性事务。

## 目标

把已完成的 300 `ProjectUiStateSnapshot Session-Bound Conditional Resolve v1` 更新到唯一 production
`rust/target/debug/editor_host.exe`，只重建 Tower `scene-main` Preview cache，并用替换后的普通 Editor
完成 Open / Play / 一次真实 AUI 点击 / Stop smoke。

## 唯一路径

```text
记录 source / production / cache identity
-> 构建标准 editor_host candidate 并执行无参 readiness
-> byte-exact 备份 Editor 与 Tower cache
-> candidate/after hash 一致替换
-> 普通 production Editor 以 run-owned state 打开 Tower
-> Play 通过正式 Preview owner 重建唯一 cache
-> 真实 AUI 点击产生业务 action
-> Stop、记录终态并保留备份
```

任一 replacement、cache rebuild 或 smoke 阶段失败，关闭本轮 owned 进程并恢复本轮 Editor/cache 备份；
不得改用其它 binary、旧 candidate 或更宽测试来掩盖失败。

## 边界

- 允许：唯一 production Editor、Tower `scene-main` Preview cache、本轮 repository-external run root。
- 不修改产品源码、Tower 源码或真实配置。
- 不运行 Local CI、完整 E2E/视觉矩阵，不导出/替换 Player，不替换 MCP 或其它安装态二进制。
- 300 owner/consumer 测试与 source-linked smoke 直接复用，不重复运行。

自审：这是已确认源码的最小 installation consistency，不是新系统或 release qualification；一个事务、
一个真实 consumer、一个 smoke 足以支持用户要求，没有新增 schema、兼容层、Runner 或重复 Gate。
