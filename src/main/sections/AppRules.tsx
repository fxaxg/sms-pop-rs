import { useState } from "react";
import { testAppRule, validateAppRule, type AppRule, type Config } from "../../shared/api";
import { useT } from "../../shared/i18n";
import { Modal, Section, Toggle } from "../components";

export function AppRules({ config, update }: { config: Config; update: (mutate: (draft: Config) => void) => void }) {
  const t = useT();
  const [draft, setDraft] = useState<AppRule | null>(null);
  const [editing, setEditing] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [removeIndex, setRemoveIndex] = useState<number | null>(null);
  const rules = config.notifications.app_rules;
  const start = (rule: AppRule, index: number | null = null) => { setDraft({ ...rule }); setEditing(index); setError(null); };
  const save = async () => {
    if (!draft || !draft.app_id.trim() || !draft.name.trim() || !draft.target.trim()) { setError(t.rules.required); return; }
    const rule = { ...draft, app_id: draft.app_id.trim(), name: draft.name.trim(), target: draft.target.trim(), enabled: editing === null ? true : draft.enabled };
    if (rule.enabled && rules.some((other, index) => index !== editing && other.enabled && other.app_id.toLowerCase() === rule.app_id.toLowerCase())) { setError(t.rules.duplicate); return; }
    setBusy(true);
    try { await validateAppRule(rule); } catch (err) { setError(String(err)); return; } finally { setBusy(false); }
    update((config) => { if (editing === null) config.notifications.app_rules.push(rule); else config.notifications.app_rules[editing] = rule; });
    setDraft(null);
  };
  const presets = [["com.tencent.xin", t.rules.wechat, "weixin://"], ["com.tencent.mqq", "QQ", "tencent://"], ["com.apple.mobilemail", t.rules.mail, ""]];
  return <Section title={t.rules.title} action={<button className="btn" onClick={() => start({ app_id: "com.tencent.xin", name: t.rules.wechat, enabled: false, target: "weixin://" })}>{t.rules.add}</button>}>
    {rules.length === 0 && <p className="connection-help">{t.ui.selectTemplate}</p>}
    {rules.map((rule, index) => <div className="app-rule" key={`${rule.app_id}-${index}`}>
      <div className="app-rule-info"><strong>{rule.name}</strong><small>{t.rules.open(rule.name)}</small></div>
      <div className="rule-controls"><Toggle checked={rule.enabled} onChange={async (enabled) => {
        if (enabled) { if (rules.some((other, otherIndex) => otherIndex !== index && other.enabled && other.app_id.toLowerCase() === rule.app_id.toLowerCase())) { start(rule, index); setError(t.rules.duplicate); return; } try { await validateAppRule(rule); } catch (err) { start(rule, index); setError(String(err)); return; } }
        update((d) => { d.notifications.app_rules[index].enabled = enabled; });
      }} />
         <button className="btn" onClick={() => start(rule, index)}>{t.rules.edit}</button><button className="btn danger" onClick={() => setRemoveIndex(index)}>{t.rules.remove}</button></div>
    </div>)}
    {draft && <Modal title={editing === null ? t.rules.add : t.rules.edit} onClose={() => { if (!busy) setDraft(null); }}>
      <div className="rule-editor">
      <label>{t.ui.chooseSource}<select value={presets.some(([id]) => id === draft.app_id) ? draft.app_id : "custom"} onChange={(event) => {
        const preset = presets.find(([id]) => id === event.target.value);
        setDraft(preset ? { ...draft, app_id: preset[0], name: preset[1], target: preset[2] } : { ...draft, app_id: "", name: "", target: "" });
      }}><option value="custom">{t.ui.custom}</option>{presets.map(([id, name]) => <option key={id} value={id}>{name}</option>)}</select></label>
      <label>{t.rules.name}<input value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} /></label>
      {!presets.some(([id]) => id === draft.app_id) && <label>{t.rules.source}<input placeholder="com.example.app" value={draft.app_id} onChange={(e) => setDraft({ ...draft, app_id: e.target.value })} /></label>}
      <label>{t.rules.target}<input placeholder="weixin:// 或 https://example.com/inbox" value={draft.target} onChange={(e) => setDraft({ ...draft, target: e.target.value })} /></label>
      <p className="connection-help">{t.rules.targetHint}</p>
      {error && <p className="bad-text" role="alert">{error}</p>}
      <div className="modal-actions"><button className="btn" disabled={busy} onClick={async () => { setBusy(true); setError(null); try { await testAppRule(draft); } catch (err) { setError(String(err)); } finally { setBusy(false); } }}>{t.rules.test}</button><button className="btn" disabled={busy} onClick={() => setDraft(null)}>{t.rules.cancel}</button><button className="btn primary" disabled={busy} onClick={save}>{t.ui.done}</button></div>
    </div></Modal>}
    {removeIndex !== null && <Modal title={t.rules.remove} onClose={() => setRemoveIndex(null)}><p>{t.rules.removeConfirm}</p><footer className="modal-actions"><button className="btn" onClick={() => setRemoveIndex(null)}>{t.rules.cancel}</button><button className="btn danger" onClick={() => { update((d) => { d.notifications.app_rules.splice(removeIndex, 1); }); setRemoveIndex(null); }}>{t.rules.remove}</button></footer></Modal>}
  </Section>;
}
