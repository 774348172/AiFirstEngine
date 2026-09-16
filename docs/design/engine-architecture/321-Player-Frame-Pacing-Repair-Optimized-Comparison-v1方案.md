# 321 Player帧调度修复与优化构建对照 v1

2026-09-11已完成，source_integration_passed；用户确认按320下一步建议施工。
证据见[321阶段记录](阶段完成记录/2026-09-11-321-Player-Frame-Pacing-Repair-Optimized-Comparison-v1/00-总览.md)。
范围是已确认的局部调度缺陷及同输入对照，不重开架构设计。

当前RedrawRequested在present_next_frame结束后设置now+16.667ms，导致处理时间与整帧等待相加。
最小修复：在处理前取frame_started，下一截止时间为max(frame_started+16.667ms, frame_finished)。
快帧只等待余量，超时帧立即具备重绘资格；不累计补帧债务，不改固定模拟步长、输入或生命周期合同。
保留既有Fifo/Vulkan与事件循环，没有新增工具、协议、运行时常驻诊断或独立调度系统。

确定性测试验证短帧余量、超时帧不追加等待以及长暂停后无补帧追赶。
复用320固定输入/测量器，当前源码构建新的项目Player，重复正常战斗并复验既有重开场景。
同一Compiler生成的Host使用Cargo release编译，在独立比较目录配合同一RuntimePackage测量，
明确这是优化编译实验，不等于新增正式windows-release产品或发布资格；不修改生成代码。
CPU帧计时不含调度等待；进程总耗时只能作为同输入端到端对照，不直接冒充显示FPS。

不在本轮扩展update内部热点优化、内存泄漏修复、Android验证、安装Provider/config替换或完整性能矩阵。
