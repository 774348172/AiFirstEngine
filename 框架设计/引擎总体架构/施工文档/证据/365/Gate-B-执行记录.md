# 365 Gate B：通用图形生产链执行记录

日期：2026-10-08。结果：`b_draw2d_source_gpu_integration_complete`。365整体仍施工，C/D/E未实施；默认安装仍为364。本记录绑定当前工作区字节，属于Windows开发集成证据，不是clean commit或Release性能资格。

## 1. 实际交付与自审

公共Path2D图元便利函数、Draw2D及扩展paint发布已接通。`aui_graphics`使用冻结的lyon 1.0.16实现有界路径展平、填充/描边与coverage fringe；输出进入既有DrawMesh、UiProjection、RuntimeRenderer UI Pass及RHI，未新增Renderer或每节点Canvas。清洁帧复用绘制产品，图形内容/精度cache有界；几何cache不持有GPU资源。Canvas最大轴倍率驱动精度，节点仿射倍率留C。

`graphics2d-asset.v1`经原Assembler/Asset Graph/RuntimePackageBuilder装配并生成hash/GUID索引；Runtime仅读取包内cooked数据，校验大小、hash、schema及ID/GUID，并向AUI Document共享只读registry。资源替换使绘制产品失效，未变描述仍命中几何。缺失资源和超限不会伪装成成功或留下半份图形。

扩展版本/SDK指纹同步v2；ABI1.6和UI_UPDATE_VERSION6不变。公开Game SDK及实际生成DLL已消费`UiExtensionPaintOutput::from_graphics`。节点generation承接paint发布生命周期，不暴露GPU handle或要求项目手写三角剖分/wire。正式合同已回填方案14.2，AA/色彩和缓存预算的限制均显式保留。

自审：上述改动均属于B的SDK、几何、资源或真实consumer边界；没有提前实施节点旋转/动画，没有改默认配置或重建塔防Release。旧控件mesh/Rect/Image/Text路径继续消费原绘制链。当前证据可以关闭B，不能关闭整个365。

## 2. 实际命令与结果

cwd `G:/gameEngin/rust`；Rust 1.96.0、Windows dev/test、既有Cargo target。真实GPU测试启用`real-wgpu`，Backend返回Vulkan，构造失败会使测试失败，不会跳过。没有取得其它平台、目标格式或性能预算资格。

| 命令 | 结果 |
| --- | --- |
| `cargo test -p project_runtime_sdk --lib --locked` | 52通过，测试0.05秒 |
| `cargo test -p project_game_sdk --lib --test aui_presentation_contract --locked` | 21+2通过，测试各0.00秒 |
| `cargo test -p engine_runtime --lib aui_graphics --locked` | 5通过，测试0.01秒 |
| `cargo test -p engine_runtime --lib ui_extension --locked` | 4通过，测试0.04秒；原`aui_extension`过滤匹配0项，已修施工命令，不算证据 |
| `cargo test -p project_authoring_execution --lib aui_graphics --locked` | cooked资产→真实包→Runtime注册→GUID/hash错误首因，1通过；最终编译8.59秒、测试0.06秒 |
| `cargo test -p project_authoring_execution --lib generated_runtime_glue --locked` | 初轮12通过、1夹具失败；保留12项身份未变证据，下面修复的实际DLL owner通过，共13项有效证据 |
| `cargo test -p project_authoring_execution --lib generated_runtime_glue_compiles_and_exposes_the_runtime_entry --locked -- --nocapture` | 修复夹具后1通过，实际DLL构建/加载/paint/生命周期总测试15.06秒；mode103的预期panic被ABI捕获，未使测试失败 |
| `cargo test -p engine_runtime --features real-wgpu --test aui_draw2d_gpu --test aui_extension_c4 --locked -- --nocapture` | GPU2+既有consumer5通过；最终编译11.95秒，测试0.80/0.00秒 |
| `cargo clippy -p project_runtime_sdk --all-targets --locked -- -D warnings` | 严格通过，6.11秒 |
| `cargo clippy -p engine_runtime --lib --features real-wgpu --locked` | 退出0，41.87秒；125项既有Runtime warning、依赖engine_input 2项，新增graphics/asset/cache模块无warning，不声称整crate零告警 |
| 修改文件的`rustfmt --check --edition 2021 --config skip_children=true` | 通过；最后仅夹具/测试变化，重新检查对应三文件通过 |

去重105项：52+21+2+5+4+1+13+2+5。失败尝试、重复编译与原型的2项GPU通过不重复累计。未跑全workspace、Local CI、安装或Release：当前声明只要求B源码/跨crate/DLL/GPU消费；它们不证明C/D/E，也无需为B重复363交付。

## 3. 真实GPU像素与缓存

真实链路：AuiRuntimePresenter → retained draw/composition → RuntimeRenderer → RHI plan → RealWgpuBackend → RGBA readback。参考输出为明确的像素断言，截图已人工查看；[最终截图](Gate-B/draw2d-gpu.png)为270×180、Unorm离屏目标。

- 同向双轮廓：EvenOdd中心黑色`[0,0,0,255]`、环内绿色；NonZero中心红色。
- 凹多边形内外、自交上下填充区域正确；二次和三次曲线中点为白色。
- 白色alpha0.5在黑底中心为125–130；小圆角图形RGBA`[0.25,0.5,1,0.6]`输出约`[38,77,153,255]`。小圆、亚像素圆、半像素位置和有界尖角确实出像素。
- 矩形裁剪外无漏画；裁剪内曲线fringe与未裁剪参考逐像素RGBA差≤1，确认颜色/alpha插值被真实消费。
- DPI映射1/1.5/2及返回1后，洞/颜色维持；7份图形、3精度档共21次构建，返回原档复用。六个静止帧新增剖分0、GPU顶点上传0。
- 映射序列共4次GPU整上传、222240字节，静止命中6次。该计数包括初始化/变更；不能表述成整个测试从未上传。几何cache打印时21构建、7命中、70698字节。
- 平移环图形不新增剖分，实际像素位置变化；同GUID更换内容只新增1份剖分（累计22），未变图形命中。registry删除后连续两帧保留缺失资源错误。
- 新建GPU backend消费旧CPU retained输出，整幅readback与首次完全相同。该证据证明backend重建；没有人为制造OS设备丢失。

截图SHA256：`32A0537BC969E73BEE3CA59E4DEFDB4AD420B745A16D24F7F5351D62C214F03A`。AA为有界外沿coverage，非解析面积积分或MSAA。这里没有采集AUI P95、完整CPU P95或跨引擎性能，不把剖分/上传计数换算成性能等价。

## 4. 首次失败、修复与证据复用

初次check发现AuiRenderReport初始化缺少新增字段、lyon PathIterator trait未导入；补齐直接consumer和trait后check通过。GPU夹具初编译发现style为Option、映射字段名/整数尺寸、font registry import和报告状态调用不匹配；修正真实API消费，没有改产品API迎合测试。后续资源替换测试误调用私有path辅助函数，改为匹配公开Draw2DItem，没有扩大SDK。

资产夹具第一首因：

```text
digest_scope_violation:
RuntimePackage v2 requires an explicit project InputMappingAsset;
use input.none for projects without input.
```

补显式input.none后第二首因：`documents_by_id["graphics-ui"]: no entry found for key`。保留的失败包`aui-manifest.json`为`{"documents":[],"schemaVersion":"runtime-aui-manifest.v1"}`：夹具只写文档、漏声明manifest。补原RuntimeAuiManifest入口后真实包加载/registry/损坏拒绝通过，不改Builder/Loader合同。

Generated Glue第一首因：mode100期望Applied，实际Faulted/`project_runtime.native_module_call_failed`。追到生成View有`speed: Slider`，运行包夹具只有label/button。补speed后Loader明确拒绝旧schema含C2控件：`output_staging_preload_failed / Invalid C2 layout at schema_version`。最终让夹具消费原`cook_aui_document`完成同一源版本规范化、四节点manifest；原mode100–104原子提交/拒绝及跨实例旧handle断言全部保留，实际DLL owner通过。其它12项测试未消费这处局部夹具改动，复用本轮通过证据，不重复生成DLL以刷数量。

没有通过增加allow、放宽像素/容量/原子性断言或修改无关owner取得通过。Clippy结构化输出复核未发现新增模块告警；既有债务不在本Gate修复范围。

## 5. 源码身份

相对仓库路径；SHA256为实际工作区字节，不能与HEAD或安装产物混用。

| 文件 | SHA256 |
| --- | --- |
| rust/Cargo.lock | C870DECC47180F4B73128028E30D29B20DB9D74D69F24C557194B908D8218C07 |
| rust/crates/engine_runtime/Cargo.toml | 5409715A8D6E50C40456098CE3BE20E63B41AB8B39B4C1E2A82F7E6D5137ADEF |
| rust/crates/project_runtime_sdk/src/aui_draw2d.rs | 8044789C843A6BB06F2F9CCAD3437CAAB92D92C66F463AC3A1604206B5EDAEA8 |
| rust/crates/project_runtime_sdk/src/aui_extension.rs | AC7D6DCF1082F5AE100AE9B951747C2D0BA6B1EEEA723F6B2BC0D9C2D34164EC |
| rust/crates/project_runtime_sdk/src/lib.rs | B438D8A6C008F67C0528AA82EC433FD0BB3E574405BBA68007A9980F36039FBA |
| rust/crates/project_game_sdk/src/lib.rs | 5749E77DD686E34F20BE56AB1FC52BC3C23B4D370D751F6E087168AE19C0DC18 |
| rust/crates/engine_runtime/src/aui_graphics.rs | 7C3182DDF2417AFE38C3C74F40FF74AAA66E078544976C853D9C52E472AF15D2 |
| rust/crates/engine_runtime/src/aui_graphics_asset.rs | 597CDE13F11DE182B083770217F04938035FE688206AF52ABC5CFEBF5DD1B858 |
| rust/crates/engine_runtime/src/aui_drawing_cache.rs | E5AF717B60DC8944DCD64C53351351653953D607B108B068E3EAD5672415AE9A |
| rust/crates/engine_runtime/src/aui.rs | D72A69F6A11E025BFFC663218FF394953EFB8A14A96E42C6BB0F34029B6706C4 |
| rust/crates/engine_runtime/src/runtime_renderer.rs | D187B1D5F1B0E200C93A865C6C07938B07C542C45F9F45846F422BA39B6CF57E |
| rust/crates/engine_runtime/src/runtime_package.rs | 5C61B81DD9FE818C83DB5F20A2905200615BD660D649ABE68EC2AD745B63B438 |
| rust/crates/engine_runtime/src/render_thread.rs | 0A3CBB94C71AC21D2E714B1A9263D6BE687057D2A785A66F659C46ACA03E5F58 |
| rust/crates/engine_runtime/src/lib.rs | 88EF65D4A24B4ED031F39A7B5DD850B433315EA0B3B98C808E74D42A326B8C29 |
| rust/crates/project_authoring_execution/src/neutral_assembler.rs | 1D127F7FB2AC18AD40F8E0B590AF2B92D50C8D43C8C5B7C54285C8A2380C5408 |
| rust/crates/project_authoring_execution/src/generated_runtime_glue.rs | 1B1D52727493E4FD59474AE698B571D9D8ACE838737AFFD719C408B95B6FAE75 |
| rust/crates/engine_runtime/tests/aui_draw2d_gpu.rs | 0D688036CC5A124AEA413320C6555FED8A038C13BEB2C79F12262DF3215BC9DF |

## 6. 成本、清理与下一块

本窗口10:22:33 CST开始，验证于约11:02完成，另含文档同步，总计约45分钟，小于3小时限制。首次引入lyon的owner编译66秒；跨crate/feature编译与Cargo锁等待出现78/128/191秒阶段，和实际测试执行时间分开，不归因成游戏UI开销。

成功资产夹具及Generated Glue夹具由原guard清理。一个失败资产夹具根`C:/Users/zenghaoran/AppData/Local/Temp/aife-365-graphics-9508-1791427969621760700`保存manifest首因后清理：核对其为Temp严格子目录、无reparse、owner进程已退出。Remove-Item被host policy拒绝，按施工skill允许的同路径原生Directory.Delete备选完成，Test-Path=false；没有残留需要用户清理的目录。仅删除测试临时包，没有删除项目、源或安装文件。未修改外部配置，无需恢复生产会话。

下一整块为C：节点仿射表现属性、共享最终矩阵、精确clip、局部命中和拖拽换算一致；随后D补间时钟/生命周期，E两普通项目、真实窗口、性能与独立Release。总施工文档保持当前，不归档整个365。
