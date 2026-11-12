//! Imagens personalizadas (ícone, capa do card, fundo) e navegador de arquivos restrito.
//! O Shell nunca recebe um caminho para ler: o Core valida, copia para a pasta de dados e serve pelo protocolo de arte.
use nexus_protocol::{FsEntry, FsListing};
use std::fs;
use std::path::{Path, PathBuf};

pub const MAX_IMAGE: u64 = 12 * 1024 * 1024;
const EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "svg"];

/// Pastas que o navegador de imagens pode mostrar: pessoal e mídias removíveis.
pub fn default_roots() -> Vec<PathBuf> {
    let mut v = vec![];
    if let Some(h) = std::env::var_os("HOME") {
        v.push(PathBuf::from(h));
    }
    let user = std::env::var("USER").unwrap_or_default();
    for p in [format!("/media/{user}"), format!("/run/media/{user}"), "/media".into(), "/mnt".into()] {
        let p = PathBuf::from(p);
        if p.is_dir() && !v.contains(&p) {
            v.push(p);
        }
    }
    v
}

fn is_image_name(n: &str) -> bool {
    Path::new(n).extension().and_then(|e| e.to_str()).is_some_and(|e| EXTS.contains(&e.to_lowercase().as_str()))
}

pub fn inside(roots: &[PathBuf], p: &Path) -> bool {
    let Ok(c) = p.canonicalize() else { return false };
    roots.iter().filter_map(|r| r.canonicalize().ok()).any(|r| c.starts_with(&r))
}

pub fn list(roots: &[PathBuf], path: Option<&str>) -> Result<FsListing, String> {
    list_mode(roots, path, "image")
}

/// `mode`: "image" (pastas e imagens), "program" (pastas e programas: .jar, AppImage, .sh, executáveis) ou "folder" (só pastas).
pub fn list_mode(roots: &[PathBuf], path: Option<&str>, mode: &str) -> Result<FsListing, String> {
    let base = match path {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => roots.first().cloned().ok_or("sem pastas disponíveis")?,
    };
    if !inside(roots, &base) {
        return Err("pasta não permitida".into());
    }
    let base = base.canonicalize().map_err(|e| e.to_string())?;
    let mut entries: Vec<FsEntry> = fs::read_dir(&base)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                return None;
            }
            let ft = e.file_type().ok()?;
            let path = e.path();
            // links simbólicos só valem se continuarem dentro das pastas permitidas
            let dir = if ft.is_symlink() { path.is_dir() && inside(roots, &path) } else { ft.is_dir() };
            let wanted = match mode {
                "folder" => false,
                "program" => crate::programs::detect(&path).is_some(),
                _ => is_image_name(&name),
            };
            if dir || (wanted && (!ft.is_symlink() || inside(roots, &path))) {
                Some(FsEntry { name, path: path.to_string_lossy().to_string(), dir })
            } else {
                None
            }
        })
        .collect();
    entries.sort_by(|a, b| b.dir.cmp(&a.dir).then(crate::util::norm_text(&a.name).cmp(&crate::util::norm_text(&b.name))));
    entries.truncate(600);
    let parent = base.parent().filter(|p| inside(roots, p)).map(|p| p.to_string_lossy().to_string());
    let rootsv = roots.iter().filter(|r| r.is_dir()).map(|r| FsEntry { name: r.file_name().map(|n| n.to_string_lossy().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| r.to_string_lossy().to_string()), path: r.to_string_lossy().to_string(), dir: true }).collect();
    Ok(FsListing { path: base.to_string_lossy().to_string(), parent, entries, roots: rootsv })
}

fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("jpg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_lowercase();
        (head.contains("<svg") || (head.contains("<?xml") && String::from_utf8_lossy(bytes).to_lowercase().contains("<svg"))).then_some("svg")
    }
}

/// Valida (pasta permitida, tamanho, tipo pelo conteúdo) e copia para `dest_dir`. Devolve o caminho novo.
pub fn import(roots: &[PathBuf], src: &str, dest_dir: &Path, item_id: &str, kind: &str) -> Result<PathBuf, String> {
    let src = PathBuf::from(src);
    if !inside(roots, &src) {
        return Err("arquivo fora das pastas permitidas".into());
    }
    let meta = fs::metadata(&src).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("não é um arquivo".into());
    }
    if meta.len() > MAX_IMAGE {
        return Err("imagem maior que 12 MB".into());
    }
    let bytes = fs::read(&src).map_err(|e| e.to_string())?;
    let ext = sniff(&bytes).ok_or("formato não suportado (use JPG, PNG, WebP ou SVG)")?;
    fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    let safe: String = item_id.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let stamp = nexus_protocol::now_unix();
    let out = dest_dir.join(format!("{safe}-{kind}-{stamp}.{ext}"));
    fs::write(&out, &bytes).map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n0000";

    #[test]
    fn lists_only_dirs_and_images_inside_roots() {
        let d = tempfile::tempdir().unwrap();
        let home = d.path().join("home");
        fs::create_dir_all(home.join("Fotos")).unwrap();
        fs::write(home.join("capa.PNG"), PNG).unwrap();
        fs::write(home.join("nota.txt"), "x").unwrap();
        fs::write(home.join(".segredo.png"), PNG).unwrap();
        let roots = vec![home.clone()];
        let l = list(&roots, None).unwrap();
        let names: Vec<_> = l.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["Fotos", "capa.PNG"], "pastas primeiro; sem ocultos nem outros arquivos");
        assert!(l.parent.is_none(), "não sobe além da raiz permitida");
        assert!(list(&roots, Some("/etc")).is_err());
        assert!(list(&roots, Some(&format!("{}/../..", home.display()))).is_err(), "../ não escapa");
        let sub = list(&roots, Some(home.join("Fotos").to_str().unwrap())).unwrap();
        assert_eq!(sub.parent.as_deref(), Some(home.to_str().unwrap()));
    }

    #[test]
    fn symlink_escape_is_blocked() {
        let d = tempfile::tempdir().unwrap();
        let home = d.path().join("home");
        let out = d.path().join("fora");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&out).unwrap();
        fs::write(out.join("x.png"), PNG).unwrap();
        std::os::unix::fs::symlink(&out, home.join("atalho")).unwrap();
        let roots = vec![home.clone()];
        assert!(list(&roots, None).unwrap().entries.is_empty());
        assert!(import(&roots, home.join("atalho/x.png").to_str().unwrap(), &d.path().join("dest"), "a", "cover").is_err());
    }

    #[test]
    fn import_validates_content_size_and_location() {
        let d = tempfile::tempdir().unwrap();
        let home = d.path().join("home");
        fs::create_dir_all(&home).unwrap();
        let roots = vec![home.clone()];
        let dest = d.path().join("dest");
        fs::write(home.join("ok.png"), PNG).unwrap();
        let p = import(&roots, home.join("ok.png").to_str().unwrap(), &dest, "native:x.desktop", "cover").unwrap();
        assert!(p.starts_with(&dest) && p.extension().unwrap() == "png" && p.file_name().unwrap().to_string_lossy().starts_with("native_x_desktop-cover-"));
        fs::write(home.join("falso.png"), "isto não é imagem").unwrap();
        assert!(import(&roots, home.join("falso.png").to_str().unwrap(), &dest, "a", "icon").unwrap_err().contains("formato"));
        fs::write(home.join("a.svg"), "<svg xmlns='http://www.w3.org/2000/svg'/>").unwrap();
        assert!(import(&roots, home.join("a.svg").to_str().unwrap(), &dest, "a", "icon").is_ok());
        let big = fs::File::create(home.join("grande.png")).unwrap();
        big.set_len(MAX_IMAGE + 1).unwrap();
        assert!(import(&roots, home.join("grande.png").to_str().unwrap(), &dest, "a", "hero").unwrap_err().contains("12 MB"));
        assert!(import(&roots, "/etc/passwd", &dest, "a", "hero").is_err());
    }
}
