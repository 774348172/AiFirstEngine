# 271 Engine Built-in Chinese Default FontPack v1 施工文档

状态：Gate A-F 已完成并归档。用户于 2026-08-04 授权生成施工文档并直接施工。

正式方案：`271-Engine-BuiltIn-Chinese-Default-FontPack-v1方案.md`。

## 1. 施工目标

在不改变 RuntimePackage 唯一运行输入、不修改任何样例项目内容的前提下，导入 hash-pinned 的
Noto Sans SC Regular 开发字体源，冻结版本化常用中文 glyph spec/lock，离线生成 sealed built-in
Core FontBundle，并让没有项目 DefaultUi 字体的普通项目由 `ProjectRuntimePackageAssembler` 默认
装配该 FontBundle。项目自定义 DefaultUi/Additional 字体继续走 261/270。

## 2. Owner、consumer 与预计文件

Owner：

```text
rust/crates/editor_core/src/engine_builtin_font_pack.rs（新增）
rust/crates/editor_core/src/bin/build_builtin_font_pack.rs（新增或等价 repo-owned producer）
rust/crates/editor_core/src/project_font_cook.rs（只提取可复用 build primitive，如确有需要）
rust/crates/editor_core/src/project_runtime_package_assembler.rs
rust/crates/editor_core/src/lib.rs
```

资源与 schema：

```text
rust/resources/runtime/font-packs/aife-default-zh-cn-common-v1/source/
rust/resources/runtime/font-packs/aife-default-zh-cn-common-v1/glyph-set/
rust/resources/runtime/font-packs/aife-default-zh-cn-common-v1/cooked/
rust/resources/runtime/font-packs/aife-default-zh-cn-common-v1/LICENSE-OFL-1.1.txt
rust/resources/runtime/font-packs/aife-default-zh-cn-common-v1/provenance.json
```

直接 consumer：

```text
rust/crates/engine_runtime/src/font_bundle.rs（优先不改）
rust/crates/engine_runtime/src/runtime_package_builder.rs（优先复用现有 typed input）
Editor Preview / Play / Export 的现有 Assembler consumer
```

## 3. 授权与禁止项

```text
允许修改引擎 Rust 源码、引擎 repo-owned 字体资源、测试和正式文档。
允许从用户提供的 zip 导入明确 hash 的 Regular face 与 OFL。
允许使用 run-owned target/cache/evidence root 生成 FontPack 和运行受影响测试。
禁止修改 Tower Defense、Complex Shooter、Switch Puzzle 的源项目内容。
禁止 Runtime 读取完整 TTF、项目源目录、Editor resource path 或 producer cache。
禁止用裁剪 TTF 替代完整生僻字源。
禁止实现 glyph shard cache、MSDF 并行化、Runtime 动态 raster 或系统字体 fallback。
禁止运行 Local CI、替换 production/安装态二进制、修改真实用户配置。
禁止自动恢复 Tower P0-5 Gate G/H。
```

## 4. Gate A：字体源、provenance 与容量 spike

### 实现

1. 从 `Noto_Sans_SC.zip` 精确导入 `static/NotoSansSC-Regular.ttf`。
2. 校验 size `10559284` 和 SHA-256
   `d45f67f0a7c0ca3f256950777ce6a61cc7ce5f9696d02900cbbaac25f8aa7d16`。
3. 导入 OFL，建立 machine-readable provenance；Desktop 路径不得进入长期 identity。
4. 选用许可明确、hash-pinned 的中文频率来源，生成小型 repo-owned ranked corpus。
5. 用真实 producer spike 记录 1200-1500 Han 候选的 glyph/page/raw byte 预算；若不能同时满足
   `<=1800 total` 和 `<=40 MiB raw pages`，停线修订方案。

### 验证

```powershell
Get-FileHash rust/resources/runtime/font-packs/aife-default-zh-cn-common-v1/source/NotoSansSC-Regular.ttf -Algorithm SHA256
cargo test -p editor_core engine_builtin_font_pack_source -- --nocapture
```

## 5. Gate B：glyph spec/lock schema

### 实现

1. 冻结 `engine-default-glyph-set-spec.v1` 和 `engine-default-glyph-set-lock.v1`。
2. spec 只声明合法来源、目标和预算；lock 保存排序 codepoint、source tags 和 digest。
3. 必含 ASCII、中文标点、常用 UI 符号、U+FFFD、引擎 zh-CN Catalog/模板字符与 1200-1500
   个 ranked Han。
4. 不扫描 Rust 源码、日志、schema key、sample 文案或整个 CJK range。
5. 相同输入双 resolve byte-identical；增删字符正确改变 digest；非法 schema/source/budget fail closed。

### 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core engine_default_glyph_set -- --nocapture
```

## 6. Gate C：离线 built-in FontPack 与 sealed manifest

### 实现

1. 建立深 `EngineBuiltInFontPack` owner，复用 261 的 `ProjectFontCookModule` backend 和
   `ProjectFontBundleBuilder`；不得复制 parser/raster/MSDF/packer。
2. 冻结 `engine-built-in-font-pack-manifest.v1`，记录 source/glyph/recipe/bundle/page/budget identity。
3. repo-owned producer 生成 metadata 与独立二进制 page payload；禁止把几十 MiB bytes 写成 JSON 数组。
4. product consumer 通过 `include_bytes!` 或等价 release-owned resource 封装 typed
   `RuntimePackageSourceFontBundle`，但 Runtime 不获得完整 TTF。
5. source/lock/recipe 变化必须使 manifest identity 改变；metadata/page corrupt、hash/size mismatch、
   schema 不兼容和预算超限 fail closed。

### 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core engine_builtin_font_pack -- --nocapture
cargo test -p engine_runtime font_bundle -- --nocapture
```

必须证明：

```text
Han 1200-1500
total codepoints <= 1800
raw pages <= 40 MiB
16/24/32 Bitmap 与 64px MSDF coverage 完整
双 load/build digest 相同
```

## 7. Gate D：Assembler 默认选择与注入

### 实现

Assembler 字体选择：

```text
无 project DefaultUi -> built-in default
只有 project Additional -> built-in default + project Additional
唯一 project DefaultUi -> project default，继续走 270
多个 project DefaultUi -> fail closed
project DefaultUi cook failure -> fail closed，不回退 built-in
```

1. built-in pack 进入现有 `RuntimePackageBuildInput.font_bundles`。
2. 为 producer report 增加 built-in selection/injection 可解释状态，不记录原文。
3. 无项目字体时不得再生成 legacy C-min atlas。
4. `RuntimePackageBuilder`、loader 和 renderer 优先保持不变。

### 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core engine_builtin_font_pack_selection -- --nocapture
cargo test -p editor_core project_runtime_package_assembler -- --nocapture
```

## 8. Gate E：RuntimePackage 与通用项目矩阵

### 验证

至少覆盖：

```text
无 Font Assets 的最小项目
只有 Additional profile 的项目
唯一 custom DefaultUi 项目
多个 DefaultUi 否定 fixture
corrupt built-in manifest/page 否定 fixture
```

证明：

```text
Preview / Play / Export 使用相同 bundle digest
RuntimePackage metadata/page 自包含
导出目录不含 TTF/OTF/TTC
Runtime 依赖闭包没有 parser/raster/MSDF generator
custom DefaultUi 仍走 270 recipe cache
```

测试：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime runtime_package_builder -- --nocapture
cargo test -p editor_core project_font_artifact_cache -- --nocapture
cargo test -p editor_core project_runtime_package_assembler -- --nocapture
```

过滤器以真实非零匹配测试为准；上层 suite 已覆盖的同配置测试不机械重复。

## 9. Gate F：受影响回归、性能/容量与闭环

### Development/Integration 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core
cargo test -p engine_runtime
cargo fmt --all -- --check
```

根据 Gate D/E 的真实 consumer 变化决定是否增加 `project_e2e_gate` 定向测试；不得机械运行完整 workspace。

### 性能/容量证据

在 run-owned root 使用无自定义字体项目，报告：

```text
fontSelection=builtInDefault
font raster duration=0
bundle/glyphSet digest
codepoint/variant/page count
raw page bytes
selection/injection duration
RuntimePackage load success
```

### 真实视觉

若现有 source-built、run-owned Windows/WGPU visual gate 可在不替换 production binary、不修改真实配置
的条件下直接复用，则运行最小中文 Bitmap/MSDF 视觉 smoke；否则记录为 release activation 证据豁免，
不得用 headless count 冒充真实视觉。

### 不运行

```text
Local CI
production/安装态二进制替换
真实用户配置修改
Tower Gate G/H
```

## 10. 完成与归档

全部 Gate 通过后：

1. 生成 `阶段完成记录/2026-08-04-Engine-BuiltIn-Chinese-Default-FontPack-v1/00-总览.md`。
2. 更新 `00-文档地图.md`、`49-当前状态与下一步入口.md`、`54-当前可自动化施工总入口.md`、
   `施工文档/README.md`、`阶段完成记录/README.md`。
3. 将本文从 `施工文档/当前/` 移入 `施工文档/已完成/`。
4. glyph shard 与 MSDF 并行化保持 deferred，后续另行讨论和授权。

## 11. 施工文档自审

```text
与 271 正式方案一致：是
复用 261/270 owner：是
RuntimePackage 仍为唯一 Runtime 输入：是
修改样例项目：否
实现 glyph shard/MSDF 并行化：否
包含每 Gate 测试和停线条件：是
外部审查文档：未发现针对 271 的其它 AI 审查；无需修改正式方案
当前施工槽原为空：是
用户已授权直接施工：是
当前可施工：是
```

## 12. 最终施工结果

```text
Gate A：通过；字体源、OFL、频率语料与 provenance 已固定
Gate B：通过；glyph spec/lock 确定性与预算测试通过
Gate C：通过；1357 codepoint / 1223 Han，3 Bitmap + 7 MSDF page，raw 32505856 bytes
Gate D：通过；built-in / Additional / custom DefaultUi / multiple DefaultUi 选择矩阵完成
Gate E：通过；Assembler、FontBundle loader 与 RuntimePackage builder 受影响矩阵通过
Gate F：通过；editor_core 631/2 ignored、engine_runtime 504、fmt 通过
完成记录：阶段完成记录/2026-08-04-Engine-BuiltIn-Chinese-Default-FontPack-v1/00-总览.md
```

未执行 Local CI、production/安装态替换、真实配置修改、Tower Gate G/H 和新增 release visual gate。
