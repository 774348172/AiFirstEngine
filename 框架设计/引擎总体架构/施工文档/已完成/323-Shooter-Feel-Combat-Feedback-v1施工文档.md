# 323 施工文档

状态：Gate A–C 完成并归档；2026-09-11 14:39 开始，15:44 收尾。具体证据与环境豁免见阶段记录。
正式方案：../../323-Shooter-Feel-Combat-Feedback-v1方案.md。

## 必要性与验证经济性自审

用户要求六项手感/反馈对齐。最小因果修改是项目 Controller、反馈模块、Scene/Assets/AUI；确认 Pointer 在 native adapter 被丢弃、AUI 缺少位置绑定，因此只补这两个既有 owner。无需新 schema 版本、工具、Provider 或架构层。无本轮外部审查要求。现有用户未提交修改全部保留。

复用 321 帧调度源码与 322 历史性能证据，不重复旧三档矩阵；新项目输入和效果变化使旧死亡固定 tick、旧可玩二进制与新玩法性能证据失效，必须重新取得实际交付证据。预计 90 分钟；超过估计先复核成本，2:45 后只收尾。

## Gate A：接口与项目实现

- 文件：project_runtime_native_adapter.rs、aui.rs、AUI editor binding consumer；samples/complex_shooter_project/RuntimeModule、Scene、Assets、AUI。
- Pointer 值不丢失、绑定位置能进入布局、非法值被拒绝：最小 owner 测试。
- 自动双发/冷却/出生位置、指针跟随/键盘优先/斜向限速、反馈预算/消退/重开：项目测试。
- 命令：cargo test --manifest-path samples/complex_shooter_project/RuntimeModule/Cargo.toml --locked；rust workspace 中受影响 owner 定向测试，按实际模块选 filter。修改文件 rustfmt；不全 workspace 重排。

## Gate B：实际交付

15:27 验证经济性复核：两次实际启动的 Computer Use 点击/拖动/关闭均无可观察效果，停止重复 UI 输入。补既有 NativePlayerInputScriptFrame 的可选 pointerPosition，owner 测试加真实 Player 两端目标截图，验证同一 RawInput -> SDK -> 项目位置 -> Present 链。无需新增工具或 Runner；OS 注入烟测作为环境未确认项明确豁免，不冒充已通过。该源码变化需要更新 Player，并仅更新消费此身份的交付证据。预计额外 20 分钟。

- 源码 Provider 的独立 MCP 进程 build 生成新包（安装版 Cooker 不认识新增 AUI 绑定，不能冒充已更新安装）；同进程现有 playtest/observe 运行实际场景。现有脚本只支持键盘，改用 Computer Use 原生指针交互验证坐标链，不扩展测试 API。
- 发现首因先回 owner 测试，不以整套构建作调试循环。必要优化版只用于新场景可玩交付对照，不扩大完整负载矩阵。

## Gate C：收尾

- 更新项目说明及验收场景，记录实际测试、二进制身份、截图、限制。
- 同步 README、49、54、施工与阶段入口，全部必要证据通过后归档本文件到已完成。
- 不替换安装/配置，不提交用户工作树，不删除未拥有目录。

## 终态

138 项相关 Rust 测试通过；最终 dev 与优化 Player 各 16 断言/4 捕获，observe 回读成功。左右指针真实窗口回放及机身像素位置检查通过。OS 输入自动化未确认，按正式方案的明确环境豁免处理，不能宣称人工鼠标验收通过。

源码/二进制/测试脚本身份分开记录；最后仅改指针测试注入时刻及项目说明，未改变已验收玩法资产。性能最新抽测 CPU 均值 5.84ms、P95 15.45ms，不能倒数为显示 FPS。记录：../../阶段完成记录/2026-09-11-323-Shooter-Feel-Combat-Feedback-v1/00-总览.md。
