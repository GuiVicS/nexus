//! Apps e jogos Linux comuns, descobertos pelos arquivos .desktop (XDG) e pelos exports do Flatpak.
use super::*;
use nexus_protocol::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub struct NativeProvider {
    dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DesktopEntry {
    pub file_id: String,
    pub name: String,
    pub comment: String,
    pub exec: String,
    pub icon: Option<String>,
    pub categories: Vec<String>,
    pub no_display: bool,
    pub terminal: bool,
    pub path: Option<String>,
}

pub fn parse_desktop(text: &str, file_id: &str) -> Option<DesktopEntry> {
    let mut in_entry = false;
    let mut kv: HashMap<String, String> = HashMap::new();
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_entry = l == "[Desktop Entry]";
            continue;
        }
        if !in_entry || l.starts_with('#') || l.is_empty() {
            continue;
        }
        if let Some((k, v)) = l.split_once('=') {
            // chaves com idioma (Name[pt_BR]) têm prioridade só para Name e Comment
            let k = k.trim();
            if k.contains('[') {
                let (base, loc) = k.split_once('[').unwrap();
                let loc = loc.trim_end_matches(']');
                if (base == "Name" || base == "Comment") && (loc == "pt_BR" || loc == "pt") {
                    kv.insert(format!("{base}.pt"), v.trim().to_string());
                }
                continue;
            }
            kv.entry(k.to_string()).or_insert_with(|| v.trim().to_string());
        }
    }
    if kv.get("Type").map(String::as_str) != Some("Application") {
        return None;
    }
    let b = |k: &str| kv.get(k).is_some_and(|v| v.eq_ignore_ascii_case("true"));
    Some(DesktopEntry {
        file_id: file_id.to_string(),
        name: kv.get("Name.pt").or(kv.get("Name"))?.clone(),
        comment: kv.get("Comment.pt").or(kv.get("Comment")).cloned().unwrap_or_default(),
        exec: kv.get("Exec")?.clone(),
        icon: kv.get("Icon").cloned(),
        categories: kv.get("Categories").map(|c| c.split(';').filter(|s| !s.is_empty()).map(String::from).collect()).unwrap_or_default(),
        no_display: b("NoDisplay") || b("Hidden"),
        terminal: b("Terminal"),
        path: kv.get("Path").cloned(),
    })
}

/// Divide o campo Exec em argv (aspas e escapes da especificação) e remove os códigos %f %u etc.
pub fn parse_exec(exec: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut cur = String::new();
    let mut has = false;
    let mut q = false;
    let mut it = exec.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '"' => {
                q = !q;
                has = true;
            }
            '\\' if q => {
                if let Some(n) = it.next() {
                    cur.push(n);
                }
            }
            ' ' | '\t' if !q => {
                if has {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            c => {
                cur.push(c);
                has = true;
            }
        }
    }
    if has {
        out.push(cur);
    }
    out.into_iter()
        .filter_map(|a| {
            if a.len() == 2 && a.starts_with('%') && "fFuUdDnNickvm".contains(&a[1..]) {
                return None;
            }
            Some(a.replace("%%", "\u{0}").replace(|c| c == '%', "").replace('\u{0}', "%"))
        })
        .collect()
}

fn category_label(cats: &[String]) -> &'static str {
    let has = |c: &str| cats.iter().any(|x| x == c);
    if has("Game") {
        "Jogos"
    } else if has("Video") || has("Player") || has("TV") {
        "Vídeo"
    } else if has("Audio") || has("Music") {
        "Música"
    } else if has("AudioVideo") {
        "Vídeo"
    } else if has("WebBrowser") || has("Network") {
        "Internet"
    } else if has("Office") {
        "Escritório"
    } else if has("Graphics") {
        "Gráficos"
    } else if has("Development") {
        "Desenvolvimento"
    } else if has("Utility") {
        "Ferramentas"
    } else if has("System") {
        "Sistema"
    } else {
        "Apps"
    }
}

impl NativeProvider {
    pub fn new(dirs: Vec<PathBuf>) -> NativeProvider {
        NativeProvider { dirs }
    }

    pub fn scan(&self) -> Vec<DesktopEntry> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut out = vec![];
        for d in &self.dirs {
            let Ok(rd) = std::fs::read_dir(d) else { continue };
            let mut files: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "desktop")).collect();
            files.sort();
            for p in files {
                let id = p.file_name().unwrap().to_string_lossy().to_string();
                if !seen.insert(id.clone()) {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&p) else { continue };
                let Some(e) = parse_desktop(&text, &id) else { continue };
                if e.no_display || e.terminal || e.categories.iter().any(|c| c == "Settings" || c == "DesktopSettings") || id.starts_with("nexus") || e.name.starts_with("Nexus") {
                    continue;
                }
                out.push(e);
            }
        }
        out.sort_by_key(|e| crate::util::norm_text(&e.name));
        out
    }

    fn find(&self, file_id: &str) -> Option<DesktopEntry> {
        self.scan().into_iter().find(|e| e.file_id == file_id)
    }

    fn icon_path(&self, name: &str) -> Option<PathBuf> {
        let p = Path::new(name);
        if p.is_absolute() {
            return p.is_file().then(|| p.to_path_buf());
        }
        for d in &self.dirs {
            let Some(base) = d.parent() else { continue };
            for size in ["512x512", "256x256", "128x128", "96x96", "64x64", "48x48", "scalable"] {
                for ext in ["png", "svg"] {
                    let f = base.join("icons/hicolor").join(size).join("apps").join(format!("{name}.{ext}"));
                    if f.is_file() {
                        return Some(f);
                    }
                }
            }
            for ext in ["png", "svg", "xpm"] {
                let f = base.join("pixmaps").join(format!("{name}.{ext}"));
                if f.is_file() {
                    return Some(f);
                }
            }
        }
        None
    }
}

#[async_trait]
impl Provider for NativeProvider {
    fn id(&self) -> &'static str {
        "native"
    }

    async fn library(&self) -> anyhow::Result<Vec<Item>> {
        let entries = self.scan();
        Ok(entries
            .into_iter()
            .map(|e| {
                let game = e.categories.iter().any(|c| c == "Game");
                let mut it = Item::new(format!("native:{}", e.file_id), "native", if game { ItemKind::Game } else { ItemKind::App }, e.name.clone());
                it.subtitle = category_label(&e.categories).to_string();
                it.category = Some(category_label(&e.categories).to_string());
                it.description = e.comment.clone();
                it.art.icon = e.icon.as_deref().is_some_and(|i| self.icon_path(i).is_some());
                it.tags = vec!["lib".into()];
                it
            })
            .collect())
    }

    async fn launch(&self, id: &str) -> anyhow::Result<LaunchPlan> {
        let (_, file_id) = split_id(id).ok_or(ProviderError::NotFound)?;
        let e = self.find(file_id).ok_or(ProviderError::NotFound)?;
        let argv = parse_exec(&e.exec);
        if argv.is_empty() {
            return Err(ProviderError::Msg("Exec vazio".into()).into());
        }
        Ok(LaunchPlan { argv, env: vec![], cwd: e.path.map(PathBuf::from).filter(|p| p.is_dir()), tracker: Tracker::Process })
    }

    fn art(&self, id: &str, kind: ArtKind) -> ArtSource {
        if kind != ArtKind::Icon {
            return ArtSource::Missing;
        }
        let Some((_, file_id)) = split_id(id) else { return ArtSource::Missing };
        self.find(file_id)
            .and_then(|e| e.icon)
            .and_then(|i| self.icon_path(&i))
            .map(ArtSource::File)
            .unwrap_or(ArtSource::Missing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIREFOX: &str = "[Desktop Entry]\nType=Application\nName=Firefox\nName[pt_BR]=Firefox Navegador\nExec=/usr/bin/firefox %u\nIcon=firefox\nCategories=Network;WebBrowser;\n[Desktop Action new-window]\nName=Nova janela\nExec=firefox --new-window\n";

    #[test]
    fn parses_desktop_group_and_locale() {
        let e = parse_desktop(FIREFOX, "firefox.desktop").unwrap();
        assert_eq!(e.name, "Firefox Navegador");
        assert_eq!(e.exec, "/usr/bin/firefox %u");
        assert!(e.categories.contains(&"WebBrowser".to_string()));
        assert!(parse_desktop("[Desktop Entry]\nType=Link\nName=x\nExec=y", "x.desktop").is_none());
    }

    #[test]
    fn exec_quotes_and_field_codes() {
        assert_eq!(parse_exec("firefox %u"), vec!["firefox"]);
        assert_eq!(parse_exec(r#""/opt/My App/run" --flag "a b" %F"#), vec!["/opt/My App/run", "--flag", "a b"]);
        assert_eq!(parse_exec("flatpak run --branch=stable --arch=x86_64 org.x.App @@u %U @@"), vec!["flatpak", "run", "--branch=stable", "--arch=x86_64", "org.x.App", "@@u", "@@"]);
        assert_eq!(parse_exec("sh -c 'echo 100%%'"), vec!["sh", "-c", "'echo", "100%'"]);
        assert_eq!(parse_exec(r#"x --msg="a \"q\"""#), vec!["x".to_string(), "--msg=a \"q\"".to_string()]);
    }

    #[tokio::test]
    async fn scans_filters_and_launches() {
        let d = tempfile::tempdir().unwrap();
        let apps = d.path().join("share/applications");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::write(apps.join("firefox.desktop"), FIREFOX).unwrap();
        std::fs::write(apps.join("hidden.desktop"), "[Desktop Entry]\nType=Application\nName=H\nExec=h\nNoDisplay=true\n").unwrap();
        std::fs::write(apps.join("term.desktop"), "[Desktop Entry]\nType=Application\nName=T\nExec=t\nTerminal=true\n").unwrap();
        std::fs::write(apps.join("nexus-shell.desktop"), "[Desktop Entry]\nType=Application\nName=Nexus Shell\nExec=n\n").unwrap();
        std::fs::write(apps.join("game.desktop"), "[Desktop Entry]\nType=Application\nName=Jogo Legal\nExec=jogo --fs\nCategories=Game;ArcadeGame;\nIcon=jogo\n").unwrap();
        let icons = d.path().join("share/icons/hicolor/64x64/apps");
        std::fs::create_dir_all(&icons).unwrap();
        std::fs::write(icons.join("jogo.png"), b"png").unwrap();
        let p = NativeProvider::new(vec![apps.clone()]);
        let lib = p.library().await.unwrap();
        let names: Vec<_> = lib.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(names, vec!["Firefox Navegador", "Jogo Legal"]);
        let g = lib.iter().find(|i| i.title == "Jogo Legal").unwrap();
        assert_eq!(g.kind, ItemKind::Game);
        assert!(g.art.icon);
        assert_eq!(g.id, "native:game.desktop");
        let plan = p.launch("native:game.desktop").await.unwrap();
        assert_eq!(plan.argv, vec!["jogo", "--fs"]);
        assert!(matches!(p.art("native:game.desktop", ArtKind::Icon), ArtSource::File(_)));
        assert!(p.launch("native:nada.desktop").await.is_err());
    }
}
