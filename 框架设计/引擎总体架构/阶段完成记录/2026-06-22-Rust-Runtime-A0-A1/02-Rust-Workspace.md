# Rust Workspace

## 功能目标

建立正式 Rust Runtime 的最小工程入口：

```text
rust/
  Cargo.toml
  crates/
    engine_runtime/
      Cargo.toml
      src/lib.rs
```

这一步只建立 runtime crate，不接编辑器、不接 Electron、不接真实渲染。

## 实现文件

```text
rust/Cargo.toml
rust/Cargo.lock
rust/crates/engine_runtime/Cargo.toml
rust/crates/engine_runtime/src/lib.rs
scripts/test-rust-runtime-workspace.cjs
```

相关 npm script：

```text
test:rustruntime
```

## 关键实现

```text
workspace members = crates/engine_runtime。
engine_runtime crate 提供最小 smoke function。
Node 测试脚本调用 cargo test -p engine_runtime。
Windows 无 link.exe 时自动使用 stable-x86_64-pc-windows-gnu。
```

## 最小测试用例

命令：

```powershell
npm.cmd run test:rustruntime
```

覆盖测试：

```text
runtime_smoke_test_passes
```

本轮结果：

```text
Rust workspace 可被 cargo 识别。
engine_runtime crate 可编译并执行测试。
测试通过。
```

## 边界

```text
不接 RuntimeService。
不接 sidecar。
不接 TypeScript Runtime。
不实现真实平台窗口或渲染。
```
