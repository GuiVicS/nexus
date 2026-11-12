//! Transportes: JSON-RPC em socket Unix (Electron), HTTP local (arte; WebSocket de RPC só em dev)
//! e HTTP na rede local exclusivo do Remote (celular).
use crate::core::{Core, Event};
use crate::remote;
use crate::rpc::{self, RpcError};
use crate::vdev::PadState;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, UnixListener};
use tokio::sync::mpsc;
use tower_http::services::{ServeDir, ServeFile};

pub struct Started {
    pub dev_addr: Option<SocketAddr>,
    pub remote_addr: Option<SocketAddr>,
}

fn event_json(e: &Event) -> String {
    json!({ "jsonrpc": "2.0", "method": e.method, "params": e.params }).to_string()
}

pub async fn handle_text(core: &Arc<Core>, text: &str) -> Option<String> {
    let req: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "JSON inválido" } }).to_string()),
    };
    let id = req.get("id").cloned();
    let Some(method) = req.get("method").and_then(|m| m.as_str()) else {
        return Some(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32600, "message": "requisição inválida" } }).to_string());
    };
    let res: Result<Value, RpcError> = rpc::dispatch(core, method, req.get("params").cloned().unwrap_or(Value::Null)).await;
    let id = id?;
    Some(match res {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": e.code, "message": e.message, "data": e.data } }),
    }
    .to_string())
}

/// Liga uma conexão (qualquer transporte) ao despachante: entrada de texto → respostas + eventos.
async fn session(core: Arc<Core>, mut inbound: mpsc::Receiver<String>, out: mpsc::Sender<String>) {
    let mut events = core.events.subscribe();
    loop {
        tokio::select! {
            msg = inbound.recv() => {
                let Some(msg) = msg else { break };
                let (c, o) = (core.clone(), out.clone());
                tokio::spawn(async move {
                    if let Some(r) = handle_text(&c, &msg).await { let _ = o.send(r).await; }
                });
            }
            ev = events.recv() => match ev {
                Ok(e) => { if out.send(event_json(&e)).await.is_err() { break } }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let _ = out.send(event_json(&Event { method: "events.lagged".into(), params: json!({}) })).await;
                }
                Err(_) => break,
            }
        }
    }
}

async fn serve_unix_rpc(core: Arc<Core>, listener: UnixListener) {
    loop {
        let Ok((stream, _)) = listener.accept().await else { continue };
        let core = core.clone();
        tokio::spawn(async move {
            let (r, mut w) = stream.into_split();
            let (in_tx, in_rx) = mpsc::channel::<String>(64);
            let (out_tx, mut out_rx) = mpsc::channel::<String>(256);
            tokio::spawn(async move {
                while let Some(m) = out_rx.recv().await {
                    if w.write_all(m.as_bytes()).await.is_err() || w.write_all(b"\n").await.is_err() {
                        break;
                    }
                }
            });
            tokio::spawn(async move {
                let mut lines = BufReader::new(r).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    if l.len() > 256 * 1024 || in_tx.send(l).await.is_err() {
                        break;
                    }
                }
            });
            session(core, in_rx, out_tx).await;
        });
    }
}

fn loopback_host(h: &HeaderMap) -> bool {
    match h.get(header::HOST).and_then(|v| v.to_str().ok()) {
        None => true,
        Some(v) => v.starts_with("127.0.0.1") || v.starts_with("localhost") || v.starts_with("[::1]"),
    }
}

async fn art_route(State(core): State<Arc<Core>>, headers: HeaderMap, Path((id, kind)): Path<(String, String)>) -> Response {
    if !loopback_host(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    crate::art::serve(&core, &id, &kind).await
}

async fn rpc_ws(State(core): State<Arc<Core>>, headers: HeaderMap, ws: WebSocketUpgrade) -> Response {
    if !loopback_host(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    ws.max_message_size(256 * 1024).on_upgrade(move |socket| async move {
        use futures_util::{SinkExt, StreamExt};
        let (mut tx, mut rx) = socket.split();
        let (in_tx, in_rx) = mpsc::channel::<String>(64);
        let (out_tx, mut out_rx) = mpsc::channel::<String>(256);
        tokio::spawn(async move {
            while let Some(m) = out_rx.recv().await {
                if tx.send(Message::Text(m.into())).await.is_err() {
                    break;
                }
            }
        });
        tokio::spawn(async move {
            while let Some(Ok(m)) = rx.next().await {
                match m {
                    Message::Text(t) => {
                        if in_tx.send(t.as_str().to_string()).await.is_err() {
                            break;
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
        });
        session(core, in_rx, out_tx).await;
    })
}

pub fn local_router(core: Arc<Core>, dev: bool) -> Router {
    let mut r = Router::new().route("/healthz", get(|| async { "ok" })).route("/art/{id}/{kind}", get(art_route));
    if dev {
        r = r.route("/rpc", get(rpc_ws));
        if let Some(dir) = core.cfg.shell_dir.clone() {
            r = r.fallback_service(ServeDir::new(&dir).not_found_service(ServeFile::new(dir.join("index.html"))));
        }
    }
    r.with_state(core)
}

// ---------- Remote (celular) ----------

async fn remote_hello(State(core): State<Arc<Core>>, ConnectInfo(addr): ConnectInfo<SocketAddr>, Query(q): Query<HashMap<String, String>>) -> Response {
    match core.hub.token_ok(addr.ip(), q.get("t").map(String::as_str).unwrap_or("")) {
        Ok(()) => {
            let p = core.hub.pairing();
            let taken: Vec<u8> = p.devices.iter().filter_map(|d| d.player).collect();
            axum::Json(json!({ "ok": true, "uinput": p.uinput, "playersTaken": taken })).into_response()
        }
        Err(e) => (StatusCode::FORBIDDEN, axum::Json(json!({ "ok": false, "error": e }))).into_response(),
    }
}

async fn remote_ws(State(core): State<Arc<Core>>, ConnectInfo(addr): ConnectInfo<SocketAddr>, Query(q): Query<HashMap<String, String>>, ws: WebSocketUpgrade) -> Response {
    if let Err(e) = core.hub.token_ok(addr.ip(), q.get("t").map(String::as_str).unwrap_or("")) {
        return (StatusCode::FORBIDDEN, e).into_response();
    }
    if core.hub.conns.load(std::sync::atomic::Ordering::SeqCst) >= remote::MAX_CONNS {
        return (StatusCode::SERVICE_UNAVAILABLE, "Conexões demais").into_response();
    }
    ws.max_message_size(16 * 1024).max_frame_size(16 * 1024).on_upgrade(move |s| remote_conn(core, s))
}

async fn remote_conn(core: Arc<Core>, mut socket: WebSocket) {
    let core2 = core.clone();
    let Some(_slot) = core2.hub.slot() else {
        let _ = socket.send(Message::Close(None)).await;
        return;
    };
    let opened = std::time::Instant::now();
    // até 240 mensagens/s por conexão (o celular manda ~60/s); o excesso é descartado
    let (mut budget, mut window) = (240u32, std::time::Instant::now());
    let mut dev: Option<nexus_protocol::RemoteDevice> = None;
    let mut pad = PadState::default();
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    let send = |v: Value| Message::Text(v.to_string().into());
    loop {
        tokio::select! {
            m = socket.recv() => {
                let Some(Ok(m)) = m else { break };
                let Message::Text(t) = m else { if matches!(m, Message::Close(_)) { break } else { continue } };
                if window.elapsed() >= Duration::from_secs(1) {
                    (budget, window) = (240, std::time::Instant::now());
                }
                if budget == 0 { continue }
                budget -= 1;
                let Ok(msg) = serde_json::from_str::<Value>(t.as_str()) else { continue };
                match (msg.get("t").and_then(|t| t.as_str()), &dev) {
                    (Some("ping"), _) => { let _ = socket.send(send(json!({ "t": "pong" }))).await; }
                    (Some("join"), None) => {
                        let mode = msg.get("mode").and_then(|m| m.as_str()).unwrap_or("gamepad");
                        let player = msg.get("player").and_then(|p| p.as_u64()).map(|p| p as u8);
                        let name = msg.get("name").and_then(|n| n.as_str()).unwrap_or("Celular");
                        match core.hub.join(name, mode, player) {
                            Ok(id) => {
                                let d = core.hub.pairing().devices.into_iter().find(|d| d.id == id).expect("aparelho recém-entrou");
                                let _ = socket.send(send(json!({ "t": "joined", "mode": d.mode, "player": d.player, "uinput": core.hub.sink.available() }))).await;
                                dev = Some(d);
                            }
                            Err(e) => { let _ = socket.send(send(json!({ "t": "error", "error": e }))).await; break; }
                        }
                    }
                    (_, Some(d)) => core.hub.handle(d, &mut pad, &msg),
                    _ => {}
                }
            }
            _ = tick.tick() => {
                if dev.is_none() && opened.elapsed() > Duration::from_secs(20) {
                    break; // conectou e não entrou: libera a vaga
                }
                if let Some(d) = &dev {
                    if !core.hub.is_joined(&d.id) {
                        let _ = socket.send(send(json!({ "t": "kicked" }))).await;
                        break;
                    }
                }
            }
        }
    }
    if let Some(d) = dev {
        core.hub.leave(&d.id);
    }
}

// ---------- envio de jogos (ROMs) pelo celular/computador ----------

const ROMS_PAGE: &str = include_str!("roms_upload.html");

async fn roms_page(State(core): State<Arc<Core>>, ConnectInfo(addr): ConnectInfo<SocketAddr>, Query(q): Query<HashMap<String, String>>) -> Response {
    if let Err(e) = core.hub.token_ok(addr.ip(), q.get("t").map(String::as_str).unwrap_or("")) {
        return (StatusCode::FORBIDDEN, axum::response::Html(format!("<meta name=viewport content='width=device-width'><p style='font:18px sans-serif;padding:24px'>{e}</p>"))).into_response();
    }
    let page = ROMS_PAGE.replace("__SYSTEMS__", &crate::roms::systems_json().to_string());
    ([(header::CACHE_CONTROL, "no-store")], axum::response::Html(page)).into_response()
}

/// POST /remote/upload?t=<token>&system=<pasta>&name=<arquivo> — corpo = o arquivo. Grava em ~/ROMs/<console>/.
async fn roms_upload(State(core): State<Arc<Core>>, ConnectInfo(addr): ConnectInfo<SocketAddr>, Query(q): Query<HashMap<String, String>>, body: axum::body::Body) -> Response {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    let err = |code: StatusCode, e: &str| (code, axum::Json(json!({ "ok": false, "error": e }))).into_response();
    if let Err(e) = core.hub.token_ok(addr.ip(), q.get("t").map(String::as_str).unwrap_or("")) {
        return err(StatusCode::FORBIDDEN, e);
    }
    let core2 = core.clone();
    let Some(_slot) = core2.hub.slot() else { return err(StatusCode::SERVICE_UNAVAILABLE, "Conexões demais. Tente de novo em instantes.") };
    let (tmp, dest) = match crate::roms::upload_target(q.get("system").map(String::as_str).unwrap_or(""), q.get("name").map(String::as_str).unwrap_or("")) {
        Ok(t) => t,
        Err(e) => return err(StatusCode::BAD_REQUEST, &e),
    };
    let Ok(mut f) = tokio::fs::File::create(&tmp).await else { return err(StatusCode::INTERNAL_SERVER_ERROR, "não foi possível gravar") };
    let mut total: u64 = 0;
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let ok = match chunk {
            Ok(b) => {
                total += b.len() as u64;
                total <= crate::roms::MAX_UPLOAD && f.write_all(&b).await.is_ok()
            }
            Err(_) => false,
        };
        if !ok {
            drop(f);
            let _ = tokio::fs::remove_file(&tmp).await;
            return err(StatusCode::BAD_REQUEST, if total > crate::roms::MAX_UPLOAD { "Arquivo grande demais (máx. 8 GB)" } else { "O envio foi interrompido" });
        }
    }
    if total == 0 || f.flush().await.is_err() {
        let _ = tokio::fs::remove_file(&tmp).await;
        return err(StatusCode::BAD_REQUEST, "Arquivo vazio");
    }
    drop(f);
    if tokio::fs::rename(&tmp, &dest).await.is_err() {
        let _ = tokio::fs::remove_file(&tmp).await;
        return err(StatusCode::INTERNAL_SERVER_ERROR, "não foi possível gravar");
    }
    let name = dest.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
    tracing::info!("ROM recebida pelo celular: {} ({total} bytes)", dest.display());
    core.emit("roms.received", json!({ "name": name, "size": total }));
    core.notify("download", "Jogo recebido", &name, None);
    core.kick.notify_one();
    axum::Json(json!({ "ok": true, "name": name, "size": total })).into_response()
}

/// Porta de entrada do Remote: só rede local e só a própria página (Origin) — vale para todas as rotas.
async fn remote_guard(ConnectInfo(addr): ConnectInfo<SocketAddr>, req: axum::extract::Request, next: axum::middleware::Next) -> Response {
    if !remote::local_peer(addr.ip()) && std::env::var("NEXUS_REMOTE_ANY_NET").as_deref() != Ok("1") {
        tracing::warn!("Remote: recusado {} (fora da rede local)", addr.ip());
        return (StatusCode::FORBIDDEN, "Somente na rede local").into_response();
    }
    let h = req.headers();
    let s = |n: header::HeaderName| h.get(n).and_then(|v| v.to_str().ok());
    if !remote::origin_ok(s(header::ORIGIN), s(header::HOST)) {
        tracing::warn!("Remote: recusada origem {:?} de {}", s(header::ORIGIN), addr.ip());
        return (StatusCode::FORBIDDEN, "Origem não permitida").into_response();
    }
    next.run(req).await
}

pub fn remote_router(core: Arc<Core>) -> Router {
    let mut r = Router::new()
        .route("/remote/hello", get(remote_hello))
        .route("/remote/ws", get(remote_ws))
        .route("/roms", get(roms_page))
        .route("/remote/upload", axum::routing::post(roms_upload).layer(axum::extract::DefaultBodyLimit::disable()));
    if let Some(dir) = core.cfg.remote_dir.clone() {
        r = r.fallback_service(ServeDir::new(&dir).not_found_service(ServeFile::new(dir.join("index.html"))));
    }
    r.layer(axum::middleware::from_fn(remote_guard)).with_state(core)
}

pub async fn start(core: Arc<Core>) -> anyhow::Result<Started> {
    let cfg = core.cfg.clone();
    let _ = std::fs::remove_file(&cfg.rpc_socket);
    let _ = std::fs::remove_file(&cfg.http_socket);
    let rpc_l = UnixListener::bind(&cfg.rpc_socket)?;
    let http_l = UnixListener::bind(&cfg.http_socket)?;
    {
        use std::os::unix::fs::PermissionsExt;
        for p in [&cfg.rpc_socket, &cfg.http_socket] {
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600))?;
        }
    }
    tokio::spawn(serve_unix_rpc(core.clone(), rpc_l));
    let app = local_router(core.clone(), false);
    tokio::spawn(async move {
        let _ = axum::serve(http_l, app).await;
    });
    let mut started = Started { dev_addr: None, remote_addr: None };
    if let Some(addr) = cfg.dev_http {
        let l = TcpListener::bind(addr).await?;
        started.dev_addr = Some(l.local_addr()?);
        let app = local_router(core.clone(), true);
        tokio::spawn(async move {
            let _ = axum::serve(l, app).await;
        });
    }
    match TcpListener::bind(cfg.remote_bind).await {
        Ok(l) => {
            let a = l.local_addr()?;
            core.hub.set_port(a.port());
            started.remote_addr = Some(a);
            let app = remote_router(core.clone());
            tokio::spawn(async move {
                let _ = axum::serve(l, app.into_make_service_with_connect_info::<SocketAddr>()).await;
            });
        }
        Err(e) => tracing::warn!("Remote indisponível ({}): {e}", cfg.remote_bind),
    }
    let _ = remote::lan_ip;
    Ok(started)
}
