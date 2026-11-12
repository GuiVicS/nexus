//! Apps e jogos Linux comuns, descobertos pelos arquivos .desktop (XDG) e pelos exports do Flatpak.
use super::*;
use crate::db::Db;
use nexus_protocol::*;
use rusqlite::params;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::path::{Path, PathBuf};

pub struct NativeProvider {
    dirs: Vec<PathBuf>,
    db: Arc<Db>,
    art_dir: PathBuf,
    roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Reg {
    pub item_id: String,
    pub file_id: String,
    pub name: Option<String>,
    pub kind: String,
    pub icon: Option<String>,
    pub cover: Option<String>,
    pub hero: Option<String>,
}

/// Campos editáveis. `Some(None)` apaga o valor personalizado.
#[derive(Default, Debug, Clone)]
pub struct RegPatch {
    pub name: Option<Option<String>>,
    pub kind: Option<String>,
    pub icon: Option<Option<String>>,
    pub cover: Option<Option<String>>,
    pub hero: Option<Option<String>>,
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

/// Entradas que nunca entram na lista de apps: o próprio Nexus, a Steam (tem provedor próprio) e atalhos de jogos Steam.
fn is_excluded(e: &DesktopEntry) -> bool {
    let id = e.file_id.to_lowercase();
    id.starts_with("nexus")
        || e.name.starts_with("Nexus")
        || id == "steam.desktop"
        || id == "com.valvesoftware.steam.desktop"
        || id.starts_with("steam_app_")
        || e.exec.contains("steam://")
        || e.categories.iter().any(|c| c == "Settings" || c == "DesktopSettings")
}

impl NativeProvider {
    pub fn new(dirs: Vec<PathBuf>, db: Arc<Db>, art_dir: PathBuf, roots: Vec<PathBuf>) -> NativeProvider {
        NativeProvider { dirs, db, art_dir, roots }
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// Todos os apps gráficos instalados no sistema (para escolher quais entram no Nexus).
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
                if e.no_display || e.terminal || is_excluded(&e) {
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

    /// Atalho (.desktop) de um programa, já lido.
    pub fn entry(&self, file_id: &str) -> Option<DesktopEntry> {
        self.find(file_id)
    }

    /// Caminho do atalho (o primeiro encontrado, na mesma ordem de `scan`).
    pub fn desktop_path(&self, file_id: &str) -> Option<PathBuf> {
        self.dirs.iter().map(|d| d.join(file_id)).find(|p| p.is_file())
    }

    pub fn registered(&self) -> Vec<Reg> {
        self.db
            .with(|c| {
                let mut st = c.prepare("SELECT item_id,file_id,name,kind,icon,cover,hero FROM custom_apps ORDER BY added_at, item_id")?;
                let v = st
                    .query_map([], |r| Ok(Reg { item_id: r.get(0)?, file_id: r.get(1)?, name: r.get(2)?, kind: r.get(3)?, icon: r.get(4)?, cover: r.get(5)?, hero: r.get(6)? }))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(v)
            })
            .unwrap_or_default()
    }

    fn reg(&self, item_id: &str) -> Option<Reg> {
        self.registered().into_iter().find(|r| r.item_id == item_id)
    }

    pub fn custom_apps(&self) -> Vec<CustomApp> {
        let entries: HashMap<String, DesktopEntry> = self.scan().into_iter().map(|e| (e.file_id.clone(), e)).collect();
        self.registered()
            .into_iter()
            .map(|r| {
                let e = entries.get(&r.file_id);
                CustomApp {
                    default_name: e.map(|e| e.name.clone()).unwrap_or_else(|| r.file_id.trim_end_matches(".desktop").to_string()),
                    name: r.name.clone().or_else(|| e.map(|e| e.name.clone())).unwrap_or_else(|| r.file_id.clone()),
                    id: r.item_id,
                    kind: r.kind,
                    has_icon: r.icon.is_some(),
                    has_cover: r.cover.is_some(),
                    has_hero: r.hero.is_some(),
                    available: e.is_some(),
                }
            })
            .collect()
    }

    pub fn system_apps(&self) -> Vec<SystemApp> {
        let added: HashSet<String> = self.registered().into_iter().map(|r| r.file_id).collect();
        self.scan().into_iter().map(|e| SystemApp { added: added.contains(&e.file_id), category: category_label(&e.categories).to_string(), name: e.name, file_id: e.file_id }).collect()
    }

    pub fn add(&self, file_id: &str, kind: Option<&str>) -> anyhow::Result<String> {
        let e = self.find(file_id).ok_or(ProviderError::NotFound)?;
        let item_id = format!("native:{file_id}");
        let kind = kind.unwrap_or(if e.categories.iter().any(|c| c == "Game") { "game" } else { "app" });
        if kind != "game" && kind != "app" {
            anyhow::bail!("tipo inválido");
        }
        self.db.with(|c| c.execute("INSERT OR IGNORE INTO custom_apps(item_id,file_id,kind,added_at) VALUES(?1,?2,?3,?4)", params![item_id, file_id, kind, now_unix()]).map(|_| ()))?;
        Ok(item_id)
    }

    pub fn remove(&self, item_id: &str) -> anyhow::Result<()> {
        if let Some(r) = self.reg(item_id) {
            for p in [r.icon, r.cover, r.hero].into_iter().flatten() {
                let _ = std::fs::remove_file(p);
            }
        }
        self.db.with(|c| c.execute("DELETE FROM custom_apps WHERE item_id=?1", params![item_id]).map(|_| ()))
    }

    /// Imagem nova vem como caminho escolhido pelo usuário; é validada e copiada para a pasta de dados.
    pub fn update(&self, item_id: &str, p: RegPatch) -> anyhow::Result<()> {
        let cur = self.reg(item_id).ok_or(ProviderError::NotFound)?;
        if let Some(k) = &p.kind {
            if k != "game" && k != "app" {
                anyhow::bail!("tipo inválido");
            }
        }
        if let Some(Some(n)) = &p.name {
            if n.trim().is_empty() || n.chars().count() > 40 {
                anyhow::bail!("nome inválido (1 a 40 caracteres)");
            }
        }
        let img = |kind: &str, new: &Option<Option<String>>, old: &Option<String>| -> anyhow::Result<Option<Option<String>>> {
            Ok(match new {
                None => None,
                Some(None) => Some(None),
                Some(Some(src)) => Some(Some(crate::customart::import(&self.roots, src, &self.art_dir, item_id, kind).map_err(|e| anyhow::anyhow!(e))?.to_string_lossy().to_string())),
            })
            .inspect(|r| {
                if r.is_some() {
                    if let Some(o) = old {
                        let _ = std::fs::remove_file(o);
                    }
                }
            })
        };
        let icon = img("icon", &p.icon, &cur.icon)?;
        let cover = img("cover", &p.cover, &cur.cover)?;
        let hero = img("hero", &p.hero, &cur.hero)?;
        self.db.with(|c| {
            if let Some(n) = &p.name {
                c.execute("UPDATE custom_apps SET name=?2 WHERE item_id=?1", params![item_id, n.as_ref().map(|s| s.trim().to_string())])?;
            }
            if let Some(k) = &p.kind {
                c.execute("UPDATE custom_apps SET kind=?2 WHERE item_id=?1", params![item_id, k])?;
            }
            if let Some(v) = &icon {
                c.execute("UPDATE custom_apps SET icon=?2 WHERE item_id=?1", params![item_id, v])?;
            }
            if let Some(v) = &cover {
                c.execute("UPDATE custom_apps SET cover=?2 WHERE item_id=?1", params![item_id, v])?;
            }
            if let Some(v) = &hero {
                c.execute("UPDATE custom_apps SET hero=?2 WHERE item_id=?1", params![item_id, v])?;
            }
            Ok(())
        })
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

    /// Só entra na biblioteca o que o usuário adicionou.
    async fn library(&self) -> anyhow::Result<Vec<Item>> {
        let entries: HashMap<String, DesktopEntry> = self.scan().into_iter().map(|e| (e.file_id.clone(), e)).collect();
        Ok(self
            .registered()
            .into_iter()
            .filter_map(|r| {
                let e = entries.get(&r.file_id)?;
                let game = r.kind == "game";
                let title = r.name.clone().unwrap_or_else(|| e.name.clone());
                let mut it = Item::new(r.item_id.clone(), "native", if game { ItemKind::Game } else { ItemKind::App }, title);
                let cat = category_label(&e.categories).to_string();
                it.subtitle = if game { "Linux".into() } else { cat.clone() };
                it.category = Some(cat);
                it.description = e.comment.clone();
                it.art.icon = r.icon.is_some() || e.icon.as_deref().is_some_and(|i| self.icon_path(i).is_some());
                it.art.cover = r.cover.is_some();
                it.art.hero = r.hero.is_some();
                it.tags = vec!["lib".into()];
                it.caps.uninstall = false;
                Some(it)
            })
            .collect())
    }

    async fn launch(&self, id: &str) -> anyhow::Result<LaunchPlan> {
        let r = self.reg(id).ok_or(ProviderError::NotFound)?;
        let e = self.find(&r.file_id).ok_or(ProviderError::NotFound)?;
        let argv = parse_exec(&e.exec);
        if argv.is_empty() {
            return Err(ProviderError::Msg("Exec vazio".into()).into());
        }
        Ok(LaunchPlan { argv, env: vec![], cwd: e.path.map(PathBuf::from).filter(|p| p.is_dir()), tracker: Tracker::Process })
    }

    fn art(&self, id: &str, kind: ArtKind) -> ArtSource {
        let Some(r) = self.reg(id) else { return ArtSource::Missing };
        let custom = match kind {
            ArtKind::Cover => r.cover,
            ArtKind::Hero => r.hero,
            ArtKind::Icon => r.icon,
        };
        if let Some(c) = custom {
            return ArtSource::File(PathBuf::from(c));
        }
        if kind != ArtKind::Icon {
            return ArtSource::Missing;
        }
        self.find(&r.file_id).and_then(|e| e.icon).and_then(|i| self.icon_path(&i)).map(ArtSource::File).unwrap_or(ArtSource::Missing)
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

    fn provider(dir: &Path, apps: PathBuf) -> NativeProvider {
        NativeProvider::new(vec![apps], Arc::new(Db::memory().unwrap()), dir.join("art"), vec![dir.join("home")])
    }

    #[tokio::test]
    async fn only_registered_apps_enter_the_library() {
        let d = tempfile::tempdir().unwrap();
        let apps = d.path().join("share/applications");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::write(apps.join("firefox.desktop"), FIREFOX).unwrap();
        std::fs::write(apps.join("calc.desktop"), "[Desktop Entry]\nType=Application\nName=Calculadora\nExec=calc\nCategories=Utility;Calculator;\n").unwrap();
        std::fs::write(apps.join("hidden.desktop"), "[Desktop Entry]\nType=Application\nName=H\nExec=h\nNoDisplay=true\n").unwrap();
        std::fs::write(apps.join("term.desktop"), "[Desktop Entry]\nType=Application\nName=T\nExec=t\nTerminal=true\n").unwrap();
        std::fs::write(apps.join("nexus-shell.desktop"), "[Desktop Entry]\nType=Application\nName=Nexus Shell\nExec=n\n").unwrap();
        std::fs::write(apps.join("steam.desktop"), "[Desktop Entry]\nType=Application\nName=Steam\nExec=steam %U\nCategories=Network;Game;\n").unwrap();
        std::fs::write(apps.join("Portal 2.desktop"), "[Desktop Entry]\nType=Application\nName=Portal 2\nExec=steam steam://rungameid/620\nCategories=Game;\n").unwrap();
        std::fs::write(apps.join("minecraft.desktop"), "[Desktop Entry]\nType=Application\nName=Minecraft\nExec=minecraft-launcher\nCategories=Game;\nIcon=mc\n").unwrap();
        let p = provider(d.path(), apps);
        assert!(p.library().await.unwrap().is_empty(), "nada entra sozinho (nem calculadora, nem Steam)");
        let names: Vec<_> = p.system_apps().into_iter().map(|a| a.name).collect();
        assert_eq!(names, vec!["Calculadora", "Firefox Navegador", "Minecraft"], "escondidos, terminal, Nexus, Steam e atalhos Steam fora do seletor");
        let id = p.add("minecraft.desktop", None).unwrap();
        assert_eq!(id, "native:minecraft.desktop");
        let lib = p.library().await.unwrap();
        assert_eq!(lib.len(), 1);
        assert_eq!((lib[0].kind, lib[0].title.as_str(), lib[0].subtitle.as_str()), (ItemKind::Game, "Minecraft", "Linux"), "jogo detectado pela categoria");
        assert!(p.system_apps().iter().find(|a| a.name == "Minecraft").unwrap().added);
        assert!(p.add("steam.desktop", None).is_err(), "a Steam não pode ser adicionada como app");
        p.add("calc.desktop", Some("app")).unwrap();
        assert_eq!(p.launch("native:calc.desktop").await.unwrap().argv, vec!["calc"]);
        assert!(p.launch("native:firefox.desktop").await.is_err(), "não registrado não abre");
        p.remove("native:calc.desktop").unwrap();
        assert!(p.launch("native:calc.desktop").await.is_err());
    }

    #[tokio::test]
    async fn custom_name_kind_and_images() {
        let d = tempfile::tempdir().unwrap();
        let apps = d.path().join("share/applications");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::write(apps.join("calc.desktop"), "[Desktop Entry]\nType=Application\nName=Calculadora\nExec=calc\nCategories=Utility;\n").unwrap();
        let home = d.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join("card.png"), b"\x89PNG\r\n\x1a\n1111").unwrap();
        std::fs::write(home.join("fundo.jpg"), [0xff, 0xd8, 0xff, 0xe0, 1]).unwrap();
        let p = provider(d.path(), apps);
        p.add("calc.desktop", None).unwrap();
        assert_eq!(p.library().await.unwrap()[0].kind, ItemKind::App);
        p.update("native:calc.desktop", RegPatch { name: Some(Some("Minha Conta".into())), kind: Some("game".into()), cover: Some(Some(home.join("card.png").to_string_lossy().into())), hero: Some(Some(home.join("fundo.jpg").to_string_lossy().into())), ..Default::default() }).unwrap();
        let it = p.library().await.unwrap().remove(0);
        assert_eq!((it.title.as_str(), it.kind, it.art.cover, it.art.hero), ("Minha Conta", ItemKind::Game, true, true));
        let ArtSource::File(c) = p.art("native:calc.desktop", ArtKind::Cover) else { panic!("capa personalizada") };
        assert!(c.starts_with(d.path().join("art")) && c.is_file());
        assert!(matches!(p.art("native:calc.desktop", ArtKind::Hero), ArtSource::File(_)));
        let info = &p.custom_apps()[0];
        assert!((info.has_cover, info.has_hero, info.has_icon, info.available) == (true, true, false, true) && info.default_name == "Calculadora");
        assert!(p.update("native:calc.desktop", RegPatch { cover: Some(Some("/etc/passwd".into())), ..Default::default() }).is_err(), "caminho fora das pastas permitidas");
        assert!(p.update("native:calc.desktop", RegPatch { name: Some(Some("  ".into())), ..Default::default() }).is_err());
        p.update("native:calc.desktop", RegPatch { cover: Some(None), name: Some(None), ..Default::default() }).unwrap();
        let it = p.library().await.unwrap().remove(0);
        assert_eq!((it.title.as_str(), it.art.cover, it.art.hero), ("Calculadora", false, true), "voltou ao padrão");
        assert!(!c.exists(), "arquivo antigo apagado");
        p.remove("native:calc.desktop").unwrap();
        assert!(std::fs::read_dir(d.path().join("art")).unwrap().next().is_none(), "imagens apagadas junto com o app");
    }
}
