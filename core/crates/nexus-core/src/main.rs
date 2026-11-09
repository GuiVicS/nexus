use nexus_core::config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into())).init();
    let args: Vec<String> = std::env::args().collect();
    let cfg = Config::from_env_args(&args)?;
    let rpc = cfg.rpc_socket.clone();
    let http = cfg.http_socket.clone();
    let r = nexus_core::run(cfg).await?;
    tracing::info!("nexus-core {} pronto · rpc {}", env!("CARGO_PKG_VERSION"), rpc.display());
    if let Some(a) = r.started.dev_addr {
        tracing::info!("dev http em http://{a} (somente loopback)");
    }
    if let Some(a) = r.started.remote_addr {
        tracing::info!("remote em {a} · {}", r.core.hub.url());
    }
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
    }
    let _ = std::fs::remove_file(rpc);
    let _ = std::fs::remove_file(http);
    Ok(())
}
