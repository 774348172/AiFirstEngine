# 261 当前施工文档：Project Font Asset / FontBundle / AutoHybrid Rendering Foundation v1

## 0. 文档状态

```text
文档编号：261
正式方案：../../261-Project-Font-Asset-FontBundle-AutoHybrid-Rendering-Foundation-v1方案.md
方案结论：G2-C + AutoHybrid，方案自审通过，无需修改正式方案
施工文档状态：已完成；Window A-H、consumer closure、production staging 与 production 视觉矩阵全部通过
是否可施工：否；施工已完成并准备归档
是否已激活：否；完成闭环后释放当前施工槽
最近授权：执行 Window H-R2；2026-07-31 14:56 +08:00 开始，最晚不得超过 17:56 +08:00，17:41 后不得开启新阶段
本轮允许：普通项目公开字体资产 seam、production export/Player、真实 Windows DPI/连续缩放视觉证据、必要受影响回归与 Gate H 文档闭环
本轮禁止：Local CI、production/安装态二进制替换、新增引擎 DPI/scale qualification seam
代码施工：Window A-G、Window H 自动化 consumer closure、塔防项目字体、非塔防第二项目 v2 seam、legacy 专项矩阵与 H-R1 production staging 已完成
Gate：Gate A-H passed
完成记录：阶段完成记录/2026-07-30-Project-Font-Asset-FontBundle-AutoHybrid-Rendering-Foundation-v1/00-总览.md；00-Window-A.md；01-Window-B.md；02-Window-C.md；03-Window-D.md；04-Window-E.md；05-Window-F.md；06-Window-G.md；07-Window-H-blocked.md；08-Window-H-unblocked-by-260-fixture-remediation.md；09-Window-H-automated-consumer-closure.md；10-Window-H-project-font-second-project-legacy-matrix.md；11-Window-H-production-visual-export-blocked-by-project-rust-dependency-contract.md；12-Window-H-R1-production-staging-and-r10-visual.md；13-Window-H-R2-production-visual-matrix.md
```

## 0.1 2026-07-31 Window H-R1 收口状态

```text
ProjectRuntimePlayerArtifact production staging R1：完成
跨盘/任意 build root trusted Engine SDK 规范化：通过
受控 serde / serde_json locked offline：通过
SourcePatch 最小依赖策略：保持不变
production DesktopExport r10：通过
真实 Windows Player present/screenshot：通过
最终 editor_core / tower / FontBundleV2 / project_e2e_gate：通过
Local CI：未运行
production/安装态二进制替换：未执行
Gate H-R1：passed
261：等待 H-R2 后归档
H-R2 输入：production multi-DPI/MSDF 清晰度；controlled fallback 同屏视觉证据
完成记录：阶段完成记录/2026-07-30-Project-Font-Asset-FontBundle-AutoHybrid-Rendering-Foundation-v1/12-Window-H-R1-production-staging-and-r10-visual.md
```

## 0.2 2026-07-31 Window H-R2 授权与执行范围

```text
授权开始：2026-07-31 14:56 +08:00
最晚停止：2026-07-31 17:56 +08:00
停止开启新阶段：2026-07-31 17:41 +08:00
H-R2-A：第二个受控 FontFace + fallback + multi-page 同屏 production 证据
H-R2-B：同文 Bitmap/MSDF shared metrics、baseline、advance 与 alignment production 证据
H-R2-C：真实 Windows multi-DPI 或连续缩放 MSDF 清晰度 production 证据
禁止：Local CI；production/安装态二进制替换；新增引擎 DPI/scale qualification seam
失败规则：保留首个可定位 evidence，停在当前 H-R2 子项，不跳项伪称 Gate H 完成
执行结果：H-R2-A/B/C passed；Gate H passed；完成记录见 13-Window-H-R2-production-visual-matrix.md
```

本文已通过激活前复核并移动到 `施工文档/当前/`。Window A 已按原独立授权完成；
用户随后连续授权 Window B-H 按 Gate 顺序推进，前一 Gate 失败即停止。

本次激活复核确认：正式方案仍有效；当前槽没有另一份正式施工计划；脏工作树中的 260
改动不回退、不覆盖；Window A 限定在新字体 owner、`editor_core` 合同、依赖和 fixture，
不修改当前有重叠的 runtime/renderer owner。Window A source identity 为激活时工作树，
只能形成 dirty-worktree development evidence，不声称 exact-commit 或 release 资格。

## 1. 施工目标

按 261 正式方案建立项目无关的完整字体基础设施：

```text
Project FontFace / FontFamily / FontStack / FontAtlasProfile assets
  -> Asset Graph / ProjectFontCookModule
  -> deterministic glyph collection and raster
  -> CookedFontBundle v2
  -> RuntimeFontRegistry
  -> AUI glyph planning / UiProjection
  -> R8 bitmap and RGBA8 MSDF texture arrays
  -> RuntimeRenderer text pipelines
  -> Editor Preview / Play / Export parity
```

最终能力必须包括：

- 真实 TTF / OTF / TTC `faceIndex`；
- family、style、weight、fallback 和 kerning；
- Hinting R8 与 MSDF，不提供普通单通道 SDF；
- `<= 32 physical px` 默认 Hinting R8，`> 32 physical px`、连续缩放或粗描边使用 MSDF；
- bitmap/MSDF 各自 multi-page，并通过 texture array layer 消费；
- AUI 默认字体和 per-node stack/family/style/weight；
- 确定性 cook、结构化 diagnostics、Off/Summary/Trace report；
- cooked generation 原子热替换；
- Preview、Play、Export 使用相同 FontBundle digest；
- 真实 Windows/GPU 字体视觉证据。

## 2. 固定边界

### 2.1 本施工允许修改

只允许修改通用引擎字体资产、cook、RuntimePackage、AUI、UiProjection、renderer 和其
直接测试/fixture/文档 owner。塔防项目只能作为后置真实 consumer，不得拥有字体底层实现。

### 2.2 禁止扩入

本施工不得实现或夹带：

```text
完整 shaping / HarfBuzz
Bidi paragraph layout
IME / composition / caret
rich text markup / inline object
color emoji / SVG glyph
系统字体扫描
Runtime TTF/OTF/TTC 解析或动态 raster
普通单通道 SDF
项目专用字体 schema
塔防玩法、波次、英雄、海克斯或战斗逻辑
新的 Font Provider 公共抽象
独立字体 Bridge 或绕开 UiProjection 的 GPU 通路
```

未来 shaping 系统只能复用 `font_face_id + glyph_id + raster_variant`，不得要求推翻本轮
FontFace、Family、Stack、page 或 glyph identity。

### 2.3 塔防项目边界

`samples/tower_defense_project` 在本施工中是普通引擎用户：

- 只能导入有明确授权的字体 Asset；
- 只能通过公开 FontFace / Family / Stack / Profile、AUI 和 RuntimePackage seam 使用字体；
- 不得修改引擎 schema/cooker/runtime/renderer；
- Window H 之前不得用项目私有 Rust、系统字体或 debug overlay 伪造中文字体能力。

## 3. 当前基线与激活前冲突检查

生成本文时的已知基线：

```text
editor_core::aui_font_atlas_cooker
  固定 font-main / ui-default-cmin
  固定 5x7 / 8px ASCII
  非 ASCII 回退为 '?'

engine_runtime::runtime_package
  已有 RuntimeFontAtlasManifest / CookedFontAtlasAsset /
  CookedFontAtlasGlyph / RuntimeAuiFontAtlasRegistry

engine_runtime::aui
  已有 AuiTextGlyphQuad / AuiTextGlyphPlan
  当前只解析 default atlas，并使用 font_size / 8.0

runtime_player_winit
  当前只上传 default atlas 单纹理

ProjectRuntimePackageAssembler
  当前只有一个 C-min 字体 cook 调用点
```

当前工作树含大量未提交用户改动，且至少与下列 261 owner 重叠：

```text
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/engine_runtime/src/aui.rs 或其同域文件
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/editor_core 的 RuntimePackage / Play / Launcher 相关 owner
```

激活前必须重新运行：

```powershell
git status --short
git diff -- rust/crates/editor_core rust/crates/engine_runtime rust/crates/runtime_player_winit
rg -n "AuiFontAtlas|RuntimeFontAtlas|CookedFontAtlas|font-main|ui-default-cmin|font_size / 8" rust/crates
```

若重叠改动尚未形成可解释基线，不得覆盖、回退或整理用户改动；应保持本文在
`待执行/`，先确定如何与现有改动合并。任何窗口开始前都要记录本窗口实际消费的 source
identity；脏工作树开发测试只能作为开发反馈，不能冒充 exact-commit 或发布证据。

## 4. 依赖与真实字体资格前置

### 4.1 不预先锁死 backend

本文不把未经验证的 crate 写成既定实现。当前候选为：

```text
字体解析 / Hinting：优先资格验证 swash；必要时比较 freetype-rs 等候选
MSDF：优先资格验证 pinned msdfgen safe binding；纯 Rust候选必须通过同一 golden
现有 ab_glyph：只可作为基线比较，不能自动证明正式 Hinting 或高质量 MSDF
```

Gate A 必须先完成 backend qualification，才能冻结依赖版本、feature、许可证、构建方式和
recipe version。资格验证失败即停止，禁止用 5x7、`?`、普通 SDF 或伪 MSDF 继续后续 Gate。

### 4.2 backend qualification 必须证明

- Windows 当前 toolchain 可重复构建；
- 依赖及传递依赖许可证可接受并被记录；
- TTF、OTF、TTC `faceIndex` 均能解析；
- 小字号中文 Hinting R8 有真实像素输出；
- 中文、尖角、曲线 glyph 的 MSDF golden 可用；
- 固定输入、参数和版本连续两次生成 byte-identical；
- 运行时 crate 不依赖 parser、rasterizer 或 MSDF generator；
- native 依赖若需要编译工具链，必须在 Build/Export 支持矩阵内明确，不得隐式依赖开发机。

### 4.3 fixture 与许可证

所有进入仓库的真实字体 fixture 必须同时提交：

```text
字体文件或合法确定性子集
上游名称和版本
原始下载 URL
许可证文件
是否允许再分发/修改/子集化
原始 SHA-256
仓库内 fixture SHA-256
faceIndex 与覆盖字符说明
```

至少覆盖真实 TTF、OTF、TTC，中文、Latin、数字、标点、fallback、kerning 和缺字。未经许可
不得复制操作系统字体；不得用系统安装字体作为可重复 Gate 输入。

## 5. 预计涉及文件

以下是激活前必须重查的 owner 清单，不表示允许一次全部修改。

### 5.1 可能修改

```text
rust/Cargo.toml
rust/Cargo.lock
rust/crates/editor_core/Cargo.toml
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/aui_font_atlas_cooker.rs
rust/crates/editor_core/src/project_runtime_package_assembler.rs
rust/crates/engine_runtime/Cargo.toml
rust/crates/engine_runtime/src/lib.rs
rust/crates/engine_runtime/src/aui.rs
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/runtime_player_winit/Cargo.toml
rust/crates/runtime_player_winit/src/lib.rs
相关 crate 内现有 tests / fixtures / shader owner
```

### 5.2 可能新建

实际文件名可按当前模块分层微调，但不得另造并行运行时：

```text
rust/crates/editor_core/src/project_font_assets.rs
rust/crates/editor_core/src/project_font_cook.rs
rust/crates/editor_core/src/project_font_cook/*
rust/crates/engine_runtime/src/runtime_font_registry.rs
rust/crates/engine_runtime/src/aui/font_planning.rs
rust/crates/runtime_player_winit/src/font_gpu.rs
rust/crates/runtime_player_winit/src/shaders/aui_text_bitmap.wgsl
rust/crates/runtime_player_winit/src/shaders/aui_text_msdf.wgsl
rust/fixtures/fonts/*
rust/fixtures/fonts/LICENSES/*
受影响 crate 的定向测试文件
```

若现有 owner 已有更合适的深模块，优先深化它；禁止为了文件名清单机械创建浅包装。
`ProjectRuntimePackageAssembler` 只能学习一个 `ProjectFontCookModule::cook` seam。

## 6. consumer closure

共享合同发生变化时，至少核对以下真实消费者：

| Producer / 合同 | 必须闭合的 consumer |
|---|---|
| FontFace / Family / Stack / Profile | Asset DB、Asset Graph、Patch/Import、Assembler、Editor diagnostics |
| CookedFontBundle v2 | RuntimePackage builder、loader、digest、Preview、Play、Export |
| glyph/metrics/kerning identity | AUI layout、glyph plan、UiProjection、runtime report |
| page format/layer/render mode | RuntimeFontRegistry、GPU upload、pipeline、batching、present |
| generation | editor rebuild/hot reload、runtime frame boundary、GPU resource retirement |
| legacy adapter | Complex Shooter、Switch Puzzle、第二项目 Gate |

不得只测 producer 后宣称共享合同完成。上层 suite 已覆盖的同配置定向测试不机械重复。

## 7. 授权与窗口规则

261 拆成八个连续窗口 A-H。每个窗口：

- 都需要用户单独、明确授权；
- 授权前必须确认前一窗口完成记录和当前基线；
- 都受三小时连续施工上限约束；
- 不能把一个窗口的授权解释为后续窗口授权；
- 2 小时 45 分后不得开启新实现或验证阶段；
- 预计无法在剩余时间完成证据与 cleanup 时，不得启动该阶段；
- 当前 Gate 失败时先保留首个可定位证据并修复当前 Gate，不得跨 Gate 堆积；
- Window A-G 完成只能称为阶段完成，Window H 闭环前不得称 261 完成。

每个窗口激活时，应把本文中的对应窗口复制/收敛为本次唯一执行范围，并在状态区记录授权
时间、deadline、source identity 和允许的外部状态。

## 8. Window A / Gate A：backend qualification、schema、Asset Graph、diagnostics

### 红测

先增加能分别捕获下列失败的定向测试：

```text
非法 FontFace schema / 路径越界 / source hash mismatch
TTF/OTF/TTC faceIndex 正负例
family exact/nearest weight resolve
stack fallback 顺序与 cycle
缺失 defaultUi stack
candidate backend 小中文 Hinting R8 为空或退化为 '?'
candidate MSDF 中文/尖角 golden 不合格
同输入双生成 bytes 不一致
```

### 施工任务

1. 用 repo-owned fixture 做候选 parser/Hinting/MSDF 资格 spike。
2. 记录版本、features、许可证、Windows build、确定性和失败结论。
3. 只有资格通过后才冻结依赖与 recipe version。
4. 建立 versioned FontFace、FontFamily、FontStack、FontAtlasProfile schema 与稳定 ID。
5. 接入 AssetRef、Asset DB、Asset Graph dependency 和结构化 diagnostics。
6. 定义 Off/Summary/Trace；正式 runtime 默认 Off，不产生逐 glyph 文件。

### 建议命令

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core font_backend_qualification -- --nocapture
cargo test -p editor_core project_font_asset -- --nocapture
cargo test -p editor_core font_family_resolution -- --nocapture
cargo test -p editor_core font_stack -- --nocapture
cargo test -p editor_core font_diagnostic -- --nocapture
```

### 完成条件

- backend 资格报告通过，依赖与字体 fixture 许可证齐全；
- schema canonical serialization 和稳定 digest 已测试；
- Asset Graph 能精确列出 source、family、stack、profile dependency；
- cycle、缺失引用、非法 faceIndex 等失败包含 `domain/stage/source/next_action`；
- 没有 runtime parser/raster/MSDF dependency；
- Gate A 完成记录生成后才允许申请 Window B。

### 停止条件

任一 backend 无法证明真实中文质量、Windows 构建、许可或 byte-identical 时停止。若所有候选
均失败，回到 261 正式方案讨论依赖策略，不得继续伪造后续合同。

## 9. Window B / Gate B：ProjectFontCookModule、glyph collection、Hinting R8

### 红测

```text
RuntimePackage 可达 AUI 文本未进入 glyph set
动态文本未通过注册 Project Text Source 进入 glyph set
required glyph 缺失却生成成功产物
中文被替换为 '?' 后仍 passed
Hinting R8 空白、尺寸/stride 不合法或 metrics 不稳定
输入文本/字体/profile/recipe 变化未改变 dependency digest
```

### 施工任务

1. 建立唯一深 `ProjectFontCookModule::cook` Interface。
2. 收集 RuntimePackage 可达 AUI、localization、显式 range、注册动态文本源。
3. 解析 FontStack 并把 codepoint 稳定映射到 `font_face_id + glyph_id`。
4. 生成 Hinting R8 variants、标准化 metrics、基础 kerning。
5. 同一 source face 和 glyph variant 在一次 cook 内复用。
6. 固定排序、量化和错误语义；不在 runtime frame 中 raster。

### 建议命令

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_font_cook -- --nocapture
cargo test -p editor_core project_font_glyph_collection -- --nocapture
cargo test -p editor_core project_font_hinting_r8 -- --nocapture
cargo test -p editor_core project_runtime_package_assembler -- --nocapture
```

### 完成条件

- 中文、Latin、数字、标点真实 glyph 均被收集并生成非空 R8；
- required glyph 缺失 fail closed；
- Assembler 只调用一个字体 cook seam；
- 两次 cook 的中间 canonical 输出和 digest 相同；
- Window B 不声称已完成 bundle loader、MSDF 或 GPU present。

## 10. Window C / Gate C：CookedFontBundle v2、multi-page、Builder/Loader、legacy

### 红测

```text
page format/size/byte_len/hash 不一致
重复 glyph key / 非有限 metrics
单页不可容纳 / page budget overflow
metadata/page/glyph 顺序不确定
legacy v1 被错误标记为 quality_gate_eligible
Runtime loader 接受半缺失 bundle
```

### 施工任务

1. 实现 CookedFontBundle v2 canonical metadata 和 page payload。
2. Bitmap/MSDF page 空间分别建模，先完成 Bitmap multi-page。
3. Builder/Loader 校验 schema、hash、byte_len、page budget、glyph uniqueness。
4. digest 覆盖 metadata、所有 page bytes 和稳定顺序。
5. 保留 208 C-min 只读 legacy adapter；新正式资产只写 v2。
6. legacy 必须报告 `legacy_mode=true`、`fallback_used=true`、
   `quality_gate_eligible=false`。

### 建议命令

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime cooked_font_bundle -- --nocapture
cargo test -p engine_runtime runtime_font_bundle_loader -- --nocapture
cargo test -p editor_core project_font_bundle_builder -- --nocapture
cargo test -p editor_core project_font_bundle_determinism -- --nocapture
```

### 完成条件

- bitmap multi-page、budget 和全部负例通过；
- metadata/page payload 连续双 cook byte-identical；
- loader 拒绝损坏与半 bundle；
- legacy 项目继续加载，但不能通过 261 Font Quality Gate。

## 11. Window D / Gate D：MSDF recipe 与 AutoHybrid

### 红测

```text
<=32 physical px 未选择 Hinting R8
>32 physical px 未选择 MSDF
bitmap scale 超出 [0.875, 1.125] 仍选择 bitmap
连续缩放或粗描边未选择 MSDF
DPI/node scale 未按 physical px 计算
Bitmap/MSDF 切换改变 layout metrics 或换行
MSDF 双生成 bytes 不一致
普通单通道 SDF 出现在正式输出
```

### 施工任务

1. 用 Gate A 已通过的 backend 实现确定性 MSDF。
2. 固定 edge coloring、em size、pixel range、量化、并行 merge 和 pack 顺序。
3. 为相同 glyph identity 生成共享 normalized metrics 与不同 raster variant。
4. 实现 AutoHybrid 选择并输出可追踪 reason。
5. 完成 MSDF multi-page、page budget 和 deterministic digest。

### 建议命令

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_font_msdf -- --nocapture
cargo test -p editor_core project_font_auto_hybrid -- --nocapture
cargo test -p editor_core project_font_variant_metrics -- --nocapture
cargo test -p editor_core project_font_bundle_determinism -- --nocapture
```

### 完成条件

- 阈值、缩放、描边、DPI 决策与正式方案一致；
- bitmap/MSDF metrics、baseline、advance 和换行一致；
- MSDF 中文与尖角 golden 通过，双 cook bytes 一致；
- 正式 schema、bundle 和 shader 输入中不存在普通 SDF。

## 12. Window E / Gate E：AUI 字体样式、RuntimeFontRegistry、glyph planning

### 红测

```text
default stack / per-node override 不生效
style/weight exact 或 nearest face 选择错误
fallback face 命中不可见或无 report
kerning 未进入 glyph positions
中文 rendered != requested 或 missing != 0
glyph plan 仍硬编码 default atlas 或 font_size / 8.0
```

### 施工任务

1. 扩展 AUI text style 的 stack/family/style/weight 与必要 raster policy。
2. RuntimeFontRegistry 原子加载完整 FontBundle generation。
3. 从 resolved run 解析 face、glyph id、variant、metrics、kerning、page/layer。
4. AUI layout/presenter 只拥有字体语义与 glyph positions，不持有 GPU handle。
5. Runtime Summary 只保留 bundle、rendered/missing/fallback/mode count 和 compact diagnostic；
   Trace 才包含 resolve、variant、page/layer。

### 建议命令

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime runtime_font_registry -- --nocapture
cargo test -p engine_runtime aui_font_style -- --nocapture
cargo test -p engine_runtime aui_text_glyph_plan -- --nocapture
cargo test -p engine_runtime aui_text_kerning -- --nocapture
```

### 完成条件

- default/per-node、weight/style、fallback、kerning 全部有定向证据；
- 中文 glyph plan 不再回退 `?`；
- bitmap/MSDF variant 切换不改变布局；
- 无 GPU handle 进入 AUI 或项目逻辑。

## 13. Window F / Gate F：UiProjection、texture arrays、双 text pipeline

### 红测

```text
R8 / RGBA8 page 格式上传错误
array layer 与 glyph page 不一致
multi-page 文本缺字或改变 draw order
Bitmap 与 MSDF pipeline 选择错误
fallback font 导致不必要的每 page draw split
shader 空白输出或 MSDF median/smoothing 错误
```

### 施工任务

1. UiProjection 生成 render-facing glyph item：render mode、array layer、UV、range、color。
2. RuntimeRenderer 分别建立 R8Unorm 和 Rgba8Unorm texture2DArray。
3. 建立 bitmap coverage 与 MSDF reconstruction 两条 pipeline。
4. 相同 render mode/page array 合批；只因 bitmap/MSDF 必要切换拆 batch。
5. renderer 只消费 UiProjection 结果，不解析 AUI binding、字体 family 或项目语义。
6. 增加非空 canvas/pixel 或离屏 GPU 定向证据，真实窗口证据留到 Window H。

### 建议命令

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime ui_projection_font -- --nocapture
cargo test -p engine_runtime font_texture_array --features real-wgpu -- --nocapture
cargo test -p engine_runtime aui_text_bitmap_pipeline --features real-wgpu -- --nocapture
cargo test -p engine_runtime aui_text_msdf_pipeline --features real-wgpu -- --nocapture
cargo test -p runtime_player_winit font_texture_array --features real-window --no-run
```

### 完成条件

- R8/RGBA8 arrays、layer、batch 和双 pipeline 定向测试通过；
- 多页 fallback 同屏不缺字、不改变 UI 顺序；
- GPU 输出非空且能区分 bitmap/MSDF；
- 没有新增独立字体 Bridge。

## 14. Window G / Gate G：generation 热更新与 Preview/Play/Export parity

### 红测

```text
新增 codepoint 未使受影响 profile 失效
仅文本变化且 glyph 已存在却强制 recook
新 generation 未完整 prepare 就替换旧 generation
metadata 与 page payload 半替换
frame 内切换或仍有 in-flight 引用时释放旧 GPU generation
Preview/Play/Export FontBundle digest 不一致
```

### 施工任务

1. dependency digest 只使受影响 FontBundle 失效。
2. 新 generation 完整 cook/load/validate/GPU prepare 后，在 frame boundary 原子替换。
3. 替换失败保持旧 generation active，并关联新 generation diagnostic。
4. 旧 GPU generation 在无 in-flight frame 引用后释放。
5. Editor Preview、Play、Export 统一走 RuntimePackage cooked 结果。
6. 验证 runtime 不扫描项目源目录、系统字体或解析源字体。

### 建议命令

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_font_hot_reload -- --nocapture
cargo test -p engine_runtime runtime_font_generation -- --nocapture
cargo test -p runtime_player_winit font_generation_gpu_retirement --features real-wgpu -- --nocapture
cargo test -p project_e2e_gate font_preview_play_export_parity -- --nocapture
```

### 完成条件

- 原子切换、失败保旧、延迟释放全部通过；
- 已有 glyph 文本变化复用 bundle，新增 glyph 精确失效；
- Preview/Play/Export digest 完全一致；
- runtime 依赖闭包不含 parser/raster/MSDF generator。

## 15. Window H / Gate H：negative matrix、真实视觉、第二项目、整体闭环

### H-R1 ProjectRuntimePlayerArtifact production staging 补救

正式方案：

```text
261-R1-ProjectRuntimePlayerArtifact-Production-Staging-v1方案.md
```

施工范围：

1. 保持 `controlled_source_patch.rs` SourcePatch v1 policy 和 diagnostics 不变。
2. 新增独立 production staging owner，解析 ProjectRust manifest/Cargo contract。
3. 保留参与 AOT digest 的源 Cargo.toml 字节；Engine SDK dependency 通过 staging-only
   `RuntimeModuleBuild/Cargo.toml` 规范化为本次 trusted SDK canonical path。Host 只依赖
   规范化构建包，artifact root 不得参与依赖能否解析的判断。
4. v1 受控第三方只允许 `serde` / `serde_json`，从 trusted SDK `Cargo.lock` 选择满足
   requirement 的唯一 crates.io identity，staging 改为 exact version。
5. 拒绝 git、任意 path、build/dev/target dependency、build script、Cargo config、
   workspace inheritance 和 source tree link/reparse escape。
6. staging copy 覆盖项目 regular source closure，并排除 Build/target/VCS/config/.aife preview 产物；
   保留 source-compatible manifest 与已有 project lock，缺 lock 时才用 trusted lock seed，
   同时写入 normalized RuntimeModuleBuild manifest。
7. artifact report/cache identity 增加 normalized manifest、dependency 和 trusted lock digest。
8. 源项目 Cargo manifest/lock 必须 byte-exact 不变。

owner 红测：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_runtime_player_production_staging -- --nocapture
```

受影响回归：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core controlled_source_patch -- --nocapture
cargo test -p editor_core
cargo test -p project_e2e_gate

Set-Location G:\gameEngin\samples\tower_defense_project\RuntimeModule
cargo test

Set-Location G:\gameEngin\samples\tower_defense_project\Tests\FontBundleV2
cargo test
```

R1 完成条件：

- production owner fixture 使用 trusted engine path、serde derive、serde_json 成功构建；
- normalized manifest/dependency/lock digest 可审计；
- source Cargo manifest/lock byte-exact 不变；
- SourcePatch v1 仍拒绝第三方/path 等超出最小合同的输入；
- `editor_core` 与实际受影响 consumer 全绿；
- 不运行 Local CI，不替换 production/安装态二进制。

R1 通过后建立新的 frozen dirty-worktree candidate，再恢复本节原 production
DesktopExportPipeline 和真实视觉矩阵；R1 失败则停在 R1，不消耗真实 GPU screenshot。

### H0 前置

Window A-G 均有完成记录；当前 source、fixture、shader、binary、profile 和 GPU identity 已冻结。
真实 Windows/GPU screenshot 是发布/激活级证据，不作为前面窗口的日常调试循环。

### 自动化矩阵

至少执行：

```powershell
Set-Location G:\gameEngin\rust
cargo fmt --check
cargo test -p editor_core
cargo test -p engine_runtime
cargo test -p runtime_player_winit --features real-wgpu
cargo test -p project_e2e_gate
cargo test -p editor_window_winit
```

实际 consumer closure 以激活时 Cargo dependency graph 和改动面为准；若上层 suite 已在相同
输入/feature/profile 下包含定向测试，不重复执行。失败时保留首个 assertion、stdout/stderr、
fixture、profile 和 trace，不用完整 workspace/Local CI 作为修复循环。

### 真实视觉矩阵

必须使用 production composition 和合法真实字体，至少保存：

```text
小字号中文 Hinting R8 screenshot
大字号中文 MSDF screenshot
同文 Bitmap/MSDF metrics overlay 对齐证据
连续缩放或多 DPI 下的 MSDF 清晰证据
multi-page + fallback 同屏证据
真实 exported Windows Player 证据
```

每份证据记录 source commit、font fixture hash、FontBundle digest、binary hash、Windows/DPI/GPU、
build profile、render mode 和截图像素尺寸。debug overlay、测试专用旁路和系统字体均不合格。

### 第二项目与兼容

- 塔防项目以普通项目资产方式显示真实中文，并验证 `rendered == requested`、`missing == 0`；
- 至少一个非塔防第二项目通过相同公开 seam；
- Complex Shooter / Switch Puzzle legacy adapter 继续可加载；
- legacy 项目必须保留迁移提示且不能通过 261 Font Quality Gate；
- 塔防语义不得出现在引擎 API、schema、fixture 或 report。

### Local CI / clean exact commit

Local CI、clean exact-commit qualification、production binary replacement 或安装态验证都不在
前述窗口授权中。只有最终声明确实需要对应证据且用户另行授权时才执行；不得默认自动运行。
一旦执行，必须绑定 exact source、harness、fixture、binary、configuration identity，并遵守
三小时、fail-fast、独占外部状态和安全 cleanup 规则。

### Gate H 完成条件

- negative matrix、受影响 consumer closure 和真实视觉矩阵全部通过；
- 第二项目、legacy、塔防普通用户路径全部通过；
- Off/Summary/Trace 没有把逐 glyph Trace 放入正式 runtime 热路径；
- 真实 production evidence 可追溯到 frozen source/bundle/binary；
- 没有未解释的 skipped、fallback、missing glyph 或 retained external state；
- 生成 261 阶段完成记录并完成入口同步后，才允许标记 261 施工完成。

## 16. 结构化 diagnostics 与报告验收

所有窗口应复用统一字段：

```text
code
domain=font
stage=asset_resolve|parse|glyph_resolve|raster|pack|package|load|gpu_prepare|present
font_face_id
font_family_id
font_stack_id
font_atlas_profile_id
codepoint
glyph_id
render_mode
source_asset_ref
message
next_action
```

正式 `font-face-asset.v2` 不允许静默回退系统字体、`?`、丢 glyph、丢 page 或伪称 MSDF。
Runtime Off 不写字体 report 文件；Summary 轻量；Trace 仅用于测试、Gate、debug 或用户显式诊断。

## 17. 施工完成后的文档闭环

仅在 Window A-H、受影响回归和所需 production evidence 全部通过后：

1. 新建 `阶段完成记录/2026-xx-xx-Project-Font-Asset-FontBundle-AutoHybrid-Rendering-Foundation-v1/00-总览.md`。
2. 记录每个窗口 source identity、测试、失败/修复、复用证据、外部状态、耗时和遗漏项。
3. 更新 `49-当前状态与下一步入口.md`。
4. 更新 `54-当前可自动化施工总入口.md`。
5. 更新 `施工文档/README.md` 和 `阶段完成记录/README.md`。
6. 更新 `00-文档地图.md`、`00-阅读顺序.md`，必要时更新 `240` 的可验证施工状态。
7. 把本文从 `施工文档/当前/` 移到 `施工文档/已完成/`，并改名为完成态名称。
8. 清空施工授权；不得自动开始塔防 P0-5 项目施工。

如实现发现正式方案错误，停止扩大实现，先修改 261 正式方案并重新自审本文。

## 18. 施工文档自审

### 18.1 正式方案一致性

```text
G2-C 完整字体基础设施：覆盖
FontFace / Family / Stack / Profile：覆盖
CookedFontBundle v2 / multi-page：覆盖
Hinting R8 + MSDF AutoHybrid：覆盖
32 physical px 与缩放规则：覆盖
普通单通道 SDF 永久不提供：覆盖
fallback / style / weight / kerning / AUI selection：覆盖
texture arrays / 双 pipeline：覆盖
deterministic cook / diagnostics / hot reload generation：覆盖
Preview / Play / Export / real visual evidence：覆盖
shaping/Bidi/IME/rich text/color emoji 边界：覆盖
```

结论：读取正式方案后，无需修改正式方案。

### 18.2 范围自审

```text
是否夹带塔防玩法：否
是否让塔防项目修改引擎：否
是否新增系统字体扫描或 runtime raster：否
是否新增普通 SDF：否
是否新增独立 Bridge / Provider 公共 seam：否
是否把真实窗口当早期调试循环：否
是否默认要求 Local CI：否
```

### 18.3 可执行性与授权自审

```text
每个窗口有红测、任务、命令、完成和停止条件：是
backend qualification 位于第一道 Gate 且失败即停止：是
真实字体 fixture 与许可证要求明确：是
共享合同 consumer closure 明确：是
legacy compatibility 明确：是
Off / Summary / Trace 明确：是
八个窗口要求分别授权：是
三小时上限与 2h45m 停止开新阶段：是
是否把 Window A-G 描述为 261 完成：否
当前是否可施工：否；Window A-H 已完成
当前是否已激活：否；完成闭环后归档到 已完成/
是否运行过 Gate：是，Gate A-H passed
是否修改过引擎代码：是，Window A-G 的字体资产/cook/bundle/registry/AUI/renderer/generation owner，以及 Window H 的 typed fontSource importer、RuntimePackage 排除和 legacy migration hint owner
```

### 18.4 自审结论

```text
施工文档自审：通过
文档位置：施工文档/已完成/
当前状态：已完成；Window A-H、consumer closure、H-R1 staging 与 H-R2 production 视觉矩阵全部通过
最近授权时段：2026-07-31 14:56 +08:00 开始，最晚不得超过 17:56 +08:00
停止开启新阶段：2026-07-31 17:41 +08:00
完成记录：阶段完成记录/2026-07-30-Project-Font-Asset-FontBundle-AutoHybrid-Rendering-Foundation-v1/00-总览.md；00-Window-A.md；01-Window-B.md；02-Window-C.md；03-Window-D.md；04-Window-E.md；05-Window-F.md；06-Window-G.md；07-Window-H-blocked.md；08-Window-H-unblocked-by-260-fixture-remediation.md；09-Window-H-automated-consumer-closure.md；10-Window-H-project-font-second-project-legacy-matrix.md；11-Window-H-production-visual-export-blocked-by-project-rust-dependency-contract.md；12-Window-H-R1-production-staging-and-r10-visual.md；13-Window-H-R2-production-visual-matrix.md
后续边界：Local CI 未运行；production/安装态二进制未替换；不自动开始塔防 P0-5
归档：已完成
```
