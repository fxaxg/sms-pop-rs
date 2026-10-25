# Windows 更新与发版

SmsPop 使用 Tauri 官方 Updater，启动 15 秒后检查 GitHub Releases 的
`latest.json`。仅稳定 Release 用于正式更新；开发构建禁用更新。
每次检查超时 20 秒；失败后依次等待 30 秒、1 分钟、2 分钟、5 分钟，
此后每 15 分钟继续重试，直到检查成功。手动检查会取代等待中的重试；
检查成功后本次运行不再定期检查，手动检查再次失败会重新开始退避。
重试由后端管理，隐藏到托盘不影响；每次检查新建客户端并遵循系统代理。
重试计划仅在当前进程内保存，退出后重新开始。
下载由 Rust 持有，关闭设置窗口或切换页面不会中断；下载包保存在内存，
退出应用后需重新下载。只有签名验证通过才允许安装。
发现新版本时，设置窗口左下角显示更新提示，点击后在模态框查看更新说明、
下载及确认安装；关于页保留手动检查入口。自动检查不弹窗或唤起隐藏窗口。
主动检查发现新版会打开模态框。关闭模态框不会中断下载，下载和安装失败需手动重试。
更新说明以 Markdown 排版展示，忽略原始 HTML，图片仅显示替代文本；
仅允许点击 HTTPS 链接，并交给默认浏览器打开。

## 首次配置

公钥已配置到 `src-tauri/tauri.conf.json`。私钥与密码在仓库外生成，
需要安全备份并在 GitHub 仓库 Settings → Secrets and variables → Actions 添加：

- `TAURI_SIGNING_PRIVATE_KEY`：私钥文件的完整内容，不是本机路径。
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：密码文件内容。

本次生成的密钥位于当前用户目录的
`.config\smspop-updater\smspop-20261006-130408.key`，密码位于同目录的
`smspop-20261006-130408.key.password`，公钥为 `.key.pub` 文件。
这是本机保存位置，不是 CI 可访问的路径；换电脑前务必安全备份。

不要提交私钥或密码，不要把它们贴到聊天、Issue 或日志中。
不要随意重新生成密钥，否则旧客户端无法验证新包。
Updater 签名不等于 Windows Authenticode 代码签名。

## 发版步骤

1. 同步根 `Cargo.toml`、`package.json`、`src-tauri/tauri.conf.json` 的版本。
2. 执行 `npm install --package-lock-only` 与 `cargo check --workspace` 同步锁文件。
3. 运行测试与构建，将功能分支通过 PR 合并到 main。
4. 从对应提交创建并推送标签，例如 `v0.2.0`。
5. Release Windows 工作流测试、构建签名安装包，创建草稿 Release，上传
   `.exe`、`.exe.sig` 和 `latest.json`。不会自动公开发布。
6. 检查安装包、签名与元数据。完成升级验证后手动发布稳定 Release。

工作流也可以手动指定已经存在的版本标签重跑，但拒绝覆盖正式发布的 Release。
修改草稿说明后如需同步应用内更新说明，重新运行工作流，或重新生成并上传
`latest.json`（notes 是生成时的快照）。

## 本地打包

启用了更新签名产物后，本地 `tauri build` 也需要设置签名环境变量。
PowerShell 示例（路径替换为自己保存的密钥）：

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content 'C:\path\smspop.key' -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = Get-Content 'C:\path\smspop.key.password' -Raw
npm run tauri -- build --bundles nsis
Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY
Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
```

## 验证清单

### 开发模式预览

运行 `npm run tauri dev`，进入“关于”，展开“开发：更新界面预览”并点击
“开启预览”。可切换新版、最新版、检查失败、未知大小下载、下载失败、
下载完成和安装失败等场景。开启后切换到其他页面，左下角仍可打开更新模态框。
普通“下载更新”按钮模拟约 6 秒的有大小下载；关闭模态框后仍会继续。
检查失败场景在 30 秒后模拟恢复，自动恢复不会打开弹窗。
“安装并重启 → 确认安装”仅模拟安装失败，不会退出应用或运行安装器。
通过常规设置切换语言与主题即可检查中英文、深浅色显示。
点击“退出预览”会取消模拟任务，并恢复后端的“开发模式禁用更新”状态。

模拟后端仅由 Vite 开发模式按需加载，不包含在正式构建中。
预览不访问更新服务器、不下载文件，不证明真实网络/代理或签名安装可用。
真实退避调度、成功停止与并发保护可运行：

```powershell
cargo test -p smspop-app --lib updater::tests --locked
```

真实升级仍使用下方独立测试地址和正式测试构建完成验证。

### 正式测试构建

- 用独立测试仓库/更新地址验证两版升级，避免把测试版本发布到正式 latest。
- 安装旧测试版，发布新测试版，检查 → 下载 → 确认安装 → 验证新版启动。
- 验证断网、无效元数据、缺失资产、签名错误均可恢复且不能误安装。
- 下载中隐藏窗口、切换页面，回来能看到最新状态。
- 更新后配置、设备记录、自启动设置保留。
- 尚未内置 Updater 的旧版需要手动安装一次带更新能力的新版。

首次尚无正式 `latest.json` 时，检查会显示错误，这是发布链路未启用，
不是“已是最新版本”。自动检查失败只记录日志和设置页状态，不抢焦点。
