import { useState } from "react";
import { testAppRule, validateAppRule, type AppRule, type Config } from "../../shared/api";
import { useT } from "../../shared/i18n";
import { Section, Toggle } from "../components";

export function AppRules({ config, update }: { config: Config; update: (mutate: (draft: Config) => void) => void }) {
  const t = useT();
  const [draft, setDraft] = useState<AppRule | null>(null);
  const [editing, setEditing] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const rules = config.notifications.app_rules;
  const start = (rule: AppRule, index: number | null = null) => { setDraft({ ...rule }); setEditing(index); setError(null); };
  const save = async () => {
    if (!draft || !draft.app_id.trim() || !draft.name.trim() || !draft.target.trim()) { setError(t.rules.required); return; }
    if (rules.some((rule, index) => index !== editing && rule.enabled && rule.app_id.toLowerCase() === draft.app_id.trim().toLowerCase())) { setError(t.rules.duplicate); return; }
    const rule = { ...draft, app_id: draft.app_id.trim(), name: draft.name.trim(), target: draft.target.trim(), enabled: true };
    try { await validateAppRule(rule); } catch (err) { setError(String(err)); return; }
    update((config) => { if (editing === null) config.notifications.app_rules.push(rule); else config.notifications.app_rules[editing] = rule; });
    setDraft(null);
  };
  return <Section title={t.rules.title} description={t.rules.desc}>
    <div className="rule-presets">
      {[["com.tencent.xin", t.rules.wechat, "weixin://"], ["com.tencent.mqq", "QQ", "tencent://"], ["com.apple.mobilemail", t.rules.mail, ""]].map(([app_id, name, target]) => <button className="btn" key={app_id} onClick={() => start({ app_id, name, enabled: false, target })}>+ {name}</button>)}
      <button className="btn" onClick={() => start({ app_id: "", name: "", enabled: false, target: "" })}>{t.rules.add}</button>
    </div>
    {rules.length === 0 && !draft && <p className="connection-help">{t.rules.empty}</p>}
    {rules.map((rule, index) => <div className="app-rule" key={`${rule.app_id}-${index}`}>
      <div className="app-rule-info"><strong>{rule.name}</strong><small>{rule.app_id}</small><small title={rule.target}>{rule.target}</small></div>
      <div className="rule-controls"><Toggle checked={rule.enabled} onChange={async (enabled) => {
        if (enabled) { try { await validateAppRule(rule); } catch (err) { start(rule, index); setError(String(err)); return; } }
        update((d) => { d.notifications.app_rules[index].enabled = enabled; });
      }} />
        <button className="btn" onClick={() => start(rule, index)}>{t.rules.edit}</button><button className="btn danger" onClick={() => { if (confirm(t.rules.removeConfirm)) update((d) => { d.notifications.app_rules.splice(index, 1); }); }}>{t.rules.remove}</button></div>
    </div>)}
    {draft && <div className="rule-editor">
      <label>{t.rules.name}<input value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} /></label>
      <label>{t.rules.source}<input placeholder="com.tencent.xin" value={draft.app_id} onChange={(e) => setDraft({ ...draft, app_id: e.target.value })} /></label>
      <label>{t.rules.target}<input placeholder="weixin:// 或 https://example.com/inbox" value={draft.target} onChange={(e) => setDraft({ ...draft, target: e.target.value })} /></label>
      <p className="connection-help">{t.rules.targetHint}</p>
      {error && <p className="bad-text" role="alert">{error}</p>}
      <div className="rule-controls"><button className="btn" onClick={async () => { setError(null); try { await testAppRule(draft); } catch (err) { setError(String(err)); } }}>{t.rules.test}</button><button className="btn" onClick={() => setDraft(null)}>{t.rules.cancel}</button><button className="btn primary" onClick={save}>{t.rules.apply}</button></div>
    </div>}
  </Section>;
}
