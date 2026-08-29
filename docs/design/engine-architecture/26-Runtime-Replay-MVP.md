# Current Status Notice

本文档包含 TypeScript 原型期 Runtime Replay 设计。当前 Rust Native Runtime 正确性以 Golden Scenario / Trace / Test Graph 验证，不再以 TypeScriptRuntimeBackend 等价测试作为标准。

# Runtime Replay MVP

本文档记录 Runtime Replay MVP 的第一版实现。

## 定位

Runtime Replay 是 AI 查 Bug 和复杂项目回归的基础能力。

它不是帧同步专用系统，也不是完整录像系统。第一版只验证：

```text
同一个 Project
同一个 Runtime Backend
同一串输入帧
应该得到同一串 frameHash
```

这能让 AI 判断一次修改是否改变了运行行为，也能在后续 Patch / Build / Golden Scenario Test 证据中复用。

## 当前实现

新增：

```text
src/runtime/runtimeReplay.ts
scripts/test-runtime-replay.cjs
```

新增命令：

```powershell
npm.cmd run test:replay
```

## 当前能力

当前支持：

```text
输入帧序列
当前 prototype RuntimeBackend 重放
每帧 frameHash
每帧 entityCount / renderableCount
每帧 systemTraceCount / irRuleCount
两次 replay 对比
```

frameHash 当前来自：

```text
Scene entity id / kind / transform / script params
RenderSnapshot renderables / hud
```

数值会做 6 位小数规整，避免微小浮点噪声导致无意义 hash 差异。

## 当前边界

暂不做：

```text
编辑器录制 UI
外部 replay 文件格式
跨 backend 等价验证
错误 replay package
资源加载时序 replay
多线程 replay
```

这些属于后续阶段。

第一版只建立最小可测试闭环。

## 测试覆盖

当前测试覆盖：

```text
同一项目 + 同一输入帧 -> frameHashes 完全一致
不同输入帧 -> replay comparison 能发现 mismatch
Replay frame 记录 systemTraceCount
Replay frame 记录 irRuleCount
```

## 下一步

下一步建议：

```text
1. 让 Runtime Trace UI 可以显示 replay hash。
2. 让 AI Patch 验证可以运行 replay smoke。
3. Rust Runtime MVP 出现后，把 replay smoke 接入 Golden Scenario Test 证据。
```


## Current Implementation Addendum: Replay Comparison v1

Runtime Replay now outputs a structured comparison report:

```text
schemaVersion: runtime-replay-comparison.v1
left / right replay summary
differences[]
summary.high / summary.medium / summary.low
```

Difference kinds:

```text
replay-error
frame-count
frame-hash
entity-count
renderable-count
system-trace-count
ir-rule-count
```

Severity rules:

```text
replay-error: high
frame-count: high
frame-hash: high
entity-count: medium
renderable-count: medium
system-trace-count: low
ir-rule-count: low
```

Patch integration:

```text
applyPatchWithRollback runs default before/after replay comparison
Patch apply result carries RuntimePatchReplayReport
Editor console logs replay unchanged / changed with H-M-L summary
```

This is not yet a full gameplay regression suite. It is a deterministic smoke layer for AI Patch review, rollback confidence, and later Golden Scenario Test evidence.

Regression coverage:

```powershell
npm.cmd run test:replay
npm.cmd run test:rollback
```

## Current Implementation Addendum: Replay Debug Package v1

Runtime Replay now has a portable debug package structure:

```text
schemaVersion: runtime-replay-debug-package.v1
packageId
reason
project summary
patch summary
frames
replay comparison report
errors
createdAt
```

Package reasons:

```text
patch-replay-changed
patch-validation-failed
runtime-error
manual
```

Patch integration:

```text
applyPatchWithRollback creates a RuntimeReplayDebugPackage
Editor console logs replay debug package id and reason
The package is currently returned as structured data, not written to disk yet
```

Design rule:

```text
Replay Debug Package is AI-facing evidence.
It should contain enough data for AI repair and user review, without requiring AI to scrape console strings.
Future versions may serialize this package to JSON files and attach it to Validation Report / Patch History.
```

Regression coverage:

```powershell
npm.cmd run test:replay
npm.cmd run test:rollback
```

## Current Implementation Addendum: Patch History Replay Debug Summary v1

Patch History now persists a compact replay debug summary:

```text
schemaVersion: runtime-replay-debug-summary.v1
packageId
reason
patchId
frameCount
differences.high / medium / low
errors
createdAt
```

Persistence rule:

```text
Full RuntimeReplayDebugPackage is returned by applyPatchWithRollback for immediate AI/debug use.
Patch History stores only ProjectPatchReplayDebugSummary to avoid unbounded history growth.
The summary keeps the stable packageId so a future file-backed package store can link back to the full JSON package.
```

Editor rule:

```text
Patch History detail shows Replay Debug package id, reason, frame count, H/M/L diff count, and first errors.
AI does not need to parse console strings to know whether a patch changed replay behavior.
```

Regression coverage:

```powershell
npm.cmd run test:rollback
npm.cmd run test:replay
```

## Current Implementation Addendum: Replay Debug Package Store v1

Runtime Replay now has a dedicated in-memory package store:

```text
schemaVersion: replay-debug-package-store.v1
packages: Record<packageId, RuntimeReplayDebugPackage>
index: ReplayDebugPackageStoreEntry[]
```

Store rule:

```text
Patch History must not persist full replay debug packages.
Patch History persists only ProjectPatchReplayDebugSummary and packageId.
Full RuntimeReplayDebugPackage objects live in ReplayDebugPackageStore.
The store keeps an indexed list for lookup, display, eviction, and future file-backed persistence.
```

Current editor integration:

```text
applyPatchWithRollback still returns RuntimeReplayDebugPackage.
App records returned packages into an in-session ReplayDebugPackageStore.
Patch History detail checks packageId against the store and shows whether the full package is available in the current session.
```

Capacity rule:

```text
The default store keeps the newest packages by createdAt.
Duplicate packageId updates the stored package instead of creating duplicate index entries.
Invalid persisted data is normalized and invalid packages / orphan index entries are dropped.
```

Future persistence rule:

```text
The current store is intentionally pure data.
Future disk-backed JSON storage or package export should reuse replay-debug-package-store.v1 instead of embedding full packages into project data.
```

Regression coverage:

```powershell
npm.cmd run test:replaystore
npm.cmd run test:replay
npm.cmd run test:rollback
```

## Current Implementation Addendum: AI Replay Debug Evidence v1

AI repair now receives replay debug evidence as compact references:

```text
schemaVersion: ai-replay-debug-evidence.v1
packageId
reason
patchId
project name / version / activeSceneId
frameCount
differences.high / medium / low
errorCount
first errors
createdAt
```

Evidence rule:

```text
AiErrorReport may attach AiReplayDebugEvidence[].
AiReplayDebugEvidence must reference ReplayDebugPackageStore by packageId.
It must not embed RuntimeReplayDebugPackage.frames.
It must not embed the full replay comparison left/right reports.
Full evidence is retrieved from ReplayDebugPackageStore or replay-debug-archive.v1 by packageId.
```

Repair rule:

```text
AiRepairResult may carry evidencePackageIds.
The AI repair pipeline can use these ids to request full evidence when needed.
The default user-facing report stays small, stable, and readable.
```

Current implementation:

```text
src/ai/aiRepair.ts
createAiReplayDebugEvidence
attachReplayDebugEvidence
scripts/test-ai-replay-debug-evidence.cjs
npm.cmd run test:aireplayevidence
```

This keeps AI repair connected to runtime replay evidence without turning project data, patch history, or AI validation reports into unbounded debug archives.

## Historical Implementation Addendum: Runtime Backend Equivalence v1

Runtime Replay currently has a prototype backend equivalence report:

```text
schemaVersion: runtime-backend-equivalence.v1
project summary
left backend summary
right backend summary
frameCount
frames[]
differences[]
summary.high / medium / low
```

Historical equivalence rule:

```text
Two RuntimeBackend implementations receive the same Project and the same input frame sequence.
Both backends load the same scene, tick the same number of frames, and produce equivalent normalized Scene + RenderSnapshot hashes.
The report also compares entity count, renderable count, system trace count, and IR rule trace count.
```

Boundary rule:

```text
The equivalence layer does not understand gameplay rules.
It only checks prototype runtime output consistency.
This layer is now classified as prototype evidence only.
It must not become the validation standard for Rust Native Runtime.
Rust Native Runtime correctness is validated by Golden Scenario Test, not by matching TypeScript Runtime behavior.
Existing runtimeEquivalence code may remain temporarily as migration evidence, but should be retired with TypeScript Runtime.
```

Current implementation:

```text
src/runtime/runtimeEquivalence.ts
scripts/test-runtime-equivalence.cjs
npm.cmd run test:runtimeequivalence
```

Current deterministic runtime rule kept for migration:

```text
ECS World owns deterministic runtime entity ids and deterministic runtime random numbers.
Runtime systems that spawn entities must use nextRuntimeEntityId(world, prefix).
Runtime systems that need random-like variation must use nextRuntimeRandom(world).
Runtime systems must not use crypto.randomUUID() or Math.random() for replay-visible gameplay state.
```

Current prototype result:

```text
starterProject is strict-equivalent across two TypeScriptRuntimeBackend instances.
shooterProject is strict-equivalent across two TypeScriptRuntimeBackend instances after deterministic runtime cleanup.
```

## Current Implementation Addendum: Replay Debug Archive v1

Runtime Replay now has a portable archive format for AI/debug evidence export:

```text
schemaVersion: replay-debug-archive.v1
createdAt
source.engine: ai-first-engine
source.purpose: ai-debug-evidence
packageCount
store: ReplayDebugPackageStore
```

Archive rule:

```text
Replay Debug Archive is not project data.
It is derived evidence for AI repair, user review, bug report attachment, and future file-backed debug storage.
It serializes ReplayDebugPackageStore instead of embedding full packages in Patch History.
```

Selection rule:

```text
Archive export can include all packages or an explicit packageId subset.
Duplicate packageIds are exported once.
Missing packageIds are reported structurally instead of silently ignored.
```

File naming rule:

```text
Archive filenames are generated from a sanitized prefix, archive createdAt, and package count.
This keeps exported debug files stable enough for users and AI tools to identify.
```

Current implementation:

```text
src/runtime/replayDebugArchive.ts
scripts/test-replay-debug-archive.cjs
npm.cmd run test:replayarchive
```

Editor export integration:

```text
The editor can export the current in-session ReplayDebugPackageStore as a replay-debug-archive.v1 JSON file.
The export is available from the top toolbar and AI/File menu entries.
If no replay debug package exists in the session, the editor logs a structured user-facing message and does not export an empty archive.
```

Current boundary:

```text
This is browser-download export, not a durable file-backed debug store yet.
Electron / CLI file-backed export should reuse the same ReplayDebugArchive schema instead of inventing a second format.
```

## Current Implementation Addendum: Replay Debug Archive File Export v1

Replay Debug Archive now has a file-backed export path:

```text
scripts/replay-debug-archive-export.cjs
scripts/export-replay-debug-archive.cjs
electron/main.cjs IPC: export-replay-debug-archive
electron/preload.cjs: gameExporter.exportReplayDebugArchive
```

File export rule:

```text
Node/Electron owns file system writes.
Runtime archive modules remain pure data and do not import fs.
Desktop editor export writes replay-debug-archive.v1 JSON into exports/replay-debug.
Browser preview keeps the previous download fallback.
```

CLI rule:

```powershell
node scripts\export-replay-debug-archive.cjs <archive.json> [output-dir]
```

This allows exported archives to become stable AI repair evidence, bug report attachments, and regression artifacts without embedding full replay packages into project data.

## Current Implementation Addendum: Replay Debug Archive Import v1

Replay Debug Archive can now be imported back into the editor:

```text
src/runtime/replayDebugArchive.ts: importReplayDebugArchiveIntoStore
src/App.tsx JSON import detects replay-debug-archive.v1 before project import
tests/replay-debug-export.spec.ts exports an archive and imports it back
```

Import rule:

```text
Importing a replay-debug-archive.v1 file only merges debug evidence into ReplayDebugPackageStore.
It must not modify Project / Scene / Entity / Asset data.
Duplicate packageId updates the package.
New packageId adds the package.
Capacity eviction follows ReplayDebugPackageStore rules.
```

Editor rule:

```text
Archive JSON files are handled before project JSON validation.
This prevents debug evidence packages from being misread as invalid projects.
The editor logs added / updated / evicted counts for user and AI traceability.
```

## Current Implementation Addendum: Persistent Replay Debug Store v1

Replay Debug Store now persists independently from project data:

```text
localStorage key: ai-game-dev-environment.replay-debug-store
schemaVersion: replay-debug-package-store.v1
```

Persistence rule:

```text
ReplayDebugPackageStore is generated debug evidence.
It is saved separately from ai-game-dev-environment.project.
It must not be embedded into GameProject / Project Patch History.
Bad persisted store data is normalized or discarded without clearing the project.
```

Editor rule:

```text
Patch History is visible in the AI panel even before a new AI result exists.
Patch History header shows replay package count.
After importing a replay-debug-archive.v1 file, refreshing the editor keeps the replay package list available.
```

## Current Implementation Addendum: Replay Debug Package Viewer v1

Patch History now includes a lightweight Replay Debug Package viewer:

```text
ReplayDebugPackageStore index list
packageId
reason
difference summary
package detail
project / version / scene
frame count
patch id / title
replay H/M/L summary
first errors
```

Viewer rule:

```text
The viewer is read-only.
It must not edit project data or replay debug package data.
It is an evidence surface for AI repair, user review, and bug triage.
It is not a replay playback UI yet.
```

Editor rule:

```text
The viewer is available even after editor refresh because ReplayDebugPackageStore is persisted independently.
The viewer uses packageId as the stable bridge between Patch History summaries and full replay packages.
```

## Current Implementation Addendum: Replay Debug Cleanup Policy v1

Replay Debug Store now has explicit cleanup operations:

```text
removeReplayDebugPackage(store, packageId)
trimReplayDebugPackageStore(store, maxPackages)
```

Cleanup rule:

```text
Cleanup only removes derived replay debug evidence.
It must not modify GameProject.
It must not remove Patch History summaries.
Patch History may continue to reference a packageId whose full package has been cleaned; in that case the UI shows summary-only evidence.
```

Editor rule:

```text
Replay Debug Package Viewer can remove the selected package.
Replay Debug Package Viewer can keep only the latest 10 packages.
Cleanup changes are persisted through ai-game-dev-environment.replay-debug-store.
```

Regression coverage:

```powershell
npm.cmd run test:replayarchiveexport
npx.cmd playwright test replay-debug-export
npm.cmd run test:replayarchive
npm.cmd run test:replaystore
npm.cmd run test:replay
npm.cmd run test:rollback
```

