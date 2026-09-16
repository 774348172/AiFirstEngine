# 339 Sprite 初始颜色编译保真与 Runtime Tint 消费修复 v1

状态：用户已确认采用方案 A；本轮完成方案自审，目标是修复已证实的编译丢字段问题，不扩建材质系统。

## 目标

将场景/Prefab 中 `SpriteRenderer2D.data.color` 的 `#RRGGBB` / `#RRGGBBAA` 保真编译到 `RuntimeSpriteRenderer2D.color`，由现有 Runtime Renderer 消费。动态项目写入 `color` 的路径保持兼容。

## 边界

- 修改 owner：`project_authoring_execution` 的场景实体装配；必要时补其定向测试。
- 消费者：已有 `RuntimeSpriteRenderer2D`、Hydration 转换和 Sprite 渲染，不改 ABI、不增工具、不改材质资产协议。
- 非目标：材质实例、发光/后处理、纹理烘焙、动画、音频、Android、安装宿主升级。
- 非法值必须在编译阶段失败并给出 `sprite_renderer2d.color_invalid`，不能静默白色回退。

## 参考判断

Unity SpriteRenderer.color 和 Godot CanvasItem/Sprite2D modulate 都是场景实例属性，导出后由 Renderer 使用；本方案只采纳“序列化字段不丢失”这一稳定边界，不复制完整材质架构。

## 自审

原因已由激光小屋交付截图确认：静态 Sprite 全白，编译器 `parse_entity` 明确写入 `color: None`。Runtime 已有 RGBA 字段和默认消费，最小反事实是只补解析。方案不引入新 owner、schema、工具或缓存层；颜色解析错误有定向红测试，消费者用已有 Runtime 类型覆盖。动态 Tint 行为由既有项目回归继续证明。

## 验收

1. `#RRGGBB` 得到 alpha 1，`#RRGGBBAA` 保留 alpha。
2. 非法长度、前缀或字符返回结构化编译错误。
3. RuntimePackage 序列化后保留数值颜色，Hydration 后 SpriteRenderer2D 使用相同值。
4. 激光小屋构建后静态颜色来自 Sprite 字段，不依赖新增染色纹理；动态开关/激光 Tint 仍可写入。
5. 受影响 crate 测试通过，未修改其它系统安装状态。
