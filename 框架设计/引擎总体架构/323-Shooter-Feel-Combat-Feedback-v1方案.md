# 323 Shooter Feel / Combat Feedback v1

用户已确认实施打飞机操作手感与战斗反馈对齐。范围：自动双发、指针跟随、动态尾焰、分层星空、爆炸粒子、AUI 得分飘字。参照用户提供的 TapTapMaker main.lua 的交互行为，独立实现，不复制其资源或引入新 Tool。

项目 Rust Module 承担玩法与有界反馈状态；Scene 提供固定视觉槽，AUI 提供四个得分槽。双发间隔 0.14 秒，键盘斜向归一化，键盘优先于指针；指针使用 Player 已映射的目标画布坐标，通过项目相机尺寸换算，指数跟随并限制边界。出生点读取移动提交后的玩家位置。死亡停火、Enter 重开清空反馈。

必要底座修复：Native Adapter 目前丢弃 Pointer action，将位置传入已有 axis2 通道，phase 标明 pointer，保留 ABI；AUI 增加 RectOffsetX / RectOffsetY / TextColor 只读绑定，使飘字经正式布局和文字渲染链运行。项目不直接访问 Renderer。目标画布采用项目既有 1280×720；不宣称任意相机/画布自动适配。

反馈固定预算：24 粒子槽、4 飘字槽、2 尾焰、2 透明前景星空平面，不随击杀无限增加实体。粒子复用有界槽，溢出覆盖最旧效果。星空与粒子在死亡后继续消退，重开清空战斗反馈。现有 3HP、无敌、计分和敌人补充保持。

不做 Boss、道具、音频、通用粒子编辑器、新工具或安装配置更新。322 性能结果只代表旧固定负载；本轮另做实际新场景 smoke，不继承完整性能资格。

实现复核：Native Adapter 的 Rule 字段/Transform 写入立即生效，只有结构变化在阶段末提交。枪口与尾焰直接读取前序移动 Rule 更新后的位置，不重复预测移动。UI 保留已注册 active binding paths，以支持 Known binding set。

验收：项目行为测试、受影响 Adapter/AUI owner 测试、Compiler 构建及实际 Player playtest/observe、截图检查。Computer Use 在两种启动方式下点击、拖动、关闭均未产生可观察效果，不能据此判定游戏鼠标通过。为保留可重复的指针完整链证据，在既有 native-player-input-script.v1 增加可选 pointerPosition（目标画布整数像素），复用现有 RawInputEvent、InputResolver、SDK 和窗口渲染，不增加 Tool 或独立 Runner。旧键盘脚本不变。实际 OS 输入注入的 UI smoke 列为环境未确认，不能写成已通过；原生回放与 DPI 映射 owner 测试承担自动验收。

AUI 新枚举需要本轮源码 Provider 构建器；使用隔离进程，不替换宿主安装/config，交付后可用现有 Player 入口启动。
