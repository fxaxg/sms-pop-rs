/**
 * i18n：中文 + English。
 *
 * 不引库 —— 一个字典对象 + 一个 React Context 就够了。
 * 语言来源：config.general.language（auto/zh/en），auto 时看 navigator.language。
 */
import { createContext, useContext } from "react";

export type Lang = "zh" | "en";
export type LanguageSetting = string; // "auto" | "zh" | "en"（宽松些，容错配置值）

export function resolveLang(setting: LanguageSetting): Lang {
  if (setting === "zh" || setting === "zh-CN") return "zh";
  if (setting === "en") return "en";
  return navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en";
}

const zh = {
  nav: {
    connection: "连接",
    notifications: "通知",
    otp: "验证码",
    general: "通用",
  },
  common: {
    save: "保存",
    saved: "✓ 已保存",
    unsaved: "有未保存的修改",
    loading: "载入配置…",
    seconds: "秒",
  },
  link: {
    // 链路状态（按 state key 翻译，detail 原样展示）
    stopped: "已停止",
    adapter_unsupported: "本机蓝牙不支持",
    advertising: "等待 iPhone 连接",
    waiting: "等待链路建立",
    connected: "已连接，正在订阅",
    subscribed: "等待通知验证",
    reconnecting: "重连中",
    faulted: "出错",
  },
  connection: {
    statusTitle: "连接状态",
    waitingTitle: "等待连接 iPhone",
    waitingBody: "保持 SmsPop 开启，然后在 iPhone 的蓝牙设置中选择这台电脑。",
    connectingTitle: "正在连接 iPhone",
    connectingBody: "已发现 iPhone，正在建立通知连接，通常只需要几秒。",
    verifyingTitle: "iPhone 已连接，等待验证",
    verifyingBody: "请向 iPhone 发送一条真实短信或通知。收到后才会显示“已就绪”。",
    readyTitle: "iPhone 通知已连接",
    readyBody: "已收到来自 iPhone 的真实通知，连接工作正常。",
    errorTitle: "连接没有完成",
    errorBody: "请查看下方原因；如果反复失败，请按“重新配对”步骤操作。",
    unsupportedTitle: "电脑蓝牙不可用",
    stoppedTitle: "SmsPop 连接已停止",
    stoppedBody: "重新启动 SmsPop 后会自动尝试连接。",
    guideTitle: "首次连接",
    guideDesc: "保持 SmsPop 开启，再完成以下操作。",
    step1: "确认电脑蓝牙已打开",
    step1Ok: "本机蓝牙就绪。",
    step1Bad:
      "这台电脑的蓝牙适配器不支持 BLE 外设角色，SmsPop 无法在这台电脑上工作。",
    openBluetooth: "打开蓝牙设置",
    step2: "在 iPhone 的蓝牙设置里找到这台电脑并配对",
    step2Desc:
      "配对时 iPhone 会弹「是否允许接收你的 iPhone 通知」—— 必须点允许，否则一条通知都收不到（而且不会有任何报错）。",
    step3: "发送一条真实通知验证连接",
    step3Desc:
      "只有真实 iPhone 通知能验证连接。下面的按钮只测试电脑弹窗和验证码功能。",
    sendTest: "测试电脑弹窗",
    sending: (n: number) => `${n} 秒后发出…`,
    sent: "已发送 ✓",
    testHint: "此测试不经过 iPhone，不能证明蓝牙连接正常。",
    repairTitle: "无法连接？重新配对",
    repairBody: "先在 Windows 中删除 iPhone，再在 iPhone 中忽略这台电脑。保持 SmsPop 开启，然后重新配对并允许“共享系统通知”。",
    technicalDetails: "技术详情",
    noTechnicalDetails: "暂时没有错误详情。",
    tipsTitle: "通知权限",
    tipsLabel: "检查共享系统通知",
    tipsBody: "去 iPhone：设置 → 蓝牙 → 点这台电脑右边的 ⓘ → 打开「共享系统通知」。",
  },
  notifications: {
    title: "通知弹出",
    desc: "把 iPhone 的通知实时弹到屏幕右下角。关掉后只剩验证码通知（如果验证码增强开着）。",
    enabled: "启用通知弹出",
    duration: "停留时长",
    durationHint: "过了就自动消失",
    maxVisible: "最多同屏",
    maxVisibleHint: "超出的先关掉最旧的",
    filterTitle: "过滤",
    filterDesc:
      "黑名单优先于白名单。App 填 iOS 的 bundle id（如 com.tencent.xin），一行一条。",
    excludeApps: "排除的 App",
    excludeAppsHint: "这些 App 的通知永不弹出",
    includeApps: "只接受这些 App",
    includeAppsHint: "留空表示不限制",
    excludeKeywords: "排除关键词",
    excludeKeywordsHint: "命中任一关键词就不弹",
    includeKeywords: "只接受关键词",
    includeKeywordsHint: "留空表示不限制",
  },
  otp: {
    title: "验证码增强",
    desc: "自动识别通知里的验证码。绝不自动填入 —— 只有你点候选条，验证码才会写进输入框。",
    enabled: "启用验证码增强",
    autoCopy: "自动复制到剪贴板",
    autoCopyHint: "收到验证码就复制，随时可以手动粘贴",
    caretTitle: "光标候选条",
    caretDesc: "收到验证码时，如果光标正好在输入框里，光标旁会出现「填入」按钮。",
    caretEnabled: "启用光标候选条",
    caretDuration: "停留时长",
    watchSeconds: "监听窗口",
    watchSecondsHint: "收到验证码后，这段时间内点进输入框仍会弹出候选条",
    gap: "与光标间距",
    gapHint: "默认 24，给中文输入法的候选窗留位置",
    insertionTitle: "填入方式",
    insertionDesc: "绝大多数情况不用动这里。",
    mode: "模式",
    modeHint: "direct 快且不要求焦点；simulate 适合顽固的输入框",
    modeDirect: "direct — 直接写入",
    modeSimulate: "simulate — 模拟键盘",
    typeDelay: "逐字间隔",
  },
  general: {
    startupTitle: "启动",
    autostart: "开机自启",
    autostartHint: "登录 Windows 后自动在后台运行",
    language: "界面语言",
    languageHint: "托盘菜单的语言重启应用后生效",
    langAuto: "跟随系统",
    filesTitle: "文件位置",
    configFile: "配置文件",
    logFile: "日志",
    openDir: "打开所在目录",
    aboutTitle: "关于",
    version: "版本",
    versionText: (v: string) => `SmsPop v${v} · MIT License`,
    promise: "安全承诺",
    promiseText: "验证码只在你点击候选条时填入；应用不联网、不上传任何通知内容。",
  },
  toast: {
    copyCode: (code: string) => `点击复制 ${code}`,
    copied: "✓ 已复制",
    dismiss: "关闭",
  },
  caret: {
    fill: "填入",
    filling: "填入中…",
    done: "✓ 已填入",
    failed: "✗ 写入失败，请手动粘贴（已复制到剪贴板）",
  },
};

export type Dict = typeof zh;

const en: Dict = {
  nav: {
    connection: "Connection",
    notifications: "Notifications",
    otp: "OTP",
    general: "General",
  },
  common: {
    save: "Save",
    saved: "✓ Saved",
    unsaved: "Unsaved changes",
    loading: "Loading config…",
    seconds: "s",
  },
  link: {
    stopped: "Stopped",
    adapter_unsupported: "Bluetooth unsupported",
    advertising: "Waiting for iPhone",
    waiting: "Waiting for link",
    connected: "Connected, subscribing",
    subscribed: "Waiting for notification test",
    reconnecting: "Reconnecting",
    faulted: "Error",
  },
  connection: {
    statusTitle: "Connection status",
    waitingTitle: "Waiting for iPhone",
    waitingBody: "Keep SmsPop open, then select this PC in your iPhone's Bluetooth settings.",
    connectingTitle: "Connecting to iPhone",
    connectingBody: "iPhone was found. SmsPop is setting up notifications; this usually takes a few seconds.",
    verifyingTitle: "iPhone connected, awaiting verification",
    verifyingBody: "Send a real SMS or notification to the iPhone. SmsPop will show Ready only after receiving it.",
    readyTitle: "iPhone notifications connected",
    readyBody: "A real iPhone notification was received. The connection is working.",
    errorTitle: "Connection did not complete",
    errorBody: "Check the reason below. If it keeps failing, follow the re-pair steps.",
    unsupportedTitle: "PC Bluetooth unavailable",
    stoppedTitle: "SmsPop connection stopped",
    stoppedBody: "Restart SmsPop to try connecting again.",
    guideTitle: "First connection",
    guideDesc: "Keep SmsPop open while completing these steps.",
    step1: "Make sure Bluetooth is on",
    step1Ok: "Bluetooth is ready.",
    step1Bad:
      "This PC's Bluetooth adapter doesn't support the BLE peripheral role — SmsPop can't work here.",
    openBluetooth: "Open Bluetooth settings",
    step2: "Pair this PC from your iPhone's Bluetooth settings",
    step2Desc:
      "iPhone will ask \"Allow receiving notifications?\" while pairing — you MUST allow it, otherwise nothing arrives (and nothing complains).",
    step3: "Verify with a real notification",
    step3Desc:
      "Only a real iPhone notification verifies the connection. The button below tests PC popups and OTP features only.",
    sendTest: "Test PC popup",
    sending: (n: number) => `Sending in ${n}s…`,
    sent: "Sent ✓",
    testHint: "This test bypasses the iPhone and does not prove that Bluetooth is working.",
    repairTitle: "Can't connect? Re-pair",
    repairBody: "Remove the iPhone in Windows, then forget this PC on the iPhone. Keep SmsPop open, pair again, and allow Share System Notifications.",
    technicalDetails: "Technical details",
    noTechnicalDetails: "No error details right now.",
    tipsTitle: "Notification permission",
    tipsLabel: "Check Share System Notifications",
    tipsBody: "On iPhone: Settings → Bluetooth → tap ⓘ next to this PC → enable \"Share System Notifications\".",
  },
  notifications: {
    title: "Notification popups",
    desc: "Mirror iPhone notifications to the bottom-right corner. Turn off to keep OTP notifications only (if OTP assist is on).",
    enabled: "Enable popups",
    duration: "Duration",
    durationHint: "Auto-dismiss after this",
    maxVisible: "Max visible",
    maxVisibleHint: "Oldest closes first",
    filterTitle: "Filters",
    filterDesc:
      "Blocklist wins over allowlist. Apps use iOS bundle ids (e.g. com.tencent.xin), one per line.",
    excludeApps: "Blocked apps",
    excludeAppsHint: "Never pop these",
    includeApps: "Only these apps",
    includeAppsHint: "Empty = no limit",
    excludeKeywords: "Blocked keywords",
    excludeKeywordsHint: "Any match suppresses the popup",
    includeKeywords: "Only these keywords",
    includeKeywordsHint: "Empty = no limit",
  },
  otp: {
    title: "OTP assist",
    desc: "Detects verification codes in notifications. Never auto-fills — the code is only written when you click the bar.",
    enabled: "Enable OTP assist",
    autoCopy: "Auto-copy to clipboard",
    autoCopyHint: "Copy codes so you can paste manually anytime",
    caretTitle: "Caret candidate bar",
    caretDesc: "When a code arrives and the caret is in a text field, a \"Fill\" pill appears next to it.",
    caretEnabled: "Enable candidate bar",
    caretDuration: "Bar duration",
    watchSeconds: "Watch window",
    watchSecondsHint: "After a code arrives, the bar still appears if you click into a field within this time",
    gap: "Gap from caret",
    gapHint: "24 by default, leaving room for IME candidate windows",
    insertionTitle: "Insertion",
    insertionDesc: "You almost never need to touch this.",
    mode: "Mode",
    modeHint: "direct is fast and needs no focus; simulate works on stubborn fields",
    modeDirect: "direct — write value",
    modeSimulate: "simulate — type keys",
    typeDelay: "Per-key delay",
  },
  general: {
    startupTitle: "Startup",
    autostart: "Run at startup",
    autostartHint: "Run in background after signing in to Windows",
    language: "Language",
    languageHint: "Tray menu language applies after restart",
    langAuto: "System",
    filesTitle: "Files",
    configFile: "Config file",
    logFile: "Logs",
    openDir: "Open folder",
    aboutTitle: "About",
    version: "Version",
    versionText: (v: string) => `SmsPop v${v} · MIT License`,
    promise: "Privacy",
    promiseText: "Codes are only inserted when you click the bar. No networking, nothing uploaded.",
  },
  toast: {
    copyCode: (code: string) => `Click to copy ${code}`,
    copied: "✓ Copied",
    dismiss: "Dismiss",
  },
  caret: {
    fill: "Fill",
    filling: "Filling…",
    done: "✓ Filled",
    failed: "✗ Failed — paste manually (already copied)",
  },
};

const DICTS: Record<Lang, Dict> = { zh, en };

export const LangContext = createContext<Lang>("zh");

/** 取当前语言的字典。 */
export function useT(): Dict {
  return DICTS[useContext(LangContext)];
}

/** 非 React 场景直接拿字典。 */
export function dict(lang: Lang): Dict {
  return DICTS[lang];
}
