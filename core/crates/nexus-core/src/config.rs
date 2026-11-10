use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub runtime_dir: PathBuf,
    /// JSON-RPC (uma mensagem JSON por linha) — usado pelo processo principal do Electron
    pub rpc_socket: PathBuf,
    /// HTTP local em socket Unix: arte dos itens
    pub http_socket: PathBuf,
    /// HTTP em loopback para desenvolvimento/testes no navegador (RPC por WebSocket + arte + Shell)
    pub dev_http: Option<SocketAddr>,
    /// HTTP na rede local: apenas o Remote (celular)
    pub remote_bind: SocketAddr,
    pub remote_host: Option<String>,
    pub remote_dir: Option<PathBuf>,
    pub shell_dir: Option<PathBuf>,
    pub demo: bool,
    pub demo_assets: PathBuf,
    pub steam_root: Option<PathBuf>,
    pub steam_bin: String,
    pub steam_api_key: Option<String>,
    pub apps_dirs: Vec<PathBuf>,
    /// pastas que o navegador de imagens pode mostrar
    pub fs_roots: Vec<PathBuf>,
    pub compositor: String,
    pub dev: bool,
    pub uinput: bool,
    pub systemd_scope: bool,
    pub guide_watch: bool,
    /// plataformas de streaming (Netflix, YouTube…) na aba Mídia
    pub web_apps: bool,
    /// emulação (RetroArch, ROMs em ~/ROMs)
    pub emulation: bool,
}

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

fn home() -> PathBuf {
    PathBuf::from(env("HOME").unwrap_or_else(|| "/tmp".into()))
}

impl Config {
    pub fn from_env_args(args: &[String]) -> anyhow::Result<Config> {
        let xdg_data = env("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local/share"));
        let xdg_cache = env("XDG_CACHE_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".cache"));
        let runtime = env("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("/tmp/nexus-{}", unsafe { libc::getuid() })));
        let data_dir = env("NEXUS_DATA_DIR").map(PathBuf::from).unwrap_or_else(|| xdg_data.join("nexus"));
        let cache_dir = env("NEXUS_CACHE_DIR").map(PathBuf::from).unwrap_or_else(|| xdg_cache.join("nexus"));
        let runtime_dir = env("NEXUS_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(|| runtime.join("nexus"));
        let mut apps_dirs: Vec<PathBuf> = vec![];
        if let Some(v) = env("NEXUS_APPS_DIRS") {
            apps_dirs = v.split(':').map(PathBuf::from).collect();
        } else {
            apps_dirs.push(xdg_data.join("applications"));
            let dirs = env("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
            for d in dirs.split(':') {
                apps_dirs.push(PathBuf::from(d).join("applications"));
            }
            apps_dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
            apps_dirs.push(xdg_data.join("flatpak/exports/share/applications"));
        }
        let mut c = Config {
            rpc_socket: runtime_dir.join("core.sock"),
            http_socket: runtime_dir.join("http.sock"),
            data_dir,
            cache_dir,
            runtime_dir,
            dev_http: env("NEXUS_DEV_HTTP").and_then(|s| s.parse().ok()),
            remote_bind: env("NEXUS_REMOTE_BIND").and_then(|s| s.parse().ok()).unwrap_or_else(|| "0.0.0.0:8787".parse().unwrap()),
            remote_host: env("NEXUS_REMOTE_HOST"),
            remote_dir: env("NEXUS_REMOTE_DIR").map(PathBuf::from).or_else(|| Some(PathBuf::from("/usr/share/nexus/remote")).filter(|p| p.is_dir())),
            shell_dir: env("NEXUS_SHELL_DIR").map(PathBuf::from),
            demo: env("NEXUS_DEMO").is_some_and(|v| v == "1"),
            demo_assets: env("NEXUS_DEMO_ASSETS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/usr/share/nexus/demo")),
            steam_root: env("NEXUS_STEAM_ROOT").map(PathBuf::from),
            steam_bin: env("NEXUS_STEAM_BIN").unwrap_or_else(|| "steam".into()),
            steam_api_key: env("NEXUS_STEAM_API_KEY"),
            apps_dirs,
            fs_roots: env("NEXUS_FS_ROOTS").map(|v| v.split(':').map(PathBuf::from).collect()).unwrap_or_else(crate::customart::default_roots),
            compositor: env("NEXUS_COMPOSITOR").unwrap_or_else(|| "none".into()),
            dev: env("NEXUS_DEV").is_some_and(|v| v == "1"),
            uinput: env("NEXUS_UINPUT").map(|v| v != "0").unwrap_or(true),
            systemd_scope: env("NEXUS_SYSTEMD_SCOPE").is_some_and(|v| v == "1"),
            guide_watch: env("NEXUS_GUIDE_WATCH").map(|v| v != "0").unwrap_or(true),
            web_apps: env("NEXUS_WEB_APPS").map(|v| v != "0").unwrap_or(true),
            emulation: env("NEXUS_EMULATION").map(|v| v != "0").unwrap_or(true),
        };
        let mut it = args.iter().skip(1);
        while let Some(a) = it.next() {
            let mut val = || it.next().cloned().ok_or_else(|| anyhow::anyhow!("falta valor para {a}"));
            match a.as_str() {
                "--demo" => c.demo = true,
                "--dev" => c.dev = true,
                "--dev-http" => c.dev_http = Some(val()?.parse()?),
                "--remote-bind" => c.remote_bind = val()?.parse()?,
                "--data-dir" => c.data_dir = val()?.into(),
                "--runtime-dir" => {
                    c.runtime_dir = PathBuf::from(val()?);
                    c.rpc_socket = c.runtime_dir.join("core.sock");
                    c.http_socket = c.runtime_dir.join("http.sock");
                }
                "--shell-dir" => c.shell_dir = Some(val()?.into()),
                "--remote-dir" => c.remote_dir = Some(val()?.into()),
                "--demo-assets" => c.demo_assets = val()?.into(),
                "--help" | "-h" => {
                    println!("nexus-core [--demo] [--dev] [--dev-http ADDR] [--remote-bind ADDR] [--data-dir D] [--runtime-dir D] [--shell-dir D] [--remote-dir D] [--demo-assets D]");
                    std::process::exit(0);
                }
                other => anyhow::bail!("argumento desconhecido: {other}"),
            }
        }
        Ok(c)
    }

    /// Configuração isolada para testes.
    pub fn for_test(root: &std::path::Path) -> Config {
        Config {
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            runtime_dir: root.join("run"),
            rpc_socket: root.join("run/core.sock"),
            http_socket: root.join("run/http.sock"),
            dev_http: Some("127.0.0.1:0".parse().unwrap()),
            remote_bind: "127.0.0.1:0".parse().unwrap(),
            remote_host: Some("127.0.0.1".into()),
            remote_dir: None,
            shell_dir: None,
            demo: false,
            demo_assets: root.join("demo"),
            steam_root: Some(root.join("nosteam")), // isola da Steam real da máquina
            steam_bin: "steam".into(),
            steam_api_key: None,
            apps_dirs: vec![],
            fs_roots: vec![root.join("home")],
            compositor: "none".into(),
            dev: true,
            uinput: false,
            systemd_scope: false,
            guide_watch: false,
            web_apps: false,
            emulation: false,
        }
    }
}
