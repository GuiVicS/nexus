//! Supervisor de jogos: lança o comando do Provider numa sessão própria, acompanha até tudo acabar
//! e devolve a tela ao Nexus. O jogo nunca é renderizado dentro da Shell.
use crate::core::Core;
use crate::procs;
use crate::providers::{LaunchPlan, Tracker};
use nexus_protocol::*;
use serde_json::json;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct ActiveGame {
    pub item_id: String,
    pub title: String,
    pub profile_id: String,
    pub started_unix: i64,
    pub state: GameState,
    pub pid: Option<u32>,
    pub sid: u32,
    pub tracker: Tracker,
    pub exit_code: Arc<AtomicI32>,
    pub stop: Arc<AtomicBool>,
    pub log: std::path::PathBuf,
}

#[derive(Debug)]
pub enum LaunchError {
    NoSession,
    Busy,
    NotFound,
    Forbidden,
    LimitReached,
    Failed(String),
}

impl ActiveGame {
    pub fn info(&self) -> GameSession {
        GameSession { item_id: self.item_id.clone(), title: self.title.clone(), pid: self.pid, started_at: self.started_unix, state: self.state }
    }
}

fn members(tracker: &Tracker, sid: u32) -> Vec<u32> {
    match tracker {
        Tracker::Process => procs::session_members(sid),
        Tracker::SteamApp(n) => procs::steam_game_pids(*n),
    }
}

fn spawn(plan: &LaunchPlan, scope_name: Option<String>, log: std::fs::File) -> std::io::Result<std::process::Child> {
    let mut argv = plan.argv.clone();
    if let Some(unit) = scope_name {
        let mut pre = vec!["systemd-run".to_string(), "--user".into(), "--scope".into(), "--collect".into(), "--quiet".into(), format!("--unit={unit}"), "--".into()];
        pre.append(&mut argv);
        argv = pre;
    }
    let mut c = Command::new(&argv[0]);
    c.args(&argv[1..]);
    for (k, v) in &plan.env {
        c.env(k, v);
    }
    if let Some(d) = &plan.cwd {
        c.current_dir(d);
    }
    let log2 = log.try_clone()?;
    c.stdin(Stdio::null()).stdout(Stdio::from(log)).stderr(Stdio::from(log2));
    unsafe {
        c.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    c.spawn()
}

impl Core {
    pub fn game_info(&self) -> Option<GameSession> {
        self.game.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|g| g.info())
    }

    pub async fn launch_item(self: &Arc<Self>, item_id: &str) -> Result<GameSession, LaunchError> {
        let profile = self.current_profile().ok_or(LaunchError::NoSession)?;
        if self.game.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
            return Err(LaunchError::Busy);
        }
        let item = self.find_item(item_id).await.ok_or(LaunchError::NotFound)?;
        if !self.visible_to(Some(&profile), &item) {
            return Err(LaunchError::Forbidden);
        }
        if self.accounts.remaining_min(&profile) == Some(0) {
            return Err(LaunchError::LimitReached);
        }
        let provider = self.provider_for(item_id).ok_or(LaunchError::NotFound)?;
        let plan = provider.launch(item_id).await.map_err(|e| LaunchError::Failed(e.to_string()))?;
        if plan.argv.is_empty() {
            return Err(LaunchError::Failed("comando vazio".into()));
        }
        let logs = self.cfg.cache_dir.join("logs");
        let _ = std::fs::create_dir_all(&logs);
        let log_path = logs.join(format!("{}.log", item_id.replace([':', '/'], "_")));
        let log = std::fs::File::create(&log_path).map_err(|e| LaunchError::Failed(e.to_string()))?;
        let scope = self.cfg.systemd_scope.then(|| format!("nexus-game-{}", crate::util::random_hex(6)));
        let mut child = spawn(&plan, scope, log).map_err(|e| LaunchError::Failed(format!("{}: {e}", plan.argv[0])))?;
        let pid = child.id();
        let exit_code = Arc::new(AtomicI32::new(i32::MIN));
        let ec = exit_code.clone();
        std::thread::spawn(move || {
            let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
            ec.store(code, Ordering::SeqCst);
        });
        let stop = Arc::new(AtomicBool::new(false));
        let g = ActiveGame {
            item_id: item_id.to_string(),
            title: item.title.clone(),
            profile_id: profile.id.clone(),
            started_unix: now_unix(),
            state: GameState::Launching,
            pid: Some(pid),
            sid: pid,
            tracker: plan.tracker.clone(),
            exit_code,
            stop,
            log: log_path,
        };
        let info = g.info();
        self.persist_game(&g);
        *self.game.lock().unwrap_or_else(|e| e.into_inner()) = Some(g);
        self.emit("game.launching", json!({ "session": info }));
        let core = self.clone();
        tokio::spawn(async move { core.track_game().await });
        Ok(info)
    }

    fn game_file(&self) -> std::path::PathBuf {
        self.cfg.runtime_dir.join("game.json")
    }

    /// Guarda o jogo/app aberto: se o Core reiniciar, ele o reencontra (o jogo vive num escopo próprio).
    fn persist_game(&self, g: &ActiveGame) {
        let tracker = match g.tracker {
            Tracker::Process => json!({ "process": g.sid }),
            Tracker::SteamApp(n) => json!({ "steam": n }),
        };
        let _ = std::fs::write(self.game_file(), json!({ "itemId": g.item_id, "title": g.title, "tracker": tracker, "startedAt": g.started_unix }).to_string());
    }

    /// Depois de o Core reiniciar, um jogo da Steam aberto antes continua rodando (escopo próprio):
    /// ao entrar num perfil, o Core volta a acompanhá-lo (tempo de jogo, Central, fechar).
    pub fn adopt_running_game(self: &Arc<Self>) {
        if self.game.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
            return;
        }
        let Some(profile) = self.current_profile() else { return };
        // 1) o que o Core anotou ao abrir (qualquer provedor), se o processo ainda existe
        if let Some(saved) = std::fs::read_to_string(self.game_file()).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
            if let Some(sid) = saved["tracker"]["process"].as_u64().map(|n| n as u32).filter(|sid| !procs::session_members(*sid).is_empty()) {
                let g = ActiveGame {
                    item_id: saved["itemId"].as_str().unwrap_or("").to_string(),
                    title: saved["title"].as_str().unwrap_or("Jogo").to_string(),
                    profile_id: profile.id.clone(),
                    started_unix: saved["startedAt"].as_i64().unwrap_or_else(now_unix),
                    state: GameState::Launching,
                    pid: Some(sid),
                    sid,
                    tracker: Tracker::Process,
                    exit_code: Arc::new(AtomicI32::new(i32::MIN)),
                    stop: Arc::new(AtomicBool::new(false)),
                    log: self.cfg.cache_dir.join("logs").join("readotado.log"),
                };
                tracing::info!("reencontrado após reiniciar: {}", g.item_id);
                *self.game.lock().unwrap_or_else(|e| e.into_inner()) = Some(g);
                let core = self.clone();
                tokio::spawn(async move { core.track_game().await });
                return;
            }
        }
        // 2) jogo da Steam (reaper "SteamLaunch AppId=N")
        let Some(appid) = crate::procs::running_steam_appid() else { return };
        let item_id = format!("steam:{appid}");
        let title = self.library.read().unwrap_or_else(|e| e.into_inner()).iter().find(|i| i.id == item_id).map(|i| i.title.clone()).unwrap_or_else(|| "Jogo da Steam".into());
        let g = ActiveGame {
            item_id: item_id.clone(),
            title,
            profile_id: profile.id.clone(),
            started_unix: now_unix(),
            state: GameState::Launching,
            pid: None,
            sid: 0,
            tracker: Tracker::SteamApp(appid),
            exit_code: Arc::new(AtomicI32::new(i32::MIN)),
            stop: Arc::new(AtomicBool::new(false)),
            log: self.cfg.cache_dir.join("logs").join(format!("{}.log", item_id.replace(':', "_"))),
        };
        tracing::info!("reencontrado jogo aberto antes do reinício: {item_id}");
        *self.game.lock().unwrap_or_else(|e| e.into_inner()) = Some(g);
        let core = self.clone();
        tokio::spawn(async move { core.track_game().await });
    }

    async fn track_game(self: Arc<Self>) {
        let (tracker, sid, exit_code, stop, profile_id, item_id, log_path) = {
            let g = self.game.lock().unwrap_or_else(|e| e.into_inner());
            let g = g.as_ref().expect("jogo ativo");
            (g.tracker.clone(), g.sid, g.exit_code.clone(), g.stop.clone(), g.profile_id.clone(), g.item_id.clone(), g.log.clone())
        };
        let start = Instant::now();
        let launch_timeout = Duration::from_secs(if matches!(tracker, Tracker::SteamApp(_)) { 120 } else { 10 });
        let grace_polls = if matches!(tracker, Tracker::SteamApp(_)) { 6 } else { 2 };
        let mut seen_at: Option<Instant> = None;
        let mut absent = 0;
        let mut last_acct = Instant::now();
        let mut warned = false;
        let mut failed = false;
        let mut surfaced = false;
        let mut unity_checks = 0u32;
        let mut unity_done = std::collections::HashSet::new();
        loop {
            tokio::time::sleep(Duration::from_millis(400)).await;
            // diálogo de configuração do Unity (só mouse): confirma "Play!" sozinho. Só nos primeiros ~90 s após o
            // processo aparecer, a cada ~1,2 s; uma vez por janela.
            if self.compositor.name() == "sway" && seen_at.is_some_and(|t| t.elapsed() < Duration::from_secs(90)) {
                unity_checks += 1;
                if unity_checks % 3 == 0 {
                    if let Some((con, _)) = tokio::task::spawn_blocking(crate::compositor::unity_config_window).await.ok().flatten() {
                        if unity_done.insert(con) {
                            self.confirm_unity_dialog(con).await;
                        }
                    }
                }
            }
            let alive = !members(&tracker, sid).is_empty();
            if alive {
                absent = 0;
                if seen_at.is_none() {
                    seen_at = Some(Instant::now());
                    let info = {
                        let mut g = self.game.lock().unwrap_or_else(|e| e.into_inner());
                        let g = g.as_mut().expect("jogo ativo");
                        if g.state == GameState::Launching {
                            g.state = GameState::Running;
                        }
                        g.info()
                    };
                    self.compositor.show_game();
                    self.emit("game.started", json!({ "session": info }));
                    self.refresh_library().await;
                }
            } else if seen_at.is_some() {
                absent += 1;
                if absent >= grace_polls {
                    break;
                }
            } else if exit_code.load(Ordering::SeqCst) != i32::MIN && matches!(tracker, Tracker::Process) {
                failed = exit_code.load(Ordering::SeqCst) != 0;
                break;
            } else if start.elapsed() > launch_timeout {
                failed = true;
                break;
            }
            if stop.load(Ordering::SeqCst) && !alive {
                break;
            }
            // A Steam pode estar pedindo login, atualização ou confirmação: mostra a janela dela em vez de deixar o Nexus "abrindo" sem fim.
            if !surfaced && seen_at.is_none() && matches!(tracker, Tracker::SteamApp(_)) && start.elapsed() > Duration::from_secs(7) {
                surfaced = true;
                self.compositor.show_game();
                self.emit("game.waiting", json!({ "itemId": item_id, "hint": "A Steam está preparando o jogo. Se ela pedir login ou atualização, resolva na janela dela e use o botão Guia para voltar ao Nexus." }));
            }
            if seen_at.is_some() && last_acct.elapsed() >= Duration::from_secs(5) {
                let secs = last_acct.elapsed().as_secs();
                last_acct = Instant::now();
                if let Some(p) = self.accounts.get(&profile_id) {
                    if p.kind == ProfileKind::Kid && p.daily_limit_min.is_some() {
                        self.accounts.add_usage(&p.id, secs);
                        let left = self.accounts.remaining_min(&p).unwrap_or(0);
                        if left == 0 {
                            self.emit("parental.limit_reached", json!({ "profileId": p.id }));
                            self.close_game(false).await;
                        } else if left <= 5 && !warned {
                            warned = true;
                            self.emit("parental.warning", json!({ "minutes": left }));
                        }
                    }
                }
            }
        }
        let code = exit_code.load(Ordering::SeqCst);
        let play_sec = seen_at.map(|t| t.elapsed().as_secs()).unwrap_or(0);
        if let Some(p) = self.accounts.get(&profile_id) {
            let rest = last_acct.elapsed().as_secs().min(play_sec);
            if p.kind == ProfileKind::Kid && p.daily_limit_min.is_some() && seen_at.is_some() {
                self.accounts.add_usage(&p.id, rest);
            }
            if play_sec > 0 {
                self.accounts.add_playtime(&p.id, &item_id, play_sec);
            }
        }
        let title = self.game.lock().unwrap_or_else(|e| e.into_inner()).take().map(|g| g.title).unwrap_or_default();
        let _ = std::fs::remove_file(self.game_file());
        self.compositor.show_shell();
        if failed {
            let tail = std::fs::read_to_string(&log_path).ok().map(|t| t.lines().rev().filter(|l| !l.trim().is_empty()).take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" · ")).unwrap_or_default();
            let base = if code > 0 { format!("saiu com código {code}") } else if matches!(tracker, Tracker::SteamApp(_)) { "a Steam não iniciou o jogo (confira se está logada e se o jogo está instalado; jogos de Windows precisam do Steam Play/Proton ativado)".to_string() } else { "o jogo não iniciou".to_string() };
            let reason = if tail.is_empty() { base } else { format!("{base} — {}", tail.chars().take(160).collect::<String>()) };
            self.notify("alert", "Não foi possível abrir", &title, None);
            self.emit("game.failed", json!({ "itemId": item_id, "reason": reason }));
        }
        self.emit("game.exited", json!({ "itemId": item_id, "code": if code == i32::MIN { Value::Null } else { json!(code) }, "playSec": play_sec }));
        self.refresh_library().await;
    }

    /// "Play!" no diálogo do Unity: Enter pelo teclado virtual (sessão real) e, se a janela continuar, clique pelo sway.
    async fn confirm_unity_dialog(&self, con: i64) {
        use crate::compositor as c;
        tokio::time::sleep(Duration::from_millis(700)).await; // deixa a janela terminar de abrir
        let nested = tokio::task::spawn_blocking(c::sway_nested).await.unwrap_or(true);
        if !nested && self.hub.sink.available() {
            tracing::info!("diálogo de configuração do Unity: Enter");
            c::focus_con(con);
            tokio::time::sleep(Duration::from_millis(250)).await;
            self.hub.sink.key("enter", &[]);
            tokio::time::sleep(Duration::from_millis(2000)).await;
        }
        if let Some((id, rect)) = tokio::task::spawn_blocking(c::unity_config_window).await.ok().flatten().filter(|(id, _)| *id == con) {
            let (x, y) = c::unity_play_point(rect);
            tracing::info!("diálogo de configuração do Unity: clique em Play! ({x},{y})");
            c::focus_con(id);
            c::click_at(x, y);
        }
    }

    /// Fecha o jogo atual: SIGTERM para a sessão e, se ainda existir após o prazo, SIGKILL.
    pub async fn close_game(&self, force: bool) -> bool {
        let (tracker, sid, stop) = {
            let mut g = self.game.lock().unwrap_or_else(|e| e.into_inner());
            let Some(g) = g.as_mut() else { return false };
            g.state = GameState::Closing;
            (g.tracker.clone(), g.sid, g.stop.clone())
        };
        stop.store(true, Ordering::SeqCst);
        procs::signal(&members(&tracker, sid), if force { libc::SIGKILL } else { libc::SIGTERM });
        let grace = if force { 0 } else { 5 };
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(grace)).await;
            procs::signal(&members(&tracker, sid), libc::SIGKILL);
        });
        true
    }
}

use serde_json::Value;
