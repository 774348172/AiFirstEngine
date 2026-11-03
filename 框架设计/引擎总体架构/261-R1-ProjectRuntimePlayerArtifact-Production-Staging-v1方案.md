# 261-R1 ProjectRuntimePlayerArtifact Production Staging v1 方案

## 1. 文档状态

```text
系统编号：261-R1
方案版本：v1
建立日期：2026-07-31
父系统：261 Project Font Asset / FontBundle / AutoHybrid Rendering Foundation v1
缺口来源：261 Window H production Windows Player 导出
用户授权：允许修改引擎代码，独立修复 ProjectRuntimePlayerArtifact production staging
当前状态：正式方案已确认并完成方案自审
```

本文只修复普通 `ProjectRust` 项目进入独立 Player artifact 的 production staging 合同。
`controlled-source-patch.v1` 仍是 AI SourcePatch 作者输入的最小能力合同，不因本方案放宽。

## 2. 问题与目标

`ProjectRuntimePlayerArtifact::build_project_rust` 当前复用
`validate_applied_project_rust_source` / `stage_validated_project_rust_source`。后两者只允许
`engine_runtime` 和可选 `engine_input` 的精确版本字符串，适用于 SourcePatch v1，却会拒绝
普通项目已经存在的合法 Cargo 清单，例如塔防项目所需的 `serde` / `serde_json`，以及
Complex Shooter、Switch Puzzle 使用的 trusted Engine SDK path dependency。

目标链路：

```text
project.aife.json + RuntimeModule source Cargo.toml
  -> ProjectRuntimePlayer production validator
  -> trusted Engine SDK resolve
  -> controlled third-party dependency resolve from trusted SDK Cargo.lock
  -> isolated RuntimeModule copy
  -> source-compatible Cargo.toml + normalized RuntimeModuleBuild + Cargo.lock
  -> locked + offline format/compile/build
  -> Player artifact report with normalized dependency identity
```

源项目清单和源码全程只读；规范化只写入独立 staging root。

## 3. 两套策略

### 3.1 SourcePatch 最小依赖策略

- owner 保持 `controlled_source_patch.rs`；
- 继续只接受精确版本 `engine_runtime` 和可选 `engine_input`；
- 继续拒绝 path/git/workspace、第三方依赖、build/dev dependency 和项目 Cargo config；
- 原有 diagnostic code 与测试保持不变。

### 3.2 普通项目 production export 策略

- owner 为独立 `project_runtime_player_staging` 模块，由
  `ProjectRuntimePlayerArtifact` 调用；
- 接受项目 manifest 声明的 `ProjectRust` RuntimeModule；
- Engine SDK 依赖只允许 `engine_runtime` / `engine_input`，源清单可使用匹配
  `engineVersion` 的 crates.io 版本或解析后恰好指向本次 trusted SDK crate 的 path；
- staging 保留源 `RuntimeModule/Cargo.toml` byte identity，避免破坏 RuntimeModule AOT
  digest；另生成只属于 staging 的 `RuntimeModuleBuild/Cargo.toml`，其 `lib.path` 指回
  `../RuntimeModule/src/lib.rs`，把 Engine SDK 依赖统一解析到 canonical trusted SDK path。
  原 `RuntimeModule/tests/*.rs` 以确定顺序生成显式 `[[test]]` target，保持 production
  tests-compile 覆盖；
  Host 只依赖该规范化构建包，因此依赖解析不得依靠 artifact root 与仓库的偶然相对布局；
- v1 受控第三方依赖只允许 `serde` 与 `serde_json`，允许 version、
  `default-features` 和 feature list，不允许 path/git/registry override/package rename；
- 第三方版本必须满足源 version requirement；项目已有 `RuntimeModule/Cargo.lock` 时以该
  lock 的唯一 crates.io package identity 为 production 真相，缺失时才以 trusted Engine SDK
  `Cargo.lock` 离线生成；规范化 exact identity 写入 report/cache；
- staging 保留已有项目 lock；缺失时使用 trusted Engine SDK lock seed。所有 Cargo
  compile/build 命令使用
  `--locked --offline`，禁止在导出时联网补依赖；
- 拒绝 project build script、`build-dependencies`、`dev-dependencies`、workspace
  inheritance、patch/replace、target-specific dependencies、任意 Cargo config 和链接/
  reparse escape；
- `[lib] path = "src/lib.rs"` 可作为普通 Cargo 默认路径兼容输入，staging 中移除冗余字段；
  非默认 lib path、proc-macro 和 crate-type 仍拒绝；默认字段保留原字节；
- production source copy 覆盖项目 regular tree，使 RuntimeModule 的合法 `include_str!` /
  `include_bytes!` 项目相对引用继续成立；排除 `Build`、任意 `target`、`.git`、`.cargo`、
  `.aife` preview cache 和 `.gitignore`，其余项目 source/fixture/lock 保持相对路径与字节。

第三方依赖扩充必须以后续版本修改显式 allowlist、owner 测试和正式方案，不能把任意
Cargo dependency 当作普通导出默认能力。

## 4. 规范化产物与报告

production staging 生成：

```text
RuntimeModule/Cargo.toml
RuntimeModule/Cargo.lock
RuntimeModuleBuild/Cargo.toml
RuntimeModuleBuild/Cargo.lock
Host/Cargo.toml
Host/Cargo.lock
```

其中 `RuntimeModule/Cargo.toml` 是源字节副本，`RuntimeModuleBuild/Cargo.toml` 是只用于
production artifact 编译的规范化清单。`ProjectRuntimePlayerArtifactBuildReport` 增加
可选/default 兼容字段：

```text
stagingPolicy
normalizedManifestDigest
normalizedDependencyDigest
normalizedDependencies[]
trustedLockDigest
```

每个 dependency entry 至少记录 dependency name、kind（engine_sdk/crates_io）、
resolved version、source identity 和 enabled features。artifact cache key 纳入 normalized
manifest/dependency/trusted lock digest，避免依赖身份变化复用旧 Player。

报告不记录逐文件源码或用户目录外的秘密；路径只记录本次已经公开到报告的 trusted SDK
和 artifact staging 身份。

## 5. 错误合同

production staging 使用 `project_runtime.player_artifact_staging_*` diagnostics，至少区分：

```text
manifest_invalid
manifest_policy_rejected
engine_dependency_untrusted
third_party_dependency_unsupported
trusted_lock_missing
trusted_lock_dependency_missing
dependency_version_mismatch
source_tree_rejected
normalized_manifest_write_failed
```

SourcePatch 仍使用 `controlled_source_patch.*`，两套错误域不能互相伪装。

## 6. 验证

owner 红测必须使用真实 ProjectRust fixture，覆盖：

- trusted Engine SDK path `engine_runtime`；
- `serde` derive 和 `serde_json`；
- source Cargo manifest/lock 在 staging 前后 byte-exact 不变；
- staged source manifest byte identity、normalized build manifest canonical SDK path、报告中的第三方精确
  identity；
- staged lock 与选定 project/trusted lock identity；
- normalized digest/report 非空；
- path/git/build script/unsupported third party 被 production policy 拒绝；
- 原 SourcePatch 严格拒绝第三方依赖的回归。

修复后执行 owner test、SourcePatch owner regression、`editor_core` 全量，以及实际受影响的
塔防 RuntimeModule、FontBundle v2 和 `project_e2e_gate` consumer。Window H production
export 只有在上述证据通过后恢复；Local CI 和 production/安装态二进制替换仍禁止。

## 7. 方案自审

```text
是否放宽 SourcePatch v1：否
是否修改项目源 Cargo.toml/Cargo.lock：否
是否允许任意 path/git dependency：否
是否允许运行时联网解析：否
是否允许项目 build script：否
是否把塔防语义写入引擎：否
是否新增第二个当前施工文档：否
是否保持 RuntimePackage/ProjectPlayerArtifact 正式链路：是
是否给依赖与缓存建立可审计 identity：是
是否覆盖现有三项目所需 Engine SDK path 兼容：是
```

结论：方案与 261、242 Project RuntimeModule 边界及当前施工治理一致，可以作为 261
当前施工文档的 R1 补救 Gate 实施，无需修改 261 字体正式方案。
