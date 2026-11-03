# 302 AUI FontBundle MSDF Orientation + Physical-Pixel AutoHybrid B-min v1 极简施工文档

## 状态

```text
正式方案：../../302-AUI-FontBundle-MSDF-Orientation-Physical-Pixel-AutoHybrid-B-min-v1方案.md
用户授权：2026-08-19，生成极度简化施工文档并直接施工
文档状态：Gate A-C 已完成，待归档
施工范围：Gate A-C 一次性完成
禁止：Local CI、production/安装态替换、真实配置修改、Tower Preview cache 重建、完整 E2E
```

## 目标与文件

只修三个已确认首因：

```text
Gate A：editor_core ProjectFontCook MSDF row orientation
Gate B：engine_runtime AUI glyph plan 使用 shared target presentation scale
Gate C：Tower 使用真实 Regular 400，FPS 文案改为 `FPS: N`
```

预计 owner 文件：

```text
rust/crates/editor_core/src/project_font_cook.rs
rust/crates/engine_runtime/src/aui.rs
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/runtime_player_winit/src/lib.rs
samples/tower_defense_project/Assets/Fonts/**
samples/tower_defense_project/AUI/generate-p1-0-battle-hud.cjs
samples/tower_defense_project/AUI/battle-hud.aui.json
samples/tower_defense_project/RuntimeModule/src/{playable_session.rs,ui_projection.rs}
samples/tower_defense_project/Tests/FontBundleV2/**
```

不新增 schema、crate、runner、cache、report family 或 compatibility layer。

## 必要性与验证经济审计

```text
请求/故障：FPS MSDF 上下倒置；720 target 中文模糊；Tower weight 400 请求绑定 weight 100 raster。
最小因果改动：MSDF RGBA rows 翻转一次；glyph planner 用 target physical px；Tower 换 static Regular 400。
排除：variable-font axis、sampler、shaping、glyph shard、Renderer/AUI rewrite、production transaction。

Gate A -> cook 坐标方向错误 -> 不对称 glyph owner test -> MSDF 正序且 Bitmap/metrics 不变。
Gate B -> logical/physical px 混用 -> 720/1080 glyph-plan test -> 24/36px 选择符合 261。
Gate C -> 项目字体身份错误/FPS 文案 -> Tower generator + FontBundle consumer -> weight/hash/text 一致。

复用证据：2026-08-19 Player 截图、MSDF/Bitmap atlas crop、261 physical-px 合同、271 Regular 400 pack。
失效条件：对应 cook、glyph planner、Tower font asset/generator 任一输入变化。
成本估计：实现与过滤测试 30-60 分钟；单个编译/测试分支超过 10 分钟且未产生 verdict 时停止复核。
```

## Gate A：MSDF orientation

1. 在 MSDF glyph variant 序列化前统一为 top-left / rows-down；Runtime UV 不加特例。
2. 增加 `F` 等不对称 glyph 的方向红测试，同时保留 deterministic/nonempty 断言。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_font_msdf -- --nocapture
```

## Gate B：physical-pixel AutoHybrid

1. 给 FontBundle glyph planner 传入由既有 GameView target/presentation 派生的 canvas scale；默认旧调用 scale=1。
2. AutoHybrid 按 physical px 和最近 bitmap bucket 选择；layout 仍按 logical metrics。
3. Editor/Player 直接 consumer 使用同一 target scale，不改 input/render presentation 数学。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime aui_text_glyph_plan -- --nocapture
```

## Gate C：Tower consumer

1. Tower font source 改为真实 static Noto Sans SC Regular 400，metadata/hash/face/family 一致。
2. generator 和 RuntimeModule 使用 `FPS: --` / `FPS: N`；生成 AUI，不手改派生 JSON。
3. 移除“多字符必须避开 MSDF”的历史规避断言，改为正常 Bitmap/MSDF 能力断言。

```powershell
Set-Location G:\gameEngin\samples\tower_defense_project
node AUI/generate-p1-0-battle-hud.cjs
cargo test --manifest-path RuntimeModule/Cargo.toml fps_sampler -- --nocapture
cargo test --manifest-path Tests/FontBundleV2/Cargo.toml -- --nocapture
```

终检只运行受影响格式/过滤测试；不运行 Local CI、完整 workspace、production 或真实 cache 流程。

## 完成与归档

Gate A-C 通过后：

```text
生成 阶段完成记录/2026-08-19-AUI-FontBundle-MSDF-Orientation-Physical-Pixel-AutoHybrid-B-min-v1/00-总览.md
同步 00 / 49 / 54 / 施工文档 README / 阶段完成记录 README
本施工文档移动到 已完成/
明确声明 source available，不冒充 production Editor/Player 已更新
```

## 施工结果

```text
Gate A：完成。MSDF cooked rows 统一为 top-left / rows-down；不对称 A owner test passed。
Gate B：完成。Editor/Player AUI glyph plan 消费 shared presentation scale；720/1080 AutoHybrid owner tests passed。
Gate C：完成。Tower 改用 static Noto Sans SC Regular 400，production font stack 不再消费 qualification face，FPS 统一为 `FPS: N`。
附带最小闭环：无轮廓空格在 MSDF 模式下复用同 family Bitmap glyph 的 advance metrics，不产生缺字 fallback。

最终定向验证：
- editor_core MSDF orientation owner：passed
- engine_runtime AUI glyph-plan owner：2 passed
- engine_runtime whitespace metrics owner：1 passed
- editor_core + runtime_player_winit real-window consumer check：passed
- Tower P1-0 validator / Battle Report validator / RuntimeModule fps_sampler：passed
- Tower FontBundleV2 consumer：1 passed（95.48s）
- Tower generator drift / JSON parse：passed

未执行：Local CI、完整 E2E/视觉矩阵、production/安装态替换、真实配置修改、Tower Preview cache 重建。
终态：source available；production Editor/Player 与现有导出包尚未包含 302。
```

## 自审

```text
与 302 正式方案一致：是
引擎/项目 ownership 分离：是
能捕获三个首因：是
存在过量 Gate/重复验证：否
修改外部状态：否
激活前施工槽：54 声明为空；当前目录的 254 preflight 是历史报告，不是活动施工文档，保留不动
激活条件：用户已明确授权，源码基线仍含三个已确认缺陷
自审结论：通过；2026-08-19 激活前复核通过，已激活施工
```
