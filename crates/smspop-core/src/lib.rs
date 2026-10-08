//! SmsPop 的纯逻辑层。
//!
//! 这一层**不依赖任何平台 API** —— 没有蓝牙、没有窗口、没有 COM。
//! 里面装的是整个项目里最容易写错、也最值得单测的东西：
//!
//! * [`ancs`] —— ANCS 报文构造与分片响应解析（字节级，中文最容易在这里乱码）
//! * [`otp`] —— 从通知文本里挑出验证码（打分制，最怕把订单号当验证码）
//! * [`insertion`] —— 要不要把验证码写进别人输入框（**最危险的一步**，写错就覆盖用户内容）
//! * [`placement`] —— 候选条该摆在光标的哪个位置（纯几何）
//! * [`filter`] / [`dedup`] —— 通知该不该弹、是不是重复
//! * [`config`] —— 配置模型

pub mod ancs;
pub mod app_rules;
pub mod config;
pub mod dedup;
pub mod devices;
pub mod filter;
pub mod insertion;
pub mod model;
pub mod otp;
pub mod otp_source;
pub mod placement;
pub mod uuid;

pub use model::PhoneNotification;
pub mod ingress;
pub mod link;
pub mod otp_candidate;

pub mod input_action;
