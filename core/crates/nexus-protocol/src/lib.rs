//! Tipos trocados entre a Shell, o Core e o Remote. Gerados para TypeScript com `ts-rs`
//! (`cargo test -p nexus-protocol` escreve em packages/protocol/src/generated).
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub type ItemId = String;

#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ItemKind {
    Game,
    App,
}

#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ItemState {
    NotInstalled,
    Installing,
    Installed,
    Updating,
    Running,
    Error,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ArtFlags {
    pub cover: bool,
    pub hero: bool,
    pub icon: bool,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Caps {
    pub install: bool,
    pub pause: bool,
    pub cancel: bool,
    pub uninstall: bool,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstallProgress {
    #[ts(type = "number")]
    pub bytes_done: u64,
    #[ts(type = "number")]
    pub bytes_total: u64,
    #[ts(type = "number")]
    pub bps: u64,
    #[ts(type = "number | null")]
    pub eta_sec: Option<u64>,
    pub percent: f32,
    pub paused: bool,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Achievements {
    pub got: u32,
    pub total: u32,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Offer {
    pub old: String,
    pub new: String,
    pub pct: String,
}

/// Aparência do ícone de apps (fundo CSS + logotipo opcional servido como arte "icon").
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Brand {
    pub bg: String,
    /// "i" = ícone sobre cor com o nome embaixo, "l" = logotipo largo, "lw" = logotipo branco
    pub mode: String,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Item {
    pub id: ItemId,
    pub provider: String,
    pub kind: ItemKind,
    pub title: String,
    pub subtitle: String,
    pub description: String,
    pub studio: Option<String>,
    pub year: Option<u16>,
    pub rating: Option<u8>,
    pub state: ItemState,
    #[ts(type = "number | null")]
    pub size_bytes: Option<u64>,
    pub art: ArtFlags,
    pub caps: Caps,
    #[ts(type = "number")]
    pub playtime_sec: u64,
    #[ts(type = "number | null")]
    pub last_played: Option<i64>,
    pub progress: Option<u8>,
    pub chapter: Option<String>,
    pub achievements: Option<Achievements>,
    pub price: Option<String>,
    pub offer: Option<Offer>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub hue: (u16, u16),
    pub brand: Option<Brand>,
    /// estilo tipográfico do título (sci, neon, serif, gold, ice, pixel, sport)
    pub logo: Option<String>,
    pub install: Option<InstallProgress>,
}

impl Item {
    pub fn new(id: impl Into<String>, provider: &str, kind: ItemKind, title: impl Into<String>) -> Self {
        let id = id.into();
        let h = id.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
        Item {
            id,
            provider: provider.into(),
            kind,
            title: title.into(),
            subtitle: String::new(),
            description: String::new(),
            studio: None,
            year: None,
            rating: None,
            state: ItemState::Installed,
            size_bytes: None,
            art: ArtFlags::default(),
            caps: Caps::default(),
            playtime_sec: 0,
            last_played: None,
            progress: None,
            chapter: None,
            achievements: None,
            price: None,
            offer: None,
            category: None,
            tags: vec![],
            hue: ((h % 360) as u16, ((h / 7 + 140) % 360) as u16),
            brand: None,
            logo: None,
            install: None,
        }
    }
}

#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ProfileKind {
    Adult,
    Kid,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// nome de um avatar pré-definido ("lia", "rafa"...), "initial" (letra sobre cor) ou "photo:<arquivo>" (foto escolhida)
    pub avatar: String,
    pub hue: u16,
    pub kind: ProfileKind,
    pub has_pin: bool,
    pub age_limit: Option<u8>,
    pub daily_limit_min: Option<u32>,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Prefs {
    pub theme: String,
    pub sfx: bool,
    pub vol: u8,
    pub reduce: bool,
    pub hc: bool,
    pub subs: bool,
    pub invy: bool,
    pub vib: bool,
    pub pins: Vec<ItemId>,
    pub favs: Vec<ItemId>,
    /// B13/B14: tamanho do texto (0 normal, 1 grande, 2 muito grande)
    pub text_size: u8,
    /// B13: fundo da interface ("art" = imagem do item em foco, "solid" = cor lisa, "blue"/"green"/"purple"/"orange" = degradê)
    pub bg: String,
    /// B14: leitor de tela (fala o item em foco)
    pub screen_reader: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { theme: "dark".into(), sfx: true, vol: 60, reduce: false, hc: false, subs: false, invy: false, vib: true, pins: vec![], favs: vec![], text_size: 0, bg: "art".into(), screen_reader: false }
    }
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Parental {
    pub has_pin: bool,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SessionInfo {
    pub profile: Profile,
    pub prefs: Prefs,
    /// minutos restantes hoje (perfil infantil com limite)
    pub remaining_min: Option<u32>,
    /// itens liberados pelos responsáveis para este perfil infantil
    pub allowed: Vec<ItemId>,
}

#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum GameState {
    Launching,
    Running,
    Closing,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GameSession {
    pub item_id: ItemId,
    pub title: String,
    pub pid: Option<u32>,
    #[ts(type = "number")]
    pub started_at: i64,
    pub state: GameState,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Notification {
    pub id: u64,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub action: Option<String>,
    #[ts(type = "number")]
    pub ts: i64,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Friend {
    pub name: String,
    pub status: String,
    pub hue: u16,
    pub online: bool,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RemoteDevice {
    pub id: String,
    pub name: String,
    /// "gamepad" ou "touchpad"
    pub mode: String,
    pub player: Option<u8>,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RemotePairing {
    pub url: String,
    pub qr_svg: String,
    pub devices: Vec<RemoteDevice>,
    /// true quando existe /dev/uinput (gamepad/mouse/teclado virtuais do sistema)
    pub uinput: bool,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HardwareProfile {
    pub architecture: String,
    pub performance_tier: String,
    #[ts(type = "number")]
    pub memory_mb: u64,
    pub gpu_vendor: String,
    pub effects: String,
    pub providers_available: Vec<String>,
    pub os: String,
    #[ts(type = "number")]
    pub disk_free_bytes: u64,
    #[ts(type = "number")]
    pub disk_total_bytes: u64,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SteamAccount {
    pub installed: bool,
    pub persona: Option<String>,
    pub account: Option<String>,
    pub library_games: u32,
}

/// App ou jogo do Linux que o usuário adicionou ao Nexus (com aparência opcionalmente personalizada).
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CustomApp {
    pub id: ItemId,
    pub name: String,
    pub default_name: String,
    /// "game" ou "app"
    pub kind: String,
    pub has_icon: bool,
    pub has_cover: bool,
    pub has_hero: bool,
    /// o arquivo .desktop original ainda existe
    pub available: bool,
    /// B17: fica fora da biblioteca, mas continua instalado e registrado
    pub hidden: bool,
    /// B17: categoria escolhida (vazio = a do atalho)
    pub category: Option<String>,
}

/// Item da lista de apps instalados no sistema, para escolher quais entram no Nexus.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemApp {
    pub file_id: String,
    pub name: String,
    pub category: String,
    pub added: bool,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FsEntry {
    pub name: String,
    pub path: String,
    pub dir: bool,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FsListing {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<FsEntry>,
    /// atalhos de partida: pasta pessoal e mídias removíveis
    pub roots: Vec<FsEntry>,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Hello {
    pub version: String,
    pub demo: bool,
    pub dev: bool,
    pub providers: Vec<String>,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LibraryPage {
    pub items: Vec<Item>,
    pub total: u32,
}

/// Teclas de navegação enviadas pelo celular quando não há gamepad virtual do sistema.
pub const REMOTE_KEYS: &[&str] = &["u", "d", "l", "r", "A", "B", "Y", "OPT", "LB", "RB", "LT", "RT", "G"];

pub fn now_unix() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}
