//! 专属的 UIA 工作线程。
//!
//! UIA 的 COM 接口**绝不跨线程**：接口全部留在这个线程上，
//! 只把纯数据传回去（理由见 crate 文档）。
//!
//! 这样带来的两个好处：
//!
//! * 调用方可以带超时等结果 —— UIA 卡住时只是"这次不给候选"，
//!   UI 线程（托盘、弹窗）一秒都不会被拖住。
//! * 完全不用碰 COM 的跨线程接口封送。

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use log::{info, warn};
use smspop_core::config::InsertMode;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationElement};

use crate::{insert, probe, InsertOutcome, ProbeOutcome};

/// 工作线程收到的活。
enum Command {
    Probe {
        reply: Sender<ProbeOutcome>,
    },
    Insert {
        code: String,
        reply: Sender<InsertOutcome>,
    },
    Shutdown,
}

/// 探测与写入的唯一入口。
///
/// 可以 `Send` 到别的线程（它内部只有一个 `Sender`）。
pub struct UiaWorker {
    commands: Sender<Command>,
    thread: Option<JoinHandle<()>>,
}

impl UiaWorker {
    /// 起工作线程并初始化 COM / UIA。
    ///
    /// `mode` 和 `type_delay` 是填入方式（`direct` / `simulate`）以及逐字间隔；
    /// 它们在启动时就定了，之后整条线程都用这一套。
    ///
    /// 会等到初始化完才返回，免得第一次探测白等一个超时。
    pub fn spawn(mode: InsertMode, type_delay: Duration) -> Result<Self, String> {
        let (commands, receiver) = mpsc::channel::<Command>();
        let (ready, ready_receiver) = mpsc::channel::<Result<(), String>>();

        let thread = thread::Builder::new()
            .name("smspop-uia".to_string())
            .spawn(move || main(receiver, ready, mode, type_delay))
            .map_err(|error| format!("起 UIA 工作线程失败：{error}"))?;

        match ready_receiver.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(())) => Ok(Self {
                commands,
                thread: Some(thread),
            }),
            Ok(Err(error)) => Err(error),
            Err(_) => Err("UIA 工作线程初始化超时".to_string()),
        }
    }

    /// 探测当前焦点。
    ///
    /// 超时返回 [`ProbeOutcome::TimedOut`] —— 调用方当作"这次不给候选"处理。
    pub fn probe(&self, timeout: Duration) -> ProbeOutcome {
        let (reply, receiver) = mpsc::channel();

        if self.commands.send(Command::Probe { reply }).is_err() {
            return ProbeOutcome::Failed("UIA 工作线程已经退出".to_string());
        }

        match receiver.recv_timeout(timeout) {
            Ok(outcome) => outcome,

            // ★ 超时只是"这次不给候选"。工作线程还在那边跑，
            //   跑完发现接收端没了就把结果丢掉，不影响下一次探测。
            Err(_) => ProbeOutcome::TimedOut,
        }
    }

    /// 把验证码写进**上一次探测到的**那个控件。
    ///
    /// ★ 只应该在用户点击候选条时调用。
    pub fn insert(&self, code: &str, timeout: Duration) -> InsertOutcome {
        let (reply, receiver) = mpsc::channel();

        let command = Command::Insert {
            code: code.to_string(),
            reply,
        };

        if self.commands.send(command).is_err() {
            return InsertOutcome::no("UIA 工作线程已经退出，请手动粘贴");
        }

        match receiver.recv_timeout(timeout) {
            Ok(outcome) => outcome,
            Err(_) => InsertOutcome::no("写入超时，请手动粘贴"),
        }
    }
}

impl Drop for UiaWorker {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);

        // ★ 故意不 join：万一工作线程正卡在某次 UIA 调用里，
        //   等它就等于让退出流程陪着一起卡住。进程马上要结束了，放着就行。
        self.thread.take();
    }
}

fn main(
    commands: Receiver<Command>,
    ready: Sender<Result<(), String>>,
    mode: InsertMode,
    type_delay: Duration,
) {
    // MTA：UIA 客户端在 MTA 下工作良好，而且不用自己再跑一个消息循环
    let com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };

    if com.is_err() {
        // 别的库可能已经用别的模式初始化过 COM 了 —— 不算致命，继续试
        warn!("CoInitializeEx 返回 {com:?}，继续尝试");
    }

    let automation: IUIAutomation =
        match unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) } {
            Ok(automation) => automation,
            Err(error) => {
                let _ = ready.send(Err(format!("创建 UIA 失败：{error}")));
                return;
            }
        };

    info!("UIA 工作线程已就绪");

    let _ = ready.send(Ok(()));

    // 上一次探测到的元素 —— 用户点击时写的就是它
    let mut target: Option<IUIAutomationElement> = None;

    while let Ok(command) = commands.recv() {
        match command {
            Command::Probe { reply } => {
                let result = probe::run(&automation, mode);
                target = result.element;

                // 接收端可能已经超时走了，发失败也没关系
                let _ = reply.send(result.outcome);
            }

            Command::Insert { code, reply } => {
                let outcome = insert::run(target.as_ref(), &code, mode, type_delay);
                let _ = reply.send(outcome);
            }

            Command::Shutdown => break,
        }
    }

    if com.is_ok() {
        unsafe { CoUninitialize() };
    }
}
