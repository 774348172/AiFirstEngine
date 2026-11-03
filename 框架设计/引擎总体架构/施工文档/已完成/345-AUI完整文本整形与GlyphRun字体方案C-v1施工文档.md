# 345 AUI 完整文本整形与 GlyphRun 字体方案 C v1 施工文档

状态：已完成并归档（2026-09-16）。Gate A–D 闭环，source_and_windows_dev_delivery_passed；Windows dev 1280×720 通过，最大化/1600×1000 按 Gate C 约定 deferred。344 保持暂停。下方过程记录按其历史时点理解。

正式依据：[345 方案 C](../../345-AUI完整文本整形与GlyphRun字体方案C-v1方案.md)。前置字体基础：[261 FontBundle](../../261-Project-Font-Asset-FontBundle-AutoHybrid-Rendering-Foundation-v1方案.md)、[302 MSDF/AutoHybrid](../../302-AUI-FontBundle-MSDF-Orientation-Physical-Pixel-AutoHybrid-B-min-v1方案.md)、[340 字体保真修复](../../340-AUI字体渲染保真修复-v1方案.md)。

## 1. 目标与边界

requested claim：AUI 文本经过完整 shaping 与 GlyphRun/LineLayout 后，中文、英文、数字、标点和 fallback 在 14–64px、普通/暂停/失败/通关状态下共享正确 baseline、advance、cluster 和裁切规则，Renderer 只消费最终 quad。

minimum causal change：新增项目无关 text shaping owner、GlyphRun/LineLayout 内部合同，并迁移 `rust/crates/engine_runtime/src/aui.rs` 的逐字符排版调用链；保留现有 FontBundle cook、RuntimePackage、AUI binding 和 atlas。

explicitly excluded：玩法、Scene、Project Rule、Core ECS、Renderer 采样器重构、Editor FontSystem 迁移、富文本/emoji 彩色字体、输入法、动态下载、全平台 release 矩阵。

## 2. 依赖与施工前复核

1. 用户已暂停 344 并激活 345，读取 49/54；不得恢复 344 或并行修改其项目美术。
2. 确认 HarfBuzz binding、FreeType/字体解析依赖、许可证、版本和平台构建矩阵；记录 cargo lock 与 provider 身份。
3. 对现有 FontBundle schema、字体 digest、AUI glyph plan、RuntimePackage 兼容性做影响分析；不直接升级旧 schema。
4. 保存 engine_runtime、project_authoring_execution、AUI 测试和项目字体项目的作者基线。
5. 若依赖无法在当前 Windows dev/provider 构建，停止 Gate A，不生成兼容性假通过。

### Gate A 实施记录（2026-09-15）

已锁定 `rustybuzz 0.20.1`，Cargo.lock 已更新；新增 `rust/crates/engine_runtime/src/text_shaping.rs`，输出项目无关的 `GlyphRun`（face、glyph、cluster、offset、advance、direction、text digest）。固定文本 `中文，句号。引号“测试” 01:23 Agpy` 重复 shaping、cluster 单调、advance 非负测试通过；`cargo check -p engine_runtime` 通过。Gate A 已闭合。

### Gate B 激活复核（2026-09-15，历史阻塞，已解除）

Gate B 暂停，未修改 AUI 排字调用链。复核发现 RuntimePackage 的 `RuntimePackageSourceFontBundle` 只包含 `CookedFontBundleAsset` 与 atlas page payload，没有 FontFace 原始字节或预生成 shaping 输入；`RuntimeFontBundleRegistry` 也只能按 codepoint resolve 已 cook glyph。当前运行时无法把动态 AUI 字符串交给 rustybuzz 做真正 shaping。直接把逐字符循环改名为 GlyphRun 会产生假迁移，因此保留 Gate A 产物并记录阻塞。

进入 Gate B 前必须在方案层选择一个明确输入：A）RuntimePackage 携带受 digest 保护的 FontFace bytes；或 B）Build 阶段为静态文本生成可消费的 shaped-run 资产，并为动态文本保留 FontFace bytes。该选择会影响 RuntimePackage schema、font cook、构建身份与跨项目回归，需先补回正式 345 方案再施工。

### 方案 A 已选定后的数据合同施工（2026-09-16）

已将 `RuntimePackageSourceFontBundle.font_face_sources` 与 `RuntimePackageSourceFontFace` 加入 Runtime 字体模型，字段包含 `font_face_id`、`face_index`、`source_digest`、`bytes`；loader 在接收 bundle 时重新计算 SHA-256 并拒绝 digest 不一致。ProjectFontCookOutput 现在保留 cook 输入的 source bytes，bundle builder 将其写入 FontFace source 列表；现有内置/测试夹具显式使用空列表保持兼容。`cargo check -p engine_runtime -p project_authoring_execution` 通过。

### Gate B AUI shaping 接入记录（2026-09-16）

已在 `aui.rs` 接入 `text_shaping::shape_text`：优先从 default FontBundle 的已校验 `font_face_sources` 读取字体 bytes，生成 GlyphRun，并按 shaped glyph id 选择对应 atlas glyph；无字体源的旧 bundle 明确保留逐字符兼容路径。当前仍处于迁移初段：offset/advance 尚未完全替换 LineLayout，复杂 cluster 与多 face fallback 尚未闭合，故不宣称 Gate B 完成。`cargo fmt`、`cargo check -p engine_runtime` 与 4 项 AUI glyph plan 测试通过。

### Gate B 继续施工记录（2026-09-16）

已接入 shaped run 的 `offset_x / offset_y / advance_x`：有 FontFace source 时跳过旧 kerning，quad 使用 shaping offset，LineLayout 位置负责绝对 x，兼容 bundle 才使用旧 cursor；AUI glyph plan 4 项测试通过；项目 `font-shaping-gateb-final` check/build/playtest/observe/verify 全部通过，三房 1020 tick／18断言保持通过。该轮证据只证明兼容路径游戏可运行；2026-09-16 复核撤回“核心迁移完成”。磁盘源字节、cluster/fallback、真实字体度量仍属于 Gate B 必须完成项。

## 3. Gate 与测试

### Gate A：Shaper owner

新增独立 `text_shaping` owner，输出确定性的 `GlyphRun`：face、glyph、cluster、offset、advance、direction、fallback。固定文本 `中文，句号。引号“测试” 01:23 Agpy` 在 CJK/Latin/标点混排下重复运行，验证 digest、cluster 单调、advance 非负和 fallback 诊断。

最小测试：provider 单元测试、依赖许可证/版本报告、同输入两次 digest 一致。失败时保留完整诊断并停止，不进入 AUI 迁移。

### Gate B：AUI LineLayout

将 `aui.rs` 的逐字符 resolve/kerning/cursor 逻辑替换为 Shaper → GlyphRun → LineLayout → GlyphQuadPlan。统一 font units 到 logical UI pixels 的换算，显式处理 ascender、descender、line_gap、baseline、line width、clipping 和按钮垂直居中。禁止叠加旧 kerning。

最小测试：中文/数字/英文/标点 baseline；fallback run；14/18/24/32/48/64px；Bitmap/MSDF；按钮文字居中；旧 AUI binding 值不变。Renderer 不新增排版计算。

### Gate C：atlas 与真实窗口

验证 atlas padding、UV 内缩、Bitmap/MSDF 采样和大字号字缘；真实 Windows dev 截图覆盖普通、断电、暂停、失败、通关。记录缺字、fallback、clipped glyph、串 atlas 和 glyph plan digest。

精确窗口尺寸按现有入口实际记录，不能用重采样图代替 1600×1000；若入口不可用，标记 deferred，不修改系统或引擎绕过。

2026-09-16 实测收敛：新包六字号双模式样板复现 MSDF 笔画交接细缝，原始 atlas 的 RGB median 也存在内部边缘，已排除 padding/串页和单纯窗口采样问题。`fdsm 0.8.0` 的 error correction 明确不支持 overlap；最小修复限定于 cook 前的 NonZero 轮廓合并，再走既有 MSDF 生成、RuntimePackage、Renderer。若需要布尔轮廓库，只作为 authoring cook 内部依赖，不改 schema/运行时 API/Shader。先用重叠笔画 owner 红测证明，再验证孔洞、曲线精度、真实字体与新窗口；不重跑无关 owner。另将原项目房间标题框适配真实行度量，保留旧位置和字号，不恢复 344 美术施工。

### Gate D：消费方与交付

运行 engine_runtime 字体/AUI/RuntimePackage 回归、项目激光小屋和第二字体项目消费者；独立 Windows dev 交付并核对 source、dependency、font、package、binary 身份。旧项目不能消费 GlyphRun 时必须通过显式兼容适配器，不静默改变旧视觉合同。

## 4. 验证经济性、复用与失效

Gate A 只验证 shaping owner，不启动真实窗口；Gate B 才验证 AUI consumer；Gate C 才消费 atlas/Renderer；Gate D 才验证跨项目交付。复用 261/302/340 中未被 shaping 依赖、schema 或 aui.rs 修改消费的证据。依赖版本、GlyphRun schema、AUI 排版代码、FontBundle schema 或 Renderer UV 变化只使相应消费者证据失效。

## 5. 失败与回滚

- HarfBuzz 构建或许可证失败：保留旧适配器，仅记录 Gate A blocked。
- shaping advance 与旧 kerning 冲突：GlyphRun advance 为唯一来源，禁止双重 kerning。
- fallback 基线漂移：LineLayout 统一行指标并输出 face offset 诊断。
- atlas 串字：回滚 QuadPlan consumer，仅修 atlas/采样 owner。
- RuntimePackage 不兼容：版本门禁拒绝加载，不降级成错误字形。

## 6. 文档同步与激活条件

本文件已按用户授权位于 `施工文档/当前/`；344 保持暂停。历史待执行条件不再约束本轮已授权施工。

完成 Gate D 后生成阶段完成记录，更新 49/54/施工文档 README/阶段记录 README；若只完成部分 Gate，保留当前文档和失败证据，不宣称方案 C 完成。

## 7. 历史待执行自审（2026-09-15）

- 用户已明确选择方案 C，正式方案已生成；本施工文档未反向扩大到 Editor、富文本或全平台。
- 依赖、shaping、layout、atlas、跨项目交付分别有 owner、失败合同和最小测试。
- 现有 344 占用当前槽，故本文明确“不可施工”，不伪造已激活或已开始。
- 施工只允许使用项目与引擎现有构建入口；不新增永久 Runner，不修改安装/宿主配置。
- 自审结论：文档完整，可在前置槽释放后激活。

## Gate B 闭环复核（2026-09-16）

最小因果变更：FontBundle digest 绑定 face 清单并导出/读取字节；cook 保留真实 face ID/index；AUI 遍历 shaped glyph、统一真实行指标。不得把 cluster/fallback 推给 Gate C。
验证：RuntimePackage 磁盘往返与损坏拒绝；shaper 连字/组合字符/fallback；AUI 六字号两栅格模式与按钮居中。源码集成证据不宣称已安装或真实窗口通过；Gate C/D 保留。预算 2 小时，2 小时 45 分钟停止新工作并保全证据。

## Gate B 终态（2026-09-16）

`source_integration_passed`。FontFace 字节真实入包和校验、GlyphRun 驱动 AUI、完整 cluster/fallback、真实行度量与 MSDF plane bounds、选定 bundle 纹理身份均已接通。127 项 runtime + 19 项 authoring 回归通过，真实 cook→磁盘→reload→AUI 覆盖六字号两模式；真实窗口 feature 编译通过。最后的 face 隔离夹具强化单独通过，不重复计数。

见[Gate B 闭环证据](../证据/345/2026-09-16-GateB闭环.md)。撤回此前仅靠兼容路径游戏测试认定核心完成的判断。旧 built-in/历史包保留显式兼容适配，不作为新路径证据。Gate C/D、真实截图和安装态不在本次完成声明内。

本轮自审：只落实已选方案 A/C 的 Gate B；为已复现的 ID、空源、atlas coverage、MSDF 边界问题修改 cook/package/布局/投影消费方，没有重开字体方案。owner lint 通过；含依赖 lint 的 SDK 既有裸指针错误按证据中的范围豁免保留，未修改无关 SDK 或降低规则。


## Gate C 启动记录（2026-09-16）

已启动 Gate C，完成 `runtime_player_winit --features real-window --locked` 新构建。现有 Gate B 历史交付 `engine-op-344-font-shaping-gateb-final-3` 的 `font-bundle-manifest.json` 不含 `fontFaces`，因此不能冒充 Gate C 新链路窗口证据；它只能作为旧兼容路径。下一步必须通过 345 当前项目导出入口重新生成带 FontFace payload 的 Windows dev 包，再做真实窗口截图。若导出包仍没有 `fontFaces` 或窗口不消费新 bundle，则 Gate C 立即判阻塞并保留首因。


## Gate C 终态（2026-09-16）

`passed_windows_dev_1280x720`。修复 MSDF overlap 内部边缘（cook NonZero 轮廓合并）及旧房间标题框过矮；27 项字体 owner 回归通过。六字号 Bitmap/MSDF 真实样板及普通/断电/暂停/失败/通关窗口通过，五状态缺字/裁切均 0、font fallback false；原版主路线 18 与失败恢复 6 条断言通过。Face/Page digest、padding、UV 和 bundle/page 身份检查通过，修复前后 Bitmap 字节保持。

最大化/精确 1600×1000 因现有入口不可用，按本 Gate 约定 deferred；不能外推为多尺寸全通过。见[Gate C 闭环证据](../证据/345/2026-09-16-GateC闭环.md)。Gate D 待执行，345 不归档，344 保持暂停；Provider/宿主配置未改。临时辅助程序源码已移入证据目录，所有本轮窗口进程已退出。


## Gate D 激活与验证经济性（2026-09-16）

用户已授权 Gate D。开始时间 15:35，预算约 1 小时；2 小时 45 分停止新工作，3 小时结束。逐文件核对 Gate C 源码与交付摘要，复用未变的 Gate B 127 项 runtime 和 Gate C 27 项 authoring 证据。新增验证只针对第二字体消费者和仓库外完整交付，不重跑未变的内部回归。使用仓库已有 font_bundle_v2_second_project 的隔离副本，补最小可运行 SDK 会话；原夹具字体/AUI保持，用新导出确认受校验 FontFace 真正被消费。游戏复用 Gate C 149 文件原样交付副本，执行既有 verify-exported-player 和原版三房间/失败恢复场景；保留字体/包/二进制身份与子进程释放证据。不改安装或宿主配置，不恢复 344。

## Gate D 终态与归档（2026-09-16）

跨项目消费与仓库外独立交付通过。复核源码身份后复用 Gate B 127 runtime / Gate C 27 authoring 测试；原样游戏 149 文件、第二字体最终包 27 文件均 hash 一致，原版 24 条玩法断言通过。第二字体真实窗口 2 字形，无缺字/裁切/fallback。占位模块格式及 handler 缺失已在隔离副本修复；首因保留，不改引擎或原夹具。所有本轮子进程已退出。

见[Gate D 闭环证据](../证据/345/2026-09-16-GateD闭环.md)及[整体完成记录](../../阶段完成记录/2026-09-16-345-AUI-GlyphRun-Font-Scheme-C-v1/00-总览.md)。同步 49/54、导航和完成记录后移入已完成；344 不自动恢复，Provider/宿主配置未改。
