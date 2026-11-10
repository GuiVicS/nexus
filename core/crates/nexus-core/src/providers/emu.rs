//! Emulação (RetroArch): ROMs em `~/ROMs/<console>/`, um núcleo libretro por console, baixado do servidor
//! oficial (buildbot.libretro.com) na primeira vez, capas do libretro-thumbnails. Abre em tela cheia com os
//! controles mapeados (autoconfig) e os jogadores reservados (Ajustes → Controles).
use super::*;
use nexus_protocol::*;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct System {
    /// pasta em ~/ROMs
    pub dir: &'static str,
    pub name: &'static str,
    pub exts: &'static [&'static str],
    /// núcleo libretro (arquivo <core>_libretro.so)
    pub core: &'static str,
    /// nome da pasta no libretro-thumbnails (capas)
    pub thumbs: &'static str,
    pub hue: (u16, u16),
}

pub const SYSTEMS: &[System] = &[
    System { dir: "nes", name: "Nintendo (NES)", exts: &["nes", "fds", "unf", "unif"], core: "mesen", thumbs: "Nintendo - Nintendo Entertainment System", hue: (0, 10) },
    System { dir: "snes", name: "Super Nintendo", exts: &["sfc", "smc", "swc", "fig", "bs"], core: "snes9x", thumbs: "Nintendo - Super Nintendo Entertainment System", hue: (265, 280) },
    System { dir: "n64", name: "Nintendo 64", exts: &["n64", "z64", "v64"], core: "mupen64plus_next", thumbs: "Nintendo - Nintendo 64", hue: (140, 210) },
    System { dir: "gb", name: "Game Boy", exts: &["gb"], core: "gambatte", thumbs: "Nintendo - Game Boy", hue: (90, 100) },
    System { dir: "gbc", name: "Game Boy Color", exts: &["gbc"], core: "gambatte", thumbs: "Nintendo - Game Boy Color", hue: (300, 190) },
    System { dir: "gba", name: "Game Boy Advance", exts: &["gba"], core: "mgba", thumbs: "Nintendo - Game Boy Advance", hue: (250, 270) },
    System { dir: "nds", name: "Nintendo DS", exts: &["nds", "dsi"], core: "melondsds", thumbs: "Nintendo - Nintendo DS", hue: (210, 220) },
    System { dir: "virtualboy", name: "Virtual Boy", exts: &["vb", "vboy"], core: "mednafen_vb", thumbs: "Nintendo - Virtual Boy", hue: (0, 350) },
    System { dir: "pokemini", name: "Pokémon Mini", exts: &["min"], core: "pokemini", thumbs: "Nintendo - Pokemon Mini", hue: (50, 0) },
    System { dir: "gamecube", name: "GameCube", exts: &["iso", "gcm", "gcz", "rvz", "ciso"], core: "dolphin", thumbs: "Nintendo - GameCube", hue: (265, 250) },
    System { dir: "wii", name: "Wii", exts: &["iso", "wbfs", "rvz", "wad", "gcz"], core: "dolphin", thumbs: "Nintendo - Wii", hue: (200, 210) },
    System { dir: "megadrive", name: "Mega Drive", exts: &["md", "gen", "smd", "bin", "sg"], core: "genesis_plus_gx", thumbs: "Sega - Mega Drive - Genesis", hue: (220, 0) },
    System { dir: "mastersystem", name: "Master System", exts: &["sms"], core: "genesis_plus_gx", thumbs: "Sega - Master System - Mark III", hue: (0, 220) },
    System { dir: "gamegear", name: "Game Gear", exts: &["gg"], core: "genesis_plus_gx", thumbs: "Sega - Game Gear", hue: (215, 0) },
    System { dir: "segacd", name: "Sega CD", exts: &["chd", "cue", "iso", "m3u"], core: "genesis_plus_gx", thumbs: "Sega - Mega-CD - Sega CD", hue: (220, 230) },
    System { dir: "sega32x", name: "Sega 32X", exts: &["32x"], core: "picodrive", thumbs: "Sega - 32X", hue: (0, 30) },
    System { dir: "saturn", name: "Sega Saturn", exts: &["chd", "cue", "ccd", "m3u"], core: "mednafen_saturn", thumbs: "Sega - Saturn", hue: (230, 240) },
    System { dir: "dreamcast", name: "Dreamcast", exts: &["chd", "gdi", "cdi", "m3u"], core: "flycast", thumbs: "Sega - Dreamcast", hue: (25, 210) },
    System { dir: "psx", name: "PlayStation", exts: &["chd", "cue", "pbp", "m3u", "ccd", "img"], core: "pcsx_rearmed", thumbs: "Sony - PlayStation", hue: (220, 0) },
    System { dir: "psp", name: "PSP", exts: &["iso", "cso", "pbp", "chd"], core: "ppsspp", thumbs: "Sony - PlayStation Portable", hue: (220, 230) },
    System { dir: "atari2600", name: "Atari 2600", exts: &["a26", "bin"], core: "stella", thumbs: "Atari - 2600", hue: (20, 30) },
    System { dir: "atari5200", name: "Atari 5200", exts: &["a52", "bin"], core: "a5200", thumbs: "Atari - 5200", hue: (20, 30) },
    System { dir: "atari7800", name: "Atari 7800", exts: &["a78", "bin"], core: "prosystem", thumbs: "Atari - 7800", hue: (20, 30) },
    System { dir: "lynx", name: "Atari Lynx", exts: &["lnx", "o"], core: "handy", thumbs: "Atari - Lynx", hue: (30, 0) },
    System { dir: "jaguar", name: "Atari Jaguar", exts: &["j64", "jag", "rom", "abs", "cof"], core: "virtualjaguar", thumbs: "Atari - Jaguar", hue: (0, 10) },
    System { dir: "pcengine", name: "PC Engine / TurboGrafx-16", exts: &["pce", "sgx", "chd", "cue"], core: "mednafen_pce_fast", thumbs: "NEC - PC Engine - TurboGrafx 16", hue: (0, 0) },
    System { dir: "neogeo", name: "Neo Geo", exts: &["zip", "7z"], core: "fbneo", thumbs: "SNK - Neo Geo", hue: (45, 0) },
    System { dir: "arcade", name: "Arcade", exts: &["zip", "7z"], core: "fbneo", thumbs: "FBNeo - Arcade Games", hue: (300, 280) },
    System { dir: "mame", name: "MAME", exts: &["zip", "7z", "chd"], core: "mame2003_plus", thumbs: "MAME", hue: (280, 300) },
    System { dir: "ngp", name: "Neo Geo Pocket", exts: &["ngp", "ngc"], core: "mednafen_ngp", thumbs: "SNK - Neo Geo Pocket Color", hue: (200, 0) },
    System { dir: "wonderswan", name: "WonderSwan", exts: &["ws", "wsc", "pc2"], core: "mednafen_wswan", thumbs: "Bandai - WonderSwan Color", hue: (40, 200) },
    System { dir: "colecovision", name: "ColecoVision", exts: &["col"], core: "gearcoleco", thumbs: "Coleco - ColecoVision", hue: (0, 10) },
    System { dir: "intellivision", name: "Intellivision", exts: &["int", "bin", "rom"], core: "freeintv", thumbs: "Mattel - Intellivision", hue: (30, 20) },
    System { dir: "vectrex", name: "Vectrex", exts: &["vec", "gam", "bin"], core: "vecx", thumbs: "GCE - Vectrex", hue: (0, 0) },
    System { dir: "odyssey2", name: "Odyssey²", exts: &["bin"], core: "o2em", thumbs: "Magnavox - Odyssey2", hue: (30, 0) },
    System { dir: "3do", name: "3DO", exts: &["iso", "chd", "cue"], core: "opera", thumbs: "The 3DO Company - 3DO", hue: (0, 30) },
    System { dir: "msx", name: "MSX", exts: &["rom", "mx1", "mx2", "dsk", "cas"], core: "bluemsx", thumbs: "Microsoft - MSX", hue: (210, 0) },
    System { dir: "c64", name: "Commodore 64", exts: &["d64", "t64", "prg", "crt", "tap"], core: "vice_x64", thumbs: "Commodore - 64", hue: (230, 240) },
    System { dir: "amiga", name: "Amiga", exts: &["adf", "hdf", "lha", "ipf", "m3u"], core: "puae", thumbs: "Commodore - Amiga", hue: (210, 0) },
    System { dir: "dos", name: "DOS", exts: &["zip", "dosz", "exe", "com", "bat"], core: "dosbox_pure", thumbs: "DOS", hue: (0, 0) },
    System { dir: "scummvm", name: "ScummVM", exts: &["scummvm"], core: "scummvm", thumbs: "ScummVM", hue: (130, 30) },
    System { dir: "doom", name: "Doom", exts: &["wad", "iwad", "pwad"], core: "prboom", thumbs: "DOOM", hue: (0, 20) },
];

pub struct EmuProvider {
    roots: Vec<PathBuf>,
    /// id → (sistema, arquivo)
    index: Mutex<HashMap<String, (usize, PathBuf)>>,
}

pub fn roms_dir() -> PathBuf {
    std::env::var_os("NEXUS_ROMS_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("ROMs"))
}

/// Onde o RetroArch está: Flatpak (instalado pelo Nexus) ou nativo.
pub fn retroarch() -> Option<(Vec<String>, PathBuf)> {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    if Path::new("/var/lib/flatpak/app/org.libretro.RetroArch").exists() || home.join(".local/share/flatpak/app/org.libretro.RetroArch").exists() {
        return Some((vec!["flatpak".into(), "run".into(), "org.libretro.RetroArch".into()], home.join(".var/app/org.libretro.RetroArch/config/retroarch/cores")));
    }
    for p in ["/usr/bin/retroarch", "/usr/local/bin/retroarch"] {
        if Path::new(p).is_file() {
            return Some((vec![p.into()], home.join(".config/retroarch/cores")));
        }
    }
    None
}

pub fn core_path(dir: &Path, core: &str) -> PathBuf {
    dir.join(format!("{core}_libretro.so"))
}

/// Pasta do buildbot para esta máquina: PC (x86_64) ou ARM 64 bits (Raspberry Pi 4/5).
pub fn buildbot_arch() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "aarch64",
        "arm" => "armhf",
        _ => "x86_64",
    }
}

/// Baixa e extrai um núcleo do buildbot oficial (zip com o .so).
pub fn fetch_core(dir: &Path, core: &str) -> Result<PathBuf, String> {
    let dest = core_path(dir, core);
    if dest.is_file() {
        return Ok(dest);
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let url = format!("https://buildbot.libretro.com/nightly/linux/{}/latest/{core}_libretro.so.zip", buildbot_arch());
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(180))).build().into();
    let bytes = agent.get(&url).call().map_err(|e| format!("não foi possível baixar o núcleo {core}: {e}"))?.body_mut().with_config().limit(200 * 1024 * 1024).read_to_vec().map_err(|e| e.to_string())?;
    let zip = dir.join(format!(".{core}.zip"));
    std::fs::write(&zip, &bytes).map_err(|e| e.to_string())?;
    let ok = std::process::Command::new("unzip").args(["-o", "-q"]).arg(&zip).arg("-d").arg(dir).status().is_ok_and(|s| s.success())
        || std::process::Command::new("python3").args(["-I", "-c", "import sys,zipfile;zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])"]).arg(&zip).arg(dir).status().is_ok_and(|s| s.success());
    let _ = std::fs::remove_file(&zip);
    if ok && dest.is_file() {
        Ok(dest)
    } else {
        Err(format!("o núcleo {core} veio num formato inesperado"))
    }
}

/// Nome "limpo" da ROM para o título e para achar a capa: sem extensão; tags (USA)[!] só saem do título.
fn title_of(stem: &str) -> String {
    let mut t = String::new();
    let mut depth = 0;
    for c in stem.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            _ if depth == 0 => t.push(c),
            _ => {}
        }
    }
    let t = t.replace('_', " ");
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() { stem.to_string() } else { t }
}

fn thumb_name(stem: &str) -> String {
    // o libretro-thumbnails troca &*/:`<>?\| por _
    stem.chars().map(|c| if "&*/:`<>?\\|\"".contains(c) { '_' } else { c }).collect()
}

fn enc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~()!'".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

impl EmuProvider {
    pub fn new() -> EmuProvider {
        let root = roms_dir();
        // pastas prontas para o usuário copiar as ROMs
        if std::fs::create_dir_all(&root).is_ok() {
            for s in SYSTEMS {
                let _ = std::fs::create_dir_all(root.join(s.dir));
            }
            let readme = root.join("LEIA-ME.txt");
            if !readme.exists() {
                let mut t = String::from("Copie cada jogo para a pasta do console dele. O Nexus encontra sozinho e baixa o emulador na primeira vez que o jogo abrir.\n\n");
                for s in SYSTEMS {
                    t += &format!("{:<14} {:<28} .{}\n", s.dir, s.name, s.exts.join(" ."));
                }
                let _ = std::fs::write(readme, t);
            }
        }
        EmuProvider { roots: vec![root], index: Mutex::new(HashMap::new()) }
    }

    fn scan(&self) -> Vec<(String, usize, PathBuf)> {
        let mut out = vec![];
        for root in &self.roots {
            for (si, s) in SYSTEMS.iter().enumerate() {
                let dir = root.join(s.dir);
                let mut stack = vec![(dir, 0)];
                while let Some((d, depth)) = stack.pop() {
                    let Ok(rd) = std::fs::read_dir(&d) else { continue };
                    for e in rd.flatten() {
                        let p = e.path();
                        if p.is_dir() && depth < 2 {
                            stack.push((p, depth + 1));
                            continue;
                        }
                        let ext = p.extension().and_then(|x| x.to_str()).map(|x| x.to_lowercase()).unwrap_or_default();
                        if !s.exts.contains(&ext.as_str()) {
                            continue;
                        }
                        // jogos em várias trilhas: com .m3u/.cue presentes, os .bin/.img soltos não viram itens próprios
                        let h = Sha256::digest(p.to_string_lossy().as_bytes());
                        let id = format!("emu:{}-{}", s.dir, h.iter().take(6).map(|b| format!("{b:02x}")).collect::<String>());
                        out.push((id, si, p));
                    }
                }
            }
        }
        out
    }
}

impl Default for EmuProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for EmuProvider {
    fn id(&self) -> &'static str {
        "emu"
    }

    async fn library(&self) -> anyhow::Result<Vec<Item>> {
        let found = self.scan();
        let mut idx = self.index.lock().unwrap_or_else(|e| e.into_inner());
        idx.clear();
        let mut items = vec![];
        for (id, si, path) in found {
            let s = &SYSTEMS[si];
            let stem = path.file_stem().and_then(|x| x.to_str()).unwrap_or("Jogo").to_string();
            let mut it = Item::new(id.clone(), "emu", ItemKind::Game, title_of(&stem));
            it.subtitle = s.name.to_string();
            it.category = Some(s.name.to_string());
            it.state = ItemState::Installed;
            it.art = ArtFlags { cover: true, hero: false, icon: false };
            it.tags = vec!["lib".into(), "emu".into(), format!("sys:{}", s.dir)];
            it.hue = s.hue;
            it.size_bytes = std::fs::metadata(&path).ok().map(|m| m.len());
            items.push(it);
            idx.insert(id, (si, path));
        }
        Ok(items)
    }

    async fn launch(&self, id: &str) -> anyhow::Result<LaunchPlan> {
        let (si, rom) = self.index.lock().unwrap_or_else(|e| e.into_inner()).get(id).cloned().ok_or(ProviderError::NotFound)?;
        let s = &SYSTEMS[si];
        let (cmd, cores) = retroarch().ok_or_else(|| ProviderError::Msg("RetroArch não está instalado (sudo nexus-installer install --emulation --yes)".into()))?;
        let core = s.core;
        let cores2 = cores.clone();
        let lib = tokio::task::spawn_blocking(move || fetch_core(&cores2, core)).await?.map_err(ProviderError::Msg)?;
        let mut argv = cmd;
        argv.extend(["-f".into(), "-L".into(), lib.display().to_string()]);
        let players = crate::controllers::players_file();
        if players.is_file() {
            argv.push(format!("--appendconfig={}", players.display()));
        }
        argv.push(rom.display().to_string());
        Ok(LaunchPlan { argv, env: vec![], cwd: None, tracker: Tracker::Process })
    }

    fn art(&self, id: &str, kind: ArtKind) -> ArtSource {
        if kind != ArtKind::Cover {
            return ArtSource::Missing;
        }
        let Some((si, path)) = self.index.lock().unwrap_or_else(|e| e.into_inner()).get(id).cloned() else { return ArtSource::Missing };
        // capa local ao lado da ROM (mesmo nome .png/.jpg) tem prioridade
        for ext in ["png", "jpg"] {
            let p = path.with_extension(ext);
            if p.is_file() {
                return ArtSource::File(p);
            }
        }
        let stem = path.file_stem().and_then(|x| x.to_str()).unwrap_or("");
        let base = format!("https://thumbnails.libretro.com/{}/Named_Boxarts/", enc(SYSTEMS[si].thumbs));
        ArtSource::Urls(vec![format!("{base}{}.png", enc(&thumb_name(stem))), format!("{base}{}.png", enc(&thumb_name(&title_of(stem))))])
    }
}

/// Situação de cada console para a tela de Ajustes: quantos jogos e se o núcleo já foi baixado.
pub fn systems_status() -> serde_json::Value {
    let root = roms_dir();
    let cores = retroarch().map(|r| r.1);
    let list: Vec<serde_json::Value> = SYSTEMS
        .iter()
        .map(|s| {
            let n = std::fs::read_dir(root.join(s.dir)).map(|r| r.flatten().filter(|e| e.path().extension().and_then(|x| x.to_str()).is_some_and(|x| s.exts.contains(&x.to_lowercase().as_str()))).count()).unwrap_or(0);
            serde_json::json!({ "dir": s.dir, "name": s.name, "exts": s.exts, "core": s.core, "games": n, "coreReady": cores.as_ref().is_some_and(|c| core_path(c, s.core).is_file()) })
        })
        .collect();
    serde_json::json!({ "retroarch": cores.is_some(), "romsDir": root, "systems": list })
}

/// Baixa os núcleos dos consoles que têm jogos (ou de todos, com `all`).
pub fn fetch_cores(all: bool) -> serde_json::Value {
    let Some((_, dir)) = retroarch() else { return serde_json::json!({ "ok": false, "error": "RetroArch não está instalado" }) };
    let st = systems_status();
    let mut done = vec![];
    let mut failed = vec![];
    let mut seen = std::collections::HashSet::new();
    for s in st["systems"].as_array().unwrap() {
        let core = s["core"].as_str().unwrap().to_string();
        if (!all && s["games"].as_u64() == Some(0)) || !seen.insert(core.clone()) {
            continue;
        }
        match fetch_core(&dir, &core) {
            Ok(_) => done.push(core),
            Err(e) => failed.push(serde_json::json!({ "core": core, "error": e })),
        }
    }
    serde_json::json!({ "ok": failed.is_empty(), "done": done, "failed": failed })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_thumbs_and_scan() {
        assert_eq!(title_of("Super Mario World (USA) [!]"), "Super Mario World");
        assert_eq!(title_of("sonic_the_hedgehog"), "sonic the hedgehog");
        assert_eq!(thumb_name("Tom & Jerry: The Movie (USA)"), "Tom _ Jerry_ The Movie (USA)");
        assert!(SYSTEMS.len() >= 40);
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("snes/rpg")).unwrap();
        std::fs::write(d.path().join("snes/Chrono Trigger (USA).sfc"), b"x").unwrap();
        std::fs::write(d.path().join("snes/rpg/Earthbound (USA).SMC"), b"x").unwrap();
        std::fs::write(d.path().join("snes/leia.txt"), b"x").unwrap();
        std::fs::create_dir_all(d.path().join("gba")).unwrap();
        std::fs::write(d.path().join("gba/Pokemon Emerald.gba"), b"x").unwrap();
        let p = EmuProvider { roots: vec![d.path().to_path_buf()], index: Mutex::new(HashMap::new()) };
        let lib = tokio::runtime::Runtime::new().unwrap().block_on(p.library()).unwrap();
        let mut names: Vec<_> = lib.iter().map(|i| (i.title.as_str(), i.category.clone().unwrap())).collect();
        names.sort();
        assert_eq!(names, vec![("Chrono Trigger", "Super Nintendo".to_string()), ("Earthbound", "Super Nintendo".to_string()), ("Pokemon Emerald", "Game Boy Advance".to_string())]);
        let id = lib.iter().find(|i| i.title == "Chrono Trigger").unwrap().id.clone();
        match p.art(&id, ArtKind::Cover) {
            ArtSource::Urls(u) => assert!(u[0].contains("Nintendo%20-%20Super%20Nintendo%20Entertainment%20System/Named_Boxarts/Chrono%20Trigger%20(USA).png"), "{u:?}"),
            o => panic!("{o:?}"),
        }
    }
}
