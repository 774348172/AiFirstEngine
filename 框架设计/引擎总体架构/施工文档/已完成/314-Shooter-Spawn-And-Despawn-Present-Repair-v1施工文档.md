# 314 发射与销毁呈现修复施工

状态completed；用户授权“先修改这两处”。2026-09-10 14:02—14:17 CST，约15分钟；未触及三小时限制。
唯一施工；313归档不变，当前目录旧254报告不是active计划，待执行仅README。无新外部审查。

## 必要性与验证经济性

313保留截图/trace已证明移动后子弹仍固定位置、实体销毁成功但图像残留。
涉及项目RuntimeModule、project_game_sdk/project_runtime_sdk既有mutation、engine_runtime command/adapter/render_extract与直接consumer测试。
排除新工具、性能、资源美化、Tower、Android、Local CI、安装/config升级、全workspace矩阵。
测试证明既有spawn请求丢弃position，最终仅补可选根position而不是完整Transform，保留Prefab旋转/缩放；不新增schema版本、事务或生成框架。
复用313输入脚本/场景和Cargo缓存；改变Runtime/项目源码使旧Player证据失效，需正常Compiler重建交付。
预算30-90分钟，单诊断分支超过15分钟无有效失败信号则收敛；超过3小时按skill停止。

## Gate

A：渲染销毁owner红灯->修复；cargo test -p engine_runtime --lib render_extract --locked。
B：发射位置project/SDK/adapter/command红灯->修复；在既有测试内覆盖默认Prefab兼容及显式position，执行受影响owner/consumer tests；实际命令随owner定位记录。
C：受影响整体回归与fmt；当前宿主正常prepare/run生成同项目新Player，真实窗口有界输入/截图/trace确认移动后发射和击杀后消失；打开新交互窗口。
D：记录终态、证据与局限，同步49/54及入口并归档。开发dirty身份不称clean发布资格。

自审通过：范围仅两缺陷；延迟结构变化保留，玩法只在项目侧。外部诊断根C:/a314-1402/evidence，不删除313目录，不触碰旧用户窗口。

## 完成结果与失败记录

A：render_extract在World释放entity映射后提前返回，未发RemoveProxy。现在从现存proxy取回runtime identity完成清理。owner复现16通过1失败（编译27.24秒、测试0秒），修复后17通过（编译7.47秒、测试0秒）；包含在最终suite，不重复计数。
B：SDK wire位置红灯左Null、右[3.5,-3.3,0]（编译2.39秒）；修复仅增加serde default/skip-none的position、writer instantiate_prefab_at、session/rule adapter到既有command的透传以及实例化后投影前写入根位置。无位置旧请求行为不变。项目发射读取当前玩家位置并加y=1.2。
测试修正：首轮SDK断言用JSON f64与f32 -3.3精确比较失败，改用二进制可精确表示的-3.25；command consumer测试曾用不存在的Vec3::zero导致编译失败，改显式零向量。均为测试问题，原日志保留。
最终整体命令：`cargo test -p engine_runtime -p project_runtime_sdk -p project_game_sdk --lib --locked`，571+7+6=584通过、0失败（最终编译1.56秒，Runtime测试0.61秒）；project命令`cargo test --manifest-path samples/complex_shooter_project/RuntimeModule/Cargo.toml --target-dir G:/gameEngin/rust/target --locked`，3通过（编译4.05秒、测试0秒）。合计587项，定向测试不另计。
受影响crate和project cargo fmt --check与scoped git diff --check通过。没有全workspace/Editor/Android/Local CI：未修改对应产品路径，当前真实Player构建与运行覆盖实际consumer组合。

C：真实已安装宿主engine_runtime_run生成engine-op-mcp-process-12864-4，新Player SHA256 0990D4ACDD5E94B89C8090AA91ECD140A1CADD64A55C8E2E55A76C75229B9131，非313旧Player。host-run.json保留完整结果；3帧headless正常退出。
真实Windows窗口分别8/60帧，用move-then-fire.json：D从第2帧到第6帧松开，7帧Space、8帧松开。玩家x=0.5，子弹首次运动前位置[0.5,-3.3,0]，scale仍[0.35,0.85,1]；8帧截图确认飞机前方发射。23帧子弹和entity-enemy-a成功销毁，60帧截图只剩一名敌人、SCORE000100、敌人数1，无残影，failedRecordCount=0。没有用renderProxyCount等初始化摘要代替最终图像。
零参数交互窗口已打开，PID904、句柄264720、AI First Engine Player、Responding=true；留给用户试玩。旧窗口/交付未删除。

D：同步入口并归档，当前/待执行为空。证据C:/a314-1402/evidence全部保留，包含red/green/final日志、输入脚本、host-run.json、两张PNG、两份trace报告和stdout/stderr。未建立新的可清理temp目录，未重试313清理。

## 身份与声明边界

沿用dirty工作树，保留既有未提交与未跟踪变更，不声明clean exact-commit发布资格。正式工具数9不变。
render_extract SHA256 2E88BB738277617C3370125969340317F4A40C29230DB2C14F86C4B0B9CE586B；gameplay_command 93FB3FD9EDA38A3F0509F1208BB24BE0615CD4160506ED263B2180CA63FAB28E；native adapter 0EAC4E2D50FAE5B1D99883F0AA147961DD0B7E7754803F2A69F3287C8CE9EA60；runtime SDK 49AFE0C2A7ECC728234C37379C4728C5C0CAC03C158E4176F624DBF49E89B852；game SDK E8B7997EA8F04F593D53459125945988336E8848D2FB3550E816D0069CE2CAE1；项目RuntimeModule DDBC0EAA6D136ABCCB9CD997CF774D65A71CB3BC04CCC6C4D512CC27F2CFCC0C。
deliveryRef engine-delivery:sha256:034179c60db95a09bab408175d6d3a3d792937972ea309d4d5c51a8120cc9917；projectRevision sha256:092ff6d34c001ad120e48e9ead57ffc0f12416d0201882e1793876d2f0251d07。
交付samples/complex_shooter_project/Library/EngineTools/Deliveries/engine-op-mcp-process-12864-4/Run/Windows/dev。
安装Provider保持F09231D1287FE3CF8EA8C21A6030858332D9680229BBD32E68A557519FA7D662，config保持8AC885527781474BF08D50F4E7A232927B848B49804ED2A9CE3ACBF3566D89B6。Provider负责构建，新项目Player含修复；不是安装Provider/Editor升级。
本次两缺陷已闭合，不代表完整小游戏设计、所有玩法、所有平台或性能验收完成。
