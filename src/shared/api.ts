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
  enabled: boolean;
  filter: FilterOptions;
  popup: PopupOptions;
}

export interface CaretOptions {
  enabled: boolean;
  duration_seconds: number;
  watch_seconds: number;
  gap: number;
}

export type InsertMode = "direct" | "simulate";

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
}

export const getConfig = () => invoke<Config>("get_config");
export const saveConfig = (config: Config) => invoke<void>("save_config", { config });

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
}

export const getLinkState = () => invoke<LinkStatePayload>("get_link_state");
export const onLinkState = (handler: (payload: LinkStatePayload) => void) =>
  listen<LinkStatePayload>("link-state", (event) => handler(event.payload));

export const sendTestNotification = () => invoke<void>("send_test_notification");

// ── toast 窗口 ────────────────────────────────────────────────

export interface ToastPayload {
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

// ── 候选条窗口 ────────────────────────────────────────────────

/** [generation, code, durationSecs] */
export type CaretOfferTuple = [number, string, number];

export const getCaretOffer = () => invoke<CaretOfferTuple | null>("get_caret_offer");
export const caretLayout = (generation: number, width: number, height: number) =>
  invoke<void>("caret_layout", { layout: { generation, width, height } });
export const caretInsert = (generation: number) =>
  invoke<[boolean, string] | null>("caret_insert", { generation });
export const caretHide = () => invoke<void>("caret_hide");
export const onCaretOffer = (handler: () => void) =>
  listen<number>("caret-offer", () => handler());
