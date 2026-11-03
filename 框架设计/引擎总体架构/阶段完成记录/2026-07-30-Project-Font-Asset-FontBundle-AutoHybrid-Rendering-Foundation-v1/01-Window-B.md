# 261 Window B / Gate B 完成记录

日期：2026-07-30

状态：Window B / Gate B 已完成

证据级别：dirty-worktree development evidence

## 1. 完成范围

Window B 建立了唯一深 `ProjectFontCookModule`。它收集 RuntimePackage 可达 AUI 文本、
localization、FontAtlasProfile 显式 range 与 `project-text-source.v1`，按
FontStack / Family / Face 顺序把 Unicode codepoint 解析为真实 glyph id，并在编辑器 cook
阶段生成确定性 Hinting R8 variant、标准化 metrics、基础 kerning 与 digest。

`ProjectRuntimePackageAssembler` 不再直接调用旧 5x7 cooker，只调用
`ProjectFontCookModule::cook_for_runtime_package`。无正式字体资产的旧项目仍由该模块内部
转入 208 C-min legacy adapter；正式字体资产缺 required glyph 时 fail closed，不把中文
替换为 `?`。

Window B 只保留单页 `RuntimePackageSourceFontAtlas` 作为进入 Window C 前的兼容 bridge，
不声称完成 FontBundle v2、multi-page、MSDF、GPU present 或 runtime generation。

## 2. 主要文件

```text
rust/crates/editor_core/src/project_font_cook.rs
rust/crates/editor_core/src/project_runtime_package_assembler.rs
rust/crates/editor_core/src/project_font_assets.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/Cargo.toml
rust/Cargo.lock
```

## 3. Gate B 验证

```text
cargo test -p editor_core project_font_cook --offline -- --nocapture
结果：5 passed；0 failed

cargo test -p editor_core project_runtime_package_assembler --offline -- --nocapture
结果：10 passed；0 failed
```

第一条过滤已同时运行模块内 `project_font_glyph_collection` 与
`project_font_hinting_r8` 测试；相同输入和二进制下没有机械重复两个子过滤。

覆盖证据：

```text
AUI / localization / Project Text Source 的 A、数字、标点、中、界、曲均进入 glyph set
中文 glyph id 与 '?' glyph id 不同
required glyph U+9F98 缯失时返回 RequiredGlyphMissing
R8 width/height/stride/byte_len 合法且中文 alpha 非空
连续两次 cook 的 dependency/output digest、variant 和 atlas bytes 相同
文本字符集变化会改变 dependency/output digest
同一 face/glyph/pixel_size 在一次 cook 内只生成一个 variant
Assembler 受影响 consumer 回归通过
```

## 4. 失败与修复

首次编译暴露两个局部实现错误：`ProjectFontCookOutput` 对 legacy atlas bridge 派生了它不支持
的 `Eq/Deserialize`，Unicode 单点/range 的可选上界解析错误。修复后 owner 编译和 Gate
均通过。未发现需要修改 261 正式方案的设计冲突。

## 5. 边界与下一步

未运行 Local CI、production replacement、真实安装态或真实 GPU/窗口验证。工作树包含
260 和用户已有改动，因此本记录不能作为 clean exact-commit 或 release 证据。

按 2026-07-30 19:56 +08:00 至 22:56 +08:00 的连续授权，Gate B 通过后进入
Window C / Gate C；前一 Gate 失败仍须立即停止。
