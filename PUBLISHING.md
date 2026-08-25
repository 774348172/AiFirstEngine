# 发布到 GitHub 和 Gitee

本目录已经是独立的 V0.0.2 源码发布根目录，并继承 V0.0.1 Git 历史。不要把上级仓库的 `samples`、`target`、`evidence` 或其它 `release` 内容带进来。

## 初始化提交

```powershell
cd G:\gameEngin\release\AiFirstGameEngine-v0.0.2-source
git add .
git commit -m "release: AI First Game Engine v0.0.2"
git tag -a v0.0.2 -m "AI First Game Engine v0.0.2"
```

## 添加双远端

先在 GitHub 和 Gitee 各创建一个空仓库，不要勾选自动生成 README、LICENSE 或 `.gitignore`。然后执行：

```powershell
git remote add github https://github.com/<account>/<repository>.git
git remote set-url origin https://gitee.com/brother-b/ai-game-engine.git
```

## 推送分支和标签

```powershell
git push -u github main
git push github v0.0.2
git push origin main
git push origin v0.0.2
```

推荐在两个平台的 Release 页面使用 [RELEASE_NOTES.md](RELEASE_NOTES.md) 作为 V0.0.2 版本介绍。本发布不附加 zip、7z 或其它压缩包。

