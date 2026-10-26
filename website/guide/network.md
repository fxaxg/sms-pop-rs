# HTTP 接收短信与通知

默认关闭，默认监听所有 IPv4 网卡。启用后复制「接收链接」到发送端。
「复制链接」复制包含 device_name、body、from 与完整 token 的 GET 示例链接，复制后即可请求。中文与特殊字符自动 URL 编码；界面以可读文本展示完整令牌。正式 POST / SmsForwarder 配置请使用「仅复制接口地址」，仅含 token，避免携带示例正文。
默认地址按启发式优先普通局域网地址，降低名称中可识别的虚拟/VPN网卡及回环、链路本地地址优先级；不能保证自动识别所有虚拟网卡或实际可达性，必要时手动选 IP。
局域网 HTTP 仅用于可信网络；公网必须使用 HTTPS 代理或加密私有网络。
软件不会自动开放防火墙或路由器端口。若无法访问，请检查本机防火墙配置。

## 统一令牌

所有发送端共用一个随机令牌，以 Windows 当前用户 DPAPI 加密保存，可重启后再次显示或复制。
重置会立即使所有旧链接失效，必须更新所有发送端。不再管理设备名单。
旧的每设备令牌配置迁移时会清空并关闭接收服务，需复制新链接重新启用。
设备名称由请求 device_name 提供，仅用于显示和去重，不是可信身份。
网络消息自动复制默认关闭，开启后仍受全局验证码设置控制。

## 请求

`GET /api/v1/notifications?token=TOKEN&device_name=Android&body=URL_ENCODED_TEXT`

`POST /api/v1/notifications?token=TOKEN`

POST 支持 application/json 和 application/x-www-form-urlencoded。
令牌也可通过 Authorization: Bearer TOKEN 或请求体 token 传入；只能选择一个位置，重复提供拒绝。

| 字段 | 说明 |
| --- | --- |
| token | 必填凭据，默认放 URL 查询参数 |
| body / content | 必填非空正文，最多 16384 UTF-8 字节 |
| device_name | 可选展示名称，最多 256 字节，默认“网络转发” |
| kind | sms（默认）或 notification |
| sender / from | 发件人，最多 256 字节 |
| title | 通知标题，最多 256 字节 |
| app_identifier | 通知 App 标识，内部加 network: 前缀 |
| message_id | 唯一消息 ID，重试沿用相同值，最多 256 字节 |

未知字段拒绝；不接受执行目标或客户端指定的验证码。
响应：202 accepted（入队，仍受过滤设置影响），200 duplicate，400 参数错误，401 鉴权错误，403 浏览器 Origin 拒绝，413 请求过大，414 URL 过长，415 类型错误，429 限流，503 不可用。
响应 Cache-Control: no-store，不回显消息或令牌。
请求体 32 KiB，URL 8 KiB，读取超时 5 秒，全局每分钟 120 次。队列 64，处理并发 16。
按设备名称 + message_id 去重 5 分钟，没有 ID 按内容去重 15 秒。队列不持久化，退出或休眠不能接收。

## curl（PowerShell）

```powershell
$token = Read-Host 'Receiver token'
curl.exe -G 'http://127.0.0.1:24836/api/v1/notifications' `
  --data-urlencode "token=$token" `
  --data-urlencode 'device_name=Android' `
  --data-urlencode 'body=Verification code: 123456'
```

GET 参数必须由客户端 URL 编码，短信包含 &、+、换行时不能直接拼接 URL。长消息优先 POST。

## Python（标准库）

保存 send_sms.py，执行 `uv run send_sms.py`：

```python
import getpass
import json
import urllib.request
from urllib.parse import urlencode

url = "http://127.0.0.1:24836/api/v1/notifications?" + urlencode({"token": getpass.getpass("Token: ")})
body = json.dumps({"device_name": "Android", "body": "验证码 123456"}).encode("utf-8")
request = urllib.request.Request(url, data=body, headers={"Content-Type": "application/json"}, method="POST")
with urllib.request.urlopen(request, timeout=10) as response:
    print(response.status, response.read().decode("utf-8"))
```

## 手机接入

- SmsForwarder：Webhook 使用复制的接收链接，POST 表单 `device_name=Android&from=[from]&content=[content]`；通知规则另加 kind=notification。GET 也可，但须由发送器正确编码参数。
- iOS：信息自动化 → 获取消息输入 → 获取 URL 内容 → POST JSON；URL 使用接收链接，字典中填写 body、sender、device_name。锁屏、网络权限和立即运行需按 iOS 版本实测；不是所有 App 通知订阅。

## 安全边界

接收 URL 含凭据，不要分享截图或日志；代理/发送器可能记录 URL，HTTPS 不消除日志泄露风险。
软件不记录完整请求 URL、正文或令牌。不要把带消息参数的 GET 链接当成普通网页链接，避免点击、预取触发通知。
IP 选择只改变展示和复制地址，不改变监听范围。VPN/虚拟网卡地址不保证手机可达。
公网 HTTPS 代理应指向仅本机监听，限制连接数、超时及路径；隧道服务商可能终止 TLS，不等于端到端加密。
