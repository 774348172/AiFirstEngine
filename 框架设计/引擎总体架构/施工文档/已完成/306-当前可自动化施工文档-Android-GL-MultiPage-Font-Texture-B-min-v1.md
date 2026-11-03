# 306 Android GL Multi-page Font Texture B-min v1 施工文档

状态：Gate A-B 已完成，文档已归档。

## 施工目标与边界

修复 Android x86_64 GL Player 对多页 Bitmap/MSDF FontBundle 非零页面的错误采样，同时保持 Vulkan/其它
backend 的现有策略。唯一源码 owner 为 `rust/crates/engine_runtime/src/wgpu_backend.rs`。

未修改 Tower、runtime_player_winit、Android exporter、真实配置、production 二进制或既有 APK。

## 必要性与验证经济性审计

```text
requested failure：GL Android 中文显示为图集碎片
minimum causal change：GL 字体 page 从 array-layer view 改为独立 D2 texture
excluded：schema/cook/字体资源/WGPU 升级/完整 APK 或设备矩阵/Local CI
Gate A -> 非零页错误采样 -> 强制 GL 多页像素回归 -> red/green owner evidence
Gate B -> 默认 backend 行为回归 -> real-wgpu owner tests + fmt -> source completion
```

## Gate A：passed

强制 GL 的 2 Bitmap + 2 MSDF 多页像素回归在旧实现上真实变红：Bitmap page 1 读回
`[0, 0, 0, 255]`。修复后 GL 每页创建独立 D2 texture、上传到 `z=0`；其它 backend 保持数组纹理。
同一回归转绿，Bitmap/MSDF 的 page 0 与非零 page 均通过。

## Gate B：passed

```text
wgpu_backend::real::tests：16 passed
aui_text_：6 passed
font generation retirement：1 passed
cargo fmt --check -p engine_runtime：passed
```

## 施工自审

```text
方案一致性：通过；采用用户确认的 B-min
范围：通过；单一源码 owner，无 production/APK/Tower 变更
过量施工：通过；无 schema、migration、runner、transaction 或完整矩阵
红能力：通过；强制 GL 实际像素 readback
完成声明：sourceAvailable；旧 APK 未更新
```
