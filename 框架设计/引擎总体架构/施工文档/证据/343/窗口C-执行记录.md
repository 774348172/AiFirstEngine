# 343 窗口C执行记录

2026-09-14 23:17开始，23:54终态记录（Asia/Shanghai，约37分钟）。状态：Gate C passed。71项去重相关测试、Windows窗口消费者编译检查、受影响Rust格式及diff检查通过。343仍占唯一当前施工槽，D–F未执行；338暂停未归档。没有安装Provider、修改宿主配置或运行双效果项目。

## 完成范围与责任边界

- RenderGraph → RHI → RealWgpuBackend增加typed模拟/绘制命令；同一encoder按提交顺序执行模拟、GPU实例准备/排序、间接绘制。直接消费B的持久records/scratch，没有逐粒子CPU模拟、ECS或Sprite。
- 2D billboard固定XY平面，3D billboard面向相机，均支持实际world-to-clip矩阵；透视夹具证明近大远小。local跟随当前源位置，world保留出生位置。
- 复用注册纹理及其采样器；flipbook按年龄取帧。网格是一次上传的真实非对称三角形数据，支持尺寸/绕Z旋转，复用现有MeshBuffer handle种类。
- Alpha/Additive与每发射器BackToFront GPU排序、透明粒子深度测试且不写深度；固定既有基本三角形提供遮挡。跨draw维持Graph顺序，不声称跨效果全局粒子排序。
- 拖尾持久环形历史在模拟子步更新，重复Present不记录伪历史；按时间淡出，槽位新生身份隔离。拖尾附属存活粒子，头粒子结束时一起结束，不包含脱离头粒子的独立尾迹生命周期。
- 大型记录、排序、indirect、history缓冲持久复用；测试核对重复帧buffer bytes不变。每次相机/排序阶段仍有小型uniform上传缓冲及draw绑定准备，不声称热路径零分配；大型负载性能在F测量，不把本轮毫秒级测试耗时当FPS。
- Offscreen与No-Editor Windows设备请求共享实际可用计算limits。粒子Shader创建可同步校验，常规模拟/绘制无GPU wait/readback；捕获只在显式测试中进行。非执行型headless后端明确unsupported。
- 资产引用到注册纹理/网格以及可选material的最终解析、SDK、Projection/session消费和退役属于D。当前可选material未绑定时明确返回`particle_render.material_binding_unresolved`，不会静默忽略。没有把网格夹具升级成全场景Mesh系统或通用材质系统。

## 实际失败与修复

### 编译夹具/枚举错误

首个测试编译出现：

```text
error[E0599]: no variant ... Mesh ... RenderResourceKind
wgpu_backend.rs:577
```

使用了不存在的枚举名，修正为已有`MeshBuffer`。没有增加同义资源种类。随后首个真实绘制测试通过，编译53.66s、测试0.68s。

### 槽位复用继承旧拖尾（真实红证据）

```powershell
cargo test -p engine_runtime --features real-wgpu --locked particle_render_recycled_child_slot_does_not_join_previous_trail -- --nocapture
```

编译11.70s、测试0.66s：

```text
a new child must not inherit the old child's trail even when its random seed repeats
left: 253
right: 0
test result: FAILED. 0 passed; 1 failed
```

父粒子重复碰撞，子发射器capacity=1、寿命1/64秒、四个模拟步；相同种子/serial不能作为新生身份，旧连线像素(73,57)错误残留。最小修复：在内部EngineRecord现有对齐空隙中增加每槽递增birth，history按birth区分新生；计算布局从align(particle+36,16)改为align(particle+40,16)，实际stride不变，作者schema/接口不变。原夹具原断言复验通过，编译10.26s、测试0.61s。

### 注册采样器未被保留（真实红证据）

```powershell
cargo test -p engine_runtime --features real-wgpu --locked particle_render_flipbook_and_texture_vertical_orientation -- --nocapture
```

编译8.91s、测试0.48s：

```text
[143, 112, 0, 255] != [255, 0, 0, 255]
test result: FAILED. 0 passed; 1 failed
```

四象限贴图使用nearestClamp，粒子路径却硬编码linear，边界像素(63,54)混色。唯一修复为复用已注册资源的Sampler和TextureView；同测试通过，编译15.44s、测试0.49s。最终截图红/绿/蓝/黄边界清楚，没有更改原纹理来规避。

开发中的8项、30项、31项以及上述窄复验不重复计入终态71项。过程存在失败并已修复，不声称首次全绿。

## 终态命令与结果

工作目录`G:/gameEngin/rust`；Windows dev/test，实际WGPU/Vulkan。除更新已存在png包的dev依赖锁文件时使用offline外，终态全部Cargo命令使用locked。png只用于测试截图，不进入Player正式依赖。

| 命令 | 结果 | 构建/检查时间 | 测试时间 |
|---|---|---|---|
| `cargo test -p engine_runtime --features real-wgpu --locked particle_ -- --nocapture` | 34 passed / 0 failed / 0 ignored | 11.11s | 2.67s |
| `cargo test -p project_authoring_execution --features real-wgpu --locked particle_effect -- --nocapture` | 15 unit + 1 integration passed / 0 failed | 2m55s | 3.69s + 3.12s |
| `cargo check -p runtime_player_winit --features real-window --locked` | passed | 30.67s | 不适用 |
| `cargo test -p engine_runtime --features real-wgpu --locked rhi_command_plan::tests:: -- --nocapture` | 9 passed | 0.25s | 0.05s |
| `cargo test -p engine_runtime --features real-wgpu --locked render_graph::tests:: -- --nocapture` | 8 passed | 0.19s | 0.03s |
| `cargo test -p engine_runtime --features real-wgpu --locked headless_rhi_backend::tests:: -- --nocapture` | 3 passed | 0.21s | 0.03s |
| `cargo test -p engine_runtime --features real-wgpu --locked real_wgpu_offscreen_backend_can_execute_basic_plan_when_adapter_exists -- --nocapture` | 1 passed，实际运行未skip | 0.18s | 0.68s |

Runtime的34项 = B GPU owner 12 + C render 12 + 作者合同9 + 既有粒子纹理producer1。新增内部模拟记录影响生成程序，因而重新验证Compiler的实际Cooked GPU consumer，而不是只复用B旧shader证据。总去重数34+16+9+8+3+1=71。

Compiler终态原始结果摘录：

```text
Finished test profile [unoptimized + debuginfo] target(s) in 2m 55s
running 15 tests
particle_effect_cook::tests::particle_effect_cook_gpu_consumer_executes_packaged_author_program ... ok
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 76 filtered out; finished in 3.69s
Running tests/game_project_compiler_interface.rs
particle_effect_check_and_prepare_share_snapshot_program_validation ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 21 filtered out; finished in 3.12s
```

以上为结果摘录，省略其它通过行，不冒称全量原始日志。Compiler的另一个integration binary过滤后0测试，不计入完成数。三个既有dead_code warning为IrInterpreter、validation_only_ir_interpreter、remove_sprite_renderer2d，未借本Gate清理无关代码。

格式命令：对本轮12个Rust文件执行`rustfmt --check --edition 2021 --config skip_children=true`；同范围与Cargo.lock/Cargo.toml执行`git diff --check`，退出0。Git提示LF将转CRLF，不是格式失败。未运行全workspace、Local CI、Android、Editor或release资格；这些不在本轮源码绘制声明内。

## 真实GPU、截图与视觉判定

设备日志：`NVIDIA GeForce RTX 3070; vendor=4318; device=9352; device_type=DiscreteGpu; driver=NVIDIA; driver_info=591.86; backend=Vulkan`。适配器获取失败会使新增render测试失败，不空返回成功。

运行Runtime终态集时设置`PARTICLE_RENDER_EVIDENCE_DIR=G:/gameEngin/框架设计/引擎总体架构/施工文档/证据/343/窗口C-图像`。128×128 RGBA8，夹具≤64容量、≤8步，像素比较容差≤2。22张实际GPU图像均已打开审查；它们是正式render_to_view的离屏捕获，不是OS窗口截图或最终美术作品。

| 测试/风险 | 实际图像与判定 |
|---|---|
| graph dispatch / indirect / duplicate present | [正面](窗口C-图像/billboard-front.png)中心红色；重复同step图像完全相同、age=0、buffer bytes不变 |
| 2D/3D朝向 | [3D侧视](窗口C-图像/billboard-side-3d.png)仍有可见面；[2D侧视](窗口C-图像/billboard-side-2d.png)XY面侧对相机而不可见，正是预期 |
| local/world、Y向上 | [local](窗口C-图像/local-moved-up-right.png)移到右上；[world](窗口C-图像/world-keeps-birth-position.png)留在出生位置 |
| 排序及真实反例 | [正确alpha](窗口C-图像/sorted-alpha.png)近红在上，中心约[128,0,64,255]；[反向相机排序](窗口C-图像/wrong-sort-direction.png)约[64,0,128,255]，同oracle拒绝；[additive](窗口C-图像/additive.png)约[128,0,128,255] |
| flipbook / sampler / UV | [帧1](窗口C-图像/flipbook-1.png)、[帧2](窗口C-图像/flipbook-2.png)、[帧3](窗口C-图像/flipbook-3.png)、[帧4](窗口C-图像/flipbook-4.png)依次红绿蓝黄；[四象限](窗口C-图像/texture-top-red-bottom-blue.png)上红绿、下蓝黄，nearest边界无混色 |
| 真实mesh | [原网格](窗口C-图像/mesh-asymmetric.png)非对称三角形；[旋转](窗口C-图像/mesh-rotated.png)方向改变，非billboard矩形替代 |
| 场景深度 | [遮挡](窗口C-图像/scene-depth-occluder.png)基本三角形遮住后方红粒子，外侧背景/粒子仍可见 |
| 拖尾历史和寿命 | [弯曲](窗口C-图像/trail-curved-history.png)沿真实历史形成向上弧线；[到期](窗口C-图像/trail-expired-tail.png)旧段消失，只留近端 |
| 重生隔离 | [复用槽位](窗口C-图像/recycled-child-no-old-trail.png)只保留新生小点，没有旧轨迹连线 |
| 透视 | [近大远小](窗口C-图像/perspective-near-large-far-small.png)近红约26像素、远蓝约9像素，深度缩放正确 |
| 既有UI叠加 | [UI后画](窗口C-图像/ui-after-particles.png)绿色半透明UI盖在红粒子上，中心约[127,128,0,255] |
| 同步与多view | [同一步双视图](窗口C-图像/two-views-one-simulation.png)左右各一红方块，中间黑色，未发生后一view uniform覆盖前一view或重复模拟 |
| 失败前置 | 缺纹理报texture_missing且live仍0；无效相机基向量拒绝；headless报unsupported。此项以状态/诊断断言留证，无伪造截图 |

## 源身份、复用与收尾自审

这是dirty worktree的源码/直接消费者证据，基准HEAD为`acaffa8cb95bbb54d093bbf45835a59315c82da5`，不是该提交发布资格。下列SHA-256固定终态产品、测试和相关输入身份；源码/生成合同变化只使消费它的证据失效。既有用户改动保留，不用reset/checkout覆盖。

| 文件（相对G:/gameEngin） | SHA-256 |
|---|---|
| `rust/Cargo.lock` | `378FFDC7F88D2B32C2FCFE15FB4A28BE5AA1D759114972056E1AE368C2161200` |
| `rust/crates/engine_runtime/Cargo.toml` | `D4A4D9D6617EA8F102EFB95726CA13EAD2E192BCB3F3515042CB9E6E37306BB9` |
| `rust/crates/engine_runtime/src/particle_render_contract.rs` | `2A94D5F9484104C0356678E2B7CD6F81ADFECE3C58C9507947CF90CD862C2C5C` |
| `rust/crates/engine_runtime/src/particle_render.rs` | `26560E2D4A7CD944158F0A0090FAA9949C06F83B150F183A093D291D4437014A` |
| `rust/crates/engine_runtime/src/particle_render/common.wgsl` | `F4F1A0CB7503398D7063C1EF52151590AC2A86C14BFB1C9B36DD65FEA2FCD44A` |
| `rust/crates/engine_runtime/src/particle_render/compute.wgsl` | `A82CCE962D4DDC3D71FC657B0E4D269E5B902969C83089C10435148F97DEDD3C` |
| `rust/crates/engine_runtime/src/particle_render/draw.wgsl` | `0F9C33EFBF64B20966A41D231DF5FB1BE9DDDCC325F8C284E5C6825A50532652` |
| `rust/crates/engine_runtime/src/particle_gpu.rs` | `1C237683DCC973DA36B5AFB64714A3C323DF4669FAE376308F41C19FAA83537D` |
| `rust/crates/engine_runtime/src/particle_effect/program.rs` | `CE2A0A4A84496E4522E02E845D24E6F7782413E9C0BA9A44238004F374D7153E` |
| `rust/crates/engine_runtime/src/particle_effect/simulation.wgsl` | `86C2FF43BF1058148EB53B182DDAC7D16E2C4DAC5A6C46AE1B673D0BE255C339` |
| `rust/crates/engine_runtime/src/wgpu_backend.rs` | `61D93F7CFA2077E8740FC1ED8F7CF37B3FADF60ED8C36CCF5DF0E78738AE3E85` |
| `rust/crates/engine_runtime/src/render_graph.rs` | `6F5B5479DDB83D5BC972B70AE941A3FA4E0C849B16A286E415B77EEB91FD923F` |
| `rust/crates/engine_runtime/src/rhi_command_plan.rs` | `CCD01D25158C3411A7257DDDEA97E70BF016749496C78649C6F1E3395B54BBC3` |
| `rust/crates/engine_runtime/src/engine_rhi.rs` | `D0CC06564D4E469853CF3E4F02CDBD21892A01C96533A88106CA0958AE04B67E` |
| `rust/crates/engine_runtime/src/headless_rhi_backend.rs` | `BF1FC04561D0062122BF1F1F593DF895C90C9AF5088D2D02E52F5965EFCDE9AA` |
| `rust/crates/engine_runtime/src/lib.rs` | `2F07CA53826A456103B25CADF88173C3958DCAF45DF0786E3F835DFA2BF24949` |
| `rust/crates/runtime_player_winit/src/lib.rs` | `8338B92CD4026E961C2B509650650216E75028E410C52BC163AE3A814F862218` |
| `rust/crates/engine_runtime/src/particle_render/tests.rs` | `73C6AB826A028C8CF5D500128349F186FFE93AC0E07A009CF37F7C5AB7A5A3C7` |
| `rust/crates/engine_runtime/src/particle_gpu/tests.rs` | `5811B19C7285961901C7BB978B66A02C83D9C0B733C07721F1DDCC82F535D1D4` |
| `rust/crates/engine_runtime/tests/fixtures/particle-effect.json` | `8CDC15EE85CC8E4B318243DFC3D01CCBDBD41CE871FC96F459D38037C388B97C` |
| `rust/crates/project_authoring_execution/src/particle_effect_cook.rs` | `FFE9F5169AD5EEF6A48EE42CE87CCE5378C524B3CC902C9ECB820F6D71B3382D` |

自审：本轮没有新AI工具、第二设备、CPU全量镜像、项目特效模块或新的Compiler。现有材料绑定未完成被明确留给D；固定遮挡夹具不被描述为完整3D游戏能力。B的模拟合同经受影响集复验；339/340/341/342既有独立能力证据保留，但不据此宣布它们与粒子双效果的最终交付已验收。

收尾检查没有遗留cargo/rustc/link进程。本轮没有需删除的独立临时目录；保留Cargo增量缓存和22张证据图像，未删除用户数据。没有生产二进制/配置事务需要回滚。

结论仅为Gate C源码与真实GPU绘制通过。下一依赖是Gate D SDK/Projection、资源绑定与生命周期；E的两例各两轮真实制作复看、F的规模与独立交付均仍为343完成的必需条件。
