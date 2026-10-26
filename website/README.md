# SmsPop 官网与文档

独立的 VitePress 静态网站，不参与 Tauri 打包。首页对应第三版设计，文档使用 Markdown。

## 本地开发

在仓库根目录运行：

```powershell
npm --prefix website ci
npm --prefix website run dev
```

打开终端显示的地址，默认是 `http://127.0.0.1:5174/sms-pop-rs/`。

```powershell
npm --prefix website run build
npm --prefix website run preview
```

构建结果在 `website/.vitepress/dist/`，已忽略，不提交到 Git。

## 内容维护

- 首页：`.vitepress/theme/Home.vue`。
- 官网与文档样式：`.vitepress/theme/style.css`。
- 导航和目录：`.vitepress/config.ts`。
- 用户文档：`guide/*.md`。
- 开发者文档：`development/*.md`，包含 Windows 更新与发版说明。
- 文档正文统一保存在本站目录内，直接编辑对应 Markdown 文件。
- 品牌图标：`public/app-icon.svg`，源文件是 `src/shared/app-icon.svg`。
- 曲面背景：`public/ribbon-light.png` 和 `ribbon-dark.png`。

不要把通知、令牌、联系方式等真实数据放入示例。

## 首次启用 GitHub Pages

1. 在仓库 Settings → Pages → Build and deployment 中，Source 选择 **GitHub Actions**。
2. 合并网站改动到 `main`，查看 **Website** 工作流。
3. 工作流构建成功后，将静态产物发布到 `github-pages` 环境。
4. 默认网址是 `https://fxaxg.github.io/sms-pop-rs/`。

PR 只构建网站，不部署。合并到 `main` 后构建并部署；也可以在 Actions 手动运行 Website。
不需要 `gh-pages` 分支，也不需要软件签名密钥或个人访问令牌。

如配置了环境部署审批，部署会遵循仓库的环境规则。

## 自定义域名

先在 Pages 设置绑定域名并配置 DNS，再将仓库 Actions 变量 `WEBSITE_BASE` 设置为 `/`。
未设置此变量时使用 `/sms-pop-rs/`。本地也可以通过环境变量 `WEBSITE_BASE` 覆盖该值。

## CI 范围

网站和文档都在 `website/` 内，变更只触发网站构建。软件 CI 保留轻量文件判断任务，仅在应用相关文件变化时启动 Windows 检查。
`Windows checks` 的名称保持不变；网站专属改动时该任务会被条件跳过。
混合修改或无法判断改动范围时，软件检查仍执行。

若设置分支保护，建议把始终返回状态的 **CI result** 设为必需检查。
Website 工作流有路径过滤，不应直接把可能不运行的 Build website 设为所有 PR 的必需检查。

VitePress 1.x 上游仍依赖 Vite 5；本站通过 `overrides` 使用已修复开发服务器已知问题的 Vite 6.4.5+，并验证构建和本地开发兼容性。升级 VitePress 时可重新评估是否移除此覆盖。
