//! Armazenamento (B9): uso do disco por categoria, discos USB (listar, montar, ejetar com segurança, usar como pasta de
//! ROMs, formatar) e limpeza do cache. Tudo por `lsblk`, `udisksctl` e o udisks2 pelo `busctl` (argv fixo, dispositivo
//! validado contra a lista de removíveis; o disco do sistema nunca entra). Programas trocáveis para os testes:
//! `NEXUS_LSBLK`, `NEXUS_UDISKSCTL`, `NEXUS_BUSCTL`.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

/// Pastas de ROMs extras (discos USB escolhidos pelo usuário), lidas pelo provedor de emuladores.
static EXTRA: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

pub fn extra_roots() -> Vec<PathBuf> {
    EXTRA.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn load_extra(db: &crate::db::Db) {
    let list: Vec<PathBuf> = db.kv_get("roms_extra").and_then(|v| serde_json::from_str::<Vec<String>>(&v).ok()).unwrap_or_default().into_iter().map(PathBuf::from).collect();
    *EXTRA.lock().unwrap_or_else(|e| e.into_inner()) = list;
}

fn save_extra(db: &crate::db::Db, list: Vec<PathBuf>) -> Result<(), String> {
    let strs: Vec<String> = list.iter().map(|p| p.to_string_lossy().to_string()).collect();
    db.kv_set("roms_extra", &serde_json::to_string(&strs).unwrap_or_default()).map_err(|e| e.to_string())?;
    *EXTRA.lock().unwrap_or_else(|e| e.into_inner()) = list;
    Ok(())
}

fn bin(env: &str, def: &str) -> String {
    std::env::var(env).unwrap_or_else(|_| def.into())
}

fn run(prog: String, args: &[&str], secs: u64) -> Result<String, String> {
    let o = Command::new("timeout").arg(secs.to_string()).arg(prog).args(args).env("LC_ALL", "C").stdin(Stdio::null()).output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);
    if o.status.success() {
        Ok(text)
    } else {
        Err(friendly(&text))
    }
}

fn friendly(t: &str) -> String {
    let l = t.to_lowercase();
    if l.contains("busy") {
        "Algum programa ainda está usando o disco. Feche o jogo ou o app e tente de novo".into()
    } else if l.contains("not authorized") || l.contains("authentication") {
        "O sistema não deixou fazer isso sem senha".into()
    } else if l.contains("not mounted") {
        "O disco já estava desmontado".into()
    } else if l.contains("mkfs") || l.contains("no such file") && l.contains("format") {
        "O sistema não tem o programa para criar esse tipo de sistema de arquivos".into()
    } else {
        "Não foi possível concluir. Tente de novo".into()
    }
}

/// Tamanho de uma pasta em bytes (`du`, no máximo 40 s). 0 se não existir.
pub fn dir_size(p: &Path, exclude: Option<&str>) -> u64 {
    if !p.exists() {
        return 0;
    }
    let mut c = Command::new("timeout");
    c.args(["40", "du", "-sb"]);
    if let Some(e) = exclude {
        c.arg(format!("--exclude={e}"));
    }
    c.arg(p).stdin(Stdio::null()).stderr(Stdio::null());
    c.output().ok().and_then(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().next().and_then(|n| n.parse().ok())).unwrap_or(0)
}

/// Uso por categoria: jogos da Steam, ROMs, capas e cache do Nexus, e o resto (sistema e outros).
pub fn usage(data_dir: &Path, cache_dir: &Path, steam_root: Option<PathBuf>, roms: &Path) -> Value {
    let (free, total) = crate::hardware::disk(if data_dir.exists() { data_dir } else { Path::new("/") });
    let steam = steam_root.map(|r| dir_size(&r.join("steamapps"), None)).unwrap_or(0);
    let roms_b = dir_size(roms, None);
    let cache = dir_size(cache_dir, Some("drive"));
    let used = total.saturating_sub(free);
    let other = used.saturating_sub(steam + roms_b + cache);
    json!({
        "total": total, "free": free, "used": used,
        "cats": [
            { "id": "steam", "label": "Jogos da Steam", "bytes": steam },
            { "id": "roms", "label": "ROMs (emuladores)", "bytes": roms_b },
            { "id": "cache", "label": "Capas e cache do Nexus", "bytes": cache },
            { "id": "other", "label": "Sistema e outros arquivos", "bytes": other },
        ],
    })
}

/// Apaga capas baixadas, ícones e registros antigos. Nunca mexe no cache do Google Drive. Devolve os bytes liberados.
pub fn clean_cache(cache_dir: &Path) -> u64 {
    let before = dir_size(cache_dir, Some("drive"));
    for d in ["art", "web-icons", "logs"] {
        let p = cache_dir.join(d);
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                for e in rd.flatten() {
                    let _ = if e.path().is_dir() { std::fs::remove_dir_all(e.path()) } else { std::fs::remove_file(e.path()) };
                }
            }
        }
    }
    before.saturating_sub(dir_size(cache_dir, Some("drive")))
}

/// Interpreta a saída de `lsblk -J -b -o PATH,PKNAME,LABEL,MOUNTPOINT,RM,HOTPLUG,TRAN,SIZE,FSTYPE,TYPE`: só os
/// volumes removíveis (pendrive, HD USB, cartão) com sistema de arquivos.
pub fn parse_devices(json_text: &str) -> Vec<Value> {
    let Ok(v) = serde_json::from_str::<Value>(json_text) else { return vec![] };
    fn walk(n: &Value, parent: &str, removable: bool, out: &mut Vec<Value>) {
        let rm = removable || n["rm"].as_bool() == Some(true) || n["hotplug"].as_bool() == Some(true) || n["tran"].as_str() == Some("usb");
        let fs = n["fstype"].as_str().unwrap_or("");
        let ty = n["type"].as_str().unwrap_or("");
        let path = n["path"].as_str().unwrap_or("");
        if rm && !fs.is_empty() && fs != "swap" && !fs.contains("LVM") && !fs.contains("crypto") && (ty == "part" || ty == "disk") && path.starts_with("/dev/") {
            let disk = if ty == "disk" { path.to_string() } else { n["pkname"].as_str().map(|p| format!("/dev/{p}")).unwrap_or_else(|| parent.to_string()) };
            out.push(json!({
                "dev": path, "disk": disk,
                "label": n["label"].as_str().filter(|l| !l.is_empty()).unwrap_or("Disco USB"),
                "mount": n["mountpoint"].as_str().filter(|m| !m.is_empty()), "size": n["size"].as_u64().or_else(|| n["size"].as_str().and_then(|s| s.parse().ok())).unwrap_or(0), "fs": fs,
            }));
        }
        for c in n["children"].as_array().into_iter().flatten() {
            walk(c, path, rm, out);
        }
    }
    let mut out = vec![];
    for d in v["blockdevices"].as_array().into_iter().flatten() {
        walk(d, "", false, &mut out);
    }
    out
}

fn free_of(mount: &str) -> Option<(u64, u64)> {
    let (f, t) = crate::hardware::disk(Path::new(mount));
    (t > 0).then_some((f, t))
}

pub fn devices() -> Vec<Value> {
    let text = run(bin("NEXUS_LSBLK", "lsblk"), &["-J", "-b", "-o", "PATH,PKNAME,LABEL,MOUNTPOINT,RM,HOTPLUG,TRAN,SIZE,FSTYPE,TYPE"], 8).unwrap_or_default();
    let extra = extra_roots();
    parse_devices(&text)
        .into_iter()
        .map(|mut d| {
            if let Some(m) = d["mount"].as_str().map(String::from) {
                if let Some((f, t)) = free_of(&m) {
                    d["free"] = json!(f);
                    d["total"] = json!(t);
                }
                d["isRoms"] = json!(extra.iter().any(|e| e.starts_with(&m)));
            } else {
                d["isRoms"] = json!(false);
            }
            d
        })
        .collect()
}

fn find(dev: &str) -> Result<Value, String> {
    if !dev.starts_with("/dev/") || dev.contains("..") || !dev[5..].chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err("Dispositivo inválido".into());
    }
    devices().into_iter().find(|d| d["dev"].as_str() == Some(dev)).ok_or_else(|| "Este não é um disco removível".into())
}

pub fn mount(dev: &str) -> Result<String, String> {
    let d = find(dev)?;
    if let Some(m) = d["mount"].as_str() {
        return Ok(m.to_string());
    }
    let text = run(bin("NEXUS_UDISKSCTL", "udisksctl"), &["mount", "--no-user-interaction", "-b", dev], 30)?;
    // "Mounted /dev/sdb1 at /media/user/PENDRIVE"
    text.find(" at ").map(|i| text[i + 4..].trim().trim_matches(|c| c == '`' || c == '\'' || c == '.' || c == '"').to_string()).filter(|p| !p.is_empty()).ok_or_else(|| "Não foi possível abrir o disco".into())
}

fn unmount(dev: &str) -> Result<(), String> {
    match run(bin("NEXUS_UDISKSCTL", "udisksctl"), &["unmount", "--no-user-interaction", "-b", dev], 30) {
        Ok(_) => Ok(()),
        Err(e) if e.contains("já estava desmontado") => Ok(()),
        Err(e) => Err(e),
    }
}

/// Ejetar com segurança: desmonta todos os volumes do disco e desliga a energia dele (pode tirar do USB).
pub fn eject(dev: &str) -> Result<(), String> {
    let d = find(dev)?;
    let disk = d["disk"].as_str().unwrap_or(dev).to_string();
    for v in devices().iter().filter(|v| v["disk"].as_str() == Some(&disk) && v["mount"].is_string()) {
        unmount(v["dev"].as_str().unwrap_or(""))?;
    }
    run(bin("NEXUS_UDISKSCTL", "udisksctl"), &["power-off", "--no-user-interaction", "-b", &disk], 30)?;
    Ok(())
}

/// Formata o volume (exfat ou ext4) com o udisks2. O volume precisa ser removível; sai da lista de pastas de ROMs.
pub fn format(dev: &str, fs: &str, label: &str) -> Result<(), String> {
    if !["exfat", "ext4"].contains(&fs) {
        return Err("Sistema de arquivos não permitido".into());
    }
    if label.is_empty() || label.len() > 11 || !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err("O nome do disco usa só letras, números, _ e - (até 11 caracteres)".into());
    }
    let d = find(dev)?;
    unmount(dev)?;
    let obj = format!("/org/freedesktop/UDisks2/block_devices/{}", &dev[5..]);
    run(bin("NEXUS_BUSCTL", "busctl"), &["call", "org.freedesktop.UDisks2", &obj, "org.freedesktop.UDisks2.Block", "Format", "sa{sv}", fs, "1", "label", "s", label], 600)?;
    let _ = d;
    Ok(())
}

/// A pasta de ROMs dentro do disco: `<disco>/ROMs`, com as pastas dos consoles já criadas.
pub fn roms_folder(mount: &str) -> PathBuf {
    Path::new(mount).join("ROMs")
}

pub fn set_roms(db: &crate::db::Db, mount: &str, on: bool) -> Result<PathBuf, String> {
    let root = roms_folder(mount);
    let mut list = extra_roots();
    list.retain(|p| p != &root);
    if on {
        std::fs::create_dir_all(&root).map_err(|_| "Não consegui criar a pasta ROMs no disco (ele está só para leitura?)".to_string())?;
        for s in crate::providers::emu::SYSTEMS {
            let _ = std::fs::create_dir_all(root.join(s.dir));
        }
        list.push(root.clone());
    }
    save_extra(db, list)?;
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LSBLK: &str = r#"{"blockdevices":[
      {"path":"/dev/sda","pkname":null,"label":null,"mountpoint":null,"rm":false,"hotplug":false,"tran":"sata","size":500000000000,"fstype":null,"type":"disk","children":[
        {"path":"/dev/sda1","pkname":"sda","label":"root","mountpoint":"/","rm":false,"hotplug":false,"tran":null,"size":400000000000,"fstype":"ext4","type":"part"}]},
      {"path":"/dev/sdb","pkname":null,"label":null,"mountpoint":null,"rm":true,"hotplug":true,"tran":"usb","size":32000000000,"fstype":null,"type":"disk","children":[
        {"path":"/dev/sdb1","pkname":"sdb","label":"JOGOS","mountpoint":"/media/lia/JOGOS","rm":true,"hotplug":true,"tran":null,"size":31000000000,"fstype":"exfat","type":"part"}]},
      {"path":"/dev/loop0","pkname":null,"label":null,"mountpoint":"/snap/x","rm":false,"hotplug":false,"tran":null,"size":1,"fstype":"squashfs","type":"loop"}]}"#;

    #[test]
    fn so_lista_removiveis() {
        let d = parse_devices(LSBLK);
        assert_eq!(d.len(), 1, "o disco do sistema e o loop não entram");
        assert_eq!(d[0]["dev"], "/dev/sdb1");
        assert_eq!(d[0]["disk"], "/dev/sdb");
        assert_eq!(d[0]["label"], "JOGOS");
        assert_eq!(d[0]["mount"], "/media/lia/JOGOS");
    }

    #[test]
    fn dispositivo_invalido_e_recusado() {
        assert!(find("/dev/../etc/passwd").is_err());
        assert!(find("sdb1").is_err());
        assert!(find("/dev/sd b").is_err());
    }

    #[test]
    fn formatacao_valida_entradas() {
        assert!(format("/dev/sdb1", "ntfs", "X").is_err());
        assert!(format("/dev/sdb1", "exfat", "").is_err());
        assert!(format("/dev/sdb1", "exfat", "nome com espaço").is_err());
        assert!(format("/dev/sdb1", "exfat", "UM_NOME_MUITO_GRANDE").is_err());
    }

    #[test]
    fn limpa_so_o_cache_do_nexus() {
        let d = tempfile::tempdir().unwrap();
        for (dir, f) in [("art", "a.png"), ("web-icons", "b.png"), ("logs", "c.log"), ("drive", "grande.bin")] {
            std::fs::create_dir_all(d.path().join(dir)).unwrap();
            std::fs::write(d.path().join(dir).join(f), vec![0u8; 4096]).unwrap();
        }
        let freed = clean_cache(d.path());
        assert!(freed >= 3 * 4096, "liberou {freed}");
        assert!(d.path().join("drive/grande.bin").exists(), "cache do Drive fica");
        assert!(!d.path().join("art/a.png").exists());
    }
}
