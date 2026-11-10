//! Contrato único dos Providers. A Shell só conhece `Item`; cada provider monta o seu `LaunchPlan`.
use async_trait::async_trait;
use nexus_protocol::Item;
use std::path::PathBuf;

pub mod demo;
pub mod emu;
pub mod native;
pub mod steam;
pub mod web;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtKind {
    Cover,
    Hero,
    Icon,
}

impl ArtKind {
    pub fn parse(s: &str) -> Option<ArtKind> {
        match s {
            "cover" => Some(ArtKind::Cover),
            "hero" => Some(ArtKind::Hero),
            "icon" => Some(ArtKind::Icon),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArtSource {
    File(PathBuf),
    Url(String),
    /// tenta cada endereço na ordem até um funcionar
    Urls(Vec<String>),
    Missing,
}

/// Como o supervisor sabe que o jogo acabou.
#[derive(Debug, Clone, PartialEq)]
pub enum Tracker {
    /// o processo lançado e tudo que ficar na mesma sessão (setsid)
    Process,
    /// o cliente Steam repassa o jogo: procurar o `reaper ... AppId=N`
    SteamApp(u32),
}

#[derive(Debug, Clone)]
pub struct LaunchPlan {
    pub argv: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: Option<PathBuf>,
    pub tracker: Tracker,
}

#[derive(Debug)]
pub enum ProviderError {
    Unsupported,
    NotFound,
    Msg(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::Unsupported => write!(f, "operação não suportada por este provedor"),
            ProviderError::NotFound => write!(f, "item não encontrado"),
            ProviderError::Msg(m) => write!(f, "{m}"),
        }
    }
}
impl std::error::Error for ProviderError {}

#[async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &'static str;
    async fn library(&self) -> anyhow::Result<Vec<Item>>;
    async fn store(&self, _category: &str) -> anyhow::Result<Vec<Item>> {
        Ok(vec![])
    }
    async fn install(&self, _id: &str) -> anyhow::Result<()> {
        Err(ProviderError::Unsupported.into())
    }
    async fn cancel(&self, _id: &str) -> anyhow::Result<()> {
        Err(ProviderError::Unsupported.into())
    }
    async fn uninstall(&self, _id: &str) -> anyhow::Result<()> {
        Err(ProviderError::Unsupported.into())
    }
    async fn launch(&self, id: &str) -> anyhow::Result<LaunchPlan>;
    /// Busca um item pelo id (inclusive itens da loja que ainda não estão na biblioteca).
    async fn get(&self, id: &str) -> anyhow::Result<Option<Item>> {
        Ok(self.library().await?.into_iter().find(|i| i.id == id))
    }
    fn art(&self, id: &str, kind: ArtKind) -> ArtSource;
}

/// "steam:570" -> ("steam", "570")
pub fn split_id(id: &str) -> Option<(&str, &str)> {
    id.split_once(':')
}
