import { usePlatform } from "../../shared/platform";
import { useContext, useEffect, useRef, useState } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { configureHttp, getHttpStatus, listHttpAddresses, resetHttpToken, setHttpCopy, type HttpStatus, type LocalAddress } from "../../shared/api";
import { LangContext } from "../../shared/i18n";
import { networkDict } from "../../shared/network-i18n";
import { Disclosure, Modal, NumberField, Row, Section, Toggle } from "../components";

export function Network() {
  const platform=usePlatform();
  const lang=useContext(LangContext), t=networkDict[lang];
  const text=(zh:string,en:string)=>lang==="zh"?zh:en;
  const [status,setStatus]=useState<HttpStatus|null>(null),[bind,setBind]=useState("0.0.0.0"),[port,setPort]=useState(24836);
  const [addresses,setAddresses]=useState<LocalAddress[]>([]),[ip,setIp]=useState(""),[dialog,setDialog]=useState<"ip"|"reset"|null>(null);
  const [busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null),[copied,setCopied]=useState<string|null>(null);
  const copyTimer=useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(()=>()=>clearTimeout(copyTimer.current),[]);
  useEffect(()=>{let disposed=false;void getHttpStatus().then(s=>{if(!disposed){setStatus(s);setBind(s.settings.bind);setPort(s.settings.port);}}).catch(e=>{if(!disposed)setError(String(e));});void listHttpAddresses().then(a=>{if(!disposed){setAddresses(a);setIp(a.find(x=>!x.address.startsWith("127.")&&!x.address.startsWith("169.254."))?.address??"");}}).catch(()=>{});return()=>{disposed=true;};},[]);
  const run=async(action:()=>Promise<void>)=>{if(busy)return;setBusy(true);setError(null);try{await action();}catch(e){setError(String(e));}finally{try{setStatus(await getHttpStatus());}catch(e){setError(String(e));}setBusy(false);}};
  const host=status?.settings.bind==="127.0.0.1"?"127.0.0.1":ip||"<PC-LAN-IP>";
  const endpoint=`http://${host}:${status?.settings.port??port}/api/v1/notifications`;
  const url=`${endpoint}?token=${encodeURIComponent(status?.token??"")}`;
  const testName=text("设备名称","Device name");
  const testBody=text("通知正文","Notification body");
  const testSender=text("发信人","Sender");
  const testUrl=`${endpoint}?${new URLSearchParams({device_name:testName,body:testBody,from:testSender,token:status?.token??""})}`;
  const displayedParams=[["device_name",testName],["body",testBody],["from",testSender],["token",status?.token??"…"]];
  const copy=async(value:string,key:string)=>{try{await writeText(value);clearTimeout(copyTimer.current);setCopied(key);copyTimer.current=setTimeout(()=>setCopied(null),2000);}catch{setError(t.clipboardError);}};
  const dirty=status&&(bind!==status.settings.bind||port!==status.settings.port);
  const curl=`curl.exe -G "${endpoint}" \`\n  --data-urlencode "token=YOUR_TOKEN" \`\n  --data-urlencode "device_name=Android" \`\n  --data-urlencode "body=Verification code: 123456"`;
  const python=`import getpass, json, urllib.request\nfrom urllib.parse import urlencode\n\nurl = "${endpoint}?" + urlencode({"token": getpass.getpass("Token: ")})\ndata = json.dumps({"device_name": "Android", "body": "验证码 123456"}).encode("utf-8")\nrequest = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json"}, method="POST")\nwith urllib.request.urlopen(request, timeout=10) as response:\n    print(response.status, response.read().decode("utf-8"))`;
  return <>
    {platform.os==="macos" && <Section title="安卓 → Mac · 同一 Wi-Fi" description="先开启下方接收服务，再配置手机。">
      <ol><li>手机与 Mac 连接同一 Wi-Fi。接收范围选择“局域网”。</li><li>点击“仅复制接口地址”，填入 SmsForwarder 的 Webhook 发送通道，选择 POST 表单。</li><li>表单字段：<code>device_name=Android&amp;from=[from]&amp;content=[content]</code>。建立短信转发规则，并授予短信和后台运行权限。</li><li>发送测试短信。收到验证码后可点击复制，或在目标输入框按 ⌘⇧V；权限与快捷键在“验证码”页配置。</li></ol>
      <p className="hint-text">HTTP 仅用于可信 Wi-Fi，令牌不加密短信。公司/访客 Wi-Fi 可能禁止设备互访；Mac 防火墙需允许 SmsPop 入站。切换网络后通过“网卡”刷新地址并重新配置手机。休眠或退出时无法接收。</p>
    </Section>}
    <Section title={t.receiver} action={<span className="network-status" role="status">{status?.running?t.running:status?.settings.enabled?t.failed:t.stopped}</span>}>
      <Row label={t.enable}><Toggle checked={status?.settings.enabled??false} disabled={busy||!status} onChange={v=>void run(()=>configureHttp(v,status!.settings.bind,status!.settings.port))}/></Row>
      <Row label={t.scope}><select className="select" value={bind} disabled={busy} onChange={e=>setBind(e.target.value)}><option value="127.0.0.1">{t.local}</option><option value="0.0.0.0">{t.lan}</option></select></Row>
      <Row label={t.port}><NumberField value={port} onChange={setPort} min={1} max={65535} disabled={busy}/></Row>
      <Row label={text("允许网络消息自动复制验证码","Allow automatic code copying")} hint={t.copyHint}><Toggle checked={status?.settings.allow_copy??false} disabled={busy||!status} onChange={v=>void run(()=>setHttpCopy(v))}/></Row>
      {dirty&&<button className="btn" disabled={busy||!Number.isInteger(port)||port<1||port>65535} onClick={()=>void run(()=>configureHttp(status!.settings.enabled,bind,port))}>{status?.settings.enabled?t.apply:t.save}</button>}
    </Section>
    <Section title={text("接收链接","Receiver link")} action={<button className="network-interface" disabled={!status||status.settings.bind==="127.0.0.1"} aria-haspopup="dialog" onClick={()=>{setDialog("ip");void listHttpAddresses().then(setAddresses).catch(e=>setError(String(e)));}}><span>{text("网卡","Interface")}</span><span>{host}</span><span aria-hidden="true">▾</span></button>}>
      <code className="network-receiver-url"><span className="network-url-endpoint">{endpoint}</span>{displayedParams.map(([key,value],index)=><span key={key}><span className="network-url-punctuation">{index===0?"?":"&"}</span><span className="network-url-key">{key}</span><span className="network-url-punctuation">=</span><span className={key==="token"?"network-url-token":"network-url-value"}>{value}</span></span>)}</code>
      <div className="network-link-actions">
        <button className="btn primary" disabled={!status?.token||host.includes("<")} onClick={()=>void copy(testUrl,"test")}>{copied==="test"?t.copied:text("复制链接","Copy link")}</button>
        <div className="network-link-secondary">
          <button className="network-quiet-action" disabled={!status?.token||host.includes("<")} onClick={()=>void copy(url,"sender")}>{copied==="sender"?t.copied:text("仅复制接口地址","Copy API URL")}</button>
          <button className="network-quiet-action" disabled={busy||!status} onClick={()=>setDialog("reset")}>{t.reset}</button>
        </div>
      </div>
    </Section>
    <Disclosure title={t.guide} hint={t.guideHint}>
      <p className="hint-text">{text("若无法访问，请检查本机防火墙配置。","If unreachable, check your local firewall configuration.")}</p>
      <Disclosure title={t.generic}>
        <code className="network-code">GET /api/v1/notifications?token=YOUR_TOKEN&amp;device_name=Android&amp;body=URL_ENCODED_MESSAGE<br/>POST /api/v1/notifications?token=YOUR_TOKEN</code>
        <p>{text("GET 参数须 URL 编码；POST 支持 JSON 或表单。令牌也可通过 Authorization: Bearer 或请求体 token 传入，只能选择一种位置。URL 包含凭据，请勿分享。","URL-encode GET parameters. POST accepts JSON or form bodies. Authorization: Bearer and body token are also supported; use exactly one location. URLs contain credentials: do not share.")}</p>
        <div className="network-table-wrap"><table className="network-table"><thead><tr><th>{t.field}</th><th>{t.requirement}</th><th>{t.meaning}</th></tr></thead><tbody>{[["token",t.required,text("统一接入令牌","Shared receiver token")],["body / content",t.required,t.bodyField],["device_name",t.optional,text("展示名称，最多 256 字节；不是可信身份","Display name, up to 256 bytes; not an authenticated identity")],["kind",t.optional,t.kindField],["sender / from",t.optional,t.senderField],["title",t.optional,t.titleField],["app_identifier",t.optional,t.appField],["message_id",t.optional,t.idField]].map(([f,r,d])=><tr key={f}><th>{f}</th><td>{r}</td><td>{d}</td></tr>)}</tbody></table></div>
        <p>{t.responseHint}</p><p className="hint-text">{t.limits}</p>
        <p className="hint-text">{text("复制链接可直接进行 GET 测试。正式 POST 配置使用“仅复制接口地址”，不带示例正文。服务需先开启；15 秒内重复发送相同内容可能被去重。","Use Copy link for a GET test. For POST, use Copy API URL without sample text. Enable the receiver first; identical messages within 15 seconds may be deduplicated.")}</p>
        <Disclosure title="curl · PowerShell"><pre className="network-code">{curl}</pre><button className="btn" onClick={()=>void copy(curl,"curl")}>{copied==="curl"?t.copied:t.copy}</button></Disclosure>
        <Disclosure title="Python · urllib"><p>{t.pythonHint}</p><pre className="network-code">{python}</pre><button className="btn" onClick={()=>void copy(python,"python")}>{copied==="python"?t.copied:t.copy}</button></Disclosure>
      </Disclosure>
      <Disclosure title={t.android}><p>{text("Webhook 填入“仅复制接口地址”的链接（不带示例正文）。选择 POST 表单，参数为 device_name=Android&from=[from]&content=[content]；也可使用 GET，将同样参数追加到链接并由发送器编码。","Use the copied URL. POST form: device_name=Android&from=[from]&content=[content]. GET is also supported; URL-encode the message parameters.")}</p></Disclosure>
      <Disclosure title={t.ios}><p>{text("信息自动化 → 获取 URL 内容 → POST JSON。URL 填入接收链接，字典填写 body、sender、device_name，无需额外鉴权头。立即运行、锁屏和网络权限需按 iOS 版本验证。","Message automation → Get Contents of URL → POST JSON. Use the receiver URL and dictionary fields body, sender and device_name; no extra auth header is needed. Test lock-screen behavior and permissions on your iOS version.")}</p></Disclosure><p className="hint-text">{t.publicHint}</p>
    </Disclosure>
    {(error||status?.error)&&<p className="bad-text" role="alert">{error||status?.error}</p>}
    {dialog&&<Modal title={dialog==="ip"?t.chooseAddress:t.resetTitle} onClose={()=>{if(!busy)setDialog(null);}}>
      {dialog==="ip"?<><p>{t.addressHint}</p><div className="network-addresses">{addresses.map(a=><button className={`network-address${a.address===ip?" selected":""}`} key={`${a.name}:${a.address}`} onClick={()=>{setIp(a.address);setCopied(null);setDialog(null);}}><strong>{a.address}</strong><small>{a.name}</small></button>)}</div>{!addresses.length&&<p>{t.noAddresses}</p>}</>:<><p>{text("重置后所有发送端的旧链接立即失效，需要重新配置。","Resetting immediately invalidates every sender's old URL. Reconfigure all senders.")}</p><button className="btn network-danger" disabled={busy} onClick={()=>void run(async()=>{await resetHttpToken();setCopied(null);setDialog(null);})}>{t.reset}</button></>}
    </Modal>}
  </>;
}
