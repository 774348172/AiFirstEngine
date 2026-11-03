# 历史文档

本目录保存已经被替换或废弃的架构方案。

历史文档只用于追溯讨论过程，不代表当前正式路线。

当前正式路线以父目录中的同名或对应编号文档为准。

## 使用规则

```text
本目录只作为历史追溯和迁移证据。
不能从本目录直接开工。
不能把本目录中的 TypeScriptRuntimeBackend / Electron / npm / Vite 记录当成当前入口。
当前正式入口以 ../00-文档地图.md、../49-当前状态与下一步入口.md、../54-当前可自动化施工总入口.md 为准。
```

## TypeScript 原型期记录

```text
当前项目改造计划-TS原型期历史记录.md
```

该文档中的 `src/`、`scripts/`、`schemas/`、`TypeScriptRuntimeBackend`、`npm.cmd` 等内容均属于历史原型期。

例如：

```text
05-逻辑系统边界-DSL-IR-WASM-ECS-历史版.md
  -> 记录早期 IR -> WASM 项目逻辑路线

../05-逻辑系统边界-DSL-IR-RustAOT-ECS.md
  -> 当前正式路线：IR Interpreter + IR -> Rust AOT + Rust ECS Runtime
```
