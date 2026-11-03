# Rust 工具链门禁

## 功能目标

建立 Rust Runtime 的第一道自动化门禁：

```text
确认 cargo / rustc 可用。
识别 Windows 默认 MSVC toolchain。
如果本机没有 MSVC link.exe，则自动确认 GNU Rust toolchain 是否可用于测试。
给后续 Rust Runtime 测试脚本提供统一 PATH 和工具链选择基础。
```

## 实现文件

```text
scripts/check-rust-toolchain.cjs
```

相关 npm script：

```text
test:rusttoolchain
```

## 关键实现

```text
自动把 %USERPROFILE%\.cargo\bin 加入 PATH。
执行 cargo --version。
执行 rustc --version。
读取 rustc -vV 的 host。
Windows 下检测 link.exe。
没有 link.exe 时检测 stable-x86_64-pc-windows-gnu 是否可用。
```

## 最小测试用例

命令：

```powershell
npm.cmd run test:rusttoolchain
```

本轮结果：

```text
cargo 1.96.0 可用。
rustc 1.96.0 可用。
host = x86_64-pc-windows-msvc。
MSVC link.exe 不存在。
GNU Rust toolchain 可用于测试。
测试通过。
```

## 边界

```text
不安装真实平台 SDK。
不改变默认 Rust toolchain。
不把 GNU toolchain 作为正式发布链路决策。
只用于当前开发机的 Rust 测试可执行性。
```
