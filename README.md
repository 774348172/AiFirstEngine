# AI First Game Engine V0.0.7 / 20261031-r1

按2026年10月31日主线真实源码冻结，包含376最终通用生成网格、静态碰撞与项目字节存储，以及371、374已归档Windows功能实现。体素、生成算法与玩法由项目提供。

源码身份见SOURCE-SNAPSHOT.json及独立Git标签；所有编译期直接资源列入COMPILE-TIME-INPUTS.json。使用Rust1.96.0、MSVC14.44、Windows SDK及合同匹配的Jolt依赖；工具链与缓存不随包。运行tools/build-provider.ps1 -Spine构建Release四产物，tools/start-provider.ps1连接MCP。

发布时本目录仅冻结源码；后续编译、安装、生产链验收见主工作区的V0.0.7安装完成记录，不将旧二进制证据转授新产物。游戏继续V0.0.5，升级需明确选择。
