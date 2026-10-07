<div align="center">
  <img src="src/shared/app-icon.svg" width="112" height="112" alt="SmsPop logo" />
  <h1>SmsPop</h1>
  <p><strong>让 iPhone 通知来到 Windows，让验证码少一次切换。</strong></p>
  <p>通过蓝牙接收通知，复制验证码，按需填入，并打开对应的电脑应用。</p>
  <p>
    <a href="https://github.com/fxaxg/sms-pop-rs/actions/workflows/ci.yml"><img src="https://github.com/fxaxg/sms-pop-rs/actions/workflows/ci.yml/badge.svg" alt="CI status" /></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-informational" alt="MIT license" /></a>
    <img src="https://img.shields.io/badge/platform-Windows-555555" alt="Windows" />
  </p>
  <p><a href="#开始使用">开始使用</a> · <a href="#连接-iphone">连接 iPhone</a> · <a href="#开发">开发</a> · <a href="CONTRIBUTING.md">参与贡献</a></p>
</div>

## 使用演示

https://github.com/user-attachments/assets/ac2c4289-9e66-47c9-bec0-208e82811a7e

## 它能做什么

- **接收手机通知**：在屏幕右下角显示 iPhone 通知。
- **HTTP 网络接入**：接收 SmsForwarder 或快捷指令转发的短信与通知，复制统一令牌链接即可使用 GET / POST 接入，见 [接入指南](docs/http-ingress.md)。
- **使用验证码**：识别通知中的验证码，可自动复制；在输入框旁提供来源提示与“填入”按钮。
- **点击打开应用**：按通知来源配置 URL 规则，例如 `weixin://`、`tencent://` 或固定 HTTPS 链接。
- **后台自动恢复**：手机离开后自动重试连接，返回后重建失效的通知会话。
- **管理接收偏好**：设备首选、应用与关键词过滤、弹窗时长、开机自启，以及中英文界面。
- **保持界面安静**：浅色、深色或跟随系统，简单设置自动保存。
- **软件更新**：正式构建启动后自动检查更新，在独立“关于”页手动下载并确认安装。

## 开始使用

项目仍处于早期开发阶段，尚不承诺稳定版兼容性。安装包发布后可从 [Releases](https://github.com/fxaxg/sms-pop-rs/releases) 下载；没有发布资产时，请按下方开发步骤从源码运行。

### 使用条件

- Windows 电脑，蓝牙适配器必须支持 **BLE 外设角色**；仅支持普通蓝牙或 BLE 中心角色并不足够。
- 支持 ANCS 通知共享的 iPhone，并在系统蓝牙设置中允许通知共享。
- Windows WebView2 运行时。

目前桌面端只支持 Windows；iPhone 可通过 BLE 接入，Android 与 iOS 也可由第三方工具通过 HTTP 转发。不同蓝牙适配器、驱动和 iOS 版本的表现可能不同。

## 连接 iPhone

1. 启动 SmsPop，在“连接”页点击“添加设备”。
2. 确认电脑和 iPhone 的蓝牙均已开启。
3. 在 **iPhone 的“设置 → 蓝牙”** 中找到电脑，完成配对。
4. 允许接收 iPhone 通知。若没有收到提示，点击电脑名称旁的信息按钮，开启 **“共享系统通知”**。
5. 让 iPhone 收到一条新通知，在电脑上确认接收成功。

本地弹窗测试只验证电脑上的显示与交互，**不验证蓝牙连接**。关闭设置窗口会隐藏到托盘；完全退出请使用托盘菜单。

### 点击通知打开应用

在“通知 → 点击打开应用”添加规则。微信与 QQ 模板分别使用 `weixin://` 和 `tencent://`；目标电脑必须安装并注册对应协议的应用。

点击通知主体或“打开”按钮后，系统将 URL 交给对应应用处理。验证码按钮只复制，不触发打开。此功能不保证进入具体聊天或消息，协议行为取决于目标应用。

## 隐私与边界

通知传输与验证码识别在本机完成，不需要云端转发服务。用户点击配置的 HTTPS 链接或应用协议后，目标浏览器或应用可能访问网络。

正式构建会访问 GitHub Releases 检查和下载更新，不会上传通知内容。更新需手动确认，安装期间应用会退出。

“关于”页从 Shields.io 加载在线徽章；点击仓库、贡献者或反馈链接时，会在默认浏览器打开 GitHub。

- 来源签名（例如 `【网易】`）只是正文提示，不是经过认证的发送方。
- 过滤规则同时影响弹窗、验证码复制与候选条。
- 开发版本的日志可能包含通知内容、联系人或验证码。反馈问题前请脱敏，**不要公开上传原始日志或配置文件**。
- 不承诺任何情况下均不丢通知；休眠、系统权限、驱动及手机通知策略仍需真机验证。

安全问题请阅读 [SECURITY.md](SECURITY.md)，不要在公开 Issue 中披露敏感内容。

## 开发

需要 Rust stable（MSVC 工具链）、Node.js 22、Visual Studio Build Tools 的 C++ 工作负载，以及 WebView2。

```powershell
npm ci
npm run tauri dev
```

### 检查与构建

```powershell
npm test
npm run build
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm run tauri build
```

Windows 安装包输出到 `target/release/bundle/nsis/`。应用图标源文件位于 `src/shared/app-icon.svg`。

打包时需要配置更新签名环境变量；密钥配置、版本同步和草稿发布步骤见 [发版说明](docs/releases.md)。

### 项目结构

| 路径 | 职责 |
| --- | --- |
| `crates/smspop-core` | 协议解析、验证码与来源提取、过滤、配置及几何计算 |
| `crates/smspop-ble` | Windows BLE、ANCS 订阅与连接监督 |
| `crates/smspop-uia` | 输入框探测与用户触发的写入 |
| `src-tauri` | 窗口、托盘、通知分发及平台操作 |
| `src` | 设置窗口、通知弹窗与候选条前端 |

反复验证首次配对时，可使用 `tools/test-ancs.ps1`。它会清理匹配的测试进程和开发服务器，使用前请查看脚本，不要在其他开发任务运行时随意执行。

## 贡献

欢迎提交可复现的问题、设备兼容性结果和改进建议。开发流程、测试要求见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 友链

- [Linux.do](https://linux.do/)

## 许可证

[MIT](LICENSE)。
