pub mod accounts;
pub mod appupdates;
pub mod art;
pub mod avatars;
pub mod compositor;
pub mod config;
pub mod controllers;
pub mod core;
pub mod customart;
pub mod db;
pub mod freeze;
pub mod game;
pub mod guide;
pub mod hardware;
pub mod inputmap;
pub mod bluetooth;
pub mod locale;
pub mod net;
pub mod procs;
pub mod providers;
pub mod remote;
pub mod roms;
pub mod sysnet;
pub mod rpc;
pub mod util;
pub mod vdev;

use std::sync::Arc;

pub struct Running {
    pub core: Arc<core::Core>,
    pub started: net::Started,
}

/// Sobe o Core: sockets, servidores e tarefas de fundo.
pub async fn run(cfg: config::Config) -> anyhow::Result<Running> {
    let guide_watch = cfg.guide_watch;
    let core = core::Core::new(cfg)?;
    freeze::thaw(); // Core reiniciado com a Shell congelada (jogo aberto): ela volta a responder
    if core.cfg.compositor == "sway" && core.db.kv_get("keyboard").is_some() {
        // o layout escolhido no Nexus vale desde o início da sessão
        let k = locale::keyboard_id(&core.db);
        std::thread::spawn(move || {
            let _ = locale::apply_keyboard(&k);
        });
    }
    let started = net::start(core.clone()).await?;
    core.adopt_running_game(); // perfil restaurado + jogo aberto antes de reiniciar
    tokio::spawn(core::library_loop(core.clone()));
    tokio::spawn(appupdates::update_loop(core.clone()));
    if guide_watch {
        guide::spawn(core.clone());
    }
    // Na sessão Nexus (sway), a Steam fica rodando escondida em segundo plano: o usuário só vê o Nexus.
    if core.cfg.compositor == "sway" {
        if let Some(st) = core.steam.clone() {
            match st.start_silent() {
                Ok(true) => tracing::info!("Steam iniciada em segundo plano (-silent)"),
                Ok(false) => {}
                Err(e) => tracing::warn!("Steam em segundo plano: {e}"),
            }
        }
    }
    if let Some(st) = core.steam.clone() {
        tokio::spawn(async move {
            if let Err(e) = st.sync_owned().await {
                tracing::warn!("Steam Web API: {e}");
            }
            st.resolve_local_names().await;
        });
    }
    Ok(Running { core, started })
}
