//! Provider da Steam: lê os arquivos locais do cliente (ACF/VDF), usa a Web API opcionalmente
//! para listar jogos ainda não instalados e delega lançar/instalar ao cliente via `steam://`.
//!
//! Itens ainda a validar em hardware real (spike S1): se `steam://install/<id>` instala sem diálogo,
//! e quais valores de StateFlags aparecem em cada fase do download.
use super::*;
use crate::config::Config;
use crate::db::Db;
use keyvalues_parser::{Obj, Value, Vdf};
use nexus_protocol::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

// StateFlags conhecidos (SteamDB)
const SF_UPDATE_REQUIRED: u64 = 2;
const SF_FULLY_INSTALLED: u64 = 4;
const SF_UPDATE_RUNNING: u64 = 256;
const SF_UPDATE_PAUSED: u64 = 512;
const SF_UPDATE_STARTED: u64 = 1024;
const SF_UNINSTALLING: u64 = 2048;
const SF_DOWNLOADING: u64 = 1_048_576;
const SF_STAGING: u64 = 2_097_152;
const SF_COMMITTING: u64 = 4_194_304;

pub struct SteamProvider {
    cfg: Config,
    db: Arc<Db>,
    speed: Mutex<HashMap<u32, (Instant, u64, u64)>>, // appid -> (quando, bytes, bps suavizado)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Acf {
    pub appid: u32,
    pub name: String,
    pub state_flags: u64,
    pub size_on_disk: u64,
    pub bytes_to_download: u64,
    pub bytes_downloaded: u64,
    pub last_updated: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SteamUser {
    pub steamid: String,
    pub account: String,
    pub persona: String,
}

#[allow(deprecated)]
fn parse_vdf(text: &str) -> Option<Vdf<'_>> {
    Vdf::parse(text).ok()
}

fn get<'a>(o: &'a Obj<'a>, key: &str) -> Option<&'a Value<'a>> {
    o.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).and_then(|(_, v)| v.first())
}

fn gets<'a>(o: &'a Obj<'a>, key: &str) -> Option<&'a str> {
    get(o, key).and_then(|v| v.get_str())
}

fn getn(o: &Obj, key: &str) -> u64 {
    gets(o, key).and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

pub fn parse_acf(text: &str) -> Option<Acf> {
    let vdf = parse_vdf(text)?;
    let o = vdf.value.get_obj()?;
    Some(Acf {
        appid: gets(o, "appid")?.trim().parse().ok()?,
        name: gets(o, "name").unwrap_or("").to_string(),
        state_flags: getn(o, "StateFlags"),
        size_on_disk: getn(o, "SizeOnDisk"),
        bytes_to_download: getn(o, "BytesToDownload"),
        bytes_downloaded: getn(o, "BytesDownloaded"),
        last_updated: getn(o, "LastUpdated") as i64,
    })
}

pub fn parse_library_folders(text: &str) -> Vec<PathBuf> {
    let Some(vdf) = parse_vdf(text) else { return vec![] };
    let Some(o) = vdf.value.get_obj() else { return vec![] };
    let mut v: Vec<(u32, PathBuf)> = o
        .iter()
        .filter_map(|(k, vals)| {
            let n: u32 = k.parse().ok()?;
            let path = gets(vals.first()?.get_obj()?, "path")?;
            Some((n, PathBuf::from(path)))
        })
        .collect();
    v.sort();
    v.into_iter().map(|(_, p)| p).collect()
}

pub fn parse_loginusers(text: &str) -> Option<SteamUser> {
    let vdf = parse_vdf(text)?;
    let users = vdf.value.get_obj()?;
    let mut best: Option<SteamUser> = None;
    for (id, vals) in users.iter() {
        let Some(o) = vals.first().and_then(|v| v.get_obj()) else { continue };
        let u = SteamUser { steamid: id.to_string(), account: gets(o, "AccountName").unwrap_or("").to_string(), persona: gets(o, "PersonaName").unwrap_or("").to_string() };
        if gets(o, "MostRecent") == Some("1") {
            return Some(u);
        }
        best.get_or_insert(u);
    }
    best
}

/// appid -> (minutos jogados, último jogo em epoch)
pub fn parse_localconfig(text: &str) -> HashMap<u32, (u64, i64)> {
    let mut out = HashMap::new();
    let Some(vdf) = parse_vdf(text) else { return out };
    let Some(mut o) = vdf.value.get_obj() else { return out };
    for key in ["Software", "Valve", "Steam", "apps"] {
        match get(o, key).and_then(|v| v.get_obj()) {
            Some(n) => o = n,
            None => return out,
        }
    }
    for (id, vals) in o.iter() {
        let (Ok(appid), Some(a)) = (id.parse::<u32>(), vals.first().and_then(|v| v.get_obj())) else { continue };
        out.insert(appid, (getn(a, "Playtime"), getn(a, "LastPlayed") as i64));
    }
    out
}

pub fn is_tool(name: &str, appid: u32) -> bool {
    const TOOLS: &[u32] = &[228980, 1070560, 1391110, 1628350, 1493710, 1826330, 2180100];
    TOOLS.contains(&appid) || name.starts_with("Proton") || name.starts_with("Steam Linux Runtime") || name.starts_with("Steamworks Common") || name.starts_with("Steam Controller")
}

pub fn state_of(a: &Acf) -> ItemState {
    let f = a.state_flags;
    if f & SF_UNINSTALLING != 0 {
        return ItemState::NotInstalled;
    }
    let active = f & (SF_UPDATE_RUNNING | SF_UPDATE_STARTED | SF_DOWNLOADING | SF_STAGING | SF_COMMITTING | SF_UPDATE_PAUSED) != 0;
    if f & SF_FULLY_INSTALLED != 0 {
        if active || (f & SF_UPDATE_REQUIRED != 0 && a.bytes_to_download > 0 && a.bytes_downloaded < a.bytes_to_download && active) {
            ItemState::Updating
        } else {
            ItemState::Installed
        }
    } else {
        ItemState::Installing
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Owned {
    pub appid: u32,
    pub name: String,
    pub playtime_min: u64,
    pub last_played: i64,
}

pub fn parse_owned_games(json: &str) -> Vec<Owned> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return vec![] };
    v["response"]["games"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|g| {
                    Some(Owned {
                        appid: g["appid"].as_u64()? as u32,
                        name: g["name"].as_str()?.to_string(),
                        playtime_min: g["playtime_forever"].as_u64().unwrap_or(0),
                        last_played: g["rtime_last_played"].as_i64().unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

impl SteamProvider {
    pub fn new(cfg: Config, db: Arc<Db>) -> SteamProvider {
        SteamProvider { cfg, db, speed: Mutex::new(HashMap::new()) }
    }

    pub fn root(&self) -> Option<PathBuf> {
        if let Some(r) = &self.cfg.steam_root {
            return r.join("steamapps").is_dir().then(|| r.clone());
        }
        let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
        [home.join(".steam/steam"), home.join(".local/share/Steam"), home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam")]
            .into_iter()
            .find(|p| p.join("steamapps").is_dir())
    }

    pub fn steamapps_dirs(&self, root: &Path) -> Vec<PathBuf> {
        let mut dirs = vec![root.join("steamapps")];
        if let Ok(t) = std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            for p in parse_library_folders(&t) {
                let d = p.join("steamapps");
                if d.is_dir() && !dirs.contains(&d) {
                    dirs.push(d);
                }
            }
        }
        dirs
    }

    pub fn user(&self) -> Option<SteamUser> {
        let root = self.root()?;
        parse_loginusers(&std::fs::read_to_string(root.join("config/loginusers.vdf")).ok()?)
    }

    fn playtimes(&self, root: &Path) -> HashMap<u32, (u64, i64)> {
        let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
        if let Ok(rd) = std::fs::read_dir(root.join("userdata")) {
            for e in rd.flatten() {
                let f = e.path().join("config/localconfig.vdf");
                if let Ok(m) = f.metadata().and_then(|m| m.modified()) {
                    if best.as_ref().is_none_or(|(t, _)| m > *t) {
                        best = Some((m, f));
                    }
                }
            }
        }
        best.and_then(|(_, f)| std::fs::read_to_string(f).ok()).map(|t| parse_localconfig(&t)).unwrap_or_default()
    }

    fn owned_cached(&self) -> Vec<Owned> {
        self.db.kv_get("steam_owned").map(|s| parse_owned_games(&s)).unwrap_or_default()
    }

    pub fn owned_count(&self) -> u32 {
        self.owned_cached().len() as u32
    }

    pub fn api_key(&self) -> Option<String> {
        self.cfg.steam_api_key.clone().or_else(|| self.db.kv_get("steam_api_key"))
    }

    /// Atualiza a lista de jogos da conta pela Web API (precisa de chave e de perfil público).
    pub async fn sync_owned(&self) -> anyhow::Result<usize> {
        let (Some(key), Some(user)) = (self.api_key(), self.user()) else { return Ok(0) };
        let url = format!("https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/?key={key}&steamid={}&include_appinfo=1&include_played_free_games=1&format=json", user.steamid);
        let body = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(15))).build().into();
            Ok(agent.get(&url).call()?.body_mut().read_to_string()?)
        })
        .await??;
        let n = parse_owned_games(&body).len();
        if n > 0 {
            self.db.kv_set("steam_owned", &body)?;
        }
        Ok(n)
    }

    fn bps(&self, a: &Acf) -> u64 {
        let mut m = self.speed.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        match m.get(&a.appid).copied() {
            Some((t, bytes, old)) => {
                let dt = now.duration_since(t).as_secs_f64();
                if dt < 0.4 {
                    return old;
                }
                let inst = (a.bytes_downloaded.saturating_sub(bytes) as f64 / dt) as u64;
                let smooth = if old == 0 { inst } else { (old as f64 * 0.6 + inst as f64 * 0.4) as u64 };
                m.insert(a.appid, (now, a.bytes_downloaded, smooth));
                smooth
            }
            None => {
                m.insert(a.appid, (now, a.bytes_downloaded, 0));
                0
            }
        }
    }

    pub fn item_from_acf(&self, a: &Acf, play: Option<&(u64, i64)>) -> Item {
        let mut it = Item::new(format!("steam:{}", a.appid), "steam", ItemKind::Game, if a.name.is_empty() { format!("App {}", a.appid) } else { a.name.clone() });
        it.state = state_of(a);
        it.subtitle = "Steam".into();
        it.size_bytes = (a.size_on_disk > 0).then_some(a.size_on_disk);
        it.art = ArtFlags { cover: true, hero: true, icon: false };
        it.caps = Caps { install: false, pause: false, cancel: false, uninstall: false };
        if let Some((min, last)) = play {
            it.playtime_sec = min * 60;
            it.last_played = (*last > 0).then_some(*last);
        }
        if matches!(it.state, ItemState::Installing | ItemState::Updating) {
            let total = a.bytes_to_download.max(1);
            let bps = self.bps(a);
            let left = a.bytes_to_download.saturating_sub(a.bytes_downloaded);
            it.install = Some(InstallProgress {
                bytes_done: a.bytes_downloaded,
                bytes_total: a.bytes_to_download,
                bps,
                eta_sec: (bps > 0).then(|| left / bps),
                percent: if a.bytes_to_download == 0 { 0.0 } else { (a.bytes_downloaded as f64 / total as f64 * 100.0).min(100.0) as f32 },
                paused: a.state_flags & SF_UPDATE_PAUSED != 0,
            });
        }
        it.tags = vec!["lib".into()];
        it
    }

    fn spawn_detached(&self, uri: &str) -> anyhow::Result<()> {
        let mut c = std::process::Command::new(&self.cfg.steam_bin);
        c.arg(uri).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        use std::os::unix::process::CommandExt;
        unsafe {
            c.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        let mut child = c.spawn().map_err(|e| ProviderError::Msg(format!("não foi possível executar a Steam ({}): {e}", self.cfg.steam_bin)))?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
}

#[async_trait]
impl Provider for SteamProvider {
    fn id(&self) -> &'static str {
        "steam"
    }

    async fn library(&self) -> anyhow::Result<Vec<Item>> {
        let Some(root) = self.root() else { return Ok(vec![]) };
        let play = self.playtimes(&root);
        let mut items: HashMap<u32, Item> = HashMap::new();
        for dir in self.steamapps_dirs(&root) {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                    continue;
                }
                let Some(a) = std::fs::read_to_string(e.path()).ok().and_then(|t| parse_acf(&t)) else { continue };
                if is_tool(&a.name, a.appid) {
                    continue;
                }
                items.insert(a.appid, self.item_from_acf(&a, play.get(&a.appid)));
            }
        }
        for o in self.owned_cached() {
            if items.contains_key(&o.appid) || is_tool(&o.name, o.appid) {
                continue;
            }
            let mut it = Item::new(format!("steam:{}", o.appid), "steam", ItemKind::Game, o.name.clone());
            it.state = ItemState::NotInstalled;
            it.subtitle = "Steam · na sua conta".into();
            it.art = ArtFlags { cover: true, hero: true, icon: false };
            it.caps.install = true;
            it.playtime_sec = o.playtime_min * 60;
            it.last_played = (o.last_played > 0).then_some(o.last_played);
            it.tags = vec!["lib".into(), "owned".into()];
            items.insert(o.appid, it);
        }
        let mut v: Vec<Item> = items.into_values().collect();
        v.sort_by(|a, b| b.last_played.cmp(&a.last_played).then(crate::util::norm_text(&a.title).cmp(&crate::util::norm_text(&b.title))));
        Ok(v)
    }

    async fn install(&self, id: &str) -> anyhow::Result<()> {
        let (_, app) = split_id(id).ok_or(ProviderError::NotFound)?;
        let n: u32 = app.parse().map_err(|_| ProviderError::NotFound)?;
        self.spawn_detached(&format!("steam://install/{n}"))
    }

    async fn launch(&self, id: &str) -> anyhow::Result<LaunchPlan> {
        let (_, app) = split_id(id).ok_or(ProviderError::NotFound)?;
        let n: u32 = app.parse().map_err(|_| ProviderError::NotFound)?;
        Ok(LaunchPlan { argv: vec![self.cfg.steam_bin.clone(), format!("steam://rungameid/{n}")], env: vec![], cwd: None, tracker: Tracker::SteamApp(n) })
    }

    fn art(&self, id: &str, kind: ArtKind) -> ArtSource {
        let Some((_, app)) = split_id(id) else { return ArtSource::Missing };
        let Ok(n) = app.parse::<u32>() else { return ArtSource::Missing };
        let (local, remote) = match kind {
            ArtKind::Cover => (format!("{n}_library_600x900.jpg"), "library_600x900.jpg"),
            ArtKind::Hero => (format!("{n}_library_hero.jpg"), "library_hero.jpg"),
            ArtKind::Icon => return ArtSource::Missing,
        };
        if let Some(root) = self.root() {
            let f = root.join("appcache/librarycache").join(&local);
            if f.is_file() {
                return ArtSource::File(f);
            }
        }
        ArtSource::Url(format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{n}/{remote}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACF: &str = "\"AppState\"\n{\n\t\"appid\"\t\t\"570\"\n\t\"name\"\t\t\"Dota 2\"\n\t\"StateFlags\"\t\t\"4\"\n\t\"SizeOnDisk\"\t\t\"37000000000\"\n\t\"BytesToDownload\"\t\t\"0\"\n\t\"BytesDownloaded\"\t\t\"0\"\n\t\"LastUpdated\"\t\t\"1700000000\"\n}\n";

    fn acf(flags: u64, dl: u64, tot: u64) -> Acf {
        Acf { appid: 1, name: "X".into(), state_flags: flags, size_on_disk: 0, bytes_to_download: tot, bytes_downloaded: dl, last_updated: 0 }
    }

    #[test]
    fn parses_acf() {
        let a = parse_acf(ACF).unwrap();
        assert_eq!((a.appid, a.name.as_str(), a.state_flags, a.size_on_disk), (570, "Dota 2", 4, 37_000_000_000));
        assert!(parse_acf("lixo").is_none());
    }

    #[test]
    fn state_mapping() {
        assert_eq!(state_of(&acf(4, 0, 0)), ItemState::Installed);
        assert_eq!(state_of(&acf(1026, 100, 1000)), ItemState::Installing); // 2|1024: update started, nunca instalado
        assert_eq!(state_of(&acf(4 | 256, 100, 1000)), ItemState::Updating);
        assert_eq!(state_of(&acf(4 | 1024 | 512, 100, 1000)), ItemState::Updating);
        assert_eq!(state_of(&acf(2048, 0, 0)), ItemState::NotInstalled);
        assert_eq!(state_of(&acf(6, 0, 0)), ItemState::Installed, "instalado com atualização pendente, mas sem baixar");
        assert_eq!(state_of(&acf(2, 0, 0)), ItemState::Installing);
    }

    #[test]
    fn library_folders_loginusers_localconfig() {
        let lf = "\"libraryfolders\"\n{\n\"0\"\n{\n\"path\"\t\"/home/u/.local/share/Steam\"\n}\n\"1\"\n{\n\"path\"\t\"/mnt/games\"\n}\n}";
        assert_eq!(parse_library_folders(lf), vec![PathBuf::from("/home/u/.local/share/Steam"), PathBuf::from("/mnt/games")]);
        let lu = "\"users\"\n{\n\"7656\"\n{\n\"AccountName\"\t\"velho\"\n\"PersonaName\"\t\"Velho\"\n\"MostRecent\"\t\"0\"\n}\n\"7657\"\n{\n\"AccountName\"\t\"novo\"\n\"PersonaName\"\t\"Novo\"\n\"MostRecent\"\t\"1\"\n}\n}";
        let u = parse_loginusers(lu).unwrap();
        assert_eq!((u.account.as_str(), u.persona.as_str(), u.steamid.as_str()), ("novo", "Novo", "7657"));
        let lc = "\"UserLocalConfigStore\"\n{\n\"Software\"\n{\n\"Valve\"\n{\n\"Steam\"\n{\n\"apps\"\n{\n\"570\"\n{\n\"LastPlayed\"\t\"1699999999\"\n\"Playtime\"\t\"125\"\n}\n}\n}\n}\n}\n}";
        assert_eq!(parse_localconfig(lc).get(&570), Some(&(125, 1_699_999_999)));
    }

    #[test]
    fn owned_games_json() {
        let j = r#"{"response":{"game_count":2,"games":[{"appid":10,"name":"CS","playtime_forever":5,"rtime_last_played":100},{"appid":20,"name":"TF"}]}}"#;
        let g = parse_owned_games(j);
        assert_eq!(g.len(), 2);
        assert_eq!((g[0].appid, g[0].playtime_min, g[0].last_played), (10, 5, 100));
        assert!(parse_owned_games("{}").is_empty());
    }

    #[test]
    fn tools_are_hidden() {
        assert!(is_tool("Proton 9.0", 2805730));
        assert!(is_tool("Steam Linux Runtime 3.0 (sniper)", 1628350));
        assert!(!is_tool("Dota 2", 570));
    }

    #[tokio::test]
    async fn library_from_disk_with_two_libraries_and_progress() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("Steam");
        let extra = d.path().join("games");
        std::fs::create_dir_all(root.join("steamapps")).unwrap();
        std::fs::create_dir_all(extra.join("steamapps")).unwrap();
        std::fs::write(root.join("steamapps/libraryfolders.vdf"), format!("\"libraryfolders\"\n{{\n\"0\"\n{{\n\"path\"\t\"{}\"\n}}\n\"1\"\n{{\n\"path\"\t\"{}\"\n}}\n}}", root.display(), extra.display())).unwrap();
        std::fs::write(root.join("steamapps/appmanifest_570.acf"), ACF).unwrap();
        std::fs::write(extra.join("steamapps/appmanifest_730.acf"), ACF.replace("570", "730").replace("Dota 2", "Counter-Strike 2").replace("\"StateFlags\"\t\t\"4\"", "\"StateFlags\"\t\t\"1026\"").replace("\"BytesToDownload\"\t\t\"0\"", "\"BytesToDownload\"\t\t\"1000\"").replace("\"BytesDownloaded\"\t\t\"0\"", "\"BytesDownloaded\"\t\t\"250\"")).unwrap();
        std::fs::write(root.join("steamapps/appmanifest_1628350.acf"), ACF.replace("570", "1628350").replace("Dota 2", "Steam Linux Runtime 3.0")).unwrap();
        let mut cfg = Config::for_test(d.path());
        cfg.steam_root = Some(root.clone());
        let db = Arc::new(Db::memory().unwrap());
        let p = SteamProvider::new(cfg, db.clone());
        let lib = p.library().await.unwrap();
        assert_eq!(lib.len(), 2, "runtime filtrado");
        let cs = lib.iter().find(|i| i.id == "steam:730").unwrap();
        assert_eq!(cs.state, ItemState::Installing);
        assert_eq!(cs.install.as_ref().unwrap().percent, 25.0);
        let dota = lib.iter().find(|i| i.id == "steam:570").unwrap();
        assert_eq!(dota.state, ItemState::Installed);
        assert!(dota.install.is_none());
        // jogos da conta ainda não instalados (cache da Web API)
        db.kv_set("steam_owned", r#"{"response":{"games":[{"appid":99,"name":"Possuído","playtime_forever":60},{"appid":570,"name":"Dota 2"}]}}"#).unwrap();
        let lib = p.library().await.unwrap();
        let own = lib.iter().find(|i| i.id == "steam:99").unwrap();
        assert_eq!((own.state, own.caps.install, own.playtime_sec), (ItemState::NotInstalled, true, 3600));
        assert_eq!(lib.iter().filter(|i| i.id == "steam:570").count(), 1);
        let plan = p.launch("steam:570").await.unwrap();
        assert_eq!(plan.argv, vec!["steam", "steam://rungameid/570"]);
        assert_eq!(plan.tracker, Tracker::SteamApp(570));
        assert!(matches!(p.art("steam:570", ArtKind::Cover), ArtSource::Url(u) if u.ends_with("/570/library_600x900.jpg")));
        std::fs::create_dir_all(root.join("appcache/librarycache")).unwrap();
        std::fs::write(root.join("appcache/librarycache/570_library_600x900.jpg"), b"x").unwrap();
        assert!(matches!(p.art("steam:570", ArtKind::Cover), ArtSource::File(_)));
    }

    #[tokio::test]
    async fn no_steam_means_empty_library() {
        let d = tempfile::tempdir().unwrap();
        let mut cfg = Config::for_test(d.path());
        cfg.steam_root = Some(d.path().join("nada"));
        let p = SteamProvider::new(cfg, Arc::new(Db::memory().unwrap()));
        assert!(p.library().await.unwrap().is_empty());
        assert!(p.user().is_none());
    }
}
