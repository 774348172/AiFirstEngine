# 324 Player 编译缓存复用修复

用户已授权按诊断建议修复。继承311，由既有ProjectPlayerArtifact owner完成，不新增工具或运行时层。

确认问题：编译目录兼容键包含整个引擎源码digest，引擎局部编辑会换target，丢失未变依赖缓存。修复将源码digest只留在最终artifact身份；编译目录继续按项目、SDK路径、manifest/lock、toolchain、target与环境隔离，源码新鲜度交给Cargo。旧目录不迁移或删除，首次切换仍可能冷编译。

开发构建继续使用既有稳定build root；独立优化比较使用已有G:/a323-opt/target，不为每次调整新建target。正式release profile扩展不在本轮范围。普通项目改动不要求编译Editor/Provider；本轮不改安装/config。

验证采用现有owner测试：真实Cargo冷/无变更/项目编辑/引擎依赖编辑回合，验证未变依赖fresh；生产兼容键回归证明源码变化不会切目录，配置变化仍隔离；最终artifact继续包含源码digest。不以小fixture耗时冒充完整打飞机构建耗时。
