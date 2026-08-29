# 145 TypeScript Prototype Retirement Cleanup 方案

## 1. 问题定义

当前正式引擎主线已经迁移到 Rust Native Runtime / Native Editor Host，但仓库根目录仍保留早期 TypeScript / Electron / Vite 原型工程。

这会造成三个问题：

```text
1. 新人和 AI 容易误以为 package.json / src / electron 仍是正式入口。
2. TypeScriptRuntimeBackend 仍存在主动运行路径，和 Rust Native Runtime 唯一正式 runtime 的规则冲突。
3. 根目录同时出现 Rust 正式工程和 TS 原型工程，增加后续维护、搜索、调试成本。
```

本系统目标是退役 TypeScript 原型层的主动入口，并把历史代码归档为 legacy reference。

## 2. 已确认规则

正式规则来自：

```text
38-Rust-Native-Runtime-MVP与TypeScript退役规则.md
21-Runtime-Core-Boundary.md
10-技术路线与迁移.md
104-Trace-Replay-GoldenScenario-C-min方案.md
```

当前结论：

```text
Rust Native Runtime 是唯一正式 runtime。
TypeScript Runtime 是历史 prototype，不是规格，不是 oracle，不是长期 backend。
不做长期 TypeScript Runtime vs Rust Runtime 等价测试。
新 runtime / editor 能力必须进入 Rust 主线。
```

## 3. 其他引擎对比

### Unity

Unity 会长期保留历史兼容层，但正式 runtime/editor 入口很清晰：编辑器、运行时、Package、测试体系都有明确归属。旧系统可以存在，但不会和正式入口同层混放。

### Unreal Engine

UE 会保留 Legacy 模块和迁移适配，但通过模块目录、Build.cs、Editor/Runtime 分区明确标记归属。旧系统不应继续作为默认启动路径。

### Godot / Bevy

Godot 和 Bevy 都倾向把实验功能放在 feature、example、tool 或独立 crate/plugin 下，正式主线入口保持清晰。旧实验代码如果继续存在，应被明显隔离。

## 4. 可选方案

| 方案 | 做法 | 优点 | 缺点 | 结论 |
|---|---|---|---|---|
| A | 只在文档里标记 TS 已退役 | 最安全 | 代码残留继续误导 AI 和人 | 不选 |
| B | 只删除 TypeScriptRuntimeBackend | 范围小 | 根目录 Electron/Vite/TS 入口仍误导 | 不选 |
| C | 将整个 TS/Electron 原型层归档到 legacy，并移除根目录主动入口 | 主线清晰，可追溯，不误删历史 | 需要同步 README / package 入口 / 文档 | 选 |
| D | 直接物理删除全部 TS/Electron/Node 文件 | 最干净 | 历史迁移证据丢失，风险较高 | 后续可选，不作为本轮 |

## 5. 正式方案

选择方案 C。

### 5.1 目录规则

新增：

```text
legacy/typescript-prototype/
```

归档范围：

```text
src/
electron/
tests/
scripts/               # Node/TS 原型脚本整体归档
schemas/               # TS 原型 JSON Schema，Rust 主线如需会另建正式 schema 归口
temp-launch/
index.html
package.json
package-lock.json
tsconfig.json
vite.config.ts
vite.runtime.config.ts
playwright.config.ts
```

不归档：

```text
rust/
框架设计/
其它AI审查目录/
.agents/
.gitignore
README.md
dist/
exports/
game-runtime/
release/
node_modules/
.tmp/
test-results/
```

其中 `dist / exports / game-runtime / release / node_modules / .tmp / test-results` 是生成物或依赖目录，本轮不移动，避免误伤本地调试状态。它们继续由 `.gitignore` 管理。

### 5.2 根目录入口规则

根目录 `README.md` 必须改为 Rust 正式主线入口：

```text
rust/                         正式 Rust workspace
框架设计/引擎总体架构/          架构与施工入口
legacy/typescript-prototype/  历史 TS/Electron 原型，仅参考，不再作为正式入口
```

根目录不得继续把 `npm.cmd run dev` / `npm.cmd run dist:win` 作为默认运行方式。

### 5.3 保留规则

TypeScript 原型归档后允许：

```text
作为历史参考阅读。
作为迁移证据检索。
必要时临时手动打开 legacy 目录研究旧实现。
```

不允许：

```text
从 legacy TypeScript Runtime 继续新增正式 runtime 功能。
把 TypeScriptRuntimeBackend 作为 Rust Runtime 的等价 oracle。
让 Native Editor / Runtime 回退调用 legacy TS runtime。
把 npm / Electron 入口写回当前主线 README。
```

### 5.4 验证规则

清理完成后必须验证：

```text
cargo fmt --check
cargo test -p editor_core
cargo test -p engine_runtime
cargo test -p editor_window_winit
```

并执行搜索门禁：

```text
根目录主动入口不得再出现 package.json / electron / vite / src 作为正式主线。
rust/ 不得引用 TypeScriptRuntimeBackend。
框架当前入口不得把 TypeScript Runtime 作为正式 runtime。
```

## 6. 方案自审

### Specification fit

满足用户要求：确认 TS 原型层未完全退役，并开始形成清理施工方案。

### Rule fit

符合 Rust Native Runtime 唯一正式 runtime、TypeScript Runtime 退役、不保留长期等价测试的已确认规则。

### Textual consistency

方案区分了“正式主线退役”和“历史代码归档”，没有把 legacy reference 当作 active runtime。

### Design fit

通过目录隔离降低 AI 和人类误判入口的概率，保留历史可追溯性，避免一次性硬删带来的风险。

### Implementation feasibility

当前 Rust workspace 不依赖根目录 TS 工程；移动 TS 原型目录后，Rust 测试可以直接验证正式主线不受影响。

### Practical reasonableness

本轮只退役主动入口和归档原型层，不处理 node_modules / dist / exports 等生成物，范围可控。

结论：

```text
方案通过自审，可以生成施工文档并执行。
```

