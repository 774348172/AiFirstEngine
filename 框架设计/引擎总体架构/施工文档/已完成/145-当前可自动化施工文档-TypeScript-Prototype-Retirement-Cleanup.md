# 145 当前可自动化施工文档：TypeScript Prototype Retirement Cleanup

## 1. 对应方案

```text
145-TypeScript-Prototype-Retirement-Cleanup方案.md
```

## 2. 施工目标

退役 TypeScript / Electron / Vite 原型层的主动入口，将其归档为 legacy reference，并确认 Rust 正式主线仍可构建和测试。

## 3. 施工边界

本轮处理：

```text
src/
electron/
tests/
scripts/
schemas/
temp-launch/
index.html
package.json
package-lock.json
tsconfig.json
vite.config.ts
vite.runtime.config.ts
playwright.config.ts
README.md
文档入口
```

本轮不处理：

```text
rust/ 正式代码逻辑
dist/
exports/
game-runtime/
release/
node_modules/
.tmp/
test-results/
```

## 4. Gate A：归档 legacy TypeScript 原型层

操作：

```text
创建 legacy/typescript-prototype/
移动 TS/Electron/Vite 原型入口和源码到 legacy/typescript-prototype/
新增 legacy/typescript-prototype/README.md，标记其为历史参考，不是正式入口
```

测试：

```powershell
Test-Path legacy/typescript-prototype/package.json
Test-Path legacy/typescript-prototype/src/runtime-backends/typescript/TypeScriptRuntimeBackend.ts
```

## 5. Gate B：更新根目录 README 和架构入口

操作：

```text
README.md 改为 Rust Native Runtime / Native Editor Host 正式入口。
00-文档地图.md 增加 145 方案，并修正 TypeScript 退役规则索引。
49-当前状态与下一步入口.md 增加 TS 原型已归档结论。
54-当前可自动化施工总入口.md 当前施工指向本施工文档。
施工文档/README.md 当前施工指向本施工文档。
```

测试：

```powershell
rg -n "React \\+ TypeScript \\+ Vite 编辑器壳|npm\\.cmd run dev|npm\\.cmd run dist:win" README.md 框架设计/引擎总体架构/49-当前状态与下一步入口.md
```

期望：无命中。

## 6. Gate C：搜索门禁

操作：

```text
确认 TypeScriptRuntimeBackend 只存在于 legacy/typescript-prototype 内。
确认 rust/ 不引用 TypeScriptRuntimeBackend / electron / vite / npm 主动入口。
```

测试：

```powershell
rg -n "TypeScriptRuntimeBackend|runtime-backends/typescript|electron|vite|npm\\.cmd" rust
rg -n "TypeScriptRuntimeBackend" . -g "!legacy/typescript-prototype/**" -g "!node_modules/**" -g "!rust/target/**"
```

期望：

```text
rust 搜索无正式依赖命中。
非 legacy 搜索只允许历史文档提及，不允许 active source / root active entry 命中。
```

## 7. Gate D：Rust 回归

测试：

```powershell
cargo fmt --check
cargo test -p editor_core
cargo test -p engine_runtime
cargo test -p editor_window_winit
```

## 8. Gate E：归档施工文档

操作：

```text
移动本施工文档到 施工文档/已完成/
新增 阶段完成记录/2026-07-02-TypeScript-Prototype-Retirement-Cleanup/00-总览.md
更新 54-当前可自动化施工总入口.md
更新 施工文档/README.md
更新 阶段完成记录/README.md
```

## 9. 施工文档自审

### Specification fit

覆盖用户要求：先形成清理施工文档，再按建议依次执行。

### Rule fit

不扩展 TypeScript Runtime，不把 legacy 作为正式 runtime，不碰 Rust 正式逻辑。

### Textual consistency

Gate A-E 从归档、入口更新、搜索门禁、Rust 回归到文档收口，顺序清晰。

### Design fit

通过 legacy 隔离减少未来 AI 误读，保留历史参考，符合长期维护。

### Implementation feasibility

移动目标清晰，Rust 工作区独立，测试可验证。

### Practical reasonableness

不删除生成物和依赖目录，避免本轮清理扩大到环境维护问题。

结论：

```text
施工文档通过自审，可以施工。
```

