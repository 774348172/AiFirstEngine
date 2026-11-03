# 365 Gate C：仿射表现、裁剪与命中执行记录

日期：2026-10-08。状态：`c_affine_presentation_source_gpu_integration_complete`。仅关闭C源码与Windows开发集成；365整体仍施工，D/E未开始。默认安装保持20261007-364，塔防Release未重建。

## 1. 实际交付与自审

公共SDK新增typed Rotation/Scale/Pivot/Opacity/Tint/Resource/LocalStyle。NodePresentation、Affine2D与Transform2D沿既有AUI Document → retained preparation/layout → UiProjection → RuntimeRenderer UI Pass消费，不新增Renderer、每节点Canvas或GPU资源owner。父局部矩阵、pivot、镜像、继承颜色、裁剪和输入共享最终几何；AABB只作宽阶段，矩形/圆角使用逆变换窄阶段。零scale不命中，不可表示的矩阵/四角诊断并剔除，不产生非有限rect或panic。

表现setter沿既有same-value和原子事务合同；纯表现不引发布局/文字重排。Controller按属性稀疏覆盖，保留同节点其它Binding；释放恢复当前Binding。资源Binding及Controller均校验包内挂载的ID/GUID/type；LocalStyle只改paint，替换/清除与fresh主题结果一致。Editor依赖收集识别typed fallback的assetId/guid，Native Snapshot保留Vector/Color/Resource/LocalStyle。

成功呈现发布最终几何，清洁Arc发布O(1)，有journal时同步变化闭包；Failed/布局诊断/扩展稳定化失败不发布。回调UiWriteBuffer转发坐标查询：属性读取可见暂存值，坐标读取已提交宿主状态。同帧接受后查询可从已提交基础属性求矩阵；输入读取最后成功发布（含反馈）的几何。旋转/镜像滑条和scroll交互使用局部轴；KeepScreen重挂修正新父逆矩阵及pivot/RS原点，不可逆父拒绝。

clip采用CPU凸轮廓裁剪与原UI Pass，支持8祖先边界及图片自身圆角。圆角屏幕弦高≤0.25像素，每角最多256段，超限诊断拒绝。Canvas边界在最终多边形三角化/映射前裁剪并插值UV/RGBA；界内多边形保持借用。反馈从base组合，必要后代继承最终矩阵；颜色在实际图元色相乘后才饱和，避免悬停提亮被白色中间Tint截断。Legacy共用几何owner并避免重复变换。

实际协议为UI_UPDATE_VERSION=7 / aui-update.v7、UI_HOST_SCHEMA标识ui-host.v5；SDK指纹同步，ABI1.6及扩展v2不变。旧项目须用新SDK/Glue重建。正式方案14.3保存实际合同。

自审结论：公开接口、真实DLL、生产缓存、输入、绘制及GPU消费者均已闭合，可关闭C。没有提前实现D动画调度、E项目/Release性能或安装切换。反馈仍有layout Vec COW复制；几何重算局限变化闭包，不宣称完全无全量拷贝。

## 2. 命令、终态与复用

cwd `G:/gameEngin/rust`，Rust1.96.0、Windows dev/test、既有Cargo target。GPU启用real-wgpu，实际后端Vulkan；适配器构造失败会使测试失败，未静默跳过。真实窗口只编译，运行与项目交付留E。

| 实际命令 | 有效证据 |
| --- | --- |
| `cargo test -p engine_runtime --lib --test aui_retained_instance --test aui_retained_layout --locked` | runtime-terminal.log：751+25+15通过，测试6.78/0.08/0.09秒；编译1m17s |
| `cargo test -p engine_runtime --lib aui_geometry --locked` | geometry-terminal.log：3通过，覆盖公共命中、镜像滑条及四角/祖先溢出；后来完整lib覆盖，不重复计数 |
| `cargo test -p project_runtime_sdk -p runtime_player_winit --lib --locked` | sdk-player-terminal.log：53+81通过，测试0.06/1.02秒；编译1m16s |
| `cargo clippy -p project_runtime_sdk --all-targets --locked -- -D warnings` | sdk-clippy.log：严格通过，10.00秒 |
| `cargo test -p project_authoring_execution --lib generated_runtime_glue --locked` | glue-final.log：13通过，实际生成/构建/加载DLL，新属性接受/回滚及坐标往返通过；测试14.61秒含子编译，外层编译2m21s |
| `cargo test -p editor_core --lib presentation_resource_dependency --locked` | editor-resource.log：1通过，测试0.00秒；编译3m33s |
| `cargo check -p editor_core -p runtime_player_winit --features runtime_player_winit/real-window --locked` | consumer-check.log：退出0，1m25s，编译消费者，不冒充窗口运行 |
| `cargo test -p engine_runtime --lib cross_canvas_polygon --locked` | canvas-red.log首次0顶点而期望6；canvas-repair.log修复后1通过，后来完整lib覆盖 |
| `cargo test -p engine_runtime --lib feedback_brightness_is_applied --locked` | brightness-red.log首次提亮断言失败；brightness-repair.log修复后1通过，后来完整lib覆盖 |
| `cargo test -p engine_runtime --lib --locked` | runtime-final.log：最终753通过，测试6.89秒，替代前751/752证据 |
| `cargo test -p engine_runtime --features real-wgpu --test aui_presentation_gpu --test aui_draw2d_gpu --test aui_extension_c4 --locked -- --nocapture` | gpu-final.log：3+2+5通过，测试0.50/0.79/0.00秒；编译2m03s。设置AIFE_365_C_EVIDENCE_ROOT到本Gate目录 |
| `cargo clippy -p engine_runtime --lib --features real-wgpu --locked --message-format=json` | runtime-clippy-final.log：退出0，23.27秒；125项既有Runtime、2项engine_input告警，与之前runtime-clippy.log数量/类别一致，不声称零warning |
| 本轮29个Rust文件的 `rustfmt --check --edition 2021 --config skip_children=true` | format-terminal.log退出0；只检查本轮owner/直接consumer及声明的测试文件 |

去重951项：753+25+15+53+81+13+3+2+5+1。定向调试、首次失败、旧52 SDK及751/752 lib不另计。最终两处Renderer修复后更新完整Runtime、GPU及Clippy；SDK/Glue/Player/Editor/retained合同owner未再修改，复用其已通过证据。Renderer的最终改动仅影响越界多边形和最终颜色饱和，未改这些公共类型/事务/资源/布局/编译消费合同；对应行为由最终owner与GPU消费更新资格。没有以文档同步触发重复回归。

## 3. 失败首因与因果修复

- 增量重建曾丢弃主题resolved style，导致七项cached/fresh对照失败；保留未变style，LocalStyle替换/释放重新从source、Binding和Controller解析主题。ring改自适应分段后旧固定168顶点断言失效，改按精度预算断言实际几何，内外ring用相同角采样，避免索引错位。
- 首个千节点用例误用rawPresenter，缺少生产宿主调度复用标志而重建1001节点。将同一需求放到既有AuiPresentCache真实owner：单rotation仅更新1节点、prepared binding重建0、无关兄弟draw Arc复用、clean帧无重建。不是通过降低负载解决。
- 真实DLL最初不能编译：UiWriteBuffer缺少UiGeometryAccess实现；补宿主转发及SDK暂存/提交区分测试。之后mode100 Faulted，诊断夹具显示aui_paragraph_layout_failed / FontBundleMissing。夹具未挂字体，成功几何发布正确拒绝失败帧；此坐标/事务夹具使用同ID同固定尺寸Panel提供几何，保留实际session Text/View类型与全部文字提交/回滚断言，不声称该夹具验证glyph。
- 公共角落命中断言曾要求完全无节点，实际child已因圆角被拒绝，父容器仍可作HitNonInteractable候选。修正为child不得命中、clip_rejected_count=1、解析clip包含检查为false；不改变产品逻辑或放宽裁剪要求。见hit-failure.log。
- 四角有限检查从两个对角扩为全部四角，counterexample为巨大非均匀父scale与45°子rotation：对角抵消而侧角溢出。诊断/cull验证通过。
- GPU noncentral pivot用例中心167.00114/145.64647输出黑色。rounded-diagnosis.log确认矩阵/可见性/clip/paint正常，整个图形没有白像素。GameView映射拒绝超Canvas顶点，旧fan跳过相关三角形而丢失可见区域；canvas-red.log在headless owner证明0而非6顶点。只在Renderer投影前裁Canvas，保留映射owner合同与插值。最终三个DPI的内部/角落像素通过，未降低断言。
- 收尾发现brightness在transform_item_polygon中先对白色中间顶点clamp到1，再乘真实图元颜色。brightness-red.log捕获0.4×1.5未得到0.6。将clamp移到project_polygon最终颜色，正常Tint/Opacity及mesh共用边界，完整753/GPU/Clippy重新通过。

失败首因保留在本Gate日志目录；普通夹具API/编译调整未被当成产品性能故障。没有添加allow或放宽精度/容量/原子性预算。

## 4. GPU与局部更新证据

真实链：Presenter → retained draw/composition → RuntimeRenderer → RHI → RealWgpuBackend → RGBA readback。DPI1/1.5/2对应300×180 / 450×270 / 600×360的Unorm离屏目标。

- pivot中心与旋转后的原rect外内部白色；AABB角黑色，镜像排除区域黑色。
- 镜像红图元Tint/Opacity输出125–130，继承双opacity绿色输出62–66；opacity变更不增加剖分。
- 八祖先clip内部绿色、外部黑色；零scale不命中且不出图，九祖先clip诊断拒绝。
- 非中心pivot[0.15,0.9]、rotation0.6、scale[1.4,0.7]、圆角25：local[40,40]/[17,17]白色、[3,3]黑色，三个DPI均与逆变换解析clip一致，且图形跨Canvas边界仍保留内部。
- 六个静止帧新增剖分0、GPU顶点上传0。单千节点叶rotation更新1几何、prepared binding重建0、兄弟绘制Arc复用；这些是操作计数，不换算成AUI P95。
- Gate B曲线/洞/AA/UV颜色/资源替换/backend重建两项GPU及既有mesh/text/image五项consumer复验通过。

[截图](Gate-C/presentation-gpu.png)实际查看，为第一组300×180 readback；其余非中心圆角场景由测试逐像素断言证明，未冒充同一截图。SHA256 `8211D09D0D0ADC9AA4C25641F07CBCB69C7413CA6202FE294DBE77C91BA2CFCB`。离散圆角与解析命中边界有≤0.25像素误差；未取得全平台、完整色彩/抗锯齿、OS窗口、Release性能或跨引擎等价资格。

## 5. 工作区身份

HEAD基准 `acaffa8cb95bbb54d093bbf45835a59315c82da5`；大量既有未提交/未跟踪修改保留。下面是本轮owner/consumer、所用harness及直接构建依赖的最终文件字节，不是clean commit、整仓源码seal或安装二进制身份。内联单测与产品代码共享文件，终态结果按实际命令配置区分。

| 文件 | SHA256 |
| --- | --- |
| rust/Cargo.lock | C870DECC47180F4B73128028E30D29B20DB9D74D69F24C557194B908D8218C07 |
| rust/crates/engine_runtime/Cargo.toml | 5409715A8D6E50C40456098CE3BE20E63B41AB8B39B4C1E2A82F7E6D5137ADEF |
| rust/crates/project_runtime_sdk/src/aui.rs | 89002C9CFFF25A156CE28E00FCD46A3BD995A09111F14BB6E7784CF98CAFA1C4 |
| rust/crates/project_runtime_sdk/src/aui_presentation.rs | 21A1D587077E013837974B12800E4D1385237C1C4F3B2FABD63610B33185BD47 |
| rust/crates/project_runtime_sdk/src/aui_wire.rs | 5CD0DFFEC800F70D56B385B66E2E00D518760C417947E208F3C5CB60407E3E23 |
| rust/crates/project_runtime_sdk/src/aui_host.rs | 8AB0B9E9C6CB91B3998EC6778C24BE448F9C177D83E5CC8D5FE68A632C2A9C90 |
| rust/crates/project_runtime_sdk/src/lib.rs | B438D8A6C008F67C0528AA82EC433FD0BB3E574405BBA68007A9980F36039FBA |
| rust/crates/engine_runtime/src/aui.rs | 676178EB5E41CAB912D14A07C87BB9223DB12D1663AD337020FED029C4CD5370 |
| rust/crates/engine_runtime/src/aui_instance.rs | A6A3BBC130D27E005DB8B1A2BE509B6247DFDA75B6F6E6493A007B2B5AF04ABD |
| rust/crates/engine_runtime/src/aui_session.rs | 6BDCEAC597C76B63D613F052ECA242E04078EB7AFF50467ED8E2CDB707E8DEB7 |
| rust/crates/engine_runtime/src/aui_geometry_cache.rs | 0AA789AF4B94670282D82DBDD6890F8F0FB356B270B4A6FBAEAFA8A647441A99 |
| rust/crates/engine_runtime/src/aui_hit_test_index.rs | BD5BFCF0C84E58BA8CC370CFD4BCFAF6B82F5FE8A5281DFD04BEB6F4AF0FF641 |
| rust/crates/engine_runtime/src/aui_layout_schema.rs | 58910EC5825C4A6685F42440A543C45BD644FF56B5E19F14AB2B4668F2505257 |
| rust/crates/engine_runtime/src/aui_control_feedback.rs | B2F53EB75526CBE3EE27886293F97CD85798721199F02BCF667806D341582FC1 |
| rust/crates/engine_runtime/src/aui_present_cache.rs | 24F3736A985A5F8A128A1A9F1B8A34E0C80B29D68971F9FC421C7C8F5786FE00 |
| rust/crates/engine_runtime/src/aui_drawing_cache.rs | E5AF717B60DC8944DCD64C53351351653953D607B108B068E3EAD5672415AE9A |
| rust/crates/engine_runtime/src/runtime_renderer.rs | 8D3E213C2D760BBFA56B5E1AFD45CA4A600E7A001A25DBC5089A4C1C1AE0E172 |
| rust/crates/engine_runtime/src/runtime_package.rs | 0E079613EB269781790CF7D256CA193ECE298466BFD18594BDB188B1863CB3B2 |
| rust/crates/engine_runtime/src/project_runtime_native_adapter.rs | B26D301B4FEBA5583D2BD876CD4B774294E925988F8D9BA7403BA7F1EE4065F7 |
| rust/crates/engine_runtime/src/project_runtime_session.rs | 000F9352BDEFE461FB7A4154C76D1E5C5F239BD4433704BD050DA1A89EBD1E13 |
| rust/crates/engine_runtime/src/engine_host_loop.rs | 86447EB530F737F703034B687A81D8A1BCC07C121DC0CDA53054685140F70DC2 |
| rust/crates/editor_core/src/aui_template.rs | 2ACBEE3A46E55112E97DEBCCCD00E0B4453D316DC91DA851CA5F0B49CC683C88 |
| rust/crates/editor_core/src/aui_document_cooker.rs | 1F40BDB6965A810304C6692BE9AC73A6A3435D34F5B7368ED628E09A68BC1523 |
| rust/crates/editor_core/src/services/aui_service.rs | 3386D200941A54E682DF147C16440DD473814290FFED5F0386BD82E27AEB1DAA |
| rust/crates/runtime_player_winit/src/lib.rs | 939D873A5646D41266E36B22D30F15F4CA5C510B3DB8C1264B2CBA735A0C8D3A |
| rust/crates/project_authoring_execution/src/generated_runtime_glue.rs | BD0E96F7DF0458B919CC65803E206D116584A01F16EE8320BA7AD4E4E9F9D3C9 |
| rust/crates/engine_runtime/tests/aui_presentation_gpu.rs | 2C44B28B72ED38E93A6FE4C554408C895C01536C67A6F21007B5520558222E1E |
| rust/crates/engine_runtime/tests/aui_draw2d_gpu.rs | 0D688036CC5A124AEA413320C6555FED8A038C13BEB2C79F12262DF3215BC9DF |
| rust/crates/engine_runtime/tests/aui_extension_c4.rs | 53FCC809E12B9B526A526078105BD06388F1F54B674BE637BF152BDB35BE2BBE |
| rust/crates/engine_runtime/tests/aui_retained_instance.rs | C11DD6F7DD7FFFDF3825EBDEBFFBED59D06D65D3739B8393B770EF8D8EE9485E |
| rust/crates/engine_runtime/tests/aui_retained_layout.rs | 227DC20F67D6E0FC53EDC2ECDD97F0209747603313131C05156313C454F1D89C |

## 6. 成本、外部状态与下一块

本连续授权03:07:51 UTC / 11:07:51 CST开始；功能/最终Clippy验收于04:55 UTC / 12:55 CST结束，文档同步另计少量时间，整体约110分钟，小于3小时。跨crate/feature编译与Cargo锁等待单独记录，最长本轮Editor编译3m33s，不归因为UI frame开销。

没有运行生产安装、切换默认配置、Local CI、塔防Release或全workspace回归；当前声明无需这些成本，E保留项目与性能资格。Generated DLL临时夹具由既有TestDirectoryGuard清理，终态同前缀目录检查为空；GPU只更新本Gate证据PNG。未修改外部配置/安装，无需恢复生产状态，没有删除项目/安装或留下需用户清理的临时包。

下一整块D：组合补间、UI/游戏时钟、活动轨道、ownership冲突/恢复及生命周期。随后E：两个普通项目、真实窗口、Release负载/三关与独立交付。总施工保持当前，不归档整个365。
