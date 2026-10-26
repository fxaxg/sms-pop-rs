# 开始使用

SmsPop 把手机通知接到 Windows，并提供验证码复制与按需填入。

## 下载安装

前往 [GitHub Releases](https://github.com/fxaxg/sms-pop-rs/releases/latest)，下载 Windows 安装包，安装后打开 SmsPop。

项目仍在早期开发阶段，设备和系统版本之间可能存在兼容性差异。若最新发布中暂时没有安装包，可以参考仓库的 [源码运行说明](https://github.com/fxaxg/sms-pop-rs#开发)。

## 选择接入方式

### iPhone 蓝牙连接

需要支持 BLE 外设角色的 Windows 蓝牙适配器、支持 ANCS 通知共享的 iPhone，以及 WebView2 运行时。

在 SmsPop 的“连接”页添加设备，然后在 iPhone 蓝牙设置中与电脑配对，并开启“共享系统通知”。完整步骤见 [连接 iPhone](./pairing)。

### HTTP 网络接入

Android 可通过 SmsForwarder 等工具转发短信或通知；iOS 可通过快捷指令转发适用的消息。网络接入需要手动启用，详见 [HTTP 网络接入](./network)。

## 使用通知和验证码

收到通知后，SmsPop 会在电脑右下角弹出通知。识别到验证码时，可以点击验证码按钮复制。

默认使用光标候选条：在适用的输入框附近显示验证码，点击“填入”后写入。自动填入需要手动开启，详见 [验证码填入](./otp)。

关闭设置窗口会隐藏到系统托盘；完全退出请使用托盘菜单。
