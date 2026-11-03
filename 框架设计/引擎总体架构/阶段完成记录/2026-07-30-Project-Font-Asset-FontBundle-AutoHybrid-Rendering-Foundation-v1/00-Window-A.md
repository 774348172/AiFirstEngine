# 261 Window A / Gate A 完成记录

## 1. 结果

```text
状态：Window A / Gate A 已完成
完成日期：2026-07-30
source identity：激活时 dirty worktree；未创建提交
证据资格：dirty-worktree development evidence
未声明：exact-commit、release、Local CI、production binary replacement
后续授权：无；Window B-H 仍需用户分别授权
```

Window A 建立了项目无关的 FontFace v2、FontFamily v1、FontStack v1 和
FontAtlasProfile v1 资产合同、稳定 ID、canonical digest、Asset Graph dependency、
family exact/nearest resolve、stack fallback/cycle 检查、项目路径/hash/faceIndex
校验和结构化 Off/Summary/Trace diagnostics。

## 2. backend 资格

冻结配置：

```text
parser：ttf-parser = 0.25.1
Hinting R8：swash = 0.2.10
MSDF：fdsm = 0.8.0
font outline adapter：fdsm-ttf-parser = 0.2.0
recipe：project-font-recipe.v1;swash=0.2.10;fdsm=0.8.0;fdsm-ttf-parser=0.2.0
许可证：ttf-parser MIT/Apache-2.0；swash MIT/Apache-2.0；fdsm 系列 MIT
Windows build：MSVC target passed；纯 Rust MSDF，不要求 Clang/CMake/C++ runtime
runtime dependency：ttf-parser/swash/fdsm/msdfgen 均为 0
```

`msdfgen 0.2.1` 首测失败于 `msdfgen-sys` 没有
`x86_64-windows-msvc` 预生成 bindings；启用 bindgen 又要求当前环境不存在的
Clang/libclang。该候选没有伪装通过，随后按施工文档用同一质量标准验证纯 Rust
`fdsm`。最终真实中文、尖角/曲线、有限值与连续两次 byte-identical 均通过。

## 3. fixture 与许可

repo-owned fixture 位于 `rust/fixtures/fonts/qualification/`：

```text
AifeNotoSansSCQualification-Regular.ttf
  SHA-256 f70ecb32e5b312ba7bc724977352139a3f691566dc2491377be3828631c9fab2

AifeNotoSCQualification.ttc
  SHA-256 278c89270cd70c8b3c9f4b284b54bfe8639f8c271e5c4fdce7cc0d90251b0d75
  face 0 Noto Sans SC；face 1 Noto Serif SC

AifeQualificationCFF-Regular.otf
  SHA-256 b89d882e3082ac1f03c337af4e1ba3250384e9b4e645907a0e6f8e606ec3bf93
```

Noto 源采用 OFL-1.1，并用 FontTools 4.59.0 先实例化 `wght=400`，再确定性子集。
项目自有 CFF fixture 使用仓库内 MIT 声明。`MANIFEST.md` 记录上游、源 hash、
字符集、生成工具、face index 和产物 hash；Gate 不读取系统字体。

## 4. Gate A 验证

以下最终命令均通过：

```text
cargo test -p editor_core font_backend_qualification --offline -- --nocapture
cargo test -p editor_core project_font_asset --offline -- --nocapture
cargo test -p editor_core font_family_resolution --offline -- --nocapture
cargo test -p editor_core font_stack --offline -- --nocapture
cargo test -p editor_core font_diagnostic --offline -- --nocapture
cargo fmt --all -- --check
git diff --check
```

覆盖：

```text
真实 TTF / OTF / TTC face 0/1 与越界
小字号中文真实 hinted Alpha/R8 非空且不退化为 '?'
中文、尖角和曲线 MSDF 非空、有限、连续生成 byte-identical
路径越界、source hash mismatch、metadata mismatch、非法 faceIndex
四类 versioned schema、重复 stable id、canonical digest
family exact/nearest/lower-weight/stable-id 决胜
stack authored fallback order、missing reference、cycle
source/family/stack/profile Asset Graph dependency
domain/stage/source/next_action 和 Off/Summary/Trace
```

## 5. 回归与边界

完整 `cargo test -p editor_core --offline` 结果为：

```text
571 passed；2 ignored；1 failed
```

唯一失败为既有 260 重叠 owner：
`project_runtime_player_artifact_project_rust_preview_project_rust_export_share_identity`。
单独重跑仍因 child player `package_failed` 失败。Window A 未修改
`project_player_artifact`、runtime 或 renderer owner，因此本窗口没有越权修复该既有
dirty-worktree 回归，也不把完整 editor_core suite 声称为 passed。

Window A 不包含 cook、glyph collection、bundle v2、MSDF production recipe、
AutoHybrid runtime selection、AUI、GPU、hot reload 或 production evidence。
施工文档保持在 `当前/`；下一步只能由用户单独授权 Window B。
