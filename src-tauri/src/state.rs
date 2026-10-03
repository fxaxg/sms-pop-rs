//! 全局应用状态。

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex, RwLock};

use log::{info, warn};
use smspop_ble::LinkState;
use smspop_core::config::Config;
use smspop_core::dedup::NotificationDeduplicator;
use smspop_uia::UiaWorker;
use tauri::AppHandle;

use crate::paths;
use crate::types::{CaretOffer, ToastPayload};

pub struct AppState {
    /// 当前生效的配置（设置界面保存后整体替换）。
    pub config: RwLock<Config>,

    /// 最近一次链路状态（人话描述 + 补充说明）。
    pub link_state: RwLock<(LinkState, Option<String>)>,

    /// 通知去重。
    pub dedup: NotificationDeduplicator,

    /// UIA 工作线程。初始化失败时为 `None`（只损失光标候选功能）。
    pub uia: RwLock<Option<UiaWorker>>,

    /// 蓝牙链路的停止开关。
    pub link_stop: Arc<AtomicBool>,

    /// 最近一次「订阅成功」的时间。
    ///
    /// iOS 会在订阅成功后重推通知中心存量 —— 之后几秒内到达的通知
    /// 要按**内容**去重（uid 是新的），否则重启/重连会重弹一堆旧通知。
    pub subscribed_at: Mutex<Option<std::time::Instant>>,

    /// 当前打开的 toast 窗口标签（旧的在前）。
    pub toasts: Mutex<VecDeque<String>>,

    /// 还没来得及被前端取走的 toast 负载（label → payload）。
    pub pending_toasts: Mutex<HashMap<String, ToastPayload>>,

    /// toast 窗口编号。
    pub toast_seq: AtomicU64,

    /// 托盘「开机自启」勾选项（切换时要同步它的勾选状态）。
    pub autostart_item: Mutex<Option<tauri::menu::CheckMenuItem<tauri::Wry>>>,

    /// 当前生效的候选条提议。
    pub caret_offer: Mutex<Option<CaretOffer>>,

    /// 候选条提议代数。
    pub caret_generation: AtomicU64,
}

impl AppState {
    pub fn new(app: &AppHandle) -> Self {
        let config_path = paths::config_path(app);

        let config = match Config::load_or_default(&config_path) {
            Ok(config) => config,
            Err(error) => {
                // 解析失败（比如旧版配置结构）—— 备份后启用默认，不让用户卡在启动上。
                warn!("配置读取失败（{error}），备份后使用默认配置");
                let backup = config_path.with_extension("json.bak");
                let _ = std::fs::copy(&config_path, backup);
                Config::default()
            }
        };

        let uia =
            match UiaWorker::spawn(config.otp.insertion.mode, config.otp.insertion.type_delay()) {
                Ok(worker) => Some(worker),
                Err(error) => {
                    warn!("UIA 工作线程启动失败：{error}（光标候选功能不可用）");
                    None
                }
            };

        Self {
            config: RwLock::new(config),
            link_state: RwLock::new((LinkState::Stopped, None)),
            dedup: NotificationDeduplicator::default(),
            uia: RwLock::new(uia),
            link_stop: Arc::new(AtomicBool::new(false)),
            subscribed_at: Mutex::new(None),
            toasts: Mutex::new(VecDeque::new()),
            pending_toasts: Mutex::new(HashMap::new()),
            toast_seq: AtomicU64::new(0),
            autostart_item: Mutex::new(None),
            caret_offer: Mutex::new(None),
            caret_generation: AtomicU64::new(0),
        }
    }

    /// 读一份配置快照。
    pub fn config(&self) -> Config {
        self.config
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// 替换配置；如果填入方式变了，顺带重启 UIA 工作线程让新配置生效。
    pub fn apply_config(&self, config: Config) {
        let old = self.config();
        let mut guard = self
            .config
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = config.clone();
        drop(guard);

        let insertion_changed = old.otp.insertion.mode != config.otp.insertion.mode
            || old.otp.insertion.type_delay() != config.otp.insertion.type_delay();

        if insertion_changed {
            info!("填入方式变化，重启 UIA 工作线程");
            let mut uia = self
                .uia
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *uia =
                UiaWorker::spawn(config.otp.insertion.mode, config.otp.insertion.type_delay()).ok();
        }
    }
}
