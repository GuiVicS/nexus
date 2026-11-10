pub mod accounts;
pub mod art;
pub mod compositor;
pub mod config;
pub mod core;
pub mod customart;
pub mod db;
pub mod game;
pub mod guide;
pub mod hardware;
pub mod inputmap;
pub mod net;
pub mod procs;
pub mod providers;
pub mod remote;
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
    let started = net::start(core.clone()).await?;
    tokio::spawn(core::library_loop(core.clone()));
    if guide_watch {
        guide::spawn(core.clone());
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
