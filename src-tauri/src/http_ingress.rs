//! 默认关闭的入站 Webhook。统一令牌用当前 Windows 用户 DPAPI 加密保存。
use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use smspop_core::ingress::IncomingNotification;
use std::{
    collections::HashMap,
    future::IntoFuture,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};
use tokio::sync::{mpsc, oneshot};

#[derive(Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub allow_copy: bool,
    #[serde(default)]
    pub last_received: Option<u64>,
    #[serde(default)]
    token_hash: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    pub enabled: bool,
    pub bind: String,
    pub port: u16,
    pub devices: Vec<Device>,
    #[serde(default)]
    pub encrypted_token: Vec<u8>,
    #[serde(default)]
    pub allow_copy: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            bind: "0.0.0.0".into(),
            port: 24836,
            devices: vec![],
            encrypted_token: vec![],
            allow_copy: false,
        }
    }
}
#[derive(Serialize)]
pub struct Status {
    pub token: Option<String>,
    pub settings: Settings,
    pub running: bool,
    pub error: Option<String>,
}
struct Inner {
    generation: u64,
    settings: Settings,
    running: bool,
    error: Option<String>,
    shutdown: Option<oneshot::Sender<()>>,
    stopped: Option<oneshot::Receiver<()>>,
    seen: HashMap<String, Instant>,
    rate: (Instant, u32),
    seq: u32,
}
pub struct HttpIngress {
    inner: Arc<Mutex<Inner>>,
    operation: tokio::sync::Mutex<()>,
}
fn hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
fn path(app: &AppHandle) -> std::path::PathBuf {
    crate::paths::data_dir(app).join("http-ingress.json")
}
fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let target = path(app);
    let temp = target.with_extension("tmp");
    std::fs::write(
        &temp,
        serde_json::to_vec_pretty(settings).map_err(|_| "Serialize failed")?,
    )
    .map_err(|_| "Save failed")?;
    std::fs::rename(temp, target).map_err(|_| "Save failed".into())
}
fn main_only(w: &tauri::WebviewWindow) -> Result<(), String> {
    if w.label() == "main" {
        Ok(())
    } else {
        Err("Settings window required".into())
    }
}

#[derive(Serialize)]
pub struct LocalAddress {
    pub name: String,
    pub address: String,
}

#[tauri::command]
pub fn list_http_addresses(window: tauri::WebviewWindow) -> Result<Vec<LocalAddress>, String> {
    main_only(&window)?;
    let mut addresses: Vec<_> = if_addrs::get_if_addrs()
        .map_err(|e| format!("Cannot enumerate network interfaces: {e}"))?
        .into_iter()
        .filter_map(|interface| match interface.ip() {
            std::net::IpAddr::V4(ip) if !ip.is_unspecified() && !ip.is_multicast() => {
                Some(LocalAddress {
                    name: interface.name,
                    address: ip.to_string(),
                })
            }
            _ => None,
        })
        .collect();
    addresses.sort_by_key(|a| {
        (
            smspop_core::ingress::address_rank(&a.name, a.address.parse().unwrap()),
            a.address.clone(),
        )
    });
    addresses.dedup_by(|a, b| a.address == b.address);
    Ok(addresses)
}

#[tauri::command]
pub fn open_firewall_rules(window: tauri::WebviewWindow) -> Result<(), String> {
    main_only(&window)?;
    std::process::Command::new("mmc.exe")
        .arg("wf.msc")
        .spawn()
        .map_err(|e| format!("Cannot open firewall rules: {e}"))?;
    Ok(())
}
impl HttpIngress {
    pub fn load(app: &AppHandle) -> Self {
        let mut settings: Settings = match std::fs::read(path(app)) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|_| {
                log::warn!("HTTP settings invalid; listener disabled");
                Settings::default()
            }),
            Err(_) => Settings::default(),
        };
        if settings.encrypted_token.is_empty() {
            settings.enabled = false;
            settings.devices.clear();
            let token = format!(
                "{}{}",
                uuid::Uuid::new_v4().simple(),
                uuid::Uuid::new_v4().simple()
            );
            match crate::token_store::protect(token.as_bytes(), false).and_then(|cipher| {
                settings.encrypted_token = cipher;
                save(app, &settings)
            }) {
                Ok(()) => {}
                Err(_) => log::warn!("Receiver token initialization failed; receiver disabled"),
            }
        }
        Self {
            inner: Arc::new(Mutex::new(Inner {
                generation: 0,
                settings,
                running: false,
                error: None,
                shutdown: None,
                stopped: None,
                seen: HashMap::new(),
                rate: (Instant::now(), 0),
                seq: 0,
            })),
            operation: tokio::sync::Mutex::new(()),
        }
    }
}
#[tauri::command]
pub fn get_http_status(window: tauri::WebviewWindow, app: AppHandle) -> Result<Status, String> {
    main_only(&window)?;
    let state = app.state::<HttpIngress>();
    let inner = state.inner.lock().unwrap();
    let mut settings = inner.settings.clone();
    for d in &mut settings.devices {
        d.token_hash.clear();
    }
    Ok(Status {
        token: crate::token_store::protect(&inner.settings.encrypted_token, true)
            .ok()
            .and_then(|b| String::from_utf8(b).ok()),
        settings,
        running: inner.running,
        error: inner.error.clone(),
    })
}

#[derive(Clone)]
struct Server {
    generation: u64,
    inner: Arc<Mutex<Inner>>,
    queue: mpsc::Sender<(Device, IncomingNotification, u32)>,
}
fn reply(code: StatusCode, status: &str) -> Response {
    let mut response = (
        code,
        Json(serde_json::json!({"ok":code.is_success(),"status":status})),
    )
        .into_response();
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("referrer-policy", "no-referrer".parse().unwrap());
    response
}
async fn receive(State(server): State<Server>, request: Request) -> Response {
    // 浏览器跨站请求不属于受支持客户端，不开放 CORS。
    if request.headers().contains_key("origin") {
        return reply(StatusCode::FORBIDDEN, "browser_not_allowed");
    }
    if request.uri().to_string().len() > 8192 {
        return reply(StatusCode::URI_TOO_LONG, "url_too_long");
    }
    let query: HashMap<String, String> =
        match serde_urlencoded::from_str(request.uri().query().unwrap_or("")) {
            Ok(q) => q,
            Err(_) => return reply(StatusCode::BAD_REQUEST, "invalid_query"),
        };
    let header_token = request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(str::to_string);
    {
        let mut i = server.inner.lock().unwrap();
        if !i.settings.enabled || i.generation != server.generation {
            return reply(StatusCode::SERVICE_UNAVAILABLE, "disabled");
        }
        if i.rate.0.elapsed() >= Duration::from_secs(60) {
            i.rate = (Instant::now(), 0);
        }
        i.rate.1 += 1;
        if i.rate.1 > 120 {
            return reply(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
        }
    }
    let is_get = request.method() == axum::http::Method::GET;
    let content_type = request
        .headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    let bytes = match tokio::time::timeout(
        Duration::from_secs(5),
        axum::body::to_bytes(request.into_body(), 32768),
    )
    .await
    {
        Ok(Ok(b)) => b,
        Ok(Err(_)) => return reply(StatusCode::PAYLOAD_TOO_LARGE, "invalid_body_size"),
        Err(_) => return reply(StatusCode::REQUEST_TIMEOUT, "timeout"),
    };
    let mut values: serde_json::Map<String, serde_json::Value> = if is_get {
        query
            .iter()
            .filter(|(k, _)| k.as_str() != "token")
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect()
    } else {
        match content_type.as_str() {
            "application/json" => {
                match serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(&bytes) {
                    Ok(v) => v,
                    Err(_) => return reply(StatusCode::BAD_REQUEST, "invalid_json"),
                }
            }
            "application/x-www-form-urlencoded" => {
                match serde_urlencoded::from_bytes::<HashMap<String, String>>(&bytes) {
                    Ok(v) => v
                        .into_iter()
                        .map(|(k, v)| (k, serde_json::Value::String(v)))
                        .collect(),
                    Err(_) => return reply(StatusCode::BAD_REQUEST, "invalid_form"),
                }
            }
            _ => {
                return reply(
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "unsupported_content_type",
                )
            }
        }
    };
    let body_token = values
        .remove("token")
        .and_then(|v| v.as_str().map(str::to_string));
    let tokens: Vec<_> = [query.get("token").cloned(), header_token, body_token]
        .into_iter()
        .flatten()
        .collect();
    if tokens.len() != 1 {
        return reply(StatusCode::UNAUTHORIZED, "provide_one_token");
    }
    let device = {
        let i = server.inner.lock().unwrap();
        let token = crate::token_store::protect(&i.settings.encrypted_token, true).ok();
        if token.as_deref() != Some(tokens[0].as_bytes()) || tokens[0].is_empty() {
            return reply(StatusCode::UNAUTHORIZED, "unauthorized");
        }
        Device {
            id: "shared".into(),
            name: "网络转发".into(),
            allow_copy: i.settings.allow_copy,
            last_received: None,
            token_hash: hash(&tokens[0]),
        }
    };
    let message =
        match serde_json::from_value::<IncomingNotification>(serde_json::Value::Object(values)) {
            Ok(m) if m.validate().is_ok() => m,
            _ => return reply(StatusCode::BAD_REQUEST, "invalid_notification"),
        };
    let mut i = server.inner.lock().unwrap();
    // 在读取 body 期间被撤销的令牌不能继续入队。
    if !i.settings.enabled
        || i.generation != server.generation
        || crate::token_store::protect(&i.settings.encrypted_token, true)
            .ok()
            .is_none_or(|token| hash(&String::from_utf8_lossy(&token)) != device.token_hash)
    {
        return reply(StatusCode::UNAUTHORIZED, "revoked");
    }
    i.seen.retain(|_, t| t.elapsed() < Duration::from_secs(300));
    let source = message
        .device_name
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "网络转发".into());
    let mut device = device;
    device.name = source.clone();
    device.id = hash(&source);
    let key = if let Some(id) = &message.message_id {
        format!("{}:id:{id}", device.id)
    } else {
        format!(
            "{}:body:{}",
            device.id,
            hash(&format!(
                "{:?}|{:?}|{}|{}",
                message.sender, message.title, message.kind, message.body
            ))
        )
    };
    let ttl = if message.message_id.is_some() {
        300
    } else {
        15
    };
    if i.seen
        .get(&key)
        .is_some_and(|t| t.elapsed() < Duration::from_secs(ttl))
    {
        return reply(StatusCode::OK, "duplicate");
    }
    i.seq = i.seq.wrapping_add(1);
    if server
        .queue
        .try_send((device.clone(), message, i.seq))
        .is_err()
    {
        return reply(StatusCode::SERVICE_UNAVAILABLE, "queue_full");
    }
    i.seen.insert(key, Instant::now());
    if let Some(d) = i.settings.devices.iter_mut().find(|d| d.id == device.id) {
        d.last_received = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
    }
    reply(StatusCode::ACCEPTED, "accepted")
}

async fn launch(app: AppHandle) -> Result<(), String> {
    let state = app.state::<HttpIngress>();
    let settings = state.inner.lock().unwrap().settings.clone();
    if !settings.enabled {
        return Ok(());
    }
    if !matches!(settings.bind.as_str(), "127.0.0.1" | "0.0.0.0") || settings.port == 0 {
        return Err("Invalid bind address or port".into());
    }
    let listener = tokio::net::TcpListener::bind((settings.bind.as_str(), settings.port))
        .await
        .map_err(|e| format!("Listener unavailable: {e}"))?;
    let (queue, mut rx) = mpsc::channel::<(Device, IncomingNotification, u32)>(64);
    let worker_app = app.clone();
    let generation = state.inner.lock().unwrap().generation;
    tauri::async_runtime::spawn(async move {
        while let Some((device, message, uid)) = rx.recv().await {
            // 执行前再确认设备仍存在且未被撤销。
            let current = {
                let s = worker_app.state::<HttpIngress>();
                let i = s.inner.lock().unwrap();
                if i.settings.enabled
                    && i.generation == generation
                    && crate::token_store::protect(&i.settings.encrypted_token, true)
                        .ok()
                        .is_some_and(|token| {
                            hash(&String::from_utf8_lossy(&token)) == device.token_hash
                        })
                {
                    let mut device = device.clone();
                    device.allow_copy = i.settings.allow_copy;
                    Some(device)
                } else {
                    None
                }
            };
            if let Some(device) = current {
                crate::link_worker::dispatch_network(
                    &worker_app,
                    message.into_notification(format!("http:{}", device.id), device.name, uid),
                    device.allow_copy,
                );
            }
        }
    });
    let router = Router::new()
        .route("/api/v1/notifications", post(receive).get(receive))
        .layer(tower::limit::ConcurrencyLimitLayer::new(16))
        .with_state(Server {
            generation,
            inner: state.inner.clone(),
            queue,
        });
    let (tx, rx) = oneshot::channel();
    let (done, stopped) = oneshot::channel();
    let server_inner = state.inner.clone();
    {
        let mut i = state.inner.lock().unwrap();
        i.shutdown = Some(tx);
        i.stopped = Some(stopped);
        i.running = true;
        i.error = None;
    }
    tauri::async_runtime::spawn(async move {
        // 停止时立即释放 listener，不被空闲 keep-alive 连接拖住。
        let result = tokio::select! {
            result = axum::serve(listener, router).into_future() => result,
            _ = rx => Ok(()),
        };
        if let Err(e) = result {
            log::warn!("HTTP listener stopped: {e}");
        }
        server_inner.lock().unwrap().running = false;
        let _ = done.send(());
    });
    Ok(())
}

#[tauri::command]
pub async fn configure_http(
    window: tauri::WebviewWindow,
    app: AppHandle,
    enabled: bool,
    bind: String,
    port: u16,
) -> Result<(), String> {
    main_only(&window)?;
    if !matches!(bind.as_str(), "127.0.0.1" | "0.0.0.0") || port == 0 {
        return Err("Invalid bind or port".into());
    }
    let state = app.state::<HttpIngress>();
    let _guard = state.operation.lock().await;
    let stopped = {
        let mut i = state.inner.lock().unwrap();
        let mut next = i.settings.clone();
        next.enabled = enabled;
        next.bind = bind;
        next.port = port;
        save(&app, &next)?;
        i.generation = i.generation.wrapping_add(1);
        if let Some(tx) = i.shutdown.take() {
            let _ = tx.send(());
        }
        i.settings = next;
        i.error = None;
        i.stopped.take()
    };
    if let Some(stopped) = stopped {
        let _ = stopped.await;
    }
    if let Err(e) = launch(app.clone()).await {
        state.inner.lock().unwrap().error = Some(e.clone());
        return Err(e);
    }
    Ok(())
}
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<HttpIngress>();
        let _guard = state.operation.lock().await;
        if let Err(e) = launch(app.clone()).await {
            app.state::<HttpIngress>().inner.lock().unwrap().error = Some(e);
        }
    });
}
pub fn stop(app: &AppHandle) {
    if let Some(tx) = app
        .state::<HttpIngress>()
        .inner
        .lock()
        .unwrap()
        .shutdown
        .take()
    {
        let _ = tx.send(());
    }
}

#[tauri::command]
pub fn reset_http_token(window: tauri::WebviewWindow, app: AppHandle) -> Result<(), String> {
    main_only(&window)?;
    let state = app.state::<HttpIngress>();
    let mut i = state.inner.lock().unwrap();
    let mut next = i.settings.clone();
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    next.encrypted_token = crate::token_store::protect(token.as_bytes(), false)?;
    next.devices.clear();
    save(&app, &next)?;
    i.settings = next;
    i.seen.clear();
    Ok(())
}
#[tauri::command]
pub fn set_http_copy(
    window: tauri::WebviewWindow,
    app: AppHandle,
    allow_copy: bool,
) -> Result<(), String> {
    main_only(&window)?;
    let state = app.state::<HttpIngress>();
    let mut i = state.inner.lock().unwrap();
    let mut next = i.settings.clone();
    next.allow_copy = allow_copy;
    save(&app, &next)?;
    i.settings = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;
    fn fixture() -> (Router, mpsc::Receiver<(Device, IncomingNotification, u32)>) {
        let (queue, rx) = mpsc::channel(1);
        let inner = Arc::new(Mutex::new(Inner {
            settings: Settings {
                encrypted_token: crate::token_store::protect(b"secret", false).unwrap(),
                enabled: true,
                devices: vec![Device {
                    id: "test".into(),
                    name: "phone".into(),
                    allow_copy: false,
                    last_received: None,
                    token_hash: hash("secret"),
                }],
                ..Default::default()
            },
            generation: 0,
            running: true,
            error: None,
            shutdown: None,
            stopped: None,
            seen: HashMap::new(),
            rate: (Instant::now(), 0),
            seq: 0,
        }));
        (
            Router::new()
                .route("/api/v1/notifications", post(receive).get(receive))
                .with_state(Server {
                    inner,
                    queue,
                    generation: 0,
                }),
            rx,
        )
    }
    fn req(token: &str, kind: &str, body: &str) -> Request {
        Request::builder()
            .method("POST")
            .uri("/api/v1/notifications")
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", kind)
            .body(axum::body::Body::from(body.to_owned()))
            .unwrap()
    }
    #[test]
    fn authentication_limits_and_duplicates() {
        tauri::async_runtime::block_on(async {
            let (router, mut rx) = fixture();
            assert_eq!(
                router
                    .clone()
                    .oneshot(req("wrong", "application/json", r#"{"body":"test"}"#))
                    .await
                    .unwrap()
                    .status(),
                StatusCode::UNAUTHORIZED
            );
            let payload = r#"{"body":"验证码 123456","message_id":"one"}"#;
            assert_eq!(
                router
                    .clone()
                    .oneshot(req("secret", "application/json", payload))
                    .await
                    .unwrap()
                    .status(),
                StatusCode::ACCEPTED
            );
            assert!(rx.recv().await.is_some());
            assert_eq!(
                router
                    .clone()
                    .oneshot(req("secret", "application/json", payload))
                    .await
                    .unwrap()
                    .status(),
                StatusCode::OK
            );
            assert_eq!(
                router
                    .clone()
                    .oneshot(req(
                        "secret",
                        "application/json",
                        r#"{"body":"a","target":"cmd"}"#
                    ))
                    .await
                    .unwrap()
                    .status(),
                StatusCode::BAD_REQUEST
            );
            assert_eq!(
                router
                    .clone()
                    .oneshot(req("secret", "application/json", &"x".repeat(33000)))
                    .await
                    .unwrap()
                    .status(),
                StatusCode::PAYLOAD_TOO_LARGE
            );
            assert_eq!(
                router
                    .clone()
                    .oneshot(req(
                        "secret",
                        "application/x-www-form-urlencoded",
                        "from=1069&content=hello"
                    ))
                    .await
                    .unwrap()
                    .status(),
                StatusCode::ACCEPTED
            );
            assert_eq!(
                router
                    .oneshot(req("secret", "application/json", r#"{"body":"another"}"#))
                    .await
                    .unwrap()
                    .status(),
                StatusCode::SERVICE_UNAVAILABLE
            );
        });
    }
    #[test]
    fn browser_requests_and_disabled_receiver_rejected() {
        tauri::async_runtime::block_on(async {
            let (router, _rx) = fixture();
            let mut request = req("secret", "application/json", r#"{"body":"test"}"#);
            request
                .headers_mut()
                .insert("origin", "https://example.com".parse().unwrap());
            assert_eq!(
                router.oneshot(request).await.unwrap().status(),
                StatusCode::FORBIDDEN
            );
        });
        assert!(!Settings::default().enabled);
        assert_ne!(hash("a"), hash("b"));
    }

    #[test]
    fn loopback_http_transport_accepts_authenticated_form() {
        tauri::async_runtime::block_on(async {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let (router, mut rx) = fixture();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(axum::serve(listener, router).into_future());
            let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
            let body = "from=1069&content=hello";
            let request=format!("POST /api/v1/notifications HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer secret\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
            socket.write_all(request.as_bytes()).await.unwrap();
            let mut response = String::new();
            tokio::time::timeout(Duration::from_secs(3), socket.read_to_string(&mut response))
                .await
                .unwrap()
                .unwrap();
            assert!(response.starts_with("HTTP/1.1 202"));
            assert!(response.contains("accepted"));
            let (_, message, _) = rx.recv().await.unwrap();
            assert_eq!(message.body, "hello");
            server.abort();
        });
    }

    #[test]
    fn shared_token_get_and_body_post() {
        tauri::async_runtime::block_on(async {
            let (router, mut rx) = fixture();
            let request = Request::builder()
                .uri("/api/v1/notifications?token=secret&device_name=Android&body=hello")
                .body(axum::body::Body::empty())
                .unwrap();
            let response = router.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::ACCEPTED);
            assert_eq!(response.headers()["cache-control"], "no-store");
            assert_eq!(rx.recv().await.unwrap().0.name, "Android");
            let request = Request::builder()
                .method("POST")
                .uri("/api/v1/notifications")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    r#"{"token":"secret","body":"body token"}"#,
                ))
                .unwrap();
            assert_eq!(
                router.oneshot(request).await.unwrap().status(),
                StatusCode::ACCEPTED
            );
        });
    }
}
