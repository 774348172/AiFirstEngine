# Gate E 首败与修复

本文件记录实际失败，不将首次制作包装成一次成功。运行原始请求/响应保存在 `.artifacts/343-particles/<version>/<timestamp>-*.json`；失败产物保留在项目 `Library/EngineTools/Deliveries`。

1. 项目 Rust 初编：`ObservationOutput` 没有 `diagnostics` 字段；`handled_action_count` 要求 `u64` 而非 `u32`。首条输出为 `error[E0560]: struct ObservationOutput has no field named diagnostics`（lib.rs:80），另一条 `error[E0308] expected u64, found u32`（lib.rs:36）。修正项目 API 用法后3项项目测试通过。原始作者文件仍保留在首个v0-a/source快照。
2. 21:36 UTC首个check：合法碰撞平面 `distance:-2.6` 被生成成 `dot(p.position,n)--2.6`，Naga报告 `expected ;, found --`。责任层为 `engine_runtime/src/particle_effect/program.rs`。新增真实Cook/Naga回归先红（negative-plane-red.log），最小修复为负数表达式加括号；Cooker与公开快照消费者16项通过（cooker-green.log）。没有修改坐标合同或作者参数去绕过缺陷。
3. 场景夹具多填 `seed:42`：当前场景通用随机种子消费者未接入，返回 `game_project_compiler.playtest.seed_unsupported`。删除场景seed，保留实际GPU粒子资产的seed42；没有为了样例扩展场景协议。
4. 项目未执行rustfmt，交付返回 `validate_project_runtime_format failed`；直接执行项目格式化后通过。首个run调用止于格式检查，不能当成粒子运行证据。
5. 21:43 UTC run及独立playtest在交付自动headless预检返回 `rhi_failed`。实际包/模块已生成；HeadlessRhiBackend明确不执行GPU粒子，源码错误码 `particle_render.unsupported_backend`，摘要仅留下 `rhi_failed`。独立playtest的构建遗漏了已有 `.with_player_verification(false)`（普通build已使用），导致真实窗口场景之前先跑不兼容的额外headless Player。最小修复仅补Provider semantic消费者调用选项，仍由同交付的真实场景进程、退出码、语义与捕获验收；不增加工具、不要求用户先调用另一个工具。现有puzzle集成回归补验证无额外预检报告，同时保持真实headless场景与observe验收。

headless摘要没有透出粒子unsupported细节是已观测诊断局限，本Gate不扩展报告架构。普通 `run` 的headless模式不是GPU粒子可见性证据；真实GPU视觉由windowed playtest取得。

6. 21:50:23 UTC首次打开真实截图（v0-a/20260914-214914）：五帧完全相同、只有背景；虽然自动报告overall passed，人工视觉审阅明确failed，不能计为首次效果可见。AUI摘要显示load_failed，项目未提供ui_state回调，补公开SDK空状态producer即可显示静态UI。背景Sprite的基础面片宽高为2，项目scale从24×13.5修正为12×6.75，完整覆盖既定相机。
7. 粒子不可见的责任层：场景2D相机投影Z为单位矩阵，相机z=10使世界原点clip.z=-10，WGPU裁剪。Sprites仅使用XY，旧测试没有检查Z。扩充既有scene_camera_2d测试，通过正式ParticleRenderView消费相机矩阵，首败 `world z=-2, clip z=-12`（camera-depth-red.log）；修复2D投影复用已有默认2D的深度跨度，检查世界层-2/0/2均在0..1且深度次序正确，owner回归通过（camera-depth-green.log）。不改变作者相机、不移动粒子绕过裁剪，也不新建相机系统。

8. 21:57:21 UTC打开v0-a-visible真实五帧，首次粒子可见，距窗口开始31分34秒（含引擎缺陷定位、编译与失败重跑，不能视作纯创作时间）。0.1秒flash硬边，检查源PNG与Cooked RGBA的中心行alpha同为0/255；责任在离线PowerShell纹理脚本的Math.Max整数重载，不是引擎Cook。改为显式浮点乘法渐变后x=0/8/16/24/31的alpha为0/22/111/214/255。重新导入同GUID纹理，不改变Renderer混合规则。
9. 首次v0-b截图只见少量重叠光点。WGSL将32位随机种子转换成大浮点后送入sin，损失有效随机分布；作者改用u32混洗后取24位转换0..1。相同种子/时间线重新捕获确认。属于首版作者修复，不计作文档指定的“反转扩大”或“高度速度”修改轮次。

10. 首轮A稳定性scenario的assertions未按fromSimulationTick排列，Compiler拒绝；只排序原有断言，未删除或放松，通过后保留失败/成功两份记录。最终q系列在修正后的相同控制要求上重新验收。
11. 对照正式方案7.2/7.3发现v系列漏了烟雾flipbook和螺旋上升；这是施工流程漏项，不是架构变更。补普通4帧图集与rise_speed/WGSL上升函数，重新从q0制作和审阅到q1/q2；v系列保留但不计作最终制作资格。详见[最终案例复核](窗口E-最终案例复核.md)。
12. q0-a-final项目observer使用Transform组件名而非既有engine.transform；改用规范名后q0-a-final-pass通过。观察值读取真实ECS，不用项目布尔变量冒充源位置。
13. 世界空间scenario首次把Number合同期望写成整数3，Compiler明确报playtest.observation_type_mismatch，定位/assertions/2/equals；修改为3.0后3条断言通过。保持既有严格类型合同，没有放宽引擎校验。

按责任问题组统计共13组，其中3组需要Engine生产源码修复（2、5、7），其余10组为项目作者/API使用、夹具、离线纹理或制作流程修正；组内可能含多个编译报错，不冒充13次精确调用失败。额外证据分析脚本首次缺.NET10的GdiPlus引用，补仅脚本引用后像素差测量成功；该分析脚本错误不影响游戏产物。失败目录全部保留，无人工修改作者文件、节点或资源。
