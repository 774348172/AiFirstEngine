# 321 Player帧调度修复与优化构建对照施工文档

2026-09-11已完成归档，source_integration_passed，当前槽释放。08:42开始，约20分钟完成。
用户已确认320后续建议，激活时当前/待执行无其它有效施工。
连续施工上限11:42，预计45–70分钟；阶段超过预计先复核成本，保留报告而不扩张验证矩阵。

## 必要性与验证经济性自审

- 已确认问题：320保留的五次CPU/进程统计与窗口调度源码证明每帧结束后追加整帧等待。
- 最小生产变更：runtime_player_winit/src/lib.rs，内部截止时间函数与唯一调用点；现有测试增加回归。
- A -> 调度等待相加风险 -> 无OS睡眠的确定性单测先红后绿，再owner测试/真实窗口feature编译。
- B -> 修复未进入实际项目Player/构建优化影响未量化 -> 既有Compiler构建，320相同720帧输入两次；
  生成Host的release比较两次；8轮重开场景64断言验证语义未变。所有运行串行，测量期间无编译。
- 复用320机器与输入方法、319项目玩法测试；玩法未改不重复项目单测；新二进制需真实重开证据。
- 不做GPU时间/显示FPS/无泄漏/发布资格宣称，不新增工具或改宿主安装。优化版独立目录不覆盖旧交付。
- 外部审查：当前未收到321外部审查；320已确认局部缺陷，无需扩展上游权威架构。
- 用户确认已覆盖修复及优化构建对照；不把内部Gate变成用户/Agent新增日常流程。

## Gate A：源码修复

保存修改前源码身份，保留工作区既有修改。先抽出原有截止时间运算，并由真实调用消费，
写测试验证快帧只等余量、慢帧不额外睡眠、暂停后无追赶。运行定向测试捕获失败后修改运算。
命令：cargo test -p runtime_player_winit --lib frame_deadline --locked；
终态：cargo test -p runtime_player_winit --lib --features real-window --locked；scoped rustfmt检查。
真实窗口测试若要求显式opt-in，仅运行确定性默认项；实际项目验证在B执行，不盲目扩大窗口矩阵。

## Gate B：交付与对照

使用现有engine_project_build得到新Windows dev项目Player，保存工具与构建报告、源码/二进制身份。
复用Tests/Performance/run-measurement.ps1，720帧/120预热/600采样/Off，无捕获：dev两次；
同一Compiler生成Host用cargo build --release --locked --offline构建，在run-owned目录复制未改包配对，
release两次；dev/release各复验重开3362tick/64断言，两个编译profile分别验证。
320测量在凌晨，当前为上午；追加旧319 Player同输入一次，排除不同时段机器状态对修复效果的混淆。
正常对照相同输入，串行，每次<=125秒。
优化编译预计<=20分钟；若工具链/组成不满足则报告真实限制，不造手写Host或替换安装产物。
复验CPU及进程耗时，明确CPU计时边界；不为FPS比较新增协议或常驻采样。

## 收尾

保存完整失败/复验报告、profile/源/产物身份及阶段记录；测量进程正常结束，保留证据与比较目录。
仅完成A/B要求后同步README、49、54、施工/阶段索引，并归档本施工文档。

A终态：旧运算两项回归均失败（短帧16.667ms而非6.667ms，慢帧仍追加等待）；
修复后real-window owner 48 passed/3 local-only ignored，编译92秒、测试0.08秒；rustfmt通过。
未更改固定模拟步长或游戏逻辑，进入B构建与实际对照。

B终态：当前宿主engine-op-mcp-process-12864-16交付已重建，CPU均值15.68/14.72ms，
720帧15.185/14.876秒；本轮旧319 Player同输入30.714秒（CPU19.61ms，存在机器时段波动）。
独立Cargo release优化编译109秒，CPU均值3.97/4.02ms，P95为5.99/6.10ms，
720帧13.616/13.217秒。不是显示FPS或正式发布资格。
两个profile各3362tick、8轮死亡重开、64断言passed，logicStatus=ok/exit0，Windows Job正常释放。
首次artifactRoot的Host锁不包含实际Host依赖，--locked在编译前失败；核验清单后取compileWorkspace实际锁，
本轮副本与实际dev生成输入逐字节一致，再离线锁定编译通过。保留首因，不改安装或共享锁。
原始证据G:/a321-0842保留；完整数据与身份见
[321阶段记录](../../阶段完成记录/2026-09-11-321-Player-Frame-Pacing-Repair-Optimized-Comparison-v1/00-总览.md)。
整体复核完成，源/测试/交付范围明确；无游戏失败，无新工具，未做三档release负载或内存泄漏资格。
README/49/54/施工与阶段索引同步，当前/待执行无有效施工，不自动启动后续优化。
