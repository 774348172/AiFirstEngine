# 306 Android GL Multi-page Font Texture B-min v1 方案

状态：方案已确认，进入最小施工。

## 问题与证据

Tower 的旧 API 35/Vulkan APK 中文正常；新 API 35/GL 与 API 37/GL 都把中文显示成字体图集碎片。
两代 APK 中 FontBundle manifest/page、AUI、Scene 与 RuntimeModule AOT 均字节一致，因此问题不属于
字体源、cook、Tower cache 或 Android API 版本。

当前 `RealWgpuBackend::register_font_texture_arrays` 把同一 render mode 的页面放入二维 array texture，
再把每个 array layer 暴露为普通 `D2` view。GL/GLES 的非零 layer 路径是唯一同时解释上述差异的 owner。

## B-min

```text
GL / GLES：每个 Bitmap/MSDF page 使用独立 2D texture，上传 origin.z=0，直接绑定该页 view
Vulkan / 其它 backend：保持现有 array texture + per-layer view
```

不修改 FontBundle schema、cook、AUI、Tower、Android exporter、WGPU 版本、ARM64/Vulkan backend policy。
显存像素量与上传字节不变；GL 只增加少量 texture 对象，既有每页 handle/bind group 和绘制切换不变。

## 验证

1. owner 回归使用至少两张 Bitmap 与两张 MSDF 页面，页面内容可区分，并覆盖非零页面。
2. GL 可用时强制 GL 做实际像素 readback；不可用时保留明确 skipped 原因，不伪装设备级资格。
3. 运行 `engine_runtime` 受影响字体测试和格式检查。

本方案只证明源码 owner 修复，不自动声明 APK、设备、production Editor 或 Tower 真机闭环。
