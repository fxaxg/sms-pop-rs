# Android → macOS LAN Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for inline execution, or superpowers:subagent-driven-development if the user selects delegation. Steps use checkbox syntax for tracking.

**Goal:** 安卓通过 SmsForwarder 向同一局域网的 Mac 发送验证码，Mac 提示、复制并支持用户按快捷键填入。

**Architecture:** 复用 Tauri 应用、HTTP 接口及 core 纯逻辑，将 Windows 系统接口隔离。Mac 使用 Keychain 保存接收令牌，以辅助功能检查焦点并用系统事件输入。HTTP 分发与 BLE 生命周期分离。

**Tech Stack:** Rust stable、Tauri 2、React 19、TypeScript、Axum、macOS Accessibility / CoreGraphics / Security、Tauri global-shortcut 插件。

**Spec:** `docs/superpowers/specs/2026-10-08-android-macos-lan-design.md`

## Global Constraints

- 同一 Wi-Fi，使用 SmsForwarder；不开发安卓 App、不部署中转服务。
- 验证码候选只驻留内存，自本机接收起 120 秒后失效。
- 默认 Command+Shift+V，可修改，注册冲突必须显示失败。
- 自动复制默认关闭；仅用户明确触发才能写入其他应用，不模拟回车。
- Keychain 失败必须关闭接收，不回退明文；正文、验证码、令牌不写日志。
- Mac 第一版无 BLE 配对与光标候选条；Windows 保持原有功能。
- HTTP 只用于可信局域网，不等同于加密传输；休眠和退出不保证收件。
- Mac 开发构建禁用上游更新通道；不承诺公证、正式分发与所有输入控件兼容。

## Review Focus

1. Keychain 拒绝访问或重置失败：服务不能用新旧混合凭据继续接收（任务 2）。
2. 非验证码/被过滤消息与过期边界：不能覆盖有效候选或填入过期码（任务 3）。
3. 快捷键按住重复触发、焦点切换、撤销权限：不得重复输入或写到错误目标（任务 4）。
4. 首次启动默认值与旧配置：Mac 默认关闭自动复制且不重置用户显式设置（任务 5）。
5. Wi-Fi 地址改变、端口占用、客户端隔离：可见错误和排查入口，不能显示虚假的已连接（任务 5、6）。

## File Structure

- 新增 `src-tauri/src/platform/{mod,windows,macos}.rs`：系统打开操作与能力信息；不包含短信解析。
- 新增 `src-tauri/src/notification_dispatch.rs`：从 `link_worker.rs` 提取共用分发。
- 新增 `src-tauri/src/token_store/{windows,macos}.rs`：系统安全存储；`token_store.rs` 保留门面。
- 新增 `crates/smspop-core/src/otp_candidate.rs`：候选生命周期纯逻辑。
- 新增 `src-tauri/src/macos_input.rs`：辅助功能与 Unicode 键盘事件。
- 新增 `src-tauri/src/shortcut.rs`：快捷键注册、用户动作协调与结果反馈。
- 新增 `src-tauri/tauri.macos.conf.json`：Mac 打包配置。
- 新增 `docs/android-macos-lan.md`：安装、手机配置、权限与联调。
- 按各任务修改现有配置、状态、命令和前端页面；不重写整个项目。

### Task 1: Mac 可编译并启动的跨平台应用

**Files:** `rust-toolchain.toml`, `Cargo.toml`, `src-tauri/Cargo.toml`, `src-tauri/src/{lib,state,commands,types,tray,devices,caret,link_worker,http_ingress,app_launcher,updater}.rs`；新增 platform 模块、notification_dispatch、Mac 配置；检查 `src-tauri/build.rs` 和 `.github/workflows/ci.yml`。

**Interfaces:** `platform::capabilities() -> PlatformCapabilities`（serde 输出 os、ble、caret、shortcut 布尔能力）；`platform::open_url(url: &str) -> Result<(), String>`；`platform::open_path(path: &Path) -> Result<(), String>`。保留分发函数名称和参数，将 `dispatch_notification` / `dispatch_network` 迁至 notification_dispatch。

- [ ] 记录 `git status`、Node/npm/Apple 工具版本以及 CPU 架构；按 using-git-worktrees 技能准备执行 checkout，保留已批准的设计和计划。
- [ ] 准备缺失的 Rust stable（优先检查现有安装路径），安装依赖，运行现有前端测试和 core 测试作为基线；记录环境失败与实际测试失败。
- [ ] 先运行 `cargo check -p smspop-app` 记录 Windows 耦合导致的失败。
- [ ] 将工具链切到主机 stable；Windows crates 作为 target 依赖。用条件编译隔离 BLE/UIA 字段、命令和启动路径，Mac 不启动它们；workspace 默认成员配置需保证 Mac 日常测试不强制构建 Windows crates。
- [ ] 提取共用分发函数，保留 Windows 去重、过滤、复制与候选条语义。Mac 暂只接入通知和复制，不伪造 BLE 已连接。
- [ ] 实现平台打开接口，Mac 使用系统打开工具且参数单独传递，不经 shell 拼接。替换 cmd/explorer/mmc/ShellExecute 调用并保留既有 URL 校验。
- [ ] 新增 Mac bundle 配置，禁用更新 artifacts，禁用 Mac 更新初始化、检查与相关命令执行；准备 app 图标。平台元数据供后续 UI 使用。
- [ ] 安全存储未就绪阶段 Mac 接收明确返回不可用，不用明文占位。运行 `cargo check -p smspop-app` 和 `npm run build`，启动开发窗口验证无 Windows 初始化错误。
- [ ] 按明确文件列表提交 `refactor: isolate Windows integrations for macOS`。

### Task 2: Keychain 接收令牌与真实 HTTP 接入

**Files:** `src-tauri/src/token_store.rs`、新增其子模块，`src-tauri/src/http_ingress.rs`、`src-tauri/Cargo.toml`。

**Interfaces:** `token_store::store(token: &str) -> Result<Vec<u8>, String>` 返回配置中保存的 opaque reference；`token_store::load(reference: &[u8]) -> Result<String, String>`；`token_store::remove(reference: &[u8]) -> Result<(), String>`。Windows reference 是 DPAPI 字节；Mac reference 是随机 Keychain account 标识，service 使用应用 identifier。

- [ ] 添加带内存存储替身的测试，断言 load/store 失败时 receiver disabled，旧令牌重置后返回 401；新配置保存失败时旧凭据仍有效且新 Keychain 项被清理。
- [ ] 运行 `cargo test -p smspop-app`，确认新增断言尚未满足。
- [ ] Windows 包装现有 DPAPI；Mac 使用 Security framework 的 generic password 接口。重置先创建新条目，保存配置成功后切换内存状态，再删除旧条目；不删除其他应用条目。旧字段 `encrypted_token` 可承载 opaque reference，保持 Windows 文件兼容。
- [ ] HTTP 测试注入存储替身，避免常规测试触碰真实 Keychain；复用已存在的参数、鉴权、大小、重复消息和回环 HTTP 测试。
- [ ] 执行 `cargo test -p smspop-app`；启动应用，使用专用临时 Keychain account 手动验证读取、重启、重置并清理测试条目，用户未授权时明确记录未验证。
- [ ] 提交 `feat: secure macOS receiver credentials with Keychain`。

### Task 3: 120 秒候选生命周期

**Files:** 新增 `crates/smspop-core/src/otp_candidate.rs`，修改 core `lib.rs`、`src-tauri/src/state.rs` 与 `notification_dispatch.rs`。

**Interfaces:** `OtpCandidateStore::default()`；`offer(&mut self, code: String, received_at: Instant)`；`current(&mut self, now: Instant) -> Option<&str>`；`clear(&mut self)`。AppState 以 Mutex 持有它。

- [ ] 添加纯逻辑测试：t+119 秒返回代码，t+120 秒返回 None；新验证码替换旧值；clear 后为空。测试使用注入时间，不 sleep。
- [ ] 运行 `cargo test -p smspop-core otp_candidate`，确认缺失模块/实现导致失败。
- [ ] 实现生命周期；分发只在消息通过过滤且 `should_offer_otp` 为真时更新候选，与自动复制和候选条开关独立。无验证码消息不调用 offer。
- [ ] 添加分发策略测试验证非验证码和被过滤消息保留旧候选，合法新码替换；运行 core 与 app 测试。
- [ ] 提交 `feat: retain expiring OTP candidates for user insertion`。

### Task 4: 用户触发的 macOS 填入

**Files:** 新增 `macos_input.rs`、`shortcut.rs`，修改 `src-tauri/{Cargo.toml,src/lib.rs,src/state.rs,src/commands.rs}`、core `config.rs` 和 `AGENTS.md`。

**Interfaces:** `macos_input::is_trusted() -> bool`；`request_access() -> bool`；`insert(code: &str) -> Result<(), String>`。`shortcut::configure(app: &AppHandle, accelerator: &str) -> Result<(), String>`；`shortcut::insert_latest(app: &AppHandle) -> Result<(), String>`。配置新增带 serde 默认值的 macOS 快捷键字段，默认 `CommandOrControl+Shift+V`，Windows 不注册。

- [ ] 使用输入后端替身测试：权限拒绝、非可编辑/只读/本应用目标、候选失效均为零次写入；释放事件和按住重复事件不重复输入；焦点快照变化时中止。
- [ ] 运行新增协调层测试，确认失败。
- [ ] 实现辅助功能读取焦点和可编辑性检查；系统调用离开 UI 线程并设置 AX 调用超时。写入前复查权限、候选有效期、目标进程及焦点；发送 Unicode 按键事件，不改剪贴板、不发送回车。异常不打印 code 或控件内容。
- [ ] 注册全局快捷键，在一次按下/释放周期只执行一次，用 busy 状态阻止并发；改键失败保留旧注册与配置。结果通过事件反馈，系统事件已发送不宣称目标一定接受，保留复制按钮。
- [ ] 更新仓库约定：Mac 快捷键也是明确的用户填入动作，所有 Mac 写入集中在 insert；不引入收件自动写入路径。
- [ ] 运行 Rust 测试，手动测试 TextEdit 和浏览器单输入框、权限撤销、焦点切换、按住快捷键；无 UI 授权则明确保留未验证项。
- [ ] 提交 `feat: insert macOS OTP on explicit shortcut`。

### Task 5: Mac 引导、配置与错误反馈

**Files:** `src/shared/{api,i18n,network-i18n}.ts`、`src/main/App.tsx`、`src/main/sections/{Connection,ConnectionGuide,Network,Otp,SoftwareUpdate}.tsx`、`src-tauri/src/{commands,state,http_ingress}.rs`；新增 `tests/macos-settings.test.mjs`（仅测试提取出的非展示策略）。

**Interfaces:** 后端 `get_platform_capabilities` 返回任务 1 能力；`get_input_status` 返回 authorized、registered_shortcut、last_error；`set_input_shortcut` 和 `request_input_access` 仅允许 main window 调用，前端 API 封装同名接口。

- [ ] 添加默认值测试：Mac 新配置 auto_copy=false，Windows 默认保持不变，Mac 已保存 true 不被覆盖；无效快捷键不能写入配置。
- [ ] 运行相关测试确认失败，然后实现仅首次创建 Mac 配置的默认覆盖及事务式快捷键保存。
- [ ] 根据能力显示网络引导、隐藏 BLE 和 caret 设置及上游更新操作；提供复制、权限状态、快捷键状态和错误反馈。未连通不显示“手机已连接”。
- [ ] 网络页保留地址刷新/选择，展示 POST 表单字段、令牌配置方式和 HTTP 可信网络说明；端口绑定失败、IP 改变后需重配、防火墙与 AP 隔离分别提供明确说明。自动检测能力不足时提供手动刷新，不声称持续检测了网络。
- [ ] 运行 `npm test`、`npm run build`、相关 Rust 测试；实际窗口检查首次启动、已保存设置、权限失败和端口占用提示；避免仅镜像 JSX 的测试。
- [ ] 提交 `feat: guide Android pairing with macOS LAN receiver`。

### Task 6: 构建、回归与真机交付

**Files:** 新增 `docs/android-macos-lan.md`，修改 `README.md`、`.github/workflows/ci.yml`，必要时新增显式使用 Mac 配置的 npm build 脚本。

- [ ] 文档写明 SmsForwarder 配置步骤、可复制字段、手机后台权限、Mac 辅助功能授权、IP/防火墙/AP 隔离排查及休眠限制；使用占位符令牌，不嵌入本机凭据。
- [ ] 执行 `cargo fmt --all -- --check`、`cargo test -p smspop-core -p smspop-app`、`cargo clippy -p smspop-core -p smspop-app --all-targets -- -D warnings`、`npm test`、`npm run build`；Windows CI 继续执行 workspace 全量测试和 Clippy，Mac CI 使用平台包集合。
- [ ] 执行 `npm run tauri build -- --config src-tauri/tauri.macos.conf.json --bundles app`，检查生成 .app 并启动；不安装到系统 Applications、不改防火墙或替用户授权。
- [ ] 使用占位短信通过回环和局域网接口验证真实运行的应用：收件提示、复制、去重、旧令牌拒绝、120 秒后快捷键不输入；检查日志无正文或令牌。
- [ ] 与用户完成手机真实收件 → Mac 提示 → TextEdit/浏览器填入；未提供手机操作时交付可运行构建和剩余验收步骤，不把模拟请求写成真机通过。
- [ ] 按 requesting-code-review 技能做整体审查，修复实质问题后重跑受影响检查；记录 Windows CI/权限/真机中尚未验证部分。
- [ ] 提交文档及 CI 改动，按 finishing-a-development-branch 技能交付本地构建、源码位置和测试结果，不自行推送或发布。

## Plan Self-Review

设计中的范围、平台隔离、Keychain、候选过期、用户触发输入、默认值、网络错误与真机验收均已分配到任务。安全存储用独立门面和测试替身，候选时间可注入，焦点与按键协调独立测试。外部系统行为由实际应用验证，未验证时明确报告。执行前需用户审阅本计划并选择 inline 或 subagent-driven 方法。
