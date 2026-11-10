//! Plataformas de streaming (apps web): já vêm configuradas na aba Mídia e abrem no navegador em modo
//! quiosque (tela cheia, sem barras), com um perfil de navegador só do Nexus para guardar os logins.
//! O YouTube abre na interface de TV (youtube.com/tv), navegável por controle.
use super::*;
use nexus_protocol::*;
use std::path::{Path, PathBuf};

pub struct Service {
    pub id: &'static str,
    pub title: &'static str,
    pub category: &'static str,
    pub desc: &'static str,
    pub url: &'static str,
    pub icon: Option<(&'static str, &'static [u8])>,
    pub bg: &'static str,
    pub mode: &'static str,
    pub hue: [u16; 2],
    /// abrir com agente de TV (interface "leanback")
    pub tv: bool,
}

macro_rules! icon {
    ($f:literal) => {
        Some(($f, include_bytes!(concat!("../../../../../assets/demo/icons/", $f)) as &[u8]))
    };
}

pub const SERVICES: &[Service] = &[
    Service { id: "youtube", title: "YouTube", category: "Vídeo", desc: "Vídeos, criadores e transmissões ao vivo — na versão para TV.", url: "https://www.youtube.com/tv", icon: icon!("logo-youtube.svg"), bg: "#f4f4f5", mode: "l", hue: [0, 350], tv: true },
    Service { id: "netflix", title: "Netflix", category: "Streaming", desc: "Séries e filmes por assinatura.", url: "https://www.netflix.com/browse", icon: icon!("logo-netflix.svg"), bg: "#0b0b0b", mode: "l", hue: [355, 340], tv: false },
    Service { id: "primevideo", title: "Prime Video", category: "Streaming", desc: "Filmes, séries e esportes ao vivo.", url: "https://www.primevideo.com", icon: icon!("logo-primevideo.svg"), bg: "linear-gradient(135deg,#1fb5ef,#0a79c9)", mode: "lw", hue: [205, 215], tv: false },
    Service { id: "disneyplus", title: "Disney+", category: "Streaming", desc: "Filmes, séries e animações.", url: "https://www.disneyplus.com", icon: icon!("logo-disneyplus.svg"), bg: "radial-gradient(120% 120% at 50% 0%,#2849c9,#0a1546)", mode: "lw", hue: [225, 240], tv: false },
    Service { id: "max", title: "Max", category: "Streaming", desc: "Séries da HBO, filmes da Warner e esportes.", url: "https://play.max.com", icon: None, bg: "linear-gradient(135deg,#002be7,#0a0f45)", mode: "i", hue: [230, 240], tv: false },
    Service { id: "globoplay", title: "Globoplay", category: "Streaming", desc: "Novelas, séries, filmes e a TV Globo ao vivo.", url: "https://globoplay.globo.com", icon: None, bg: "linear-gradient(135deg,#ff2d55,#b0122e)", mode: "i", hue: [350, 340], tv: false },
    Service { id: "appletv", title: "Apple TV+", category: "Streaming", desc: "Séries e filmes originais da Apple.", url: "https://tv.apple.com", icon: icon!("appletv.svg"), bg: "linear-gradient(135deg,#2b2b2b,#0d0d0d)", mode: "i", hue: [0, 0], tv: false },
    Service { id: "crunchyroll", title: "Crunchyroll", category: "Anime", desc: "Animes simultâneos com o Japão, legendados e dublados.", url: "https://www.crunchyroll.com", icon: icon!("crunchyroll.svg"), bg: "#F47521", mode: "i", hue: [25, 20], tv: false },
    Service { id: "twitch", title: "Twitch", category: "Ao vivo", desc: "Transmissões de jogos ao vivo.", url: "https://www.twitch.tv", icon: icon!("twitch.svg"), bg: "#9146FF", mode: "i", hue: [265, 290], tv: false },
    Service { id: "spotify", title: "Spotify", category: "Música", desc: "Músicas, podcasts e playlists.", url: "https://open.spotify.com", icon: icon!("logo-spotify.svg"), bg: "#121212", mode: "l", hue: [140, 160], tv: false },
    Service { id: "deezer", title: "Deezer", category: "Música", desc: "Milhões de músicas, Flow personalizado e letras sincronizadas.", url: "https://www.deezer.com", icon: icon!("deezer.svg"), bg: "linear-gradient(135deg,#A238FF,#5b1bb0)", mode: "i", hue: [275, 265], tv: false },
    Service { id: "applemusic", title: "Apple Music", category: "Música", desc: "Catálogo completo com áudio espacial.", url: "https://music.apple.com", icon: icon!("applemusic.svg"), bg: "linear-gradient(135deg,#fa233b,#fb5c74)", mode: "i", hue: [352, 350], tv: false },
    Service { id: "tidal", title: "TIDAL", category: "Música", desc: "Música em alta fidelidade e áudio sem perdas.", url: "https://listen.tidal.com", icon: icon!("tidal.svg"), bg: "#000", mode: "i", hue: [0, 0], tv: false },
    Service { id: "soundcloud", title: "SoundCloud", category: "Música", desc: "Artistas independentes, remixes e sets ao vivo.", url: "https://soundcloud.com", icon: icon!("soundcloud.svg"), bg: "linear-gradient(135deg,#FF5500,#ff8800)", mode: "i", hue: [20, 30], tv: false },
];

/// Agente de usuário de Smart TV: o YouTube só entrega a interface de TV (youtube.com/tv) para ele.
const TV_UA: &str = "Mozilla/5.0 (SMART-TV; Linux; Tizen 6.5) AppleWebKit/537.36 (KHTML, like Gecko) 85.0.4183.93/6.5 TV Safari/537.36";

pub struct WebProvider {
    data_dir: PathBuf,
    icon_dir: PathBuf,
}

#[derive(Debug, PartialEq)]
pub enum BrowserKind {
    Chromium,
    Firefox,
}

/// Navegador para os apps web. Chrome/Chromium primeiro (tocam Netflix/Prime com DRM Widevine e têm modo --app).
pub fn find_browser() -> Option<(PathBuf, BrowserKind)> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let dirs: Vec<PathBuf> = std::env::split_paths(&path).chain([PathBuf::from("/usr/bin"), PathBuf::from("/snap/bin"), PathBuf::from("/usr/local/bin")]).collect();
    for (name, kind) in [
        ("google-chrome", BrowserKind::Chromium),
        ("google-chrome-stable", BrowserKind::Chromium),
        ("chromium", BrowserKind::Chromium),
        ("chromium-browser", BrowserKind::Chromium),
        ("microsoft-edge", BrowserKind::Chromium),
        ("brave-browser", BrowserKind::Chromium),
        ("firefox", BrowserKind::Firefox),
    ] {
        if let Some(p) = dirs.iter().map(|d| d.join(name)).find(|p| p.is_file()) {
            return Some((p, kind));
        }
    }
    None
}

pub fn launch_argv(browser: &Path, kind: &BrowserKind, s: &Service, profile: &Path) -> Vec<String> {
    let mut v = vec![browser.display().to_string()];
    match kind {
        BrowserKind::Chromium => {
            v.extend([
                format!("--app={}", s.url),
                "--kiosk".into(),
                format!("--user-data-dir={}", profile.display()),
                "--no-first-run".into(),
                "--no-default-browser-check".into(),
                "--disable-features=Translate,MediaRouter".into(),
                "--autoplay-policy=no-user-gesture-required".into(),
                "--ozone-platform-hint=auto".into(),
                "--class=nexus-web".into(),
            ]);
            if s.tv {
                v.push(format!("--user-agent={TV_UA}"));
            }
        }
        // Firefox (às vezes snap, que não lê pastas ocultas): sem perfil próprio, só quiosque
        BrowserKind::Firefox => v.extend(["--kiosk".into(), "--new-window".into(), s.url.to_string()]),
    }
    v
}

impl WebProvider {
    pub fn new(data_dir: PathBuf, cache_dir: PathBuf) -> WebProvider {
        let icon_dir = cache_dir.join("web-icons");
        let _ = std::fs::create_dir_all(&icon_dir);
        for s in SERVICES {
            if let Some((f, bytes)) = s.icon {
                let p = icon_dir.join(f);
                if std::fs::metadata(&p).map(|m| m.len() != bytes.len() as u64).unwrap_or(true) {
                    let _ = std::fs::write(&p, bytes);
                }
            }
        }
        WebProvider { data_dir, icon_dir }
    }

    fn service(id: &str) -> Option<&'static Service> {
        let (_, local) = split_id(id)?;
        SERVICES.iter().find(|s| s.id == local)
    }

    fn item(s: &Service) -> Item {
        let mut it = Item::new(format!("web:{}", s.id), "web", ItemKind::App, s.title);
        it.subtitle = s.category.to_string();
        it.category = Some(s.category.to_string());
        it.description = s.desc.to_string();
        it.state = ItemState::Installed;
        it.art = ArtFlags { cover: false, hero: false, icon: s.icon.is_some() };
        it.tags = vec!["lib".into(), "web".into()];
        it.hue = (s.hue[0], s.hue[1]);
        it.brand = Some(Brand { bg: s.bg.to_string(), mode: if s.icon.is_some() { s.mode.to_string() } else { "i".into() } });
        it
    }
}

#[async_trait]
impl Provider for WebProvider {
    fn id(&self) -> &'static str {
        "web"
    }

    async fn library(&self) -> anyhow::Result<Vec<Item>> {
        Ok(SERVICES.iter().map(Self::item).collect())
    }

    async fn launch(&self, id: &str) -> anyhow::Result<LaunchPlan> {
        let s = Self::service(id).ok_or(ProviderError::NotFound)?;
        let (browser, kind) = find_browser().ok_or_else(|| ProviderError::Msg("nenhum navegador encontrado (instale o Google Chrome, o Chromium ou o Firefox)".into()))?;
        let profile = self.data_dir.join("web-profile");
        let _ = std::fs::create_dir_all(&profile);
        Ok(LaunchPlan { argv: launch_argv(&browser, &kind, s, &profile), env: vec![], cwd: None, tracker: Tracker::Process })
    }

    fn art(&self, id: &str, kind: ArtKind) -> ArtSource {
        match (kind, Self::service(id).and_then(|s| s.icon)) {
            (ArtKind::Icon, Some((f, _))) => {
                let p = self.icon_dir.join(f);
                if p.is_file() { ArtSource::File(p) } else { ArtSource::Missing }
            }
            _ => ArtSource::Missing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_items_and_argv() {
        let d = tempfile::tempdir().unwrap();
        let w = WebProvider::new(d.path().join("data"), d.path().join("cache"));
        let lib = tokio::runtime::Runtime::new().unwrap().block_on(w.library()).unwrap();
        assert!(lib.len() >= 10 && lib.iter().all(|i| i.kind == ItemKind::App && i.state == ItemState::Installed));
        let ids: Vec<_> = lib.iter().map(|i| i.id.as_str()).collect();
        for want in ["web:netflix", "web:youtube", "web:primevideo", "web:disneyplus", "web:spotify", "web:twitch"] {
            assert!(ids.contains(&want), "{want}");
        }
        assert!(matches!(w.art("web:netflix", ArtKind::Icon), ArtSource::File(_)), "ícone embutido gravado no cache");
        assert_eq!(w.art("web:max", ArtKind::Icon), ArtSource::Missing);
        let yt = SERVICES.iter().find(|s| s.id == "youtube").unwrap();
        let a = launch_argv(Path::new("/usr/bin/google-chrome"), &BrowserKind::Chromium, yt, Path::new("/p"));
        assert!(a.contains(&"--app=https://www.youtube.com/tv".to_string()) && a.contains(&"--kiosk".to_string()) && a.iter().any(|x| x.starts_with("--user-agent=")));
        let nf = SERVICES.iter().find(|s| s.id == "netflix").unwrap();
        assert!(!launch_argv(Path::new("/c"), &BrowserKind::Chromium, nf, Path::new("/p")).iter().any(|x| x.starts_with("--user-agent=")));
        assert_eq!(launch_argv(Path::new("/f"), &BrowserKind::Firefox, nf, Path::new("/p")), vec!["/f", "--kiosk", "--new-window", "https://www.netflix.com/browse"]);
    }
}
