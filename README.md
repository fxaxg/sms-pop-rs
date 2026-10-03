# sms-pop-rs

**在 Windows 上接住 iPhone 的通知和验证码。**

iPhone 收到短信验证码 → Windows 右下角弹出通知 → 自动复制到剪贴板 →
如果光标正好在输入框里，光标旁会出现「填入」候选条 —— **只有你点它才会填入，绝不自动写入。**

基于 Tauri 2 构建，现代简约的界面，开箱即用的新手引导。

> 🚧 本项目正在重写中（前身是一个 MVP 实验项目），尚未发布稳定版本。

## 功能

- 📱 **通知弹出**（一等公民）：把 iPhone 的通知实时弹到 Windows 上，可按 App / 关键词过滤
- 🔢 **验证码增强**：自动识别通知里的验证码，复制到剪贴板，并提供光标候选条一键填入
- 🔒 **绝不自动填入**：全项目只有点击候选条这一个动作会写入别的程序
- 🌙 开机自启、系统托盘常驻

## 路线图

- [x] iPhone 连接（ANCS over BLE）
- [ ] Tauri 2 全新界面与新手引导
- [ ] Android 支持（传输层已预留抽象）

## 开发

环境要求：Rust (MSVC toolchain)、Node.js、Visual Studio Build Tools（C++ 工作负载）、WebView2。

```powershell
npm install
npm run tauri dev
```

## License

MIT
