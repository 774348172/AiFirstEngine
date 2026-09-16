# 320 Shooter Performance / Restart Baseline v1

2026-09-11已完成，状态measurement_completed；基线建立，不代表性能达标或无泄漏资格。
证据见[320阶段记录](阶段完成记录/2026-09-11-320-Shooter-Performance-Restart-Baseline-v1/00-总览.md)。
用户确认建立性能与重开稳定性基线；319玩法已由用户人工试玩认可。
测量当前319 Windows dev Player，1280x720/Vulkan/Fifo，固定输入、预热后采样，
无截图、RuntimeReport Off；记录CPU frame/update/render-submit/present-wait均值及P95/P99。
60FPS/16.67ms仅为候选比较目标。当前产物为Cargo opt-level0，不声称发布优化构建或GPU时间。

三组测量：正常战斗（同一输入重复两次检查噪声）；同进程多轮真实死亡/Enter重开
（语义断言验证每轮reset，进程PrivateBytes/WorkingSet定期采样）；隔离项目固定敌机/弹体分档
（从项目Scene/Prefab资产生成合成负载，Compiler生成正式RuntimePackage，禁止手改运行包）。
合成负载关闭自身移动/发射/死亡，仅测固定人口下更新/碰撞/渲染成本，不冒充正常生存玩法。

复用现有CLI/Compiler/MCP；只添加项目测试输入、一次性运行脚本与记录，不新增引擎工具。
本轮不改生产玩法/引擎、不优化瓶颈、不改安装/config。实时渲染对象数与GPU内存暂无采样接口，
明确列为证据边界；以已有业务数量重置断言和进程内存趋势检验有界重开，不宣称完全无泄漏。
若基础采样或场景不符合要求，先修测量输入；成本超预算先缩小诊断，不能用假指标替代缺失指标。
