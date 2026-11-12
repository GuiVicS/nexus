//! Apps instalados (B17): programas Linux sem atalho de menu (`.jar` como o TLauncher, AppImage, `.sh` e executáveis)
//! que viram um `.desktop` em `~/.local/share/applications`, tamanho ocupado e desinstalação.
//! Programas trocáveis nos testes: `NEXUS_JAVA`, `NEXUS_DPKGQ`, `NEXUS_FLATPAK`.
use crate::customart;
use crate::sysops::{bin, run};
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Jar,
    AppImage,
    Script,
    Binary,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Jar => "Programa Java (.jar)",
            Kind::AppImage => "AppImage",
            Kind::Script => "Script (.sh)",
            Kind::Binary => "Programa executável",
        }
    }
}

pub const JAVA_HINT: &str = "sudo apt install default-jre";

fn has_ext(p: &Path, e: &str) -> bool {
    p.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case(e))
}

pub fn is_executable(p: &Path) -> bool {
    std::fs::metadata(p).map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

/// Tipo do arquivo para o navegador de programas e para `add_file`.
pub fn detect(p: &Path) -> Option<Kind> {
    if has_ext(p, "jar") {
        Some(Kind::Jar)
    } else if has_ext(p, "appimage") {
        Some(Kind::AppImage)
    } else if has_ext(p, "sh") {
        Some(Kind::Script)
    } else if is_executable(p) {
        Some(Kind::Binary)
    } else {
        None
    }
}

/// Programa `java` (variável de teste ou o do PATH).
pub fn java() -> Option<String> {
    which(&bin("NEXUS_JAVA", "java"))
}

pub fn which(prog: &str) -> Option<String> {
    if prog.contains('/') {
        return is_executable(Path::new(prog)).then(|| prog.to_string());
    }
    std::env::var("PATH").unwrap_or_default().split(':').map(|d| Path::new(d).join(prog)).find(|c| is_executable(c)).map(|c| c.to_string_lossy().to_string())
}

fn stem_name(p: &Path) -> String {
    let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("Programa");
    let mut s = n.to_string();
    for e in [".AppImage", ".appimage", ".jar", ".JAR", ".sh", ".SH"] {
        if let Some(x) = s.strip_suffix(e) {
            s = x.to_string();
            break;
        }
    }
    let s = s.replace(['_', '-'], " ");
    let s = s.trim();
    if s.is_empty() { "Programa".into() } else { s.chars().take(40).collect() }
}

/// Aspas do campo Exec (especificação XDG): `\`, `"`, `$` e crase com barra; `%` dobrado.
pub fn quote_exec(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' | '"' | '$' | '`' => {
                o.push('\\');
                o.push(c);
            }
            '%' => o.push_str("%%"),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn checked(roots: &[PathBuf], path: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(path);
    if !customart::inside(roots, &p) {
        return Err("Arquivo fora das pastas permitidas".into());
    }
    let p = p.canonicalize().map_err(|_| "Arquivo não encontrado".to_string())?;
    if !p.is_file() {
        return Err("Isso não é um arquivo".into());
    }
    Ok(p)
}

/// O que a tela mostra antes de adicionar: tipo, nome sugerido e se falta o Java.
pub fn file_info(roots: &[PathBuf], path: &str) -> Result<Value, String> {
    let p = checked(roots, path)?;
    let kind = detect(&p).ok_or("Esse arquivo não parece um programa. Escolha um .jar, .AppImage, .sh ou executável.")?;
    let need_java = kind == Kind::Jar;
    let java_ok = !need_java || java().is_some();
    Ok(json!({
        "kind": kind.label(), "name": stem_name(&p), "needsJava": need_java, "javaOk": java_ok,
        "hint": if java_ok { Value::Null } else { json!(format!("Falta o Java. Instale com: {JAVA_HINT}")) },
    }))
}

fn slug(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let s = s.split('-').filter(|x| !x.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "programa".into() } else { s.chars().take(30).collect() }
}

/// Cria `programa-<nome>-<hash>.desktop` em `apps_dir` e devolve o id do atalho. Marca AppImage/script como executáveis.
pub fn add_file(roots: &[PathBuf], apps_dir: &Path, path: &str, name: Option<&str>, game: bool) -> Result<String, String> {
    let p = checked(roots, path)?;
    let kind = detect(&p).ok_or("Esse arquivo não parece um programa. Escolha um .jar, .AppImage, .sh ou executável.")?;
    let exec = match kind {
        Kind::Jar => {
            let j = java().ok_or_else(|| format!("Falta o Java. Instale com: {JAVA_HINT}"))?;
            // com NEXUS_JAVA o atalho guarda o caminho dele; normalmente é só "java" (resolvido pelo PATH na hora de abrir)
            let jbin = if std::env::var("NEXUS_JAVA").is_ok() { j } else { "java".to_string() };
            format!("{} -jar {}", quote_exec(&jbin), quote_exec(&p.to_string_lossy()))
        }
        Kind::AppImage | Kind::Script | Kind::Binary => {
            if !is_executable(&p) {
                let mut perm = std::fs::metadata(&p).map_err(|_| "Não consegui ler o arquivo")?.permissions();
                perm.set_mode(perm.mode() | 0o755);
                std::fs::set_permissions(&p, perm).map_err(|_| "Não consegui marcar o arquivo como executável (sem permissão nesta pasta)")?;
            }
            quote_exec(&p.to_string_lossy())
        }
    };
    let name = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| stem_name(&p));
    if name.chars().count() > 40 || name.contains(['\n', '\r']) {
        return Err("Nome inválido (1 a 40 caracteres)".into());
    }
    use sha2::{Digest, Sha256};
    let h = Sha256::digest(p.to_string_lossy().as_bytes());
    let id = format!("programa-{}-{}.desktop", slug(&name), h.iter().take(3).map(|b| format!("{b:02x}")).collect::<String>());
    std::fs::create_dir_all(apps_dir).map_err(|_| "Não consegui criar a pasta de atalhos")?;
    let dir = p.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
    let text = format!(
        "[Desktop Entry]\nType=Application\nName={name}\nComment={}\nExec={exec}\nPath={dir}\nTerminal=false\nCategories={}\nX-Nexus-Program={}\n",
        kind.label(),
        if game { "Game;" } else { "Utility;" },
        p.to_string_lossy().replace('\n', " ")
    );
    std::fs::write(apps_dir.join(&id), text).map_err(|_| "Não consegui gravar o atalho")?;
    Ok(id)
}

// ---------------------------------------------------------------- tamanho e desinstalação

fn dir_size(p: &Path, budget: &mut u32) -> u64 {
    let Ok(md) = std::fs::symlink_metadata(p) else { return 0 };
    if md.is_file() {
        return md.len();
    }
    if !md.is_dir() || *budget == 0 {
        return 0;
    }
    let Ok(rd) = std::fs::read_dir(p) else { return 0 };
    let mut t = 0;
    for e in rd.flatten() {
        *budget = budget.saturating_sub(1);
        t += dir_size(&e.path(), budget);
        if *budget == 0 {
            break;
        }
    }
    t
}

/// Origem do programa e o que a tela pode fazer com ele.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// atalho criado pelo Nexus a partir de um arquivo (.jar, AppImage…): desinstalar = tirar o atalho
    File(PathBuf),
    Flatpak { id: String, user: bool },
    Apt(String),
    Other,
}

fn flatpak_id(argv: &[String]) -> Option<String> {
    if argv.first().map(|a| a.rsplit('/').next().unwrap_or(a)) != Some("flatpak") {
        return None;
    }
    argv.iter().skip(1).find(|a| !a.starts_with('-') && *a != "run" && a.contains('.') && a.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))).cloned()
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

/// Programa principal do `Exec`: o primeiro argumento que é um caminho absoluto existente (pula `java -jar`, `sh`…).
fn main_path(argv: &[String]) -> Option<PathBuf> {
    if let Some(i) = argv.iter().position(|a| a == "-jar") {
        if let Some(j) = argv.get(i + 1) {
            return Some(PathBuf::from(j));
        }
    }
    argv.iter().find(|a| a.starts_with('/') && Path::new(a).exists()).map(PathBuf::from).or_else(|| argv.first().and_then(|a| which(a)).map(PathBuf::from))
}

pub fn dpkg_owner(path: &Path) -> Option<String> {
    let (ok, out, _) = run(&bin("NEXUS_DPKGQ", "dpkg-query"), &["-S", &path.to_string_lossy()], 10);
    if !ok {
        return None;
    }
    let first = out.lines().next()?;
    let (pkgs, _) = first.split_once(": /")?;
    let pkg = pkgs.split(',').next()?.trim().split(':').next()?.trim();
    (!pkg.is_empty()).then(|| pkg.to_string())
}

pub fn source_of(file_id: &str, exec: &str, desktop_path: Option<&Path>) -> Source {
    if file_id.starts_with("programa-") {
        return Source::File(desktop_path.map(Path::to_path_buf).unwrap_or_default());
    }
    let argv = crate::providers::native::parse_exec(exec);
    if let Some(id) = flatpak_id(&argv) {
        let user = home().join(".local/share/flatpak/app").join(&id).exists();
        return Source::Flatpak { id, user };
    }
    if let Some(p) = main_path(&argv) {
        if let Some(pkg) = dpkg_owner(&p) {
            return Source::Apt(pkg);
        }
    }
    Source::Other
}

/// Tamanho ocupado em disco (bytes), com limite de arquivos lidos para não travar.
pub fn size_of(src: &Source, exec: &str) -> Option<u64> {
    let argv = crate::providers::native::parse_exec(exec);
    let mut budget = 200_000u32;
    match src {
        Source::Flatpak { id, user } => {
            let base = if *user { home().join(".local/share/flatpak/app") } else { PathBuf::from("/var/lib/flatpak/app") };
            let d = base.join(id);
            d.exists().then(|| dir_size(&d, &mut budget))
        }
        _ => {
            let p = main_path(&argv)?;
            let abs = p.to_string_lossy().to_string();
            // programas em /opt/<nome> ou em pastas próprias ocupam a pasta inteira; os de /usr/bin, só o arquivo
            let own_dir = abs.strip_prefix("/opt/").and_then(|r| r.split('/').next()).map(|d| Path::new("/opt").join(d));
            if let Some(d) = own_dir {
                return Some(dir_size(&d, &mut budget));
            }
            Some(dir_size(&p, &mut budget))
        }
    }
}

/// Desinstala. `File` e `Other` não passam por aqui (o RPC trata). Apt e Flatpak do sistema pedem ao ajudante root;
/// Flatpak do usuário roda direto.
pub fn uninstall(src: &Source) -> Result<(), String> {
    match src {
        Source::Apt(pkg) => {
            if pkg.starts_with("nexus") {
                return Err("O Nexus não pode remover a si mesmo por aqui".into());
            }
            crate::sysops::run_sys(&format!("remove-pkg:{pkg}")).map(|_| ())
        }
        Source::Flatpak { id, user: false } => crate::sysops::run_sys(&format!("flatpak-rm:{id}")).map(|_| ()),
        Source::Flatpak { id, user: true } => {
            let (ok, _, e) = run(&bin("NEXUS_FLATPAK", "flatpak"), &["uninstall", "--user", "-y", "--noninteractive", id], 600);
            if ok { Ok(()) } else { Err(format!("O Flatpak não conseguiu remover: {}", e.lines().last().unwrap_or("erro"))) }
        }
        _ => Err("Não sei remover este programa".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_and_names() {
        assert_eq!(quote_exec("/home/u/Meus Jogos/TL \"x\".jar"), "\"/home/u/Meus Jogos/TL \\\"x\\\".jar\"");
        assert_eq!(quote_exec("100%"), "\"100%%\"");
        assert_eq!(stem_name(Path::new("/x/TLauncher.jar")), "TLauncher");
        assert_eq!(stem_name(Path::new("/x/Cool_App-x86_64.AppImage")), "Cool App x86 64");
        assert_eq!(slug("Meu Jogo!"), "meu-jogo");
    }

    #[test]
    fn jar_needs_java_and_appimage_gets_executable() {
        let d = tempfile::tempdir().unwrap();
        let home = d.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let roots = vec![home.clone()];
        let apps = d.path().join("apps");
        std::fs::write(home.join("TL Launcher.jar"), "PK").unwrap();
        std::fs::write(home.join("Foo.AppImage"), "x").unwrap();
        std::fs::write(home.join("nota.txt"), "x").unwrap();
        // sem java
        std::env::set_var("NEXUS_JAVA", "/nao/existe/java");
        let info = file_info(&roots, home.join("TL Launcher.jar").to_str().unwrap()).unwrap();
        assert_eq!(info["javaOk"], false);
        assert!(info["hint"].as_str().unwrap().contains("sudo apt install default-jre"));
        assert!(add_file(&roots, &apps, home.join("TL Launcher.jar").to_str().unwrap(), None, true).unwrap_err().contains("default-jre"));
        // com java (qualquer executável)
        let fake = home.join("java");
        std::fs::write(&fake, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("NEXUS_JAVA", &fake);
        let id = add_file(&roots, &apps, home.join("TL Launcher.jar").to_str().unwrap(), None, true).unwrap();
        let text = std::fs::read_to_string(apps.join(&id)).unwrap();
        assert!(text.contains("Name=TL Launcher") && text.contains("-jar \"") && text.contains("TL Launcher.jar\"") && text.contains("Categories=Game;"), "{text}");
        let e = crate::providers::native::parse_desktop(&text, &id).unwrap();
        let argv = crate::providers::native::parse_exec(&e.exec);
        assert_eq!(argv[1], "-jar");
        assert!(argv[2].ends_with("TL Launcher.jar"));
        // AppImage vira executável
        assert!(!is_executable(&home.join("Foo.AppImage")));
        let id2 = add_file(&roots, &apps, home.join("Foo.AppImage").to_str().unwrap(), Some("Foo"), false).unwrap();
        assert!(is_executable(&home.join("Foo.AppImage")));
        assert!(std::fs::read_to_string(apps.join(&id2)).unwrap().contains("Categories=Utility;"));
        // arquivo qualquer e fora das pastas
        assert!(add_file(&roots, &apps, home.join("nota.txt").to_str().unwrap(), None, false).is_err());
        assert!(add_file(&roots, &apps, "/etc/passwd", None, false).is_err());
        std::env::remove_var("NEXUS_JAVA");
    }

    #[test]
    fn sources() {
        assert_eq!(flatpak_id(&["flatpak".into(), "run".into(), "--branch=stable".into(), "org.mozilla.firefox".into()]).as_deref(), Some("org.mozilla.firefox"));
        assert_eq!(source_of("programa-x-1.desktop", "x", Some(Path::new("/a/b"))), Source::File(PathBuf::from("/a/b")));
        assert!(matches!(source_of("a.desktop", "flatpak run org.x.App", None), Source::Flatpak { .. }));
    }
}
