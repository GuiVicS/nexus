//! Autologin por gerenciador de login. Sempre faz backup e escreve um manifesto para desfazer.
//! `root` é a raiz do sistema (/ em produção, um diretório temporário nos testes).
use std::fs;
use std::path::{Path, PathBuf};

const SESSION_FILE: &str = "nexus.desktop";
const MANIFEST: &str = "var/lib/nexus/autologin.manifest";

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Dm {
    Sddm,
    Gdm,
    Greetd,
    /// Raspberry Pi OS, Debian com LXDE/Xfce
    Lightdm,
}

pub fn dm_from_name(n: &str) -> Option<Dm> {
    match n {
        "sddm" => Some(Dm::Sddm),
        "gdm" | "gdm3" => Some(Dm::Gdm),
        "greetd" => Some(Dm::Greetd),
        "lightdm" => Some(Dm::Lightdm),
        _ => None,
    }
}

fn path(root: &Path, rel: &str) -> PathBuf {
    root.join(rel.trim_start_matches('/'))
}

fn backup(root: &Path, rel: &str, manifest: &mut Vec<String>) -> std::io::Result<()> {
    let p = path(root, rel);
    if p.exists() {
        let b = path(root, &format!("{rel}.nexus-backup"));
        if !b.exists() {
            fs::copy(&p, &b)?;
        }
        manifest.push(format!("restore {rel}"));
    } else {
        manifest.push(format!("remove {rel}"));
    }
    Ok(())
}

/// Define a chave `key` na seção `[section]` de um arquivo INI simples, preservando o resto.
pub fn ini_set(text: &str, section: &str, key: &str, value: &str) -> String {
    let header = format!("[{section}]");
    let mut out: Vec<String> = vec![];
    let (mut in_sec, mut done, mut seen_sec) = (false, false, false);
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            if in_sec && !done {
                out.push(format!("{key}={value}"));
                done = true;
            }
            in_sec = t == header;
            seen_sec |= in_sec;
        } else if in_sec && t.split('=').next().map(str::trim) == Some(key) {
            out.push(format!("{key}={value}"));
            done = true;
            continue;
        }
        out.push(line.to_string());
    }
    if in_sec && !done {
        out.push(format!("{key}={value}"));
    } else if !seen_sec {
        if !out.last().is_none_or(|l| l.is_empty()) {
            out.push(String::new());
        }
        out.push(header);
        out.push(format!("{key}={value}"));
    }
    out.join("\n") + "\n"
}

pub fn configure(root: &Path, dm: Dm, user: &str) -> std::io::Result<()> {
    if user.is_empty() || !user.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.') {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "nome de usuário inválido"));
    }
    let mut m: Vec<String> = vec![];
    match dm {
        Dm::Sddm => {
            let rel = "etc/sddm.conf.d/10-nexus-autologin.conf";
            backup(root, rel, &mut m)?;
            fs::create_dir_all(path(root, rel).parent().unwrap())?;
            fs::write(path(root, rel), format!("[Autologin]\nUser={user}\nSession={SESSION_FILE}\n"))?;
        }
        Dm::Gdm => {
            let rel = "etc/gdm3/custom.conf";
            let alt = "etc/gdm/custom.conf";
            let rel = if path(root, alt).exists() && !path(root, rel).exists() { alt } else { rel };
            backup(root, rel, &mut m)?;
            let cur = fs::read_to_string(path(root, rel)).unwrap_or_default();
            let t = ini_set(&ini_set(&cur, "daemon", "AutomaticLoginEnable", "True"), "daemon", "AutomaticLogin", user);
            fs::create_dir_all(path(root, rel).parent().unwrap())?;
            fs::write(path(root, rel), t)?;
            // a sessão padrão do usuário vem do AccountsService
            let acc = format!("var/lib/AccountsService/users/{user}");
            backup(root, &acc, &mut m)?;
            let cur = fs::read_to_string(path(root, &acc)).unwrap_or_default();
            let t = ini_set(&ini_set(&cur, "User", "Session", "nexus"), "User", "XSession", "nexus");
            fs::create_dir_all(path(root, &acc).parent().unwrap())?;
            fs::write(path(root, &acc), t)?;
        }
        Dm::Lightdm => {
            // edita o próprio lightdm.conf: ele é lido por último e o raspi-config grava o autologin nele
            let rel = "etc/lightdm/lightdm.conf";
            backup(root, rel, &mut m)?;
            let cur = fs::read_to_string(path(root, rel)).unwrap_or_default();
            let t = ini_set(&ini_set(&ini_set(&cur, "Seat:*", "autologin-user", user), "Seat:*", "autologin-session", "nexus"), "Seat:*", "user-session", "nexus");
            fs::create_dir_all(path(root, rel).parent().unwrap())?;
            fs::write(path(root, rel), t)?;
        }
        Dm::Greetd => {
            let rel = "etc/greetd/config.toml";
            backup(root, rel, &mut m)?;
            let cur = fs::read_to_string(path(root, rel)).unwrap_or_default();
            let mut keep: Vec<&str> = vec![];
            let mut skip = false;
            for l in cur.lines() {
                let t = l.trim();
                if t.starts_with('[') {
                    skip = t == "[initial_session]";
                }
                if !skip {
                    keep.push(l);
                }
            }
            let mut text = keep.join("\n");
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&format!("\n[initial_session]\ncommand = \"nexus-session\"\nuser = \"{user}\"\n"));
            fs::create_dir_all(path(root, rel).parent().unwrap())?;
            fs::write(path(root, rel), text)?;
        }
    }
    let mp = path(root, MANIFEST);
    fs::create_dir_all(mp.parent().unwrap())?;
    fs::write(mp, m.join("\n") + "\n")?;
    Ok(())
}

/// Desfaz o que `configure` fez. Devolve quantos arquivos foram tratados.
pub fn revert(root: &Path) -> std::io::Result<usize> {
    let mp = path(root, MANIFEST);
    let text = fs::read_to_string(&mp).map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, "nenhum autologin do Nexus para desfazer"))?;
    let mut n = 0;
    for l in text.lines() {
        if let Some(rel) = l.strip_prefix("restore ") {
            let b = path(root, &format!("{rel}.nexus-backup"));
            if b.exists() {
                fs::rename(&b, path(root, rel))?;
                n += 1;
            }
        } else if let Some(rel) = l.strip_prefix("remove ") {
            if fs::remove_file(path(root, rel)).is_ok() {
                n += 1;
            }
        }
    }
    fs::remove_file(mp)?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ini_editing_preserves_content() {
        let t = "[daemon]\n# nota\nWaylandEnable=true\n\n[security]\nX=1\n";
        let r = ini_set(t, "daemon", "AutomaticLoginEnable", "True");
        assert!(r.contains("# nota\nWaylandEnable=true"), "preserva o conteúdo existente");
        assert!(r.find("AutomaticLoginEnable=True").unwrap() < r.find("[security]").unwrap(), "chave entra na seção certa");
        let again = ini_set(&r, "daemon", "AutomaticLoginEnable", "False");
        assert_eq!(again.matches("AutomaticLoginEnable").count(), 1);
        assert!(again.contains("AutomaticLoginEnable=False"));
        assert!(ini_set("", "User", "Session", "nexus").contains("[User]\nSession=nexus"));
    }

    #[test]
    fn sddm_roundtrip() {
        let d = tempfile::tempdir().unwrap();
        configure(d.path(), Dm::Sddm, "lia").unwrap();
        let f = d.path().join("etc/sddm.conf.d/10-nexus-autologin.conf");
        assert_eq!(fs::read_to_string(&f).unwrap(), "[Autologin]\nUser=lia\nSession=nexus.desktop\n");
        assert_eq!(revert(d.path()).unwrap(), 1);
        assert!(!f.exists());
        assert!(revert(d.path()).is_err(), "sem manifesto não há o que desfazer");
    }

    #[test]
    fn gdm_backs_up_and_restores() {
        let d = tempfile::tempdir().unwrap();
        let conf = d.path().join("etc/gdm3/custom.conf");
        fs::create_dir_all(conf.parent().unwrap()).unwrap();
        fs::write(&conf, "[daemon]\nWaylandEnable=true\n").unwrap();
        configure(d.path(), Dm::Gdm, "lia").unwrap();
        let now = fs::read_to_string(&conf).unwrap();
        assert!(now.contains("AutomaticLoginEnable=True") && now.contains("AutomaticLogin=lia") && now.contains("WaylandEnable=true"));
        let acc = fs::read_to_string(d.path().join("var/lib/AccountsService/users/lia")).unwrap();
        assert!(acc.contains("Session=nexus"));
        revert(d.path()).unwrap();
        assert_eq!(fs::read_to_string(&conf).unwrap(), "[daemon]\nWaylandEnable=true\n");
        assert!(!d.path().join("var/lib/AccountsService/users/lia").exists());
    }

    #[test]
    fn greetd_replaces_initial_session() {
        let d = tempfile::tempdir().unwrap();
        let conf = d.path().join("etc/greetd/config.toml");
        fs::create_dir_all(conf.parent().unwrap()).unwrap();
        fs::write(&conf, "[terminal]\nvt = 1\n\n[initial_session]\ncommand = \"outra\"\nuser = \"x\"\n").unwrap();
        configure(d.path(), Dm::Greetd, "lia").unwrap();
        let t = fs::read_to_string(&conf).unwrap();
        assert_eq!(t.matches("[initial_session]").count(), 1);
        assert!(t.contains("nexus-session") && t.contains("[terminal]") && !t.contains("outra"));
        revert(d.path()).unwrap();
        assert!(fs::read_to_string(&conf).unwrap().contains("outra"));
    }

    #[test]
    fn lightdm_raspberry_pi_os() {
        let d = tempfile::tempdir().unwrap();
        let conf = d.path().join("etc/lightdm/lightdm.conf");
        fs::create_dir_all(conf.parent().unwrap()).unwrap();
        let orig = "[LightDM]\n\n[Seat:*]\n#autologin-user=\nautologin-user=pi\ngreeter-session=pi-greeter-labwc\n";
        fs::write(&conf, orig).unwrap();
        configure(d.path(), Dm::Lightdm, "lia").unwrap();
        let t = fs::read_to_string(&conf).unwrap();
        assert!(t.contains("autologin-user=lia") && !t.contains("autologin-user=pi\n") && t.contains("autologin-session=nexus"));
        assert!(t.contains("greeter-session=pi-greeter-labwc"), "preserva o resto");
        revert(d.path()).unwrap();
        assert_eq!(fs::read_to_string(&conf).unwrap(), orig);
    }

    #[test]
    fn rejects_bad_usernames() {
        let d = tempfile::tempdir().unwrap();
        assert!(configure(d.path(), Dm::Sddm, "lia\nSession=evil").is_err());
        assert!(configure(d.path(), Dm::Sddm, "").is_err());
        assert_eq!(dm_from_name("gdm3"), Some(Dm::Gdm));
        assert_eq!(dm_from_name("lightdm"), Some(Dm::Lightdm));
        assert_eq!(dm_from_name("xdm"), None);
    }
}
