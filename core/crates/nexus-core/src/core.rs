//! Estado central do daemon: provedores, cache da biblioteca, sessão de perfil, eventos.
use crate::accounts::Accounts;
use crate::compositor::{self, Compositor};
use crate::config::Config;
use crate::db::Db;
use crate::providers::{demo::DemoProvider, native::NativeProvider, steam::SteamProvider, Provider};
use crate::remote::Hub;
use nexus_protocol::*;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::broadcast;

#[derive(Clone, Debug)]
pub struct Event {
    pub method: String,
    pub params: Value,
}

pub struct Core {
    pub cfg: Config,
    pub db: Arc<Db>,
    pub accounts: Accounts,
    pub providers: Vec<Arc<dyn Provider>>,
    pub steam: Option<Arc<SteamProvider>>,
    pub native: Arc<NativeProvider>,
    pub demo: Option<Arc<DemoProvider>>,
    pub events: broadcast::Sender<Event>,
    pub library: RwLock<Vec<Item>>,
    pub session: Mutex<Option<String>>,
    pub game: Mutex<Option<crate::game::ActiveGame>>,
    pub notifs: Mutex<Vec<Notification>>,
    pub next_notif: AtomicU64,
    pub compositor: Box<dyn Compositor>,
    pub hub: Hub,
    /// instalações pedidas cujo progresso ainda não apareceu no provedor (id -> quando)
    pub pending: Mutex<std::collections::HashMap<String, std::time::Instant>>,
    /// acorda o laço de atualização da biblioteca
    pub kick: tokio::sync::Notify,
    /// a Steam está na tela porque o usuário pediu (login em modo TV, página de compra); F1/Guia esconde
    pub steam_ui: AtomicBool,
    /// a Shell está na frente com o menu rápido aberto por cima de um jogo (F1/Guia de novo volta ao jogo)
    pub guide_open: AtomicBool,
    /// resposta do usuário ao contrato de licença (EULA) de um jogo sendo instalado
    pub eula: Mutex<Option<bool>>,
    /// atualizações encontradas dos apps instalados (id do item → atualização), ver `appupdates.rs`
    pub app_updates: Mutex<std::collections::HashMap<String, crate::appupdates::AppUpdate>>,
    /// apps sendo atualizados/restaurados agora
    pub app_jobs: Mutex<std::collections::HashSet<String>>,
}

impl Core {
    pub fn new(cfg: Config) -> anyhow::Result<Arc<Core>> {
        std::fs::create_dir_all(&cfg.data_dir)?;
        std::fs::create_dir_all(&cfg.cache_dir)?;
        std::fs::create_dir_all(&cfg.runtime_dir)?;
        let db = Arc::new(Db::open(&cfg.data_dir.join("nexus.db"))?);
        let steam = Arc::new(SteamProvider::new(cfg.clone(), db.clone()));
        let native = Arc::new(NativeProvider::new(cfg.apps_dirs.clone(), db.clone(), cfg.data_dir.join("custom-art"), cfg.fs_roots.clone()));
        crate::drive::init(&cfg.data_dir, &cfg.cache_dir, &cfg.runtime_dir);
        let mut providers: Vec<Arc<dyn Provider>> = vec![steam.clone(), native.clone(), Arc::new(crate::providers::drive::DriveProvider::new(cfg.data_dir.clone()))];
        if cfg.emulation {
            providers.push(Arc::new(crate::providers::emu::EmuProvider::new()));
        }
        if cfg.web_apps {
            providers.push(Arc::new(crate::providers::web::WebProvider::new(cfg.data_dir.clone(), cfg.cache_dir.clone())));
        }
        let demo = cfg.demo.then(|| Arc::new(DemoProvider::new(cfg.demo_assets.clone(), db.clone())));
        if let Some(d) = &demo {
            providers.push(d.clone());
        }
        let (events, _) = broadcast::channel(256);
        let hub = Hub::new(&cfg, db.clone(), events.clone());
        let core = Arc::new(Core {
            accounts: Accounts::new(db.clone()),
            compositor: compositor::from_name(&cfg.compositor),
            cfg,
            db,
            providers,
            steam: Some(steam),
            native,
            demo,
            events,
            library: RwLock::new(vec![]),
            session: Mutex::new(None),
            game: Mutex::new(None),
            notifs: Mutex::new(vec![]),
            next_notif: AtomicU64::new(1),
            hub,
            pending: Mutex::new(Default::default()),
            kick: tokio::sync::Notify::new(),
            steam_ui: AtomicBool::new(false),
            guide_open: AtomicBool::new(false),
            eula: Mutex::new(None),
            app_updates: Mutex::new(Default::default()),
            app_jobs: Mutex::new(Default::default()),
        });
        if core.demo.is_some() {
            core.seed_demo()?;
        }
        core.restore_session();
        Ok(core)
    }

    pub fn emit(&self, method: &str, params: Value) {
        let _ = self.events.send(Event { method: method.to_string(), params });
    }

    pub fn provider_names(&self) -> Vec<String> {
        self.providers.iter().map(|p| p.id().to_string()).collect()
    }

    pub fn provider_for(&self, id: &str) -> Option<Arc<dyn Provider>> {
        let (p, _) = id.split_once(':')?;
        self.providers.iter().find(|x| x.id() == p).cloned()
    }

    pub fn notify(&self, kind: &str, title: &str, body: &str, action: Option<&str>) {
        let n = Notification { id: self.next_notif.fetch_add(1, Ordering::Relaxed), kind: kind.into(), title: title.into(), body: body.into(), action: action.map(String::from), ts: now_unix() };
        {
            let mut v = self.notifs.lock().unwrap_or_else(|e| e.into_inner());
            v.insert(0, n.clone());
            v.truncate(30);
        }
        self.emit("notification.added", serde_json::to_value(&n).unwrap());
    }

    /// Mostra a Steam a pedido do usuário (login em modo TV, compra). Enquanto durar, janelas novas dela também
    /// aparecem; F1/Guia (`guide::pressed`) ou abrir um jogo encerram e voltam ao Nexus.
    pub fn show_steam_ui(self: &Arc<Self>) {
        if self.compositor.name() != "sway" || self.steam_ui.swap(true, Ordering::SeqCst) {
            return;
        }
        let c = self.clone();
        self.emit("steam.ui", serde_json::json!({ "open": true }));
        std::thread::spawn(move || {
            let t0 = std::time::Instant::now();
            while c.steam_ui.load(Ordering::SeqCst) && c.game_info().is_none() && t0.elapsed() < std::time::Duration::from_secs(30 * 60) {
                compositor::steam_show_all();
                c.compositor.show_game();
                std::thread::sleep(std::time::Duration::from_millis(800));
            }
            c.hide_steam_ui();
        });
    }

    pub fn hide_steam_ui(&self) {
        if !self.steam_ui.swap(false, Ordering::SeqCst) {
            return;
        }
        if self.compositor.name() == "sway" {
            compositor::steam_hide_all();
            if self.game_info().is_none() {
                self.compositor.show_shell();
            }
        }
        self.emit("steam.ui", serde_json::json!({ "open": false }));
    }

    // ---- sessão de perfil ----
    /// Troca o perfil ativo e anota em `runtime/session` (tmpfs, 0600): se o Core reiniciar no mesmo boot
    /// ele volta ao mesmo perfil em vez da escolha de perfis. Reiniciar o aparelho começa do zero.
    pub fn set_session(&self, id: Option<String>) {
        let f = self.cfg.runtime_dir.join("session");
        match &id {
            Some(i) => {
                use std::os::unix::fs::OpenOptionsExt;
                let w = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&f);
                if let Ok(mut w) = w {
                    let _ = std::io::Write::write_all(&mut w, i.as_bytes());
                }
            }
            None => {
                let _ = std::fs::remove_file(&f);
            }
        }
        *self.session.lock().unwrap_or_else(|e| e.into_inner()) = id;
    }

    /// Perfil anotado antes de o Core reiniciar (se ainda existir).
    pub fn restore_session(&self) {
        let Ok(id) = std::fs::read_to_string(self.cfg.runtime_dir.join("session")) else { return };
        let id = id.trim().to_string();
        if self.accounts.get(&id).is_some() {
            tracing::info!("sessão restaurada após reiniciar: {id}");
            *self.session.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
        } else {
            self.set_session(None);
        }
    }

    pub fn current_profile(&self) -> Option<Profile> {
        let id = self.session.lock().unwrap_or_else(|e| e.into_inner()).clone()?;
        self.accounts.get(&id)
    }

    pub fn session_info(&self, p: Profile) -> SessionInfo {
        SessionInfo { prefs: self.accounts.prefs(&p.id), remaining_min: self.accounts.remaining_min(&p), allowed: self.accounts.allowed(&p.id), profile: p }
    }

    /// Regra de visibilidade: perfis infantis só veem o que a classificação permite ou o responsável liberou.
    pub fn visible_to(&self, p: Option<&Profile>, it: &Item) -> bool {
        let Some(p) = p else { return false };
        if p.kind == ProfileKind::Adult {
            return true;
        }
        if self.accounts.allowed(&p.id).contains(&it.id) {
            return true;
        }
        it.kind == ItemKind::Game && matches!((it.rating, p.age_limit), (Some(r), Some(max)) if r <= max)
    }

    pub fn with_profile_data(&self, p: &Profile, mut it: Item) -> Item {
        if it.provider != "steam" {
            let (secs, last) = self.accounts.playtime(&p.id, &it.id);
            it.playtime_sec += secs;
            if last.is_some() {
                it.last_played = last.or(it.last_played);
            }
        }
        if self.game.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|g| g.item_id == it.id) {
            it.state = ItemState::Running;
        }
        it
    }

    pub async fn find_item(&self, id: &str) -> Option<Item> {
        if let Some(i) = self.library.read().unwrap_or_else(|e| e.into_inner()).iter().find(|i| i.id == id) {
            return Some(i.clone());
        }
        self.provider_for(id)?.get(id).await.ok().flatten()
    }

    pub fn nexus_hello(&self) -> Hello {
        Hello { version: env!("CARGO_PKG_VERSION").into(), demo: self.cfg.demo, dev: self.cfg.dev, providers: self.provider_names() }
    }

    fn seed_demo(&self) -> anyhow::Result<()> {
        if self.accounts.list()?.is_empty() {
            use crate::accounts::NewProfile;
            for (n, h, kid) in [("Lia", 180, false), ("Rafa", 320, false), ("Teo", 40, false), ("Pipo", 120, true)] {
                self.accounts.create(NewProfile { name: n.into(), avatar: n.to_lowercase(), hue: h, kid, daily_limit_min: kid.then_some(60), age_limit: kid.then_some(10), ..Default::default() })?;
            }
            self.accounts.set_parental(Some("1234"), None).ok();
        }
        self.notify("download", "Atualização pronta", "Lâmina do Norte 1.4 · 2,1 GB", Some("hub:demo:g4"));
        self.notify("users", "Rafa te convidou", "Rota Neon · partida casual", Some("hub:demo:g2"));
        self.notify("cloud", "Novo na nuvem", "Cidade Submersa — jogue sem baixar", Some("hub:demo:g6"));
        Ok(())
    }

    /// Reconstrói o cache da biblioteca; emite `library.changed` com o que mudou.
    pub async fn refresh_library(&self) -> bool {
        let mut all: Vec<Item> = vec![];
        for p in &self.providers {
            match p.library().await {
                Ok(v) => all.extend(v),
                Err(e) => tracing::warn!("provedor {} falhou: {e}", p.id()),
            }
        }
        self.apply_pending(&mut all);
        for it in all.iter_mut() {
            self.tag_update(it);
        }
        let (changed, removed, done) = {
            let mut cache = self.library.write().unwrap_or_else(|e| e.into_inner());
            let changed: Vec<Item> = all.iter().filter(|n| cache.iter().find(|o| o.id == n.id) != Some(n)).cloned().collect();
            let removed: Vec<String> = cache.iter().filter(|o| !all.iter().any(|n| n.id == o.id)).map(|o| o.id.clone()).collect();
            let done: Vec<Item> = changed.iter().filter(|n| n.state == ItemState::Installed && cache.iter().any(|o| o.id == n.id && matches!(o.state, ItemState::Installing | ItemState::Updating))).cloned().collect();
            *cache = all;
            (changed, removed, done)
        };
        for it in &done {
            self.notify("download", "Pronto para jogar", &it.title, Some(&format!("hub:{}", it.id)));
        }
        if changed.is_empty() && removed.is_empty() {
            return false;
        }
        // só ids: cada Shell busca o que o perfil ativo pode ver (library.get_many)
        self.emit("library.changed", json!({ "ids": changed.iter().map(|i| i.id.clone()).collect::<Vec<_>>(), "removed": removed }));
        true
    }

    /// App com atualização encontrada ganha a etiqueta "update" (ícone no card, aviso no Início e na página).
    pub fn tag_update(&self, it: &mut Item) {
        if self.app_updates.lock().unwrap_or_else(|e| e.into_inner()).contains_key(&it.id) && !it.tags.iter().any(|t| t == "update") {
            it.tags.push("update".into());
        }
    }

    pub fn any_installing(&self) -> bool {
        !self.pending.lock().unwrap_or_else(|e| e.into_inner()).is_empty() || self.library.read().unwrap_or_else(|e| e.into_inner()).iter().any(|i| matches!(i.state, ItemState::Installing | ItemState::Updating))
    }

    /// Mostra "instalando" assim que o pedido sai, antes do provedor (ex.: Steam) começar a baixar.
    fn apply_pending(&self, all: &mut [Item]) {
        let mut p = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        let mut expired = vec![];
        p.retain(|id, at| {
            let Some(it) = all.iter_mut().find(|i| &i.id == id) else { return false };
            match it.state {
                ItemState::Installed | ItemState::Updating | ItemState::Installing => false,
                _ if at.elapsed() > std::time::Duration::from_secs(180) => {
                    expired.push(it.title.clone());
                    false
                }
                _ => {
                    it.state = ItemState::Installing;
                    it.install = Some(InstallProgress { percent: 0.0, ..Default::default() });
                    true
                }
            }
        });
        drop(p);
        for t in expired {
            self.notify("alert", "A instalação não começou", &t, None);
        }
    }
}

pub async fn library_loop(core: Arc<Core>) {
    loop {
        core.refresh_library().await;
        // com jogo aberto, a biblioteca quase não muda e cada varredura é CPU/disco tirados do jogo: 1 min
        // (download em andamento continua a cada 0,7 s para o progresso ficar certo)
        let ms = if core.any_installing() {
            700
        } else if core.game_info().is_some() {
            60_000
        } else {
            6000
        };
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_millis(ms)) => {}
            _ = core.kick.notified() => {}
        }
    }
}
