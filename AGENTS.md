# AGENTS.md

给 AI / 人类贡献者的项目说明。

## 项目定位

SmsPop：把 iPhone 的通知和短信验证码接到 Windows（Tauri 2 + Rust）。
定位从「验证码工具」扩展为「手机通知弹出工具」，验证码填充是通知之上的增强层。

## 目录结构

```
crates/
├─ smspop-core/   纯逻辑，零平台依赖：ANCS 协议、OTP 提取、写入策略、候选条几何、过滤、去重、配置
│                 ★ 不依赖 windows crate，全部可 cargo test
├─ smspop-ble/    Windows BLE：ANCS 广播/GATT 服务端/订阅/监督循环（独立线程 + LinkEvent）
└─ smspop-uia/    UIA 焦点探测与写入（专属工作线程，COM 接口不跨线程）

src-tauri/        Tauri 主程序
├─ src/link_worker.rs  BLE 事件 → 应用动作（去重 → 过滤 → 复制/弹窗/候选条）
├─ src/popups.rs       toast 弹窗管理（动态窗口，右下堆叠，不抢焦点）
├─ src/caret.rs        光标候选条（点了才填入）
├─ src/tray.rs         系统托盘
├─ src/commands.rs     前端命令
└─ src/state.rs        全局状态

src/              React 前端（Vite 多页构建）
├─ main/          设置窗口（index.html）
├─ toast/         通知弹窗（toast.html）
├─ caret/         光标候选条（caret.html）
└─ shared/        API 封装 + 设计令牌
```

## 硬性纪律（继承自旧项目，都是用坑换来的）

1. **绝不自动填入**：全项目唯一写别的程序内容的地方是 `smspop-uia` 的 `insert`，
   Windows 只由用户点击候选条触发；macOS 只由用户按已注册快捷键触发，写入集中在 `macos_input`。不许加任何收件自动填入路径。
2. **弹窗/候选条不抢焦点**：窗口一律 `focusable(false)`。
3. **UIA 不拖住 UI**：探测 900ms / 写入 1.5s 超时，COM 接口不跨线程，日志只记长度不记内容。
4. **字节级协议、写入策略、几何计算必须留在 smspop-core 并有单测**。
5. **ANCS 顺序**：先订阅 Data Source 再订阅 Notification Source；配对必须勾选「共享系统通知」。

## 构建 / 测试

```powershell
npm install
npm run tauri dev        # 开发
cargo test --workspace   # Rust 测试（core 无需真机）
npm run build            # 前端构建
```

环境：Rust MSVC toolchain + VS Build Tools（C++ 负载）+ WebView2 + Node 22。

## Git 规范

GitHub Flow：`main` 永远可发布；功能分支 `feat/xxx`、修复 `fix/xxx` 从 main 切出，
PR 合回 main。CI 跑 `cargo test --workspace`、`clippy -D warnings`、`cargo fmt --check`、`npm run build`。

## Android 扩展预留

上层只认 `smspop_core::model::PhoneNotification`，不认识 ANCS。
将来 Android 传输层（局域网/WebRTC/…）只要产出同一模型即可接入。
