//! Fotos de perfil. O valor do avatar é um nome pronto ("lia"…), "initial" ou "photo:<arquivo>".
//! A Shell só manda o caminho escolhido no navegador de imagens; o Core valida (pasta permitida, tamanho,
//! tipo pelo conteúdo), copia para `<dados>/avatars` e serve em `/art/photo:<arquivo>/photo`.
use crate::core::Core;
use std::path::PathBuf;
use std::time::Duration;

pub const PRESETS: &[&str] = &["lia", "rafa", "teo", "bia", "nina", "pipo"];
const PREFIX: &str = "photo:";

fn dir(core: &Core) -> PathBuf {
    core.cfg.data_dir.join("avatars")
}

fn safe_file(f: &str) -> bool {
    !f.is_empty() && f.len() <= 96 && !f.starts_with('.') && f.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn file_of(avatar: &str) -> Option<&str> {
    avatar.strip_prefix(PREFIX).filter(|f| safe_file(f))
}

/// Caminho da foto de um avatar "photo:…" (só dentro da pasta de avatares).
pub fn path(core: &Core, avatar: &str) -> Option<PathBuf> {
    let p = dir(core).join(file_of(avatar)?);
    p.is_file().then_some(p)
}

pub fn valid(core: &Core, avatar: &str) -> bool {
    avatar == "initial" || PRESETS.contains(&avatar) || path(core, avatar).is_some()
}

fn in_use(core: &Core, avatar: &str) -> bool {
    core.accounts.list().map(|l| l.iter().any(|p| p.avatar == avatar)).unwrap_or(true)
}

/// Apaga a foto se nenhum perfil a usa mais.
pub fn forget(core: &Core, avatar: &str) {
    if let Some(p) = path(core, avatar) {
        if !in_use(core, avatar) {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// Fotos escolhidas e abandonadas (editor cancelado) somem depois de uma hora.
fn sweep(core: &Core) {
    let Ok(rd) = std::fs::read_dir(dir(core)) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|d| d > Duration::from_secs(3600));
        if old && !in_use(core, &format!("{PREFIX}{name}")) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// Importa a imagem escolhida e devolve o valor de avatar ("photo:<arquivo>").
pub fn import(core: &Core, src: &str) -> Result<String, String> {
    sweep(core);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    let out = crate::customart::import(core.native.roots(), src, &dir(core), &format!("p{nanos:08x}"), "avatar")?;
    let name = out.file_name().and_then(|n| n.to_str()).ok_or("nome inválido")?;
    Ok(format!("{PREFIX}{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_restricted() {
        assert_eq!(file_of("photo:p1-avatar-2.png"), Some("p1-avatar-2.png"));
        assert_eq!(file_of("photo:../nexus.db"), None);
        assert_eq!(file_of("photo:a/b.png"), None);
        assert_eq!(file_of("photo:.oculto"), None);
        assert_eq!(file_of("lia"), None);
    }
}
