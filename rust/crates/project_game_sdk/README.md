# Project Game SDK

公开项目接口与注册例子见 `src/lib.rs`。项目规则使用 `WorldRead` 与 deferred mutations，不直接持有引擎世界。

## 运行时生成内容与项目字节（376，开发源码）

定义按需增加 `.generated_content()` / `.project_storage()`，并注册 FixedUpdate、AUI 等能提交 mutation 的 session 回调；只有 Rule 的定义会在编译检查期拒绝。
回调返回 `Applied` 只表示该回调提交；用 `world.query_generated_content` 查询 request/ticket/target，
等待真实 `Applied`、读取 `applied_revision` 后再提交下一修订。`Accepted`/`Preparing` 不能当已显示；
`commit_epoch`、新模拟步、首次实际呈现是不同事件。没有 native consumer 的宿主明确返回 CapabilityGap。

四个可编译短例在 `examples/generated_content.rs`：完整替换、固定布局位置更新、同次几何与静态碰撞、
字节保存。运行 `cargo run -p project_game_sdk --example generated_content --locked`。
例子只检验 SDK 数据形状；实际 consumer 位于 Compiler 的 `generated_content_376_` 定向集成测试。

新建生成 renderer 默认没有项目层；相机的空 `layers` 选择已有的全层语义，或由项目预先配置目标 renderer 的层。引擎不代填 `world` 等项目层名。

几何/碰撞数组由 typed builder 复制到有界二进制附件；项目可以立即释放原数组。属性更新不能改布局、
索引或数量，越过旧包围盒的位置更新会重算边界。碰撞只支持静态目标，`Keep` 保留其修订，
独立 `replace_collision3d` 不撤网格。不要让项目线程保存 host context；线程只生成项目数据，
在合法回调提交，Session 的 Drop 必须 cancel/join 后返回，随后宿主才可卸载项目 DLL。

Player 使用已校验的 projectId、宿主 profile 和 `%LOCALAPPDATA%/OriginEngine/ProjectData`；版本/hash
不改变 namespace。嵌入宿主可提供独立绝对测试根，项目 API 只接受相对逻辑 key 和 opaque bytes。
读取完成后 `take_project_bytes(ticket)` 一次领取；不用的结果通过 deferred `abandon_project_bytes` 放弃。
短 transport buffer 不消费读取结果。写回执区分 NotCommitted、Committed、Unknown；Unknown 时项目读回核对，
不能自动重放。格式、迁移、多 key 恢复、体素与玩法都由项目负责。当前磁盘资格限本机 Windows/NTFS。

错误保留 `code/field_path/request_id/expected/actual/next_action`。例如 RevisionConflict 指向
`expectedAppliedRevision`：重新查询 target 的 applied/pending 修订，以实际已应用修订生成新的 requestId；
BudgetExceeded 指向 `queue`：等待在途工作、领取/放弃旧读取后重试新请求；CapabilityGap 指向
`storage.binding` 或 native owner：检查可信宿主装配，不能靠更换 key 或猜测 handle 修复。

## 首次接入（355）

最小定义是 `ProjectGameDefinition::new(Game::create)`。只注册需要的回调；不需要每个项目写空
FixedUpdate。注册项生成capability与ABI入口，合法缺省session回调由Runtime跳过。
如果UI确实需要项目binding值，则仍须注册对应ui_state producer；缺值不会被自动补成假数据。

新项目推荐 `.rule_typed("gameplay", Game::gameplay)`，handler接收 `&mut RuleContext<'_, '_>`。
`context.world()`读取实体；`context.mutations()`只允许Rule阶段的transform/component/prefab/despawn。
音频、动画、粒子与退出放在FixedUpdate/AUI/lifecycle回调，它们继续使用GameCallContext。
不要把媒体操作写在持续状态分支中每帧重播；让Rule记录项目意图，再由适当session回调处理。
调度顺序可能使意图到下一次session回调才提交，项目须明确时机；SDK不自动跨帧搬运命令。
回调返回Applied才提交；NoOp、Rejected、Faulted不提交。直接构造底层output仍受Runtime阶段检查。

旧 `.rule(...)`、handler签名和已有项目保持兼容；`rules()`只列旧注册，`rule_ids()`列所有注册。
推荐新增逻辑逐步使用typed入口，不要求一次迁移整个项目。

| 意图 | 输入资产trigger推荐写法 | 旧名称（含义不变） | 收到action后的查询 |
|---|---|---|---|
| 一次按下，如冲刺 | `JustPressed` | `Down` | `is_just_pressed()` |
| 按住持续，如移动 | `WhilePressed` | `Pressed` | `is_pressed()` |
| 松开一次 | `JustReleased` | `Released` | `is_just_released()` |

这些是JSON读入别名，序列化仍输出旧名称。Rust引擎侧构造对应
`InputTriggerPreset::{just_pressed,while_pressed,just_released}()`。
`InputAction::input_phase()`返回已知InputPhase；未知或无phase为None。axis输入读取axis1/axis2。
这些方法只解释收到的action，不是全局状态查询：Down配置没有held事件，辅助方法不能补造held。

运行可执行例子（包含断言）：

```powershell
cd G:/gameEngin/rust
cargo run -p project_game_sdk --example sdk_usability --locked
```

例子覆盖最小注册、typed分发、按住移动/按下冲刺、合法音频动画意图顺序；它不声称实际播放设备发声。
阶段误用的compile_fail用例随`cargo test -p project_game_sdk --locked`运行。

## 按钮一次绑定（355）

在按钮的AUI `action_refs`中使用 `{"event":"Activate","action_id":"game.start"}`，
即可消费鼠标Click或焦点按钮的Submit（仍须满足现有可交互/导航条件）。
项目AUI callback按action_id处理业务，输出event为`Activate`。
同一按钮/同一action_id若同时配置对应的显式Click或Submit，则显式绑定优先，本次command只触发一次；
不同action_id可并存，独立输入操作不合并。旧Click/Submit含义保持。
Rust资产构造端使用`AuiActionRef::activate(...)`。新事件/别名需包含355的工具链，旧安装不能直接消费。

本页与SDK符号元数据同源发布到既有开发知识入口；源码完成不代表默认安装已更新。

`kinematic2d::slide_aabb(body, displacement, solids)` 提供纯数据的轴对齐矩形运动辅助：输入世界坐标中心、半尺寸与本次位移，输出受阻后的中心、受阻轴及脚下 solid 索引。先处理 X，再处理 Y；调用方负责时间、重力、平台位移、触发器和世界写回。

它要求初始位置无穿透，不处理斜坡、旋转矩形、穿透恢复或刚体冲量。参考 `samples/laser_house_project/RuntimeModule/src/game.rs` 的平台承载与跳离；这些游戏规则不属于 SDK。

`context.mutations().audio_source(&entity)` 返回 `AudioSourceWriter`，控制实体上的 `engine.audio_source`。组件在 Scene/Prefab 中配置音频 `clipRef` 与 `[0,1]` 音量；命令不接受文件路径或临时换音源。

```rust
context.mutations().audio_source(&entity).play();
context.mutations().audio_source(&entity).stop();
context.mutations().audio_source(&entity).set_paused(true);
context.mutations().audio_source(&entity).set_paused(false);
```

在 FixedUpdate/AUI session 回调中提交，回调必须返回 `HandlerStatus::Applied`；NoOp、拒绝、失败和准备阶段都不播放。`play()` 从头重播该 Source 的唯一播放实例，保留暂停标志；`stop()` 释放播放实例而不解除暂停；`set_paused` 暂停/继续当前进度，恢复不会自行播放。不同 Source 可同时发声。

实体禁用、组件移除、销毁或 Scene 卸载时，Runtime 自动停止并释放声音；重新启用保持停止。项目负责游戏暂停、重开等业务事件，只在事件发生时提交命令，不按持续状态每帧重播。342 首版为 Windows 非空间短音效（PCM16 WAV，单/双声道，44.1/48 kHz）；设备输出失败会进入运行诊断，headless 控制验收不证明实际发声。


使用 `.lifecycle(Game::lifecycle)` 注册可选 `LifecycleRequest` 回调，事件为
`LifecycleEvent::FocusChanged { focused }`。宿主在玩法和 AUI 动作前按顺序分派，
零 fixed tick、失焦或暂停时仍可处理；项目决定暂停策略，重新获得焦点不会替项目恢复游戏。
生命周期回调同样可提交媒体和其他受控 mutation，必须返回 Applied 才会生效。

正常结束当前游戏会话时，调用 `context.request_exit()` 并返回 `HandlerStatus::Applied`。
该方法写入 typed exit 意图；引擎校验并成功提交后由宿主清理。
项目侧 `SessionOutput` 保留原有字段，生成胶水负责内部控制输出；NoOp 不会退出。
Player 关闭游戏窗口，Editor 只结束 Play，Headless 返回会话终态。
不要从项目调用进程退出或用 observe 布尔字段驱动宿主。新合同需要匹配的生成胶水和运行时。


## 3D 项目控制（359-A3.1）

从 `context.world().resolve_entity("camera")` 或 `query` 获取真实 `EntityHandle` 并按对象生命周期缓存；不要用 `from_stable_identity` 构造3D句柄。generation是不透明的运行身份，不是可自行递增的计数。销毁重生后重新解析，旧句柄在真正提交点拒绝。

```rust
let camera = context.world().resolve_entity("camera")?;
let mut transform = context.world().read_transform3d(&camera)?;
transform.translation[0] += 0.25;
context.mutations().write_local_transform3d(&camera, transform)?;
context.mutations().select_main_camera(&camera)?;
```

Transform3D 使用米、右手/Y向上、相机朝-Z、四元数xyzw；旧 `write_transform` 仍为旧2D/legacy合同。显隐用 `set_mesh_visible`；材质用 `set_material(handle,"surface",SpatialAssetRef)`，槽必须已存在，id/GUID/sub_asset必须指向同一个RuntimePackage条目，不能传文件路径。

空间修改在 AUI / FixedUpdate session 回调返回 Applied 后整批验证并提交；非法旋转、奇异变换、错误槽/资产、无效相机、过期句柄或通用写reserved组件导致该批零写入。RuleSlot不接受空间命令。资产绑定、实例化和销毁需要正常前进的运行帧携带RuntimePackage；没有该上下文的失焦/暂停专用分派拒绝这些命令。

`instantiate_spatial_prefab(request_id,prefab_ref)` 返回 `SpawnTicket`，不是实体。结构意图使用宿主可配置容量（371默认每tick共1024，多个批次共享），ID非零、批内唯一，新批次的ID必须大于本session上次成功spawn批次的最大ID。下一次session回调调用 `world.resolve_spawn(ticket)` 得到实际根实体句柄；提交前、旧批次、已销毁对象返回None。新成功spawn批次替换旧结果，因此及时保存解析到的handle。新相机Prefab不会夺取主视图，必须显式select。`despawn_spatial_prefab(root)` 通过原实例owner释放整实例与资源；禁止同一回调同时销毁和修改该实例，拆成两个回调。旧名称式despawn不能绕过3D句柄检查。

运行 `cargo test -p project_game_sdk --locked --example three_d_control`。它验证SDK命令形状；真实World/帧提交由Runtime测试证明，普通项目真实DLL动作和画面观察由A3.2验收。项目示例见 `samples/three_d_lab_project/RuntimeModule`，要求匹配的新SDK、生成胶水与Host；源码施工不更新默认安装357。

359-A3.2：`three_d_control`示例的SDK命令与普通3D项目共享接口；自动验收应使用引擎派生的`engine.spatial/<entity>/x`、`visible`、`material/<slot>`等路径，不让项目Rust自报“画面已渲染”。版本化按需请求、Summary/Trace上限、同帧捕获及封存回读见[普通3D项目观察合同](../../../samples/three_d_lab_project/OBSERVATION.md)。


## 空间基础批量接口（371 S1-B）

`write_local_transforms3d(&[(handle, transform), ...])` 先检查整个输入，再追加意图。一个回调返回 Applied 后原子验证、提交；同对象多次写取最终值，相同最终值不产生 dirty。默认每 tick 共16384个值意图、1024个结构意图，拆批不能绕过；宿主通过正值 `SpatialProfile` 配置，容量不足整批拒绝。

`teleport3d(handle, transform)` 重置子树历史；`set_spatial_interpolation(handle, enabled)` 切换子树插值并重置历史。`reparent3d(child, parent_or_none, preserve_world)` 使用当前父子句柄；保持全局姿态时，不能表示为 local TRS 的 shear 会拒绝整批。`set_world_anchor3d(root, [f64;3], preserve_world)` 只用于最终根实体；冻结坐标域为 anchor ±1e12m，局部/视图相对坐标±32768m、转换误差≤5mm。锚点不是 cell 激活接口。

生产3D宿主默认按60Hz累计模拟，每frame最多4 tick，保留债务（超120 tick终态拒绝），暂停不推进；显式Editor step/Headless固定步请求仍可用，单frame最多4步。time_scale改变tick频率，自动3D每个tick的模拟delta固定1/60，避免重复缩放。零新tick仍产生表现alpha；表现数据独立于World，项目读取的Transform3D是已提交模拟local TRS。Presentation/Projection只读模拟快照，不能让游戏逻辑读取插值后的坐标。当前为单主相机origin，多view及GPU历史属于S1-C。

```rust
let body = context.world().resolve_entity("body")?;
let mut pose = context.world().read_transform3d(&body)?;
pose.translation[0] += 1.0;
context.mutations().write_local_transforms3d(&[(body.clone(), pose)])?;
// 跳转时改用 teleport3d；结构变更需要真实的当前父子句柄。
```

这些接口要求匹配含 `spatial-foundation-intents.v1` 的Runtime SDK/生成胶水/Host；源码实现不表示默认安装已更新。
