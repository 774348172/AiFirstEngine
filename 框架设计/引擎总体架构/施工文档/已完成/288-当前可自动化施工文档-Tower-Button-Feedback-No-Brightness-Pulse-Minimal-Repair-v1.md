# 288 当前可自动化施工文档：Tower Button Feedback No-Brightness-Pulse Minimal Repair v1

> 状态：已完成，待归档
> 日期：2026-08-14
> 上游：277 AUI Control Feedback Mini Default Profile v1、284 Editor GameView Normal-Frame Immediate AUI Input Dispatch v1
> 授权：应用按钮点击闪烁的最小修复

## 目标

为 Tower AUI 注册一个项目级默认 Button feedback profile，移除 hover、pressed、activated 的亮度变化，
保留轻微按压缩放与 1 logical px 下移反馈。所有现有 Button 继续使用 `feedback=auto`，通过
`default_button_profile` 统一解析，不逐节点覆写。

## 唯一 Gate：Tower 项目 profile 与生成产物闭环

1. 在 `AUI/generate-p1-0-battle-hud.cjs` 的 document 中声明 `tower-button-subtle-v1`。
2. 生成并同步 `AUI/battle-hud.aui.json`。
3. 在现有 P1-0 项目合同验证器中断言 profile 可解析、亮度恒等、pressed scale=985、offset=(0,1)。
4. 运行 generator drift check 与 P1-0 contracts validator。
5. 写最小完成记录、同步施工入口并归档本文。

## 必要性审计

```text
confirmed failure:
Tower 57 个 Button 均省略项目 profile，继承 built-in pressed brightness -80 与 activated brightness +60；
284 同帧发布 PointerDown/PointerUp 的反馈画面，使一次点击表现为变暗、变亮、恢复的闪烁。

minimum causal fix:
在 Tower AUI document 注册并设为默认的无亮度脉冲 profile；保留轻微 scale/offset 触感。

files that must change:
- samples/tower_defense_project/AUI/generate-p1-0-battle-hud.cjs
- samples/tower_defense_project/AUI/battle-hud.aui.json
- samples/tower_defense_project/Features/MirroredBoardInterface/tests/validate-p1-0-contracts.cjs
- 本施工文档、完成记录和四个施工入口索引

tests that can fail before and pass after the fix:
- node samples/tower_defense_project/AUI/generate-p1-0-battle-hud.cjs --check
- node samples/tower_defense_project/Features/MirroredBoardInterface/tests/validate-p1-0-contracts.cjs

explicitly deferred nearby enhancements:
- 引擎 feedback evaluator、renderer、input dispatch 的修改
- production Editor replacement、Tower Preview cache rebuild、真实配置
- Editor/Play/input smoke、视觉矩阵、Local CI
```

## 激活前复核与自审

- 当前施工槽无活动 Markdown，待执行队列为空；本次授权与唯一 Gate 一致。
- 三个产品目标均为 `project-owned`；引擎文件只读，不进入修改范围。
- 既有 AUI v2 已支持 document registry 和 `auto -> default_button_profile` 解析，无需 schema、migration 或兼容层。
- 测试只覆盖生成一致性与项目 profile 合同；不扩大为引擎、production 或真实窗口资格验证。

自审结论：范围最小、因果直接、可由现有项目验证器捕获；允许施工。

## 完成结果

- Tower AUI 已注册 `tower-button-subtle-v1` 并设为 57 个 `feedback=auto` Button 的项目默认 profile。
- hover、pressed、activated brightness 均为 0；pressed scale 为 985，offset 为 `(0,1)`。
- 红测试在旧产物上以 `aui.button_feedback_profile` 失败；生成新产物后 generator drift check 与 P1-0
  contracts validator 均通过。
- production Editor SHA-256 保持
  `A6B89322F2560E66ADF9AD31BD61994C9DE2E4CF525BECCE324CC586D0AC35CD`，未触碰任何禁止项。

完成记录见
`../../阶段完成记录/2026-08-14-Tower-Button-Feedback-No-Brightness-Pulse-Minimal-Repair-v1/00-总览.md`。
