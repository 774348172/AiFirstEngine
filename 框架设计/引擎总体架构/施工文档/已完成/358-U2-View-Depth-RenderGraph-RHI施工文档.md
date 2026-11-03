# 358-U2 View / Depth / RenderGraph / RHI 施工文档

日期：2026-09-22。依据：[358正式方案](../../358-2D3D-Unified-Render-Contract-and-Asset-Binding-v1方案.md)、[U0合同](../../358-U0-render-contract-v1合同冻结附录.md)、[U1合同](../../358-U1-native-render-assets与命名绑定合同附录.md)。用户“开始下一步施工”已授权推进U2。生成时状态：待执行、不可施工；激活前必须复核空槽、U1完成、源码及命令。本轮授权不涉及安装。当前状态：已完成归档；A–D终态见第7节。

## 1. 目标与边界

让U1真实几何/材质通过类型化Camera、逐视图可见性、DrawGeometry、现有RenderGraph/RHI进入Headless与真实WGPU。通过离屏像素证明相机、索引与深度确实影响画面；不是只增加协议字段。

保持U0 legacy矩阵/数值/排序和U1资产身份；AUI继续独立composition，Sprite、Particle、Font默认行为保持。新增接缝接收Projection/Prepare产品，不扫描项目源文件、不另建场景状态或资源resolver。源场景Camera/Mesh组件、generated cube/plane、glTF、完整3D游戏闭环留给后续方案A。PBR、阴影、Physics3D、Navigation3D、SDK/ABI/tool新增、默认安装更新和发布资格均不在本轮；Tower P1-2仍stopped_by_user。

当前基线：U1已归档，154项测试与real-wgpu编译通过，默认安装357不含U0/U1。脏工作区必须保留，实际增量以 `.artifacts/358-U2/20260922-1923/baseline.json` 及源码副本区分，不用HEAD替代实际源码。

## 2. 最小owner与实现约束

| Owner | 本轮修改 |
| --- | --- |
| `render_contract/view.rs`、新camera helper | typed orthographic/perspective + 显式position/unit quaternion相机姿态；列主序、右手-Y-up-camera负Z、clip depth 0..1；legacy不猜投影参数 |
| 新 `render_geometry.rs` 及子模块 | 从U1 Prepared资产转换immutable共享buffer与typed DrawGeometry；输入instance world matrix；逐view层/可见性/8角AABB保守视锥剔除；opaque/transparent稳定排序；Off不生成逐instance trace |
| `render_graph.rs`、`rhi_command_plan.rs`、`engine_rhi.rs` | 显式view begin、depth attachment、typed geometry命令，验证target/viewport/format/extent/view identity；复用既有graph与trait |
| `runtime_renderer.rs` | 新显式消费入口，复用现有graph与report装配；Core3D在Particle及ScreenOverlay/Modal之前，UI不参与世界depth排序 |
| `headless_rhi_backend.rs`、`wgpu_backend.rs`及子模块 | Headless确定性意图/绑定摘要；WGPU真实vertex/index、矩阵uniform、depth、unlit/named texture、完整pipeline key；与已有粒子命令按序组合 |
| `tests/unified_render_execution_u2.rs` | 合同负例、逐view、graph/RHI、真实GPU像素和旧命令兼容证据 |

旧DrawSprite/DrawMesh保留显式兼容converter；有真实顶点/资源才转换，缺数据拒绝，不能把旧测试三角形包装为真实Mesh。默认legacy命令不强制切换。新geometry CPU数据使用共享不可变产品；WGPU缓冲复用按资源身份/内容，不把material recipe seed当完整pipeline key。深度按view/target创建并按extent失效，比较LessEqual、清1；透明默认依材质TestOnly或Disabled，不擅改作者策略。

## 3. 必要性与验证经济性审查

- 请求声明：U2源码执行合同与Windows真实WGPU离屏验收。确认缺口：typed view仍ContractOnly；普通graph depth槽未执行；MeshBasic仍测试顶点；U1 Prepared不是GPU上传。
- 最小因果改动：上述现有consumer和同域helper。无需第二套Scene/Asset/Renderer。完整3D产品入口不借本轮自动扩大。
- 复用U1 Cook/Package/Load/Prepare已通过证据，其owner/夹具未修改时不重跑Cook全链；U0 legacy消费因graph/RHI改变需定向回归。分别记录产品源码、测试、feature/binary环境；文档修改不使源码证据失效。

| Gate | 风险/失败合同 | 最便宜可变红测试 | 证据与估时 |
| --- | --- | --- | --- |
| A | 相机方向/near-far/pose错误、非法矩阵、跨view可见性混用 | U2 camera/culling定向测试及负例 | gate-a日志；25分钟 |
| B | 假几何、错slot/stale句柄、depth/target身份丢失、排序及UI污染 | U2 converter/graph/headless测试，U0兼容 | gate-b日志；35分钟 |
| C | GPU只编译未执行、depth/viewport/texture/cull错误、旧consumer破坏 | U2 real-wgpu必需adapter的像素断言；原owner定向测试 | gate-c日志含adapter；50分钟 |
| D | 报告范围与实际源码/安装混淆 | 变更文件format、baseline增量/源hash、文档链接 | summary/source manifest；15分钟 |

预计125分钟，预留30分钟。窗口始于2026-09-22 19:23 +08；连续3小时上限、22:08后不启动实现/验证。单探索15分钟无信号回到owner，编译20分钟未结束先检查环境；普通失败保存首因并在授权内修复。真实GPU不可用则保留阻塞事实，不用跳过测试冒充通过，不因此新建重型harness或安装。

## 4. Gate任务与命令

所有Cargo命令在 `G:/gameEngin/rust`、现有target、Windows test profile、`--locked`。

A：实现相机与逐view判定，`cargo test -p engine_runtime --locked --test unified_render_execution_u2 camera`。再以culling过滤验证逐view和诊断等级。

B：实现真实资产converter及graph/RHI消费，`cargo test -p engine_runtime --locked --test unified_render_execution_u2`；`cargo test -p engine_runtime --locked --test unified_render_contract_u0`。B终态包含A则不重跑A。

C：`cargo test -p engine_runtime --locked --features real-wgpu --test unified_render_execution_u2 gpu -- --nocapture`，必须实际获取adapter并断言像素。旧owner选择 `render_graph::tests`、`rhi_command_plan::tests`、`headless_rhi_backend::tests`、`runtime_renderer::tests`、`wgpu_backend::tests`，各以 `cargo test -p engine_runtime --locked --lib <filter>` 执行。新路径与粒子组合时追加受影响粒子owner定向测试；GPU legacy/font测试只运行受本次pass分派影响的已有用例，不扩到全workspace。若同feature终态suite已经包含前置用例，计为一次证据。

D：对实际修改Rust文件执行 `rustfmt --edition 2021 --check --config skip_children=true <files>`，保存最终hash/命令终态与首轮失败，核对未改Cook/SDK/安装。写U2实际合同与阶段记录、同步49/54/文档索引，施工文档移入已完成。

## 5. 方案复审与施工自审

已核对358第5/7/9/10节、U0/U1冻结合同和实际RenderGraph/EngineRhi/RealWgpuBackend源码。无需修改正式方案路线。相机姿态从显式Projection输入派生，不能为typed视图暗中假定原点；旧矩阵不反推FOV。343已有真实粒子mesh及depth，复用同backend并保持其执行顺序，不能声称引擎此前没有GPU几何。

自审修正：headless只证明意图；GPU资格必须有像素断言且无adapter不能静默return。DrawGeometry必须带真实index/vertex，pipeline加入layout/附件/策略，U1资产摘要不能直接充当可执行key。逐view诊断沿Off/Summary/Trace，避免逐帧JSON。兼容converter对没有数据的legacy mesh失败，默认路径保持。未扩大为方案A、发布或安装。自审通过，待激活前复核。

## 6. 执行记录

2026-09-22激活前复核通过：54空槽，当前目录仅254历史证据，U1已归档，源码baseline已保存，命令使用现有Cargo target。用户授权有效。现状态：已激活，开始Gate A；完成前不归档。



## 7. 终态与施工自审

2026-09-22 A–D完成，状态 `u2_source_view_depth_rhi_and_offscreen_wgpu_passed`。121项去重相关测试通过，含18项U2（6项实际GPU像素）、19项U0、RenderGraph/RHI/Headless/Renderer/EngineRhi/WGPU/Particle consumer，以及额外单项执行的原ignored AUI主题像素测试。对应范围的旧owner测试统一在real-wgpu配置执行以复用同一个lib test binary；U0另保留default编译路径，不机械再跑同组default owner测试。

首轮编译错误与夹具错误保存在gate-a-first/gate-c-composition-first日志；已按实际合同修复。自审补齐相机实体/version及几何/材质各自GUID、cooked digest和source path，共享资产provenance避免逐帧深拷贝。补充18项U2复验通过，未变legacy consumer复用前一通过证据。未产生新的外部配置、安装、SDK/ABI或项目组件变动。

scoped格式、空白、源身份/范围和文档链接核对通过。完整证据、源码与harness身份、失败/复用、时间和资格边界见[完成记录](../../阶段完成记录/2026-09-22-358-U2-View-Depth-RHI/00-总览.md)及[实际合同](../../358-U2-View-Depth-DrawGeometry实际合同.md)。默认安装357不含U0/U1/U2；下一步原方案A未激活。文档移入已完成并清空54唯一槽位。
