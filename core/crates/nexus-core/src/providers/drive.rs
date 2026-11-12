//! Item escondido `drive:login`: a janela de navegador do login do Google Drive (aberta como um app, então
//! volta ao Nexus sozinha quando a Core a fecha ao fim da autorização).
use super::web::{find_browser, BrowserKind};
use super::{ArtKind, ArtSource, LaunchPlan, Provider, ProviderError, Tracker};
use async_trait::async_trait;
use nexus_protocol::{Item, ItemKind, ItemState};
use std::path::PathBuf;

pub struct DriveProvider {
    profile: PathBuf,
}

impl DriveProvider {
    pub fn new(data_dir: PathBuf) -> DriveProvider {
        DriveProvider { profile: data_dir.join("drive-login-profile") }
    }
}

pub fn login_argv(browser: &std::path::Path, kind: &BrowserKind, url: &str, profile: &std::path::Path) -> Vec<String> {
    let mut v = vec![browser.display().to_string()];
    match kind {
        BrowserKind::Chromium => v.extend([
            format!("--app={url}"),
            "--kiosk".into(),
            format!("--user-data-dir={}", profile.display()),
            "--no-first-run".into(),
            "--no-default-browser-check".into(),
            "--disable-features=Translate,MediaRouter".into(),
            "--ozone-platform-hint=auto".into(),
            "--class=nexus-web".into(),
        ]),
        BrowserKind::Firefox => v.extend(["--kiosk".into(), "--new-window".into(), url.to_string()]),
    }
    v
}

#[async_trait]
impl Provider for DriveProvider {
    fn id(&self) -> &'static str {
        "drive"
    }

    async fn library(&self) -> anyhow::Result<Vec<Item>> {
        Ok(vec![])
    }

    async fn get(&self, id: &str) -> anyhow::Result<Option<Item>> {
        Ok((id == "drive:login").then(|| {
            let mut it = Item::new("drive:login", "drive", ItemKind::App, "Entrar no Google Drive");
            it.state = ItemState::Installed;
            it
        }))
    }

    async fn launch(&self, id: &str) -> anyhow::Result<LaunchPlan> {
        if id != "drive:login" {
            return Err(ProviderError::NotFound.into());
        }
        let url = crate::drive::login_url().ok_or_else(|| ProviderError::Msg("o login do Google Drive não foi iniciado".into()))?;
        let (browser, kind) = find_browser().ok_or_else(|| ProviderError::Msg("nenhum navegador encontrado (instale o Google Chrome, o Chromium ou o Firefox)".into()))?;
        // perfil novo a cada login: nada da conta Google fica guardado no navegador
        let _ = std::fs::remove_dir_all(&self.profile);
        let _ = std::fs::create_dir_all(&self.profile);
        Ok(LaunchPlan { argv: login_argv(&browser, &kind, &url, &self.profile), env: vec![], cwd: None, tracker: Tracker::Process })
    }

    fn art(&self, _id: &str, _kind: ArtKind) -> ArtSource {
        ArtSource::Missing
    }
}
