import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Row, Section } from "../components";
interface Status { authorized: boolean; registered_shortcut: string | null; last_error: string | null }
export function MacInput() {
  const [status,setStatus]=useState<Status|null>(null);
  const [key,setKey]=useState("CommandOrControl+Shift+V");
  const [message,setMessage]=useState("");
  const [busy,setBusy]=useState(false);
  const refresh=async()=>{ const s=await invoke<Status>("get_input_status");setStatus(s);return s; };
  useEffect(()=>{
    let disposed=false;
    void invoke<Status>("get_input_status").then(s=>{if(!disposed){setStatus(s);setKey(s.registered_shortcut??"CommandOrControl+Shift+V");}}).catch(e=>{if(!disposed)setMessage(String(e));});
    const onFocus=()=>{void refresh().catch(e=>{if(!disposed)setMessage(String(e));});};
    window.addEventListener("focus",onFocus);
    const subscription=listen<string>("input-result",e=>{if(!disposed)setMessage(e.payload);});
    return()=>{disposed=true;window.removeEventListener("focus",onFocus);void subscription.then(stop=>stop());};
  },[]);
  const action=async(fn:()=>Promise<unknown>)=>{setBusy(true);setMessage("");try{await fn();await refresh();}catch(e){setMessage(String(e));}finally{setBusy(false);}};
  return <Section title="Mac 快捷键填入" description="点击目标输入框，再按快捷键并松开。仅使用最近 120 秒内的验证码，不自动提交表单。">
    <Row label="辅助功能权限" hint="未授权或控件不兼容时，仍可点击验证码复制。">
      <span>{status?.authorized?"已授权":"未授权"}</span>
      <button className="btn" disabled={busy} onClick={()=>void action(()=>invoke("request_input_access"))}>打开权限设置</button>
    </Row>
    <Row label="填入快捷键" hint="默认 ⌘⇧V；CommandOrControl 表示 Mac 的 Command 键。">
      <input className="input" aria-label="填入快捷键" value={key} onChange={e=>setKey(e.target.value)} disabled={busy}/>
      <button className="btn" disabled={busy||!key.trim()} onClick={()=>void action(()=>invoke("set_input_shortcut",{accelerator:key.trim()}))}>保存快捷键</button>
    </Row>
    <p className="hint-text">{status?.registered_shortcut?`当前已注册：${status.registered_shortcut}`:"快捷键尚未注册，请检查冲突并重新保存。"}</p>
    {(message||status?.last_error)&&<p role="status">{message||status?.last_error}</p>}
  </Section>;
}
