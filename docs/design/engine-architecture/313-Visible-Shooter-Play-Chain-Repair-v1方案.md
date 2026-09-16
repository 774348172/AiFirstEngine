# 313 Visible Shooter Play Chain Repair v1

2026-09-10：用户已确认修复当前可见试玩链；这是既有Compiler/AUI合同的局部修复，不是新系统。

## 已确认首因

真实宿主导出的Game.exe显示全屏放大的HUD纹理；同一产物3帧截图复现。
neutral_assembler::cook_aui_node无条件使用AuiRect::stretch_full，丢失legacy rect/anchor。
Editor旧转换器有布局处理，但包含按节点名猜游戏绑定的历史逻辑，不能整体复制进入中立Compiler。
报告退出0/presented不能证明玩家、敌人、HUD实际可见；312无HUD夹具捕获不能覆盖此项目组合。

## 修复决定

保留现有owner，恢复legacy通用rect/anchor及明确style/visible字段；不增schema/crate/工具。
打飞机HUD的位置、文字样式与业务绑定在项目侧明确声明；优先采用已有canonical AUI v2。
真实窗口以当前项目重新准备的RuntimePackage和已链接同项目Player验证，可复用未变Player二进制。
若发现相机/输入等第二首因，先在当前施工记录其证据和最小修复范围，不扩为完整玩法开发。
不自动更换运行中MCP安装/config；源码修复与当前宿主加载的旧Provider身份分开报告。

## 验收

owner红灯测试 -> 最小修复 -> Compiler consumer回归 -> 同项目实际截图/输入验证 -> 可交互窗口。
不以无崩溃、draw count或有PNG替代画面检查。不承诺性能、完整游戏设计或其它平台资格。
