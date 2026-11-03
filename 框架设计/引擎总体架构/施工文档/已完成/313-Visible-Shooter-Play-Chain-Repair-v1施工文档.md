# 313 可见试玩链修复施工

状态：completed；开始2026-09-10 11:39 CST，约20分钟内完成。14:24停止新实现/验证，14:39硬停止。
授权：用户“开始下一步修复这条可见试玩链”。正式依据：../../313-Visible-Shooter-Play-Chain-Repair-v1方案.md。
执行时唯一计划，完成后释放施工槽；不重开311/312，不修改真实MCP配置或生产Editor，不执行Local CI/Tower/Android。

## 必要性与自审

已复现同产物全屏HUD覆盖，Compiler丢失legacy布局；修复既有语义，无新设计选择。
允许文件：neutral_assembler.rs及其owner/Compiler接口测试、打飞机AUI/hud.aui.json；
必要时既有project owner/consumer中由失败证明确需修复的局部范围先记录再实施。
HEAD和dirty源码分别记录，不撤销既有变化。预算60-100分钟，构建成本明显超出即复核。
不机械重跑312或全workspace；复用Player源码/二进制未变部分，只重新准备实际受影响项目包。

## Gates

1. A：legacy布局转换红灯及修复。cargo test -p project_authoring_execution --lib legacy_aui --locked。
2. B：项目HUD明确布局/绑定；cargo test -p project_authoring_execution --lib --test game_project_compiler_interface --locked；cargo fmt -p project_authoring_execution -- --check。
3. C：通过当前已安装Provider消费canonical v2 HUD准备新交付；源码legacy转换修复由A/B单独验证，不替换安装。既有Player窗口3帧截图，检查真实纹理/字形/布局；有界输入验证移动/射击，再打开修复后的交互窗口。
4. D：终态记录、证据、入口同步、归档。安装Provider保持原身份，未资格化的宿主修复状态不报通过。

外部诊断根C:/a313-1139；仅temp子项可清理，evidence保留。旧用户窗口不强行终止。
自审：通过，范围为已确认修复；无新外部审查输入；先捕获红灯后修改实现。

## 终态证据

A通过：修复前legacy定向测试1通过2失败（编译64秒），实际stretch全屏与预期有限rect不符；修复后3通过（编译5.87秒）。首次失败原始输出在本施工会话，旧图保留于C:/Users/ZENGHA~1/AppData/Local/Temp/aife-visible-audit-1717fe8446964e8d95a0507a3104fea1/frame.png。
B通过：最终单次suite为59 lib + 20 interface = 79通过、0失败；编译32.74秒，测试18.80+20.69秒。已包含3项A测试，不重复计数。fmt及受影响diff检查通过。
C通过：真实宿主engine_runtime_run得到engine-op-mcp-process-12864-3；3帧真实Windows/Vulkan截图可见飞机、敌人、字形与有限HUD。40帧输入trace中玩家x从0变3.5，2次prefab-player-bullet实例化，failedRecordCount=0；截图SCORE从000000变000100，敌人数2变1。截图和trace共同支持最小可见/移动/发射链。
交互Game.exe零参数启动，PID18060，窗口AI First Engine Player，句柄394470且Responding=true；保留给用户。启动立即调用WaitForInputIdle曾报无GUI，稍后原进程的真实窗口核验成功，无需重启/重建。
D：阶段记录与入口同步并归档。证据根C:/a313-1139/evidence包含compiler-tests.log、initial-fixed.png/report、move-fire.json/png/report和进程stdout/stderr；不将此开发工作树验证写成clean exact-commit发布资格。

## 身份、复用与边界

HEAD acaffa8cb95bbb54d093bbf45835a59315c82da5，原有dirty/untracked源码保留；本次修改Compiler legacy转换、owner/consumer测试及项目HUD，不改Player/RuntimeModule。
Compiler源SHA256 B0159C388774FDE33DEF34F628E76F5271E69424F3C9150A1F5ED458EE445C45；接口测试FCFF2DE283F2208AEBF4C5D88CA55306E61DA275C9EF9B62AB0C0DE8CCF61604；HUD 846F6B7961103097FDD57259CCA4A3AC9CC582EE23FF11F399658ED10EE29D93。
复用已链接同项目RuntimeModule且含real-window的Game.exe，SHA256 92B9012F0C6FDDED9B51F6AD7559273DD92357F32B2A1EF83B30D670EAEBC0C7。新RuntimePackage因HUD改变而重装配，不能复用旧截图。
安装Provider SHA256 F09231D1287FE3CF8EA8C21A6030858332D9680229BBD32E68A557519FA7D662未替换；它支持canonical v2，但尚不含本次legacy源码修复。
当前config只读SHA256 8AC885527781474BF08D50F4E7A232927B848B49804ED2A9CE3ACBF3566D89B6，最后写入11:05，早于本次11:39施工；不沿用旧接入记录的config哈希，也不声称本次有配置事务。
deliveryRef engine-delivery:sha256:948e5941cf42034d0e58b2960137985d1f31e811fa56774ca08a457365d6182c；projectRevision sha256:ed5541f6feae02a44756ef0f284b770c42781ecb4a30a13df2d8f6b9e323002c。
交付目录samples/complex_shooter_project/Library/EngineTools/Deliveries/engine-op-mcp-process-12864-3/Run/Windows/dev。旧交付与旧窗口不删除。
未做全workspace/Local CI/性能/Android/安装升级：不属于本次局部可见链声明。
残留：美术是低分辨率验证资源；射击沿用Prefab固定出生位置，没有跟随移动后飞机；截图敌人数降到1但仍见两个敌人形状，实体销毁到呈现的同步需下一次定向诊断。313不据此宣称完整玩法、完整击杀视觉闭环或性能验收完成。
清理：C:/a313-1139/temp的Remove-Item调用被宿主策略在进程启动前拒绝，未删除任何内容，记retained_by_host_policy；证据、交付和交互窗口保留。未为清理申请额外授权或更改策略。
