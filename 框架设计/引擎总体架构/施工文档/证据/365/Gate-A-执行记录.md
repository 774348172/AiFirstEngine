# 365 Gate A：公共创作与数学合同执行记录

日期：2026-10-08。结果：`a_sdk_authoring_contracts_complete_with_existing_lint_exemption`。只证明SDK公共合同与直接消费者；Runtime绘制/表现/播放、GPU/Release与安装资格尚未取得。

## 1. 实际范围

新增`aui_draw2d`：有界PathBuilder、Move/Line/Quadratic/Cubic/Arc/Close、fill/stroke/cap/join、Graphics2D版本和聚合预算，失败不修改合法前缀。新增`aui_presentation`：仿射/pivot/parent*local、镜像/奇异判定、较高精度中间运算、实际f32矩阵一致性及屏幕曲线误差换算。新增`aui_animation`：typed目标、序列/并行区间冲突、循环/反向/yoyo描述和时长校验、缓动/插值与帧索引时钟数学。

两SDK入口公开模块，Game SDK符号索引明确为创作合同；两个公开消费测试不引用engine internals。未修改旧UiProperty、UI_UPDATE_VERSION6、UI_EXTENSION_VERSION1、ABI1.6或Generated Glue，未增加依赖/更改Cargo.lock。

## 2. 命令、结果与去重

cwd `G:/gameEngin/rust`；Windows本机、Cargo debug默认feature、既有target；脏工作区增量施工，不是clean-commit资格。

| 最终验证 | 实际结果 |
| --- | --- |
| `cargo test -p project_runtime_sdk --lib -p project_game_sdk --locked` | SDK50 + Game SDK21通过，0失败；编译3.04秒，测试0.06/0.00秒 |
| `cargo test -p project_game_sdk --test aui_presentation_contract --test three_d_sdk_a --locked` | 新consumer2 + 既有consumer3通过，0失败；编译2.85秒，测试各0.00秒 |
| `cargo clippy -p project_runtime_sdk --all-targets --locked -- -D warnings` | 通过，0warning，1.37秒 |
| `cargo clippy -p project_game_sdk --all-targets --locked` | 退出0；只有下面记录的1项既有warning，3.29秒 |
| 新增3模块、两入口与新consumer的`rustfmt --check --edition 2021 --config skip_children=true` | 通过，没有递归格式化其它模块 |

最终去重76项（新增24项：SDK22 + 公开consumer2）。首轮70项不与最终76叠加计数；补容量/周期/序列化边界和SDK符号索引后复验是有效变更后的验证。首次owner编译23.60秒，与最终增量编译分开记录。

覆盖：路径非法起点、Close后Move、有限数值、退化/空路径、路径/聚合/项容量和序列化校验；pivot/镜像往返、祖先旋转、正常极小尺度/奇异变换、overflow、shear/DPI精度；串行同属性、并行交叉/空隙/零时长、generation、循环/时间溢出、深度/容器/叶预算、类型/版本、暂停/重复和倒序帧/长帧clamp/小数速度/失败原子性。

## 3. 首次lint失败与精确豁免

最初命令`cargo clippy -p project_runtime_sdk -p project_game_sdk --all-targets --locked -- -D warnings`退出1，首因：

```text
error: the loop variable `axis` is used to index `delta`
  --> crates/project_game_sdk/src/kinematic2d.rs:47:17
47 |     for axis in 0..2 {
   = clippy::needless_range_loop
   = -D warnings implies -D clippy::needless-range-loop
```

分类：已有未修改kinematic2d owner lint。该文件不属本Gate因果修改集合，没有为过lint改运动算法或新增allow。豁免仅限此告警；SDK仍严格-D warnings，Game SDK完整all-target lint保留告警且无其它warning。新告警不适用此豁免。

## 4. 源码身份

SHA256绑定当前实际字节，不绑定HEAD；编辑这些消费者时对应证据失效。

| 文件（相对rust） | SHA256 |
| --- | --- |
| crates/project_runtime_sdk/src/aui_draw2d.rs | BCDFD33D43793B7A4E827A349C8271DD42195D6463E55B1E9380C9A43852D045 |
| crates/project_runtime_sdk/src/aui_presentation.rs | DC0EC072CC6CBC683C328C23BBA351D61027E3CE731BA25CFFED7F45CE42A559 |
| crates/project_runtime_sdk/src/aui_animation.rs | 525983430892CF4F7F64C51331600017540323F5F0CC6C4859780AE411352716 |
| crates/project_runtime_sdk/src/lib.rs | E06D9D09F49DD1D74F96EDDC515765704CE42BEECA6665FEDC46C82AD70B00BB |
| crates/project_game_sdk/src/lib.rs | BBA6BF558B04CB143E71EDB156C7305068DD9D86CBD1E66462B89D671691EAE5 |
| crates/project_game_sdk/tests/aui_presentation_contract.rs | 1D0874C38DCB005028FDD37B45C1BF9D46AC7DA0F65983AA8A78C9FF5D0ADE7E |

协议/Glue未修改：ABI源码`164B3575405239A30DDC6E2DF8D190EDD8DC6689ED4E3E67664CFFEF9932F849`；aui_wire`786101653F8F451284ED010786B95F9DC5A580016F49730EF9372814C21AACC2`；aui_extension`88FBF9D398C1B28886995610F2339D31E58E5177CBAE89D9F0770CE33ED1AAA0`；Generated Glue`5F05993696DC89914CE713D89156BDDB9A5FF5C1BDE65ADF3D53A3D724121F1D`。

## 5. 时间与资格边界

已记录核对/实现/验证窗口00:49:46–01:00:11 UTC（北京时间08:49:46–09:00:11），约10分25秒，文档整理另计。未触及3小时限制；未改配置/安装，未启动真实游戏或临时角色进程，未删除文件；Cargo产物保留原target。

本Gate未改Runtime/Renderer，故不运行GPU/真实窗口/全Runtime suite，也不声称新增效果已在游戏中生效。B接通生产绘制/资产/AA，C接通表现/clip/hit，D接通播放/owner/lifetime，E闭合两项目和Release。总施工继续占当前槽，不归档整个365。
