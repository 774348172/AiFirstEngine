# 发布到 GitHub 和 Gitee

本目录已经是独立的 V0.0.1 源码发布根目录，不需要再复制上级仓库内容，也不要把上级仓库的 `.git`、`samples`、`target`、`evidence` 或 `release` 目录带进来。

## 初始化提交

```powershell
cd G:\gameEngin\release\AiFirstGameEngine-v0.0.1-source
git init -b main
git add .
git commit -m "release: AI First Game Engine v0.0.1"
git tag -a v0.0.1 -m "AI First Game Engine v0.0.1"
```

## 添加双远端

先在 GitHub 和 Gitee 各创建一个空仓库，不要勾选自动生成 README、LICENSE 或 `.gitignore`。然后执行：

```powershell
git remote add github https://github.com/<account>/<repository>.git
git remote add gitee https://gitee.com/<account>/<repository>.git
```

## 推送分支和标签

```powershell
git push -u github main
git push github v0.0.1
git push -u gitee main
git push gitee v0.0.1
```

推荐在两个平台的 Release 页面使用 [RELEASE_NOTES.md](RELEASE_NOTES.md) 作为 V0.0.1 版本介绍。本发布不附加 zip、7z 或其它压缩包。

