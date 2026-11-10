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
use serde::{Deserialize, Serialize};
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

/// Item da vitrine da Steam (loja pública, sem chave de API).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoreEntry {
    pub appid: u32,
    pub name: String,
    pub final_cents: Option<u64>,
    pub orig_cents: Option<u64>,
    pub discount: u32,
    pub free: bool,
    pub linux: bool,
}

/// Detalhes de um app (tela do item). Guardados em cache.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Details {
    pub name: String,
    pub age: u8,
    pub free: bool,
    pub desc: String,
    pub genres: Vec<String>,
    pub developer: String,
    pub year: Option<u16>,
    pub linux: bool,
}

pub const STORE_CATS: &[&str] = &["destaques", "linux", "gratis", "ofertas", "lancamentos"];

pub fn fmt_brl(cents: u64) -> String {
    let reais = cents / 100;
    let mut s = reais.to_string();
    let mut out = String::new();
    for (i, c) in s.drain(..).rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    format!("R$ {},{:02}", out.chars().rev().collect::<String>(), cents % 100)
}

pub fn parse_featured(json: &str, key: &str) -> Vec<StoreEntry> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return vec![] };
    v[key]["items"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|i| i["type"].as_i64() == Some(0))
                .filter_map(|i| {
                    let final_cents = i["final_price"].as_u64();
                    Some(StoreEntry {
                        appid: i["id"].as_u64()? as u32,
                        name: i["name"].as_str()?.to_string(),
                        final_cents,
                        orig_cents: i["original_price"].as_u64(),
                        discount: i["discount_percent"].as_u64().unwrap_or(0) as u32,
                        free: final_cents == Some(0),
                        linux: i["linux_available"].as_bool().unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A busca devolve só nome e imagem; o appid sai do endereço da imagem.
pub fn parse_search(json: &str) -> Vec<(u32, String)> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return vec![] };
    v["items"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|i| {
                    let logo = i["logo"].as_str()?;
                    let rest = logo.split("/apps/").nth(1)?;
                    let id: u32 = rest.split('/').next()?.parse().ok()?;
                    Some((id, i["name"].as_str()?.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// appid -> (preço final, preço original, desconto%). Apps sem `price_overview` são gratuitos.
pub fn parse_prices(json: &str) -> HashMap<u32, (Option<u64>, Option<u64>, u32)> {
    let mut out = HashMap::new();
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return out };
    if let Some(o) = v.as_object() {
        for (k, e) in o {
            let (Ok(id), true) = (k.parse::<u32>(), e["success"].as_bool().unwrap_or(false)) else { continue };
            let po = &e["data"]["price_overview"];
            out.insert(id, (po["final"].as_u64(), po["initial"].as_u64(), po["discount_percent"].as_u64().unwrap_or(0) as u32));
        }
    }
    out
}

pub fn parse_details(json: &str, appid: u32) -> Option<Details> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let e = &v[appid.to_string()];
    if !e["success"].as_bool()? {
        return None;
    }
    let d = &e["data"];
    let age = match &d["required_age"] {
        serde_json::Value::Number(n) => n.as_u64().unwrap_or(0),
        serde_json::Value::String(s) => s.trim().parse().unwrap_or(0),
        _ => 0,
    };
    let year = d["release_date"]["date"].as_str().and_then(|s| s.rsplit(' ').next()).and_then(|y| y.trim().parse::<u16>().ok()).filter(|y| *y > 1970);
    Some(Details {
        name: d["name"].as_str().unwrap_or("").to_string(),
        age: age.min(18) as u8,
        free: d["is_free"].as_bool().unwrap_or(false),
        desc: d["short_description"].as_str().unwrap_or("").to_string(),
        genres: d["genres"].as_array().map(|g| g.iter().filter_map(|x| x["description"].as_str().map(String::from)).collect()).unwrap_or_default(),
        developer: d["developers"].as_array().and_then(|x| x.first()).and_then(|x| x.as_str()).unwrap_or("").to_string(),
        year,
        linux: d["platforms"]["linux"].as_bool().unwrap_or(false),
    })
}

fn http_get(url: &str) -> Option<String> {
    if std::env::var("NEXUS_OFFLINE").is_ok_and(|v| v == "1") {
        return None;
    }
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(12))).build().into();
    agent.get(url).call().ok()?.body_mut().read_to_string().ok()
}

fn fetch_prices(ids: &[u32]) -> HashMap<u32, (Option<u64>, Option<u64>, u32)> {
    let mut all = HashMap::new();
    for chunk in ids.chunks(25) {
        let list = chunk.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        if let Some(b) = http_get(&format!("https://store.steampowered.com/api/appdetails?appids={list}&filters=price_overview&cc=br&l=portuguese")) {
            all.extend(parse_prices(&b));
        }
    }
    all
}

/// Busca na loja (rede): `params` vai para /search/results (ex.: "term=…", "filter=topsellers").
fn store_search(params: &str, linux: bool, free: bool) -> Vec<StoreEntry> {
    let Some(b) = http_get(&format!("https://store.steampowered.com/search/results/?query&start=0&count=30&{params}&json=1&cc=br&l=portuguese")) else { return vec![] };
    let found = parse_search(&b);
    let ids: Vec<u32> = found.iter().map(|f| f.0).collect();
    let prices = if free { HashMap::new() } else { fetch_prices(&ids) };
    found
        .into_iter()
        .map(|(appid, name)| {
            let (f, o, d) = prices.get(&appid).copied().unwrap_or((None, None, 0));
            StoreEntry { appid, name, free: free || (prices.contains_key(&appid) && f.is_none()), final_cents: if free { Some(0) } else { f }, orig_cents: o, discount: d, linux }
        })
        .collect()
}

pub fn url_encode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// Baixa a vitrine de uma categoria (rede). Roda em thread própria.
pub fn fetch_feed(cat: &str) -> Vec<StoreEntry> {
    // gancho de teste: com NEXUS_OFFLINE=1 a vitrine vem de um arquivo JSON {categoria: [entradas]}
    if std::env::var("NEXUS_OFFLINE").is_ok_and(|v| v == "1") {
        return std::env::var("NEXUS_STEAM_FIXTURE")
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str::<HashMap<String, Vec<StoreEntry>>>(&t).ok())
            .and_then(|mut m| m.remove(cat))
            .unwrap_or_default();
    }
    let search = store_search;
    match cat {
        "destaques" => search("filter=topsellers&category1=998", false, false),
        "linux" => search("filter=topsellers&category1=998&os=linux", true, false),
        "gratis" => search("maxprice=free&category1=998&sort_by=Reviews_DESC", false, true),
        "ofertas" => search("specials=1&category1=998", false, false),
        "lancamentos" => http_get("https://store.steampowered.com/api/featuredcategories?cc=br&l=portuguese").map(|b| parse_featured(&b, "new_releases")).unwrap_or_default(),
        _ => vec![],
    }
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

    /// Comando que inicia a Steam: caminho configurado, /usr/games/steam (Debian/Ubuntu), PATH ou Flatpak.
    pub fn steam_cmd(&self) -> Vec<String> {
        if self.cfg.steam_bin != "steam" {
            return vec![self.cfg.steam_bin.clone()];
        }
        for p in ["/usr/games/steam", "/usr/bin/steam", "/usr/local/bin/steam"] {
            if Path::new(p).is_file() {
                return vec![p.to_string()];
            }
        }
        let home = std::env::var("HOME").unwrap_or_default();
        if Path::new(&format!("{home}/.var/app/com.valvesoftware.Steam")).exists() || Path::new("/var/lib/flatpak/app/com.valvesoftware.Steam").exists() {
            return vec!["flatpak".into(), "run".into(), "com.valvesoftware.Steam".into()];
        }
        vec!["steam".into()]
    }

    /// Máquina sem Steam possível (Raspberry Pi e outros ARM: não há cliente Linux para ARM).
    /// Aí a loja da Steam some: mostrar jogos que não dá para instalar só confundiria.
    pub fn impossible_here(&self) -> bool {
        !cfg!(target_arch = "x86_64") && self.root().is_none() && !self.client_available()
    }

    /// O programa da Steam existe (mesmo que nunca tenha sido aberto e ainda não haja pasta de dados).
    pub fn client_available(&self) -> bool {
        let cmd = self.steam_cmd();
        if cmd[0] == "flatpak" {
            return true;
        }
        let p = Path::new(&cmd[0]);
        if p.is_absolute() {
            return p.is_file();
        }
        std::env::var_os("PATH").is_some_and(|paths| std::env::split_paths(&paths).any(|d| d.join(&cmd[0]).is_file()))
    }

    /// O cliente da Steam (ubuntu12_32/steam) está rodando.
    pub fn running(&self) -> bool {
        crate::procs::list().into_iter().any(|p| crate::procs::cmdline(p.pid).first().is_some_and(|a| a.ends_with("ubuntu12_32/steam")))
    }

    /// Pasta com um `zenity`/`yad` que responde "sim" só à pergunta de instalação do launcher Debian
    /// (`/usr/games/steam`: "Steam is proprietary software… Install/Cancel"). Quem instalou o Nexus com a Steam já
    /// aceitou; na TV essa janela não pode ser respondida com controle. Outras janelas (progresso) usam o programa real.
    fn dialog_shim(&self) -> Option<PathBuf> {
        let dir = self.cfg.runtime_dir.join("steam-shim");
        std::fs::create_dir_all(&dir).ok()?;
        for (name, real) in [("zenity", "/usr/bin/zenity"), ("yad", "/usr/bin/yad")] {
            let p = dir.join(name);
            let body = format!("#!/bin/sh\ncase \" $* \" in *\" --question \"*'Steam installer'*) exit 0 ;; esac\nexec {real} \"$@\"\n");
            std::fs::write(&p, body).ok()?;
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).ok()?;
        }
        Some(dir)
    }

    fn spawn_detached(&self, args: &[&str]) -> anyhow::Result<()> {
        let mut cmd = self.steam_cmd();
        // na sessão, a Steam vive num escopo próprio: reiniciar o nexus-core.service não a derruba (nem o jogo que ela abriu)
        if self.cfg.systemd_scope && !self.running() {
            let mut pre: Vec<String> = ["systemd-run", "--user", "--scope", "--collect", "--quiet"].iter().map(|s| s.to_string()).collect();
            pre.push(format!("--unit=nexus-steam-{}", crate::util::random_hex(4)));
            pre.push("--".into());
            pre.append(&mut cmd);
            cmd = pre;
        }
        let mut c = std::process::Command::new(&cmd[0]);
        c.args(&cmd[1..]).args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        if let Some(shim) = self.dialog_shim() {
            let path = std::env::var("PATH").unwrap_or_else(|_| "/usr/local/bin:/usr/bin:/bin:/usr/games".into());
            c.env("PATH", format!("{}:{path}", shim.display()));
        }
        use std::os::unix::process::CommandExt;
        unsafe {
            c.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        let mut child = c.spawn().map_err(|e| ProviderError::Msg(format!("não foi possível executar a Steam ({}): {e}. A Steam está instalada?", cmd[0])))?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }

    /// Abre a Steam no modo TV (Big Picture): login com QR code, compras e ajustes navegáveis por controle.
    pub fn open_client(&self) -> anyhow::Result<()> {
        if self.running() {
            self.spawn_detached(&["steam://open/bigpicture"])
        } else {
            self.spawn_detached(&["-gamepadui"])
        }
    }

    /// Inicia a Steam escondida (sem janela), para os jogos abrirem rápido. Só com uma conta já conectada.
    pub fn start_silent(&self) -> anyhow::Result<bool> {
        if self.running() || !self.client_available() || self.user().is_none() {
            return Ok(false);
        }
        self.spawn_detached(&["-silent"])?;
        Ok(true)
    }

    pub fn open_store(&self, appid: u32) -> anyhow::Result<()> {
        self.spawn_detached(&[&format!("steam://store/{appid}")])
    }

    fn installed_map(&self) -> HashMap<u32, Item> {
        let Some(root) = self.root() else { return HashMap::new() };
        let play = self.playtimes(&root);
        let mut m = HashMap::new();
        for dir in self.steamapps_dirs(&root) {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                    continue;
                }
                let Some(a) = std::fs::read_to_string(e.path()).ok().and_then(|t| parse_acf(&t)) else { continue };
                if !is_tool(&a.name, a.appid) {
                    m.insert(a.appid, self.item_from_acf(&a, play.get(&a.appid)));
                }
            }
        }
        m
    }

    fn owned_ids(&self) -> HashMap<u32, Owned> {
        let mut m: HashMap<u32, Owned> = self.owned_cached().into_iter().map(|o| (o.appid, o)).collect();
        // fallback sem chave: jogos que já foram abertos nesta conta (localconfig), com nome resolvido uma vez
        if let Some(root) = self.root() {
            for (appid, (min, last)) in self.playtimes(&root) {
                if m.contains_key(&appid) {
                    continue;
                }
                let name = self.db.kv_get(&format!("sn:{appid}")).filter(|n| !n.is_empty() && n != "!");
                if let Some(name) = name {
                    if !is_tool(&name, appid) {
                        m.insert(appid, Owned { appid, name, playtime_min: min, last_played: last });
                    }
                }
            }
        }
        m
    }

    /// Resolve nomes de jogos antigos da conta que ainda não têm nome (limitado para não estourar a taxa da loja).
    pub async fn resolve_local_names(&self) -> usize {
        let Some(root) = self.root() else { return 0 };
        let todo: Vec<u32> = self.playtimes(&root).keys().copied().filter(|id| self.db.kv_get(&format!("sn:{id}")).is_none()).take(30).collect();
        let mut n = 0;
        for id in todo {
            let body = tokio::task::spawn_blocking(move || http_get(&format!("https://store.steampowered.com/api/appdetails?appids={id}&filters=basic&cc=br&l=portuguese"))).await.ok().flatten();
            let name = body.as_deref().and_then(|b| parse_details(b, id)).map(|d| d.name).filter(|n| !n.is_empty());
            let _ = self.db.kv_set(&format!("sn:{id}"), name.as_deref().unwrap_or("!"));
            n += name.is_some() as usize;
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
        n
    }

    async fn feed(&self, cat: &str) -> Vec<StoreEntry> {
        let key = format!("sf:{cat}");
        let cached: Option<(i64, Vec<StoreEntry>)> = self.db.kv_get(&key).and_then(|s| serde_json::from_str(&s).ok());
        if let Some((ts, v)) = &cached {
            if now_unix() - ts < 6 * 3600 && !v.is_empty() {
                return v.clone();
            }
        }
        let c = cat.to_string();
        let fresh = tokio::task::spawn_blocking(move || fetch_feed(&c)).await.unwrap_or_default();
        if fresh.is_empty() {
            return cached.map(|c| c.1).unwrap_or_default();
        }
        if let Ok(j) = serde_json::to_string(&(now_unix(), &fresh)) {
            let _ = self.db.kv_set(&key, &j);
        }
        fresh
    }

    /// Busca por nome na loja da Steam (jogos ainda não instalados). A última busca fica guardada para abrir os detalhes.
    pub async fn search_store(&self, q: &str) -> Vec<Item> {
        let q = q.trim();
        if q.chars().count() < 3 || self.impossible_here() {
            return vec![];
        }
        let params = format!("term={}&category1=998", url_encode(q));
        let entries = tokio::task::spawn_blocking(move || store_search(&params, false, false)).await.unwrap_or_default();
        if !entries.is_empty() {
            if let Ok(j) = serde_json::to_string(&(now_unix(), &entries)) {
                let _ = self.db.kv_set("sf:busca", &j);
            }
        }
        let (installed, owned) = (self.installed_map(), self.owned_ids());
        entries.iter().map(|e| self.item_from_entry(e, "busca", &installed, &owned)).collect()
    }

    fn cached_entry(&self, appid: u32) -> Option<StoreEntry> {
        STORE_CATS.iter().chain(["busca"].iter()).find_map(|c| {
            let (_, v): (i64, Vec<StoreEntry>) = serde_json::from_str(&self.db.kv_get(&format!("sf:{c}"))?).ok()?;
            v.into_iter().find(|e| e.appid == appid)
        })
    }

    fn item_from_entry(&self, e: &StoreEntry, cat: &str, installed: &HashMap<u32, Item>, owned: &HashMap<u32, Owned>) -> Item {
        let mut it = installed.get(&e.appid).cloned().unwrap_or_else(|| {
            let mut it = Item::new(format!("steam:{}", e.appid), "steam", ItemKind::Game, e.name.clone());
            it.state = ItemState::NotInstalled;
            it.subtitle = if e.linux { "Steam · Linux".into() } else { "Steam".into() };
            it.art = ArtFlags { cover: true, hero: true, icon: false };
            let own = owned.contains_key(&e.appid);
            it.caps.install = own || e.free;
            if own {
                it.tags.push("owned".into());
            } else if e.free {
                it.price = Some("Grátis".into());
            } else if let Some(f) = e.final_cents {
                it.price = Some(fmt_brl(f));
                if e.discount > 0 {
                    it.offer = Some(Offer { old: e.orig_cents.map(fmt_brl).unwrap_or_default(), new: fmt_brl(f), pct: format!("-{}%", e.discount) });
                }
            }
            it
        });
        it.tags.retain(|t| !t.starts_with("store:"));
        it.tags.push(format!("store:{cat}"));
        it
    }

    fn apply_details(it: &mut Item, d: &Details) {
        if !d.desc.is_empty() {
            it.description = d.desc.clone();
        }
        if !d.genres.is_empty() {
            it.subtitle = d.genres.join(" · ");
        }
        if !d.developer.is_empty() {
            it.studio = Some(d.developer.clone());
        }
        it.year = d.year;
        it.rating = Some(d.age);
    }

    async fn details(&self, appid: u32) -> Option<Details> {
        let key = format!("sd:{appid}");
        if let Some(d) = self.db.kv_get(&key).and_then(|s| serde_json::from_str::<Details>(&s).ok()) {
            return Some(d);
        }
        let body = tokio::task::spawn_blocking(move || http_get(&format!("https://store.steampowered.com/api/appdetails?appids={appid}&cc=br&l=portuguese"))).await.ok().flatten()?;
        let d = parse_details(&body, appid)?;
        let _ = self.db.kv_set(&key, &serde_json::to_string(&d).ok()?);
        Some(d)
    }
}

#[async_trait]
impl Provider for SteamProvider {
    fn id(&self) -> &'static str {
        "steam"
    }

    async fn library(&self) -> anyhow::Result<Vec<Item>> {
        if self.root().is_none() {
            return Ok(vec![]);
        }
        let mut items = self.installed_map();
        for (appid, o) in self.owned_ids() {
            if items.contains_key(&appid) || is_tool(&o.name, appid) {
                continue;
            }
            let mut it = Item::new(format!("steam:{appid}"), "steam", ItemKind::Game, o.name.clone());
            it.state = ItemState::NotInstalled;
            it.subtitle = "Steam · na sua conta".into();
            it.art = ArtFlags { cover: true, hero: true, icon: false };
            it.caps.install = true;
            it.playtime_sec = o.playtime_min * 60;
            it.last_played = (o.last_played > 0).then_some(o.last_played);
            it.tags = vec!["lib".into(), "owned".into()];
            items.insert(appid, it);
        }
        let mut v: Vec<Item> = items.into_values().collect();
        v.sort_by(|a, b| b.last_played.cmp(&a.last_played).then(crate::util::norm_text(&a.title).cmp(&crate::util::norm_text(&b.title))));
        Ok(v)
    }

    async fn store(&self, category: &str) -> anyhow::Result<Vec<Item>> {
        if !STORE_CATS.contains(&category) || self.impossible_here() {
            return Ok(vec![]);
        }
        let entries = self.feed(category).await;
        let (installed, owned) = (self.installed_map(), self.owned_ids());
        Ok(entries.iter().map(|e| self.item_from_entry(e, category, &installed, &owned)).collect())
    }

    async fn get(&self, id: &str) -> anyhow::Result<Option<Item>> {
        let (_, app) = split_id(id).ok_or(ProviderError::NotFound)?;
        let n: u32 = app.parse().map_err(|_| ProviderError::NotFound)?;
        let (installed, owned) = (self.installed_map(), self.owned_ids());
        let mut it = if let Some(i) = installed.get(&n) {
            i.clone()
        } else if let Some(o) = owned.get(&n) {
            self.library().await?.into_iter().find(|i| i.id == id).unwrap_or_else(|| {
                let mut it = Item::new(id, "steam", ItemKind::Game, o.name.clone());
                it.state = ItemState::NotInstalled;
                it
            })
        } else if let Some(e) = self.cached_entry(n) {
            self.item_from_entry(&e, "destaques", &installed, &owned)
        } else {
            let d = self.details(n).await.ok_or(ProviderError::NotFound)?;
            let mut it = Item::new(id, "steam", ItemKind::Game, d.name.clone());
            it.state = ItemState::NotInstalled;
            it.art = ArtFlags { cover: true, hero: true, icon: false };
            it.caps.install = d.free;
            it
        };
        if let Some(d) = self.details(n).await {
            Self::apply_details(&mut it, &d);
        }
        Ok(Some(it))
    }

    async fn install(&self, id: &str) -> anyhow::Result<()> {
        let (_, app) = split_id(id).ok_or(ProviderError::NotFound)?;
        let n: u32 = app.parse().map_err(|_| ProviderError::NotFound)?;
        self.spawn_detached(&[&format!("steam://install/{n}")])
    }

    async fn launch(&self, id: &str) -> anyhow::Result<LaunchPlan> {
        let (_, app) = split_id(id).ok_or(ProviderError::NotFound)?;
        let n: u32 = app.parse().map_err(|_| ProviderError::NotFound)?;
        let mut argv = self.steam_cmd();
        argv.push(format!("steam://rungameid/{n}"));
        Ok(LaunchPlan { argv, env: vec![], cwd: None, tracker: Tracker::SteamApp(n) })
    }

    fn art(&self, id: &str, kind: ArtKind) -> ArtSource {
        let Some((_, app)) = split_id(id) else { return ArtSource::Missing };
        let Ok(n) = app.parse::<u32>() else { return ArtSource::Missing };
        let local = match kind {
            ArtKind::Cover => format!("{n}_library_600x900.jpg"),
            ArtKind::Hero => format!("{n}_library_hero.jpg"),
            ArtKind::Icon => return ArtSource::Missing,
        };
        if let Some(root) = self.root() {
            let f = root.join("appcache/librarycache").join(&local);
            if f.is_file() {
                return ArtSource::File(f);
            }
        }
        let base = format!("https://cdn.akamai.steamstatic.com/steam/apps/{n}");
        ArtSource::Urls(match kind {
            ArtKind::Cover => vec![format!("{base}/library_600x900.jpg"), format!("{base}/header.jpg")],
            _ => vec![format!("{base}/library_hero.jpg"), format!("{base}/page_bg_generated_v6b.jpg"), format!("{base}/header.jpg")],
        })
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
        cfg.steam_bin = "/opt/fake/steam".into();
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
        assert_eq!(plan.argv, vec!["/opt/fake/steam", "steam://rungameid/570"]);
        assert_eq!(plan.tracker, Tracker::SteamApp(570));
        assert!(matches!(p.art("steam:570", ArtKind::Cover), ArtSource::Urls(u) if u[0].ends_with("/570/library_600x900.jpg") && u[1].ends_with("/570/header.jpg")));
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

    #[test]
    fn money_format() {
        assert_eq!(fmt_brl(5949), "R$ 59,49");
        assert_eq!(fmt_brl(0), "R$ 0,00");
        assert_eq!(fmt_brl(109900), "R$ 1.099,00");
        assert_eq!(fmt_brl(5), "R$ 0,05");
    }

    #[test]
    fn parses_store_feeds() {
        let feat = r#"{"new_releases":{"items":[
            {"id":3218570,"type":0,"name":"TANUKI","discounted":false,"discount_percent":0,"original_price":4999,"final_price":4999,"linux_available":false},
            {"id":54029,"type":1,"name":"Upgrade (pacote)","final_price":7499},
            {"id":99,"type":0,"name":"Gratuito","final_price":0,"original_price":0,"discount_percent":0,"linux_available":true}]}}"#;
        let v = parse_featured(feat, "new_releases");
        assert_eq!(v.len(), 2, "pacotes (type 1) ficam de fora");
        assert_eq!((v[0].appid, v[0].final_cents, v[0].free), (3218570, Some(4999), false));
        assert!(v[1].free && v[1].linux);
        assert!(parse_featured("lixo", "x").is_empty());
        let search = r#"{"items":[{"name":"Counter-Strike 2","logo":"https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/730/capsule_sm_120.jpg?t=1"},{"name":"sem imagem","logo":"https://x/y.jpg"}]}"#;
        assert_eq!(parse_search(search), vec![(730, "Counter-Strike 2".to_string())]);
        let prices = r#"{"10":{"success":true,"data":{"price_overview":{"final":2999,"initial":4999,"discount_percent":40}}},"11":{"success":true,"data":[]},"12":{"success":false}}"#;
        let p = parse_prices(prices);
        assert_eq!(p[&10], (Some(2999), Some(4999), 40));
        assert_eq!(p[&11], (None, None, 0), "sem price_overview = gratuito");
        assert!(!p.contains_key(&12));
    }

    #[test]
    fn parses_details() {
        let j = r#"{"570":{"success":true,"data":{"name":"Dota 2","required_age":"13","is_free":true,"short_description":"Muitos heróis.","genres":[{"description":"Ação"},{"description":"Estratégia"}],"release_date":{"date":"9 jul. 2013"},"developers":["Valve"],"platforms":{"linux":true}}}}"#;
        let d = parse_details(j, 570).unwrap();
        assert_eq!((d.name.as_str(), d.age, d.free, d.year, d.developer.as_str()), ("Dota 2", 13, true, Some(2013), "Valve"));
        assert_eq!(d.genres, vec!["Ação", "Estratégia"]);
        assert!(d.linux);
        assert!(parse_details(r#"{"1":{"success":false}}"#, 1).is_none());
    }

    #[tokio::test]
    async fn store_items_from_cached_feed() {
        std::env::set_var("NEXUS_OFFLINE", "1");
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("Steam");
        std::fs::create_dir_all(root.join("steamapps")).unwrap();
        std::fs::write(root.join("steamapps/appmanifest_570.acf"), ACF).unwrap();
        let mut cfg = Config::for_test(d.path());
        cfg.steam_root = Some(root);
        let db = Arc::new(Db::memory().unwrap());
        let entries = vec![
            StoreEntry { appid: 570, name: "Dota 2".into(), final_cents: Some(0), orig_cents: None, discount: 0, free: true, linux: true },
            StoreEntry { appid: 1, name: "Pago".into(), final_cents: Some(5949), orig_cents: Some(6999), discount: 15, free: false, linux: false },
            StoreEntry { appid: 2, name: "Grátis novo".into(), final_cents: Some(0), orig_cents: None, discount: 0, free: true, linux: false },
            StoreEntry { appid: 3, name: "Da conta".into(), final_cents: Some(2000), orig_cents: None, discount: 0, free: false, linux: false },
        ];
        db.kv_set("sf:ofertas", &serde_json::to_string(&(now_unix(), &entries)).unwrap()).unwrap();
        db.kv_set("steam_owned", r#"{"response":{"games":[{"appid":3,"name":"Da conta","playtime_forever":0}]}}"#).unwrap();
        let p = SteamProvider::new(cfg, db);
        let s = p.store("ofertas").await.unwrap();
        assert_eq!(s.len(), 4);
        assert_eq!(s[0].state, ItemState::Installed, "já instalado aparece como instalado");
        assert_eq!((s[1].price.as_deref(), s[1].caps.install, s[1].offer.as_ref().map(|o| o.pct.as_str())), (Some("R$ 59,49"), false, Some("-15%")), "pago e não é seu: não instala direto");
        assert_eq!((s[2].price.as_deref(), s[2].caps.install), (Some("Grátis"), true));
        assert_eq!((s[3].caps.install, s[3].price.clone()), (true, None), "da sua conta: instala sem preço");
        assert!(s.iter().all(|i| i.tags.contains(&"store:ofertas".to_string())));
        assert!(p.store("categoria-que-nao-existe").await.unwrap().is_empty());
        assert_eq!(p.get("steam:1").await.unwrap().unwrap().title, "Pago", "item da vitrine abre pelo cache");
    }

    #[tokio::test]
    async fn owned_from_local_playtime_when_no_api_key() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("Steam");
        std::fs::create_dir_all(root.join("steamapps")).unwrap();
        std::fs::create_dir_all(root.join("userdata/42/config")).unwrap();
        std::fs::write(root.join("userdata/42/config/localconfig.vdf"), "\"UserLocalConfigStore\"\n{\n\"Software\"\n{\n\"Valve\"\n{\n\"Steam\"\n{\n\"apps\"\n{\n\"620\"\n{\n\"Playtime\"\t\"300\"\n}\n\"7\"\n{\n\"Playtime\"\t\"9\"\n}\n}\n}\n}\n}\n}").unwrap();
        let mut cfg = Config::for_test(d.path());
        cfg.steam_root = Some(root);
        let db = Arc::new(Db::memory().unwrap());
        db.kv_set("sn:620", "Portal 2").unwrap();
        db.kv_set("sn:7", "!").unwrap();
        let p = SteamProvider::new(cfg, db);
        let lib = p.library().await.unwrap();
        assert_eq!(lib.len(), 1, "só o que tem nome resolvido");
        assert_eq!((lib[0].title.as_str(), lib[0].state, lib[0].playtime_sec, lib[0].caps.install), ("Portal 2", ItemState::NotInstalled, 18000, true));
    }
}
