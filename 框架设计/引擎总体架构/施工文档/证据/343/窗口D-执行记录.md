# 343 窗口 D：项目控制、包内资源与生命周期

状态：2026-09-15（Asia/Shanghai）Gate D `source_integration_passed`。239 项去重相关测试通过，实际 RTX 3070/Vulkan 离屏像素与资源退役检查通过，Windows real-window consumer 编译通过。343 整体未完成、未归档；E/F 未执行，338 继续暂停。没有安装 Provider 或修改宿主配置。

授权为本轮“开始施工 GateD”。窗口于 00:24 开始，约 01:29 收尾，约 65 分钟；在 60–120 分钟估算内。基准 HEAD 为 `acaffa8cb95bbb54d093bbf45835a59315c82da5`，验证实际 dirty worktree，不宣称该提交的发布资格。产品与测试输入见[42 项源码/依赖/夹具身份](窗口D-源码与测试身份.json)，完整终态输出与共享纹理首败见[原始输出](窗口D-原始测试输出.txt)。

## 实现和责任边界

- `engine.particle_effect` 成为纯数据 typed 组件，含 `effectRef`、默认 false 的 `playOnAwake`/`paused`、默认空的 `parameters`。Scene/Prefab 的真实资源引用进入 Builder 依赖与实例加载，Hydration 不启动 GPU。
- `Project Game SDK` 的 `writer.particle_effect(&entity)` 提供 `play / restart / stop_emitting / clear / set_paused / set_parameter`。共享 SDK JSON 意图经现有 native adapter、session 成功提交、FrameLoop、Host 进入控制 owner；没有新 Tool、MCP 聚合入口或独立消息总线。
- NoOp/Unhandled/Rejected/Faulted 输出不提交；混合 mutation 的预检失败不泄漏粒子命令。成功提交捕获实体 runtime generation，旧实体命令不能作用到同名重建实体。FixedUpdate 与立即 AUI dispatch 的命令各消费一次，空帧不重放。
- `RuntimeParticles` 只持有每源控制和包内资源，不保存 CPU 粒子数组。不可变资源/参数字符串用 Arc 共享到 RenderScene → Graph/RHI 快照；WGPU 拥有持久模拟、绘制缓冲和退役。为 Arc 序列化启用已有 serde 的 rc feature，未增加依赖包，Cargo.lock SHA 与 C 相同。
- play 幂等；restart 清零模拟但保留暂停；stop-emitting 保留活跃粒子自然更新，clear 清除粒子与事件并停止播放。参数校验名字、类型、有限值和范围；初始覆盖值及运行时修改都消费效果描述的类型真相。Spawn/Update/Both 作用域由生成的 GPU 参数视图落实，另一阶段读该参数的描述默认值。
- 组件移除、禁用、实体重建和 Scene unload 在下一 Host 控制帧退役，不要求该帧有 FixedUpdate。会话/Host 使用独立控制状态，实例编号在进程中不复用；现有整次 Player 会话结束随 Host、包加载器、Renderer 生命周期释放。未新增“保留旧 Loader/Renderer 的任意热换 Session”协议。
- 普通 `.asset` 的 mesh/material 可用 `runtimeData` 保存真实数据：mesh 为 `positions / uvs / indices`，material 为 unlit `baseColor / texture`。既有 Assembler 从不可变快照校验并提取 payload，材质纹理按 id/guid/type 唯一解析并写依赖。未知 runtimeData 字段、非法网格/颜色或资源引用拒绝；旧的数量摘要不会被当成真实顶点。不是通用材质图或模型导入器。
- Backend 解析实际包内效果、纹理、网格和 tint，使用 C 的同一间接绘制路径。粒子专属资源按剩余源引用退役；已由其它渲染消费者重新注册的纹理转为共享资源，不被粒子清理删除。WGPU 通过资源引用保留已提交工作，不手动 destroy 在途 buffer。

## 失败、修复及证据审查

1. 首次开发编译在 native adapter 命中了 `E0277`（返回 `Result<(), String>` 的分支错误地构造 `LogicError`），另有尚未实现 `reconcile_project_particles` 的 `E0599`。前者在 adapter 保留 String 错误类型，后者补齐既定 Backend 接入；随后 `cargo check -p engine_runtime --features real-wgpu --locked` 通过（15.50s）。这些是开发中间状态，不计作验收。
2. 生命周期首轮编译 16.38s、测试 1.00s，5 通过/2 失败。两夹具采用 PID+时间戳，Windows 上取得同一时间戳，Builder 正确报 `output_publish_busy`：

   ```text
   runtime_particles_tests.rs:34
   another publisher owns the output lock: lock acquisition failed because the operation would block
   C:\Users\ZENGHA~1\AppData\Local\Temp\particle-gated-28000-1789404672368791700\.package.publish.lock
   left: Failed; right: Success
   ```

   加入进程内 AtomicU64 序号隔离测试目录，保留单写锁和原断言。随后 9 项通过（编译 14.87s，测试 0.82s）。
3. SDK 测试错误地直接使用未声明的 `serde_json`，发生 `E0433`，位置 `project_game_sdk/src/lib.rs:215/217/222/228` 等。没有为测试扩 SDK 依赖：SDK owner 断言实际 typed intent 顺序/值/世代，JSON 类型与未知字段检查放在已有 serde_json 的 native adapter consumer。最终 SDK 测试通过。
4. 新增反例证明两个控制 owner 都从实例 1 开始：

   ```text
   particle_lifecycle_replacement_owner_never_reuses_gpu_instance_identity
   assertion `left != right` failed: a retained renderer must not reuse a previous session's simulation
   left: 1
   right: 1
   ```

   该红测编译 38.89s，测试 0.06s；修复为内部不复用的实例编号，删除可克隆控制 owner。11 项生命周期复验通过（17.19s / 0.79s）。未引入新 registry 或版本协议。
5. 源码复核发现只测试裸 Builder payload 不足以证明普通作者资产可消费，补齐既有 Assembler 的 runtimeData 提取和直接资源 consumer 测试。初始参数覆盖同步纳入组件与 owner，仍服从已批准的描述类型，不增加每个效果专用接口。
6. 216 项相关回归通过后，资源审查追加“粒子先加载纹理 → 普通 Sprite 后注册同纹理 → 清除粒子”的真实后端反例，测出：

   ```text
   retiring particles must retain a subsequently registered Sprite texture:
   SpriteTextured command references texture handle RenderResourceHandle {
     kind: Texture, index: 16906573690585755676, generation: 1
   }, but RealWgpuBackend has no registered GPU texture.
   ```

   红测编译 46.58s、测试 0.50s。唯一产品修复在 `register_texture_bytes` 成功注册后撤销粒子独占删除权；粒子自身的新上传随后仍显式标为专属。15 项直接消费者复验通过，不新增全局资源引用计数系统。
7. 证据自审发现原“在途退役”用截图 helper，helper 会等待回读；原版本只能证明清理，不能证明无等待退役。改为实际 `simulate_particle_effect` 提交一子步后、不 poll/不回读直接移除 owner，再由后续帧等待并检查错误。修订测试通过。证明的是提交后可不等待地安全退役，不宣称捕获到了 GPU 正在执行的某个硬件瞬间。
8. 一次复验命令误将 `--nocapture` 放到 Cargo 参数侧，Cargo 拒绝且没有运行测试；改为 `-- --nocapture ...` 后正常执行。最终格式检查只发现 `lib.rs` 新模块声明未排序，已用 scoped rustfmt 修正并复查；仅声明顺序变化不重跑行为测试。

## 终态命令与结果

下列 Cargo 命令均在 `G:/gameEngin/rust`，使用现有共享 `target`，没有同时启动竞争 Cargo 任务。所有时间分开记录编译和测试，不据此声称性能优化。

| 命令/消费范围 | 构建耗时 | 执行结果 |
|---|---:|---|
| Runtime 受影响域联合 suite（见下方完整过滤器） | 1m16s | 216 passed / 0 failed / 0 ignored，3.34s |
| `cargo test -p engine_runtime --features real-wgpu --locked --lib -- --nocapture particle_lifecycle real_wgpu_textured real_wgpu_cooked_texture real_wgpu_rejects_missing_texture` | 13.40s | 15 passed，0.96s；更新 11 项生命周期，另加 4 项普通纹理 consumer |
| `cargo test -p engine_runtime --features real-wgpu --locked particle_lifecycle_real_gpu_control_pause_clear_and_inflight_retirement -- --nocapture` | 54.59s | 1 passed，0.53s；替换同名测试证据，不新增计数 |
| `cargo test -p project_authoring_execution --features real-wgpu --locked particle -- --nocapture` | 2m42s | 16 unit + 1 integration passed，3.84s + 2.95s |
| `cargo test -p project_game_sdk --locked particle -- --nocapture` | 12.43s | 1 passed，0.00s |
| `cargo test -p engine_tool_provider --locked --test headless_execution_owner switch_puzzle_semantic_outcome_uses_its_own_bool_contract -- --nocapture` | 1m22s | 1 passed，100.98s（含内部真实准备/构建/执行，不能当纯 Runtime 时间） |
| `cargo check -p runtime_player_winit --features real-window --locked` | 最终 12.78s | passed；此前修纹理前的检查 37.32s 仅作开发记录 |
| 修改范围 `rustfmt --check --edition 2021 --config skip_children=true`、`git diff --check` | — | passed；现有 3 条 dead_code warning 保留 |

联合 Runtime 命令：

```powershell
cargo test -p engine_runtime --features real-wgpu --locked --lib -- particle_ project_runtime_session project_runtime_native_adapter engine_host_loop::tests frame_loop::tests runtime_entity_hydration::tests runtime_instance_loader::tests runtime_package_builder::tests runtime_asset_loader::tests runtime_renderer::tests rhi_command_plan::tests engine_rhi::tests headless_rhi_backend::tests
```

去重计数为 `216 + (15 - 11) + 17 + 1 + 1 = 239`。Runtime 中 46 项粒子相关测试包含 A/B/C 的实际 owner/consumer，未把开发复跑累加。共享纹理修复后仅更新相关生命周期/纹理与窗口消费者；Compiler 的描述/生成 WGSL/快照检查和不消费该注册删除权分支的其它证据复用。最后只改在途测试夹具和模块排序，不使产品行为证据整体失效。

Provider 没有生产代码改动；按施工文档的替代项复用现有独立 playtest/observe 集成测试，运行临时谜题项目的布尔断言及 runRef 读取/失效。它证明原工具链和真实项目模块消费正常，不是 GPU 粒子 headless 回退证据，不代替 E 的粒子项目制作及窗口验收。

## 实际图像与合同判定

设备为 `NVIDIA GeForce RTX 3070`，vendor 4318 / device 9352，驱动 NVIDIA 591.86，Vulkan。四项旧纹理测试虽有设备不可用早退分支，本轮日志包含七次实际适配器创建（3 项粒子 GPU + 4 项纹理测试），没有以不可用早退冒充 GPU 通过。

[包内材质/网格/纹理图像](窗口D-图像/gate-d-package-material-mesh.png)已在最终复验后打开审查：黑背景、朝上的黄橄榄色三角形；真实 triangle positions/UV/indices 决定轮廓，纹理 `[128,255,0,255]` 乘 unlit tint `[1,.5,.25,1]`，中心像素期望 `[128,128,0,255]`、每通道容差 2。没有以矩形或测试几何替代网格。它是责任层诊断图，不是最终美术案例。

参数 GPU 反例使用 Spawn 默认 1/设为 3、Update 默认 2/设为 4；出生速度实际 `[3,2,0]`，下一更新 `[1,4,0]`。暂停重复帧年龄不变，暂停时 restart 清空、恢复后新生 age=0；clear 后 live=0。local/world 与相机消费继续由本轮重跑的 B/C GPU 测试约束：跟随源平移/保留出生位置；不扩展声称完整父级 TRS 或任意模型材质能力。

## 收尾与下一依赖

自审通过：没有新增 AI 工具、CPU 全粒子镜像、独立 Compiler/总线/设备、通用材质图或生产安装。既有音频/动画的直接共享提交/加载消费已回归；不重复它们的历史真实游戏全量验收。Stage E 的双效果首版及各两轮修改、窗口关键帧/5次重播仍未执行；Stage F 的规模、GPU计时、局部失效及独立交付仍必须完成，239 项测试不能替代这些证据。

收尾时 cargo/rustc/link 为 0，`particle-gated-*` 与本轮 `aife-provider-puzzle-*` 临时根无遗留，失败输入夹具未产生输出目录。测试自行清理其独占副本，未删除用户项目；保留增量构建缓存、图像和原始证据。没有生产二进制或配置需要回滚。49/54、343 方案状态与当前施工同步为 D 完成、下一窗口 E；338 暂停未归档、安装基线不变。
