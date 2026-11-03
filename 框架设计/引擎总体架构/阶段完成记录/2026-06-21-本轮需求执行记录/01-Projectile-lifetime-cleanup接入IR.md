# Projectile lifetime cleanup 接入 IR

## 需求目标

把 projectile 生命周期清理判断从普通 runtime system 内部逻辑，收敛成一个可追踪、可验证、可由 AI 审查的 IR 规则。

目标规则：

```text
ruleId: prototype.projectile.cleanup_lifetime
input:
  projectile.age
  projectile.lifetime
output:
  shouldDestroy
```

## 框架定位

该需求验证的是：

```text
项目规则可以由 IR 表达。
Runtime System 负责从 ECS 读取快照并调用 IR。
IR 只做决策，不直接操作 ECS World。
实体销毁仍由 engine.Cleanup system 通过 command queue 执行。
Runtime Trace 必须记录 ruleId / sourceMap，方便 AI 和人类查错。
```

它属于：

```text
逻辑系统边界
IR Interpreter MVP
Runtime System 接入 IR
Trace / SourceMap 可审查链路
```

相关主文档：

```text
05-逻辑系统边界-DSL-IR-RustAOT-ECS.md
22-Canonical-Rule-IR-v1.md
23-IR-Interpreter-MVP.md
24-阶段6收敛-真实RuntimeSystem接入IR.md
```

## 执行方式

实现采用“受控接入”方式：

```text
1. Runtime System 读取 projectile.age / projectile.lifetime。
2. Runtime System 构造 IR input snapshot。
3. IR Interpreter 执行 prototype.projectile.cleanup_lifetime。
4. IR 输出 shouldDestroy。
5. Runtime System 根据 shouldDestroy 发出 destroy command。
6. engine.Cleanup 在统一位置处理实体销毁。
7. Runtime Trace 记录 IR 执行证据。
```

这条链路没有让 IR 直接持有 ECS World，也没有让 IR 直接销毁 entity。

## 修改文件

```text
src/engine/systems.ts
scripts/test-runtime-backend.cjs
框架设计/引擎总体架构/24-阶段6收敛-真实RuntimeSystem接入IR.md
框架设计/引擎总体架构/施工文档/历史/31-可直接实现任务清单.md
框架设计/引擎总体架构/施工文档/历史/32-当前可直接实现施工文档.md
框架设计/引擎总体架构/施工文档/历史/33-可直接实现落地路线.md
框架设计/引擎总体架构/施工文档/历史/34-当前剩余可直接实现清单与施工顺序.md
```

## 验证方式

执行过的测试：

```powershell
npm.cmd run test:ir
npm.cmd run test:interpreter
npm.cmd run test:runtime
npm.cmd run build
```

验证点：

```text
expired projectile 会被销毁。
未过期 projectile 不会被销毁。
RuntimeTraceReport.irRules 能看到 prototype.projectile.cleanup_lifetime。
sourceMap / ruleId 能作为查错证据。
```

## 剩余边界

本需求没有实现：

```text
完整碰撞流程。
完整命中流程。
完整伤害流程。
完整 projectile spawn 流程。
IR 直接读写 ECS。
ECS 调度规则修改。
```

## 当前结论

该需求是一次成功的小型验证：

```text
IR 可以承接局部项目规则。
Runtime System 可以作为 IR 与 ECS 的边界层。
Trace 可以把行为追到规则。
```

但它仍然只是 prototype system 接入 IR，不代表完整玩法系统已经迁移完成。

