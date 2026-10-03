import { useEffect, useState } from "react";
import {
  getSettingsMeta,
  openConfigDir,
  openLogsDir,
  setAutostart,
  type SettingsMeta,
} from "../../shared/api";
import { Row, Section, Toggle } from "../components";

/** 「通用」页：自启、文件位置、关于。 */
export function General() {
  const [meta, setMeta] = useState<SettingsMeta | null>(null);

  useEffect(() => {
    getSettingsMeta().then(setMeta).catch(() => {});
  }, []);

  return (
    <>
      <Section title="启动">
        <Row label="开机自启" hint="登录 Windows 后自动在后台运行">
          <Toggle
            checked={meta?.autostart_enabled ?? false}
            onChange={async (value) => {
              const enabled = await setAutostart(value).catch(() => value);
              setMeta((prev) => (prev ? { ...prev, autostart_enabled: enabled } : prev));
            }}
          />
        </Row>
      </Section>

      <Section title="文件位置">
        <Row label="配置文件" hint={meta?.config_path}>
          <button className="btn" onClick={() => openConfigDir()}>
            打开所在目录
          </button>
        </Row>
        <Row label="日志" hint={meta?.log_dir}>
          <button className="btn" onClick={() => openLogsDir()}>
            打开所在目录
          </button>
        </Row>
      </Section>

      <Section title="关于">
        <Row label="版本">
          <span className="hint-text">SmsPop v{meta?.version ?? "…"} · MIT License</span>
        </Row>
        <Row label="安全承诺">
          <span className="hint-text">
            验证码只在你点击候选条时填入；应用不联网、不上传任何通知内容。
          </span>
        </Row>
      </Section>
    </>
  );
}
