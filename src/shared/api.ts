/**
 * 与 Rust 后端的全部交互：invoke 命令 + 事件订阅。
 * 类型与 src-tauri/src/types.rs 里的 serde 结构保持一致。
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

// ── 配置（与 smspop-core 的 Config 对应） ─────────────────────

export interface FilterOptions {
  exclude_apps: string[];
  include_apps: string[];
  exclude_keywords: string[];
  include_keywords: string[];
}

export interface PopupOptions {
  duration_seconds: number;
  max_visible: number;
  width: number;
}

export interface NotificationOptions {
  app_rules: AppRule[];
  enabled: boolean;
  filter: FilterOptions;
  popup: PopupOptions;
}

export interface AppRule {
  app_id: string;
  name: string;
  enabled: boolean;
  target: string;
}
export const testAppRule = (rule: AppRule) => invoke<void>("test_app_rule", { rule });
export const validateAppRule = (rule: AppRule) => invoke<void>("validate_app_rule", { rule });
export const toastOpenApp = () => invoke<void>("toast_open_app");

export interface CaretOptions {
  enabled: boolean;
  duration_seconds: number;
  watch_seconds: number;
  gap: number;
}

export type InsertMode = "direct" | "simulate";
export type ThemeSetting = "auto" | "light" | "dark";

export interface InsertionOptions {
  mode: InsertMode;
  type_delay_ms: number;
}

export interface OtpOptions {
  enabled: boolean;
  auto_copy: boolean;
  caret: CaretOptions;
  insertion: InsertionOptions;
}

export interface Config {
  notifications: NotificationOptions;
  otp: OtpOptions;
  general: {
    language: string; // "auto" | "zh" | "en"
    theme: ThemeSetting;
  };
}

export const getConfig = () => invoke<Config>("get_config");
export const saveConfig = (config: Config) => invoke<void>("save_config", { config });
export const onThemeChanged = (handler: (theme: ThemeSetting) => void) =>
  listen<ThemeSetting>("theme-changed", (event) => handler(event.payload));

// ── 元信息 ────────────────────────────────────────────────────

export interface SettingsMeta {
  version: string;
  config_path: string;
  log_dir: string;
  autostart_enabled: boolean;
}

export const getSettingsMeta = () => invoke<SettingsMeta>("get_settings_meta");
export const setAutostart = (enabled: boolean) =>
  invoke<boolean>("set_autostart", { enabled });
export const openConfigDir = () => invoke<void>("open_config_dir");
export const openLogsDir = () => invoke<void>("open_logs_dir");
export const openBluetoothSettings = () => invoke<void>("open_bluetooth_settings");

// ── 链路状态 ──────────────────────────────────────────────────

export interface LinkStatePayload {
  state: string;
  label: string;
  detail: string | null;
  ready: boolean;
  awaiting_verification: boolean;
}

export const getLinkState = () => invoke<LinkStatePayload>("get_link_state");
export const onLinkState = (handler: (payload: LinkStatePayload) => void) =>
  listen<LinkStatePayload>("link-state", (event) => handler(event.payload));

export const sendTestNotification = () => invoke<void>("send_test_notification");

export interface ManagedDevice {
  id: string;
  name: string;
  address_hint: string;
  preferred: boolean;
  enabled: boolean;
  current: boolean;
  connected: boolean;
  online: boolean;
  verified: boolean;
  last_connected_at: number | null;
  last_verified_at: number | null;
  battery_level: number | null;
}

export const listDevices = () => invoke<ManagedDevice[]>("list_devices");
export const setPreferredDevice = (id: string) =>
  invoke<void>("set_preferred_device", { id });
export const setDeviceEnabled = (id: string, enabled: boolean) =>
  invoke<void>("set_device_enabled", { id, enabled });
export const forgetDevice = (id: string) => invoke<void>("forget_device", { id });
export const onDevicesChanged = (handler: (devices: ManagedDevice[]) => void) =>
  listen<ManagedDevice[]>("devices-changed", (event) => handler(event.payload));

// ── toast 窗口 ────────────────────────────────────────────────

export interface ToastPayload {
  app_id: string | null;
  open_app_name: string | null;
  origin: string;
  body: string;
  code: string | null;
  duration_secs: number;
}

export const getToastPayload = () => invoke<ToastPayload | null>("get_toast_payload");
export const toastClick = () =>
  invoke<string | null>("toast_click", { label: getCurrentWindow().label });
export const toastClose = () =>
  invoke<void>("toast_close", { label: getCurrentWindow().label });
export const toastResize = (height: number) =>
  invoke<void>("toast_resize", { label: getCurrentWindow().label, height });

// ── 候选条窗口 ────────────────────────────────────────────────

/** [generation, code, durationSecs] */
export type CaretOfferTuple = [number, string, number, string | null];

export const getCaretOffer = () => invoke<CaretOfferTuple | null>("get_caret_offer");
export const caretLayout = (generation: number, width: number, height: number) =>
  invoke<void>("caret_layout", { layout: { generation, width, height } });
export const caretInsert = (generation: number) =>
  invoke<[boolean, string] | null>("caret_insert", { generation });
export const caretHide = (generation: number) => invoke<void>("caret_hide", { generation });
export const onCaretOffer = (handler: () => void) =>
  listen<number>("caret-offer", () => handler());
