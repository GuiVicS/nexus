//! Gerenciar jogos de console (ROMs) pela TV, sem terminal:
//! - pendrive/HD externo: lista, monta (udisksctl, sem sudo — o polkit permite na sessão ativa) e navega;
//! - copia as ROMs escolhidas para `~/ROMs/<console>/` (console pela extensão ou pelo nome da pasta);
//! - recebe ROMs enviadas do celular/computador pela página `/roms` do servidor do celular (mesmo token do QR).
use crate::providers::emu::{roms_dir, SYSTEMS};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Arquivos compactados valem para qualquer console (o RetroArch abre .zip/.7z), mas aí o console tem de ser escolhido.
const ANY_EXT: [&str; 2] = ["zip", "7z"];
/// Limite por arquivo enviado (DVD de GameCube/Wii cabe; evita encher o disco por engano).
pub const MAX_UPLOAD: u64 = 8 * 1024 * 1024 * 1024;

/// Consoles que aceitam esta extensão (vários = ambíguo, ex.: .iso, .bin, .chd).
pub fn systems_for_ext(ext: &str) -> Vec<&'static str> {
    let ext = ext.to_lowercase();
    SYSTEMS.iter().filter(|s| s.exts.contains(&ext.as_str())).map(|s| s.dir).collect()
}

/// Console sugerido pelo nome de uma pasta ("snes", "Super Nintendo", "PS1"…).
pub fn system_from_folder(name: &str) -> Option<&'static str> {
    let n = name.to_lowercase().replace(['-', '_', ' '], "");
    const ALIASES: [(&str, &str); 14] = [
        ("ps1", "psx"), ("playstation", "psx"), ("genesis", "megadrive"), ("supernintendo", "snes"), ("nintendo64", "n64"),
        ("gameboyadvance", "gba"), ("gameboycolor", "gbc"), ("gameboy", "gb"), ("nintendods", "nds"), ("ds", "nds"),
        ("famicom", "nes"), ("nintendo", "nes"), ("turbografx", "pcengine"), ("mastersystem", "mastersystem"),
    ];
    if let Some(s) = SYSTEMS.iter().find(|s| s.dir == n || s.name.to_lowercase().replace(['-', '_', ' ', '(', ')'], "") == n) {
        return Some(s.dir);
    }
    ALIASES.iter().find(|(a, _)| *a == n).map(|(_, d)| *d)
}

pub fn is_rom_name(name: &str) -> bool {
    let ext = Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    !ext.is_empty() && (ANY_EXT.contains(&ext.as_str()) || !systems_for_ext(&ext).is_empty())
}

/// Nome de arquivo seguro para gravar: só o nome (sem pastas), sem começar com ponto, sem caracteres de controle.
pub fn safe_name(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next()?.trim();
    let clean: String = base.chars().filter(|c| !c.is_control()).collect();
    if clean.is_empty() || clean.starts_with('.') || clean.len() > 200 || !is_rom_name(&clean) {
        return None;
    }
    Some(clean)
}

// ---------- pendrive / HD externo ----------

fn user_media_roots() -> Vec<PathBuf> {
    let user = std::env::var("USER").unwrap_or_default();
    let mut v = vec![PathBuf::from(format!("/media/{user}")), PathBuf::from(format!("/run/media/{user}")), PathBuf::from("/media")];
    // pasta extra tratada como "pendrive" (testes e simulação)
    if let Some(extra) = std::env::var_os("NEXUS_MEDIA_ROOT") {
        v.push(PathBuf::from(extra));
    }
    v
}

/// Dispositivos removíveis (pendrive, HD USB, cartão SD) com sistema de arquivos: montados ou não.
pub fn media() -> Value {
    let out = Command::new("lsblk").args(["-J", "-b", "-o", "PATH,LABEL,MOUNTPOINT,RM,HOTPLUG,TRAN,SIZE,FSTYPE,TYPE"]).output();
    let mut list = vec![];
    if let Ok(o) = out {
        if let Ok(v) = serde_json::from_slice::<Value>(&o.stdout) {
            fn walk(n: &Value, parent_removable: bool, list: &mut Vec<Value>) {
                let removable = parent_removable || n["rm"].as_bool() == Some(true) || n["hotplug"].as_bool() == Some(true) || n["tran"].as_str() == Some("usb");
                let fs = n["fstype"].as_str().unwrap_or("");
                if removable && !fs.is_empty() && fs != "swap" && n["type"].as_str() != Some("loop") || (n["type"].as_str() == Some("loop") && n["mountpoint"].as_str().is_some_and(|m| m.starts_with("/media/") || m.starts_with("/run/media/"))) {
                    list.push(json!({
                        "dev": n["path"], "label": n["label"].as_str().filter(|l| !l.is_empty()).unwrap_or("Pendrive"),
                        "mount": n["mountpoint"], "size": n["size"], "fs": fs,
                    }));
                }
                if let Some(c) = n["children"].as_array() {
                    c.iter().for_each(|x| walk(x, removable, list));
                }
            }
            if let Some(a) = v["blockdevices"].as_array() {
                a.iter().for_each(|d| walk(d, false, &mut list));
            }
        }
    }
    json!({ "media": list, "home": std::env::var("HOME").unwrap_or_default(), "romsDir": roms_dir() })
}

/// Monta um dispositivo removível (sem senha na sessão ativa) e devolve onde ficou.
pub fn mount(dev: &str) -> Result<String, String> {
    if !dev.starts_with("/dev/") || dev.contains("..") {
        return Err("dispositivo inválido".into());
    }
    let ok = media()["media"].as_array().is_some_and(|a| a.iter().any(|m| m["dev"].as_str() == Some(dev)));
    if !ok {
        return Err("não é um dispositivo removível".into());
    }
    let o = Command::new("udisksctl").args(["mount", "--no-user-interaction", "-b", dev]).output().map_err(|e| format!("udisksctl: {e}"))?;
    let text = String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);
    // "Mounted /dev/sdb1 at /media/user/PENDRIVE" (ou já montado: "...already mounted at `/media/..'")
    if let Some(i) = text.find(" at ") {
        let p = text[i + 4..].trim().trim_matches(|c| c == '`' || c == '\'' || c == '.' || c == '"').to_string();
        if Path::new(&p).is_dir() {
            return Ok(p);
        }
    }
    media()["media"].as_array().and_then(|a| a.iter().find(|m| m["dev"].as_str() == Some(dev)).and_then(|m| m["mount"].as_str().map(String::from))).ok_or_else(|| format!("não foi possível abrir o pendrive: {}", text.trim()))
}

/// Pastas que a TV pode navegar: pendrives montados e a pasta pessoal (Downloads etc.).
fn allowed(p: &Path) -> bool {
    let Ok(c) = p.canonicalize() else { return false };
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    c.starts_with(&home) || user_media_roots().iter().any(|r| c.starts_with(r))
}

/// Conteúdo de uma pasta: subpastas e ROMs reconhecidas (com os consoles possíveis de cada uma).
pub fn browse(path: &str) -> Result<Value, String> {
    let p = PathBuf::from(path);
    if !allowed(&p) || !p.is_dir() {
        return Err("pasta indisponível".into());
    }
    let hint = p.file_name().and_then(|n| n.to_str()).and_then(system_from_folder);
    let mut dirs = vec![];
    let mut files = vec![];
    for e in std::fs::read_dir(&p).map_err(|e| e.to_string())?.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let path = e.path();
        if path.is_dir() {
            dirs.push(json!({ "name": name, "path": path, "system": system_from_folder(&name) }));
        } else if is_rom_name(&name) {
            let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
            let mut systems = systems_for_ext(&ext);
            if let Some(h) = hint {
                if ANY_EXT.contains(&ext.as_str()) || systems.contains(&h) {
                    systems = vec![h];
                }
            }
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            files.push(json!({ "name": name, "path": path, "size": size, "systems": systems }));
        }
    }
    dirs.sort_by_key(|d| d["name"].as_str().unwrap_or("").to_lowercase());
    files.sort_by_key(|d| d["name"].as_str().unwrap_or("").to_lowercase());
    let parent = p.parent().filter(|pp| allowed(pp)).map(|pp| pp.to_path_buf());
    Ok(json!({ "path": p, "parent": parent, "system": hint, "dirs": dirs, "files": files }))
}

/// Copia ROMs para `~/ROMs/<console>/`. `items`: [{path, system}]. Avisa o progresso por `progress(feitos, total, nome)`.
pub fn import(items: &[(PathBuf, String)], progress: &dyn Fn(usize, usize, &str)) -> Value {
    let root = roms_dir();
    let mut copied = vec![];
    let mut failed = vec![];
    for (i, (src, sys)) in items.iter().enumerate() {
        let name = src.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        progress(i, items.len(), &name);
        let res = (|| -> Result<(), String> {
            if !allowed(src) || !src.is_file() {
                return Err("arquivo indisponível".into());
            }
            let s = SYSTEMS.iter().find(|s| s.dir == sys).ok_or("console desconhecido")?;
            let name = safe_name(&name).ok_or("tipo de arquivo não reconhecido")?;
            let ext = Path::new(&name).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if !s.exts.contains(&ext.as_str()) && !ANY_EXT.contains(&ext.as_str()) {
                return Err(format!("{} não abre arquivos .{ext}", s.name));
            }
            let dir = root.join(s.dir);
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let dest = dir.join(&name);
            if dest.is_file() && dest.metadata().map(|m| m.len()).ok() == src.metadata().map(|m| m.len()).ok() {
                return Ok(()); // já estava lá
            }
            let tmp = dir.join(format!(".{name}.parcial"));
            std::fs::copy(src, &tmp).map_err(|e| e.to_string())?;
            std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())
        })();
        match res {
            Ok(()) => copied.push(name),
            Err(e) => failed.push(json!({ "name": name, "error": e })),
        }
    }
    progress(items.len(), items.len(), "");
    json!({ "ok": failed.is_empty(), "copied": copied, "failed": failed })
}

/// Destino de um envio pelo celular (valida console e nome). Grava primeiro como `.<nome>.parcial`.
pub fn upload_target(system: &str, name: &str) -> Result<(PathBuf, PathBuf), String> {
    let s = SYSTEMS.iter().find(|s| s.dir == system).ok_or("Escolha o console")?;
    let name = safe_name(name).ok_or("Tipo de arquivo não reconhecido")?;
    let ext = Path::new(&name).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if !s.exts.contains(&ext.as_str()) && !ANY_EXT.contains(&ext.as_str()) {
        return Err(format!("{} não abre arquivos .{ext}", s.name));
    }
    let dir = roms_dir().join(s.dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok((dir.join(format!(".{name}.parcial")), dir.join(&name)))
}

/// Lista de consoles para a página de envio: (pasta, nome, extensões).
pub fn systems_json() -> Value {
    json!(SYSTEMS.iter().map(|s| json!({ "dir": s.dir, "name": s.name, "exts": s.exts })).collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_systems() {
        assert_eq!(systems_for_ext("SFC"), vec!["snes"]);
        assert!(systems_for_ext("iso").len() > 1, ".iso é ambíguo");
        assert_eq!(system_from_folder("Super Nintendo"), Some("snes"));
        assert_eq!(system_from_folder("PS1"), Some("psx"));
        assert_eq!(system_from_folder("gba"), Some("gba"));
        assert_eq!(system_from_folder("Fotos"), None);
        assert_eq!(safe_name("../../etc/passwd"), None);
        assert_eq!(safe_name("Mario.sfc").as_deref(), Some("Mario.sfc"));
        assert_eq!(safe_name("C:\\jogos\\Sonic.md").as_deref(), Some("Sonic.md"));
        assert_eq!(safe_name(".escondido.gba"), None);
        assert_eq!(safe_name("foto.jpg"), None);
        assert!(upload_target("snes", "x.gba").is_err(), "extensão de outro console");
        assert!(upload_target("nada", "x.sfc").is_err());
    }

    #[test]
    fn import_copies_into_console_folder() {
        let home = tempfile::tempdir().unwrap();
        // SAFETY (teste): variáveis de ambiente só deste processo de teste
        unsafe {
            std::env::set_var("NEXUS_MEDIA_ROOT", home.path());
            std::env::set_var("NEXUS_ROMS_DIR", home.path().join("ROMs"));
        }
        let src = home.path().join("Downloads/Zelda.gb");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::write(&src, b"rom").unwrap();
        let r = import(&[(src.clone(), "gb".into()), (home.path().join("Downloads/nada.gb"), "gb".into())], &|_, _, _| {});
        assert_eq!(r["copied"][0], "Zelda.gb");
        assert_eq!(r["failed"].as_array().unwrap().len(), 1);
        assert_eq!(std::fs::read(home.path().join("ROMs/gb/Zelda.gb")).unwrap(), b"rom");
        let b = browse(src.parent().unwrap().to_str().unwrap()).unwrap();
        assert_eq!(b["files"][0]["systems"], json!(["gb"]));
        assert!(browse("/etc").is_err(), "fora da pasta pessoal e dos pendrives");
    }
}
