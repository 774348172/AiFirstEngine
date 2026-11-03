# 258-W7 W6 Role-owned State Cleanup v1 施工文档

状态：W7 cleanup 已完成并归档

用户授权：允许递归删除 W6 role root 内已验证的 build/temp 与失败 state 目录，并执行 W7。

## 1. 定位与范围

W7 是 258 完成后的窄 cleanup remediation，不重新打开 258 产品功能，不修改产品源码，不重跑
W1-W6，不新增资格测试，也不改变 `b7b5607936e65984e5147200ce1a9ef84031316d` 的通过结论。

精确 role root：

```text
G:\EngineTest\258-W6-20260727-140000
```

允许删除的精确子目录：

```text
G:\EngineTest\258-W6-20260727-140000\cargo-target
G:\EngineTest\258-W6-20260727-140000\temp
G:\EngineTest\258-W6-20260727-140000\state-before-final-b7b5607
G:\EngineTest\258-W6-20260727-140000\state-failed-before-5b11c1b
G:\EngineTest\258-W6-20260727-140000\state-failed-before-6c487cb
G:\EngineTest\258-W6-20260727-140000\state-failed-before-731be67
```

必须保留：

```text
G:\EngineTest\258-W6-20260727-140000\evidence
G:\EngineTest\258-W6-20260727-140000\fixture
G:\EngineTest\258-W6-20260727-140000\state
G:\gameEngin\rust\target
```

## 2. W7 执行合同

1. 对 role root 和每个删除目标解析 literal absolute path，确认目标是 role root 的 strict child。
2. 检查 role root、目标及其中间祖先不是 symlink、junction 或可逃逸 reparse point。
3. 确认 W6 authority owned process 已退出；不得终止或重启 production Editor。
4. 按公共施工 skill 对每个精确目标执行受控递归删除，结果只记为 `removed`、`missing` 或
   `retained_by_host_policy`。
5. 验证 evidence、fixture、最终 state 和 production target 均保留。
6. 只更新 `w6-summary.json` cleanup 字段、258 完成记录与入口文档。
7. cleanup 失败不得触发 W1-W6、Local CI、真实 Windows authority 或 production replacement 重跑。

## 3. 完成条件

- 六个精确目标均得到结构化终态；
- 保留目录存在；
- 本次未终止、启动或替换 production Editor；
- `w6-summary.json` 与完成记录反映 W7 实际结果；
- 本文档移入 `施工文档/已完成/`，入口恢复为无当前施工。

## 4. 自审

- [x] 用户已明确授权本次精确递归 cleanup 和 W7
- [x] 范围仅为 repository-external W6 role-owned disposable state
- [x] 未把授权扩展到 role root、production target、用户项目或其它 Wn root
- [x] evidence、fixture 和最终 state 明确保留
- [x] 不重跑任何已经通过的资格验证
- [x] 不修改 258 正式方案或产品源码
- [x] 单窗口预计远低于三小时

## 5. 执行结果

```text
Remove-Item -Recurse：宿主在 PowerShell 启动前拒绝，未执行
受控 fallback：System.IO.Directory.Delete
cargo-target：removed
temp：removed
state-before-final-b7b5607：removed
state-failed-before-5b11c1b：removed
state-failed-before-6c487cb：removed
state-failed-before-731be67：removed
evidence / fixture / final state / rust target：preserved
production Editor action：none
qualification rerun：false
w6-summary sha256：d8522b3d3daa17cb0cd8337635a0c76983a07cf7aef05c219d526b330574838c
```
