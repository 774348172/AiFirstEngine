# 343 窗口B执行记录

2026-09-14 22:27开始，23:03终态记录（Asia/Shanghai，约36分钟）。状态：Gate B passed，39项去重测试及Windows窗口消费者编译检查通过。343仍为唯一当前施工，C–F未执行，338暂停未归档；未改安装或宿主配置。

## 首次失败

`cargo test -p project_authoring_execution --locked particle_effect_cook_defaults_are_valid_without_author_code -- --nocapture`

编译2分16秒，测试0.01秒。Naga校验捕获生成代码缺陷：

```text
particle_effect_cook_defaults_are_valid_without_author_code
called Result::unwrap() on an Err value
game_project_compiler.particle_effect.wgsl_invalid
Assets/test.particle-effect.json emitter 'main': name `active` is a reserved keyword
source_location.generated: true
test result: FAILED. 0 passed; 1 failed
```

归类实现缺陷，修复引擎生成Record字段active→alive，保留原始作者接口和失败断言。终态Cooker测试集已复验通过，见下文。

## 后续开发失败与定点修复

1. 首个GPU测试编译失败：`error[E0433]: cannot find RealWgpuBackend in wgpu_backend`，测试夹具用了错误路径；修正为已有`wgpu_backend::real::RealWgpuBackend`，没有改生产模块结构。
2. 真GPU首次编译shader失败（Rust编译47.90s、测试0.73s）：`particle_gpu.pipeline_invalid:main` → `Shader 'main' parsing error: name target is a reserved keyword`，WGSL第32行`EngineEvent.target`。内部字段及函数参数改为`emitter_target`。随后作者执行/反例测试通过（编译10.48s，测试0.69s）。
3. 首轮10项GPU测试通过（编译30.65s，测试1.50s），自审后增加两个窄边界反例，未把这10项重复计入终态总数。
4. `particle_gpu_loop_rate_retains_fractional_cycle_emissions`：循环周期1/128s、rate=64/s、步长1/64s，实际live=0、预期1（编译16.41s，测试0.45s）。首因是每周期先floor导致小数永久丢失；改为rate沿累计有效时间取整，burst按周期计算，质量缩放沿累计量做差。
5. 极大速度碰撞的第一次测试夹具预期不成立：法线[-1,0,0]、速度[3e38,0,0]实际反射为[-3e38,0,0]，是合法有限结果，不能把`live=1,invalid=0`认定产品失败。最初增加碰撞后检查后该断言仍失败，随后加实际状态输出查明上述原因。相关失败分别为测试0.50s、12项集11通过/1失败（编译10.03s、测试1.80s）、带状态输出的窄测试（编译15.94s、测试0.45s）；全部保留为夹具诊断过程，不冒称有效红证据。
6. 修正夹具为法线[0.6,0.8,0]、速度[-3e38,-3e38,0]，并移除待验证的碰撞后检查，真正复现：

```text
particle_gpu_collision_overflow_is_recycled_before_events
GPU state: live:1, invalid:0, collisions:1, indirect_instances:1
position:[-7.499996e35,5.625004e35,0.0], velocity:[inf,inf,0.0]
age:0.015625, lifetime:1.0
assertion left:(1,0), right:(0,1)
test result: FAILED. 0 passed; 1 failed; test time 0.51s; build 13.45s
```

由此证明碰撞可能在作者更新校验之后产生无效数。唯一修复是在碰撞结果进入事件/live索引前再次检查并回收；相同夹具不放宽断言，复验通过（编译14.58s、测试0.52s）。没有引入新schema、错误恢复框架或CPU粒子镜像。

## 本窗口实现边界

- Compiler复用typed布局生成真正compute程序，仍由真实Naga校验后进入普通资产payload；Runtime不读取作者源码，也不在运行时拼装作者WGSL。
- `ParticleGpuEffect`托管持久records、free/live索引、间接绘制参数、只读参数、双事件buffer；现有RealWgpuBackend用同一device/queue创建、模拟、清理和移除实例。
- GPU依次update/free→route上一子步事件→spawn→finish。作者init/update、自定义状态/输入、标准更新、形状、碰撞、事件实际执行；新生age=0，下一子步开始更新。子发射最多配置代数、总事件预算及目标容量；超过容量只丢弃，不扩容。
- 显式stepId拒绝重复/过期模拟；单次最多8个≤1/60s子步，过多时间显式丢弃；暂停不累积追赶，清理保留buffer并清空事件/种子时钟。stop-emitting停止常规发射，现存粒子继续更新。
- 同一encoder连续提交多个逻辑步时，用有序copy上传各步uniform，防止queue.write_buffer把前一步参数覆盖。常规热路径没有GPU poll/map/readback；≤64粒子读回仅由显式验收调用开启。
- 保留原有graphics device需求，仅在支持时增补该kernel所需compute limits；不支持返回明确诊断。Window设备与RenderGraph绘制属于C，SDK/session生命周期与参数更新属于D。

## 终态验证

环境：Windows，测试profile unoptimized+debuginfo，工作树源码（非发布精确提交）；`real-wgpu`、wgpu/Naga26。真实选中适配器：

```text
AdapterInfo { name:"NVIDIA GeForce RTX 3070", vendor:4318, device:9352,
device_type:DiscreteGpu, driver:"NVIDIA", driver_info:"591.86", backend:Vulkan }
```

预登记位置/速度绝对容差≤1e-4，年龄≤1e-5，计数精确；通用near断言实际使用1e-5。每个已知输入场景≤64容量、≤8个子步。只证明本机Windows/Vulkan模拟，不证明其它GPU/Android、画面质量或100k性能。

`cargo test -p engine_runtime --features real-wgpu --locked particle_ -- --nocapture`

```text
Finished test profile in 0.21s
running 22 tests
343 author variant=0: position=[1.046875,2.03125,3.0], age=0.015625, counter=8
343 author variant=1: position=[1.046875,2.0,3.0], age=0.015625, counter=7
343 author variant=2: position=[1.09375,2.03125,3.0], age=0.015625, counter=8
test result: ok. 22 passed; 0 failed; 0 ignored; test time 0.80s
```

包括12项真实GPU测试、9项原有描述合同、1项已有粒子纹理producer消费。variant1确实不执行作者update，variant2确实重复积分；同一位置/状态oracle拒绝两者，非只检查生成字符串。其余真实读回验证容量4请求10丢6、delay/loop/recycle、质量缩放、小数跨循环、四种形状、有序重力/drag/noise/曲线/rotation、plane/sphere、death/显式子事件、事件预算1产生2次碰撞丢1且下步生子、代数上限、customOwnsPosition无二次积分、step参数不覆盖、pause/clear/seed replay、布局/非法步拒绝和Backend资源所有权。

后续正式消费端命令（cwd同为`G:/gameEngin/rust`）：

```powershell
cargo test -p project_authoring_execution --features real-wgpu --locked particle_effect -- --nocapture
cargo test -p engine_runtime --features real-wgpu --locked real_wgpu_offscreen_backend_can_execute_basic_plan_when_adapter_exists -- --nocapture
cargo check -p runtime_player_winit --features real-window --locked
```

```text
Compiler: Finished test profile in 2m47s
unit: 15 passed; 0 failed; 0 ignored; test time 4.09s
desktop_export_owner: 0 matching tests (不计入通过数)
game_project_compiler_interface: 1 passed; 0 failed; test time 3.04s

existing offscreen draw: build/fresh 0.38s
1 passed; 0 failed; test time 0.72s

runtime_player_winit real-window: Finished dev profile in 38.29s; exit 0
```

Compiler的15项包含`particle_effect_cook_gpu_consumer_executes_packaged_author_program`：真实快照经Cook序列化为普通asset payload，解析后丢弃作者快照，现有Backend执行两步，实际position.y=0.03125、age=0.015625、live=1；与包字节roundtrip、Assembler消费和独立快照check/prepare测试共同覆盖本轮跨crate影响面。该测试不是自行拼装测试shader；不冒称完成SDK/Scene或最终独立交付。

终态计数22+15+1+1=39，含13项真实GPU执行测试（12 owner+1 Compiler consumer），不累计开发中的重复运行。已有offscreen draw回归与本轮Backend强制成功创建/执行测试共同覆盖device limits修改的直接影响；没有把可跳过的adapter smoke单独当GPU资格。

所有触及Rust文件的定向`rustfmt --check`通过，已跟踪入口`git diff --check`通过；方案/施工文档本地链接检查通过。3条dead_code警告均来自原有`project_logic/world`，无新增粒子warning。未跑全workspace、Local CI、Android/release、安装或真实粒子窗口：本轮声明为Windows源级GPU模拟，绘制/效果创作/规模/最终交付留C–F。没有因改纯布局生成而重测字体/音频/动画；原基线证据只按原身份复用，不作为粒子证据。

## 自审与收尾

本轮保持单一Compiler、RuntimePackage普通资产路径、现有Renderer设备/队列与九项工具；无CPU逐粒子镜像、项目玩法owner或新AI工具。生成helper是共享纯语言/布局代码，Runtime只使用Cooked程序；当前343从未安装，新增内部record stride不新增发布迁移机制。常规模拟buffer持久复用、事件预算/实例容量有界，回读严格显式。被接收的并发饱和事件顺序不是确定性承诺；大规模速度及内存预算等待F实测。

本轮应用项目/施工Skill限定为Gate B，所有普通失败自行修复，没有借此展开C–F。49/54与343方案/施工当前块已同步；目录索引只引用49/54，无需复制动态状态。未归档343、未恢复338、未更新安装Provider/config。23:02进程复核无cargo/rustc/link残留；测试自有快照/包目录由成功测试清理，未执行递归清理或改动用户项目。下一窗口Gate C：正式RenderGraph/RHI消费当前持久模拟buffer，完成真实粒子绘制及图像验证。

## 消费的源码/测试身份

路径相对`G:/gameEngin/rust`，SHA-256绑定本轮工作树文件，不代表历史HEAD、安装Provider或发布DLL。无关既有修改保留。以下纯源码和夹具是本轮测试输入，后续修改只失效实际消费它的证据。

| 文件 | SHA-256 |
|---|---|
| crates/engine_runtime/src/particle_effect.rs | A1AE057410EDF35EF9C887FE57235F68648716193D098E3469BD0CBC252D56D3 |
| crates/engine_runtime/src/particle_effect/program.rs | 97FF9F05788D3FF9ECC85FC59222BD477FEC49DEBFD50B7D424A7663D0F10B08 |
| crates/engine_runtime/src/particle_effect/simulation.wgsl | E1A2D7F3D1785F1B8795EBA74FB92CB8558BA459C993D0B4BC3BF2C238A2C8C9 |
| crates/engine_runtime/src/particle_gpu.rs | 270F7BC91ABB3E12230DCB6A75F184C35AF4EDCBE7BA2C0FE4FE916989B4ACFA |
| crates/engine_runtime/src/particle_gpu/tests.rs | 5811B19C7285961901C7BB978B66A02C83D9C0B733C07721F1DDCC82F535D1D4 |
| crates/engine_runtime/src/wgpu_backend.rs | 9BED84A6B9CA4FA70A2EEF417CDA5179CBA0ABE219FA90D1E89679F7256F187A |
| crates/engine_runtime/src/lib.rs | FB973DF320577FCBC4366E0590885FF9CC465348C1B28163DCC5A5F6BB8225E5 |
| crates/project_authoring_execution/src/particle_effect_cook.rs | FFE9F5169AD5EEF6A48EE42CE87CCE5378C524B3CC902C9ECB820F6D71B3382D |
| crates/project_authoring_execution/src/particle_effect_cook/tests.rs | C7CFC8659A88EBA14D3F9E3829B1FEA3597212AA52B5ED47445994D19A1C0252 |
| crates/project_authoring_execution/Cargo.toml | 7941F2634C967F37137D95032EC3CA47472D31F8CC681B5DFCA0ADE796586259 |
| Cargo.lock（沿用A） | 191AC9AEF1D7A2D9F0329521AD4B89BBC6339E80176431687881EA25987216AB |

描述validation、原9项类型测试、JSON fixture及Compiler快照消费者夹具均沿用A记录身份。Compiler新增`real-wgpu`仅转发既有Engine feature，供正式Cooker产物直接消费验证；没有新增依赖包、独立runner或生产安装。
