//! Atualizações dos apps instalados (programas do Linux adicionados ao Nexus).
//!
//! Cada app vem de um gerenciador de pacotes (Flatpak, Snap ou apt). A TV não tem como pedir senha, então:
//! - **Flatpak do usuário** (`~/.local/share/flatpak`): o Nexus atualiza e volta versão direto (`--commit`).
//! - **Flatpak do sistema**: atualizar é liberado sem senha pela política do Flatpak; **voltar versão exige root**
//!   ("Can't update to a specific commit without root permissions"), então vai pelo ajudante.
//! - **Snap e apt**: atualizar e voltar versão pelo ajudante.
//!
//! O ajudante é `nexus-app@<update|restore>:<flatpak|snap|apt>:<nome>.service` (root, `system/sysd/`), que o Core
//! inicia com `systemctl start`; uma regra do polkit libera só essa unidade para a sessão local ativa. Quem decide a
//! versão é o ajudante (sempre a anterior à instalada). Sem o ajudante instalado (desenvolvimento), Snap/apt só avisam.
//!
//! **Dependências do Nexus** (RetroArch, Steam, navegador do streaming) ficam de fora: não avisam nem oferecem
//! Atualizar/Restaurar. O RetroArch segue a versão travada em `deps.lock` (a da máquina de desenvolvimento) e só muda
//! com uma release do Nexus; a Steam se atualiza sozinha (Valve) e o navegador fica com o sistema (segurança).
use crate::core::Core;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Source {
    Flatpak { app: String, system: bool },
    Snap { name: String },
    Apt { pkg: String },
}

impl Source {
    pub fn label(&self) -> &'static str {
        match self {
            Source::Flatpak { .. } => "Flatpak",
            Source::Snap { .. } => "Snap",
            Source::Apt { .. } => "Sistema (apt)",
        }
    }
    /// O Nexus consegue atualizar este app (sem senha)?
    pub fn can_update(&self) -> bool {
        matches!(self, Source::Flatpak { .. }) || helper_available()
    }
    /// ... e voltar para a versão anterior?
    pub fn can_restore(&self) -> bool {
        match self {
            Source::Flatpak { system: false, .. } => true,
            Source::Flatpak { .. } | Source::Snap { .. } => helper_available(),
            Source::Apt { pkg } => helper_available() && Path::new(&bin("NEXUS_APP_STATE", "/var/lib/nexus/apps")).join(format!("{pkg}.prev")).is_file(),
        }
    }
    fn kind(&self) -> &'static str {
        match self {
            Source::Flatpak { .. } => "flatpak",
            Source::Snap { .. } => "snap",
            Source::Apt { .. } => "apt",
        }
    }
    fn name(&self) -> &str {
        match self {
            Source::Flatpak { app, .. } => app,
            Source::Snap { name } => name,
            Source::Apt { pkg } => pkg,
        }
    }
}

/// O ajudante root está instalado? (`NEXUS_APP_HELPER` = script para rodar direto, nos testes)
pub fn helper_available() -> bool {
    std::env::var("NEXUS_APP_HELPER").is_ok() || ["/usr/lib/systemd/system/nexus-app@.service", "/etc/systemd/system/nexus-app@.service", "/lib/systemd/system/nexus-app@.service"].iter().any(|p| Path::new(p).is_file())
}

/// Nome que o ajudante e a regra do polkit aceitam (sem espaço, barra, '+', etc.).
pub fn helper_name_ok(n: &str) -> bool {
    !n.is_empty() && n.len() <= 128 && n.chars().next().is_some_and(|c| c.is_ascii_alphanumeric()) && n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Pede ao ajudante root (pela unidade do systemd) para atualizar/restaurar; devolve a versão instalada no fim.
fn via_helper(action: &str, s: &Source) -> Result<String, String> {
    if !helper_available() {
        return Err("Falta o ajudante do Nexus para isso (reinstale o pacote do Nexus)".into());
    }
    if !helper_name_ok(s.name()) {
        return Err("Nome de pacote não suportado".into());
    }
    let inst = format!("{action}:{}:{}", s.kind(), s.name());
    let out_dir = bin("NEXUS_APP_OUT", "/run/nexus-app");
    let res = Path::new(&out_dir).join(format!("{}-{}", s.kind(), s.name()));
    let _ = std::fs::remove_file(&res);
    let (ok, _, err) = match std::env::var("NEXUS_APP_HELPER") {
        Ok(h) => run(&h, &[&inst]),
        Err(_) => run("systemctl", &["start", "--no-ask-password", &format!("nexus-app@{inst}.service")]),
    };
    let msg = std::fs::read_to_string(&res).unwrap_or_default();
    let msg = msg.trim();
    if let Some(v) = msg.strip_prefix("ok") {
        return Ok(v.trim().to_string());
    }
    if let Some(e) = msg.strip_prefix("err ") {
        return Err(e.to_string());
    }
    if ok {
        return Ok(String::new());
    }
    Err(if err.contains("authentication") || err.contains("Access denied") { "Sem permissão para o ajudante do Nexus (reinstale o pacote)".into() } else { short_err(&err) })
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdate {
    pub id: String,
    pub title: String,
    pub source: String,
    pub current: String,
    pub available: String,
    pub can_update: bool,
}

fn bin(var: &str, def: &str) -> String {
    std::env::var(var).unwrap_or_else(|_| def.to_string())
}

/// Roda um comando com mensagens em inglês (a saída é lida pelo Nexus) e devolve (sucesso, stdout, stderr).
fn run(prog: &str, args: &[&str]) -> (bool, String, String) {
    match Command::new(prog).args(args).env("LC_ALL", "C").env("LANG", "C").output() {
        Ok(o) => (o.status.success(), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned()),
        Err(e) => (false, String::new(), e.to_string()),
    }
}

/// De onde veio o app, pelo comando do atalho (.desktop) e pela pasta onde o atalho está.
pub fn source_from(argv: &[String], desktop: Option<&Path>, file_id: &str) -> Option<Source> {
    let dpath = desktop.map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let prog = argv.first().map(String::as_str).unwrap_or("");
    if prog.ends_with("flatpak") && argv.iter().any(|a| a == "run") {
        let after_run = argv.iter().skip_while(|a| *a != "run").skip(1);
        let app = after_run.filter(|a| !a.starts_with('-') && !a.starts_with('@')).next()?.clone();
        // instalação do sistema (/var/lib/flatpak) ou do usuário (~/.local/share/flatpak)
        let system = if dpath.contains("/.local/share/flatpak/") {
            false
        } else if dpath.contains("/var/lib/flatpak/") {
            true
        } else {
            !std::env::var("HOME").map(|h| Path::new(&h).join(".local/share/flatpak/app").join(&app).exists()).unwrap_or(false)
        };
        return Some(Source::Flatpak { app, system });
    }
    if dpath.contains("/var/lib/snapd/desktop/applications") || prog.starts_with("/snap/bin/") {
        let name = if let Some(b) = prog.strip_prefix("/snap/bin/") { b.split('.').next().unwrap_or(b).to_string() } else { file_id.split('_').next().unwrap_or(file_id).trim_end_matches(".desktop").to_string() };
        return (!name.is_empty()).then_some(Source::Snap { name });
    }
    None
}

/// `dpkg-query -S <arquivo>` → pacote dono do atalho (`pkg:arq: /caminho`).
pub fn parse_dpkg_owner(out: &str) -> Option<String> {
    let line = out.lines().find(|l| l.contains(": /"))?;
    let pkg = line.split(": /").next()?.split(',').next()?.trim();
    let pkg = pkg.split(':').next()?.trim();
    (!pkg.is_empty() && !pkg.contains(' ')).then(|| pkg.to_string())
}

/// `flatpak remote-ls --updates --app --columns=application,version` → {app: versão nova (pode ser vazia)}.
pub fn parse_flatpak_updates(out: &str) -> HashMap<String, String> {
    out.lines()
        .filter_map(|l| {
            let mut c = l.split('\t');
            let app = c.next()?.trim();
            (app.contains('.') && !app.eq_ignore_ascii_case("application id")).then(|| (app.to_string(), c.next().unwrap_or("").trim().to_string()))
        })
        .collect()
}

/// `flatpak list --app --columns=application,version` → {app: versão instalada}.
pub fn parse_flatpak_list(out: &str) -> HashMap<String, String> {
    parse_flatpak_updates(out)
}

/// `snap refresh --list` (ou `snap list`): tabela "Name Version Rev …" → {nome: versão}.
pub fn parse_snap_table(out: &str) -> HashMap<String, String> {
    out.lines()
        .skip_while(|l| !l.starts_with("Name"))
        .skip(1)
        .filter_map(|l| {
            let mut c = l.split_whitespace();
            Some((c.next()?.to_string(), c.next()?.to_string()))
        })
        .collect()
}

/// `apt list --upgradable` (LC_ALL=C): "pkg/suite nova arq [upgradable from: atual]" → {pkg: (atual, nova)}.
pub fn parse_apt_upgradable(out: &str) -> HashMap<String, (String, String)> {
    out.lines()
        .filter_map(|l| {
            let (pkg, rest) = l.split_once('/')?;
            let newv = rest.split_whitespace().nth(1)?.to_string();
            let cur = l.split("upgradable from: ").nth(1)?.trim_end_matches(']').trim().to_string();
            Some((pkg.to_string(), (cur, newv)))
        })
        .collect()
}

/// Campo de `flatpak info` (LC_ALL=C), ex.: "Commit", "Parent", "Version", "Origin".
pub fn info_field(out: &str, key: &str) -> Option<String> {
    out.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        (k.trim() == key).then(|| v.trim().to_string()).filter(|v| !v.is_empty() && v != "-")
    })
}

fn inst_flag(system: bool) -> &'static str {
    if system {
        "--system"
    } else {
        "--user"
    }
}

/// Apps adicionados ao Nexus com a origem de cada um: (id do item, título, origem).
pub const DEP_MSG: &str = "Este programa é atualizado junto com o Nexus";

/// Programas que o Nexus usa por baixo (motores): a versão é do Nexus, não do usuário.
pub fn is_dependency(s: &Source) -> bool {
    const FLATPAK: &[&str] = &["org.libretro.RetroArch", "com.valvesoftware.Steam", "org.chromium.Chromium", "com.google.Chrome"];
    const DEB: &[&str] = &["retroarch", "steam", "steam-installer", "steam-launcher", "steam-libs-amd64", "chromium", "chromium-browser", "google-chrome-stable"];
    match s {
        Source::Flatpak { app, .. } => FLATPAK.contains(&app.as_str()),
        Source::Snap { name } => matches!(name.as_str(), "retroarch" | "steam" | "chromium"),
        Source::Apt { pkg } => DEB.contains(&pkg.as_str()) || pkg.starts_with("libretro-"),
    }
}

/// Apps que o usuário adicionou, com a origem; sem as dependências do Nexus.
fn sources(core: &Core) -> Vec<(String, String, Source)> {
    all_sources(core).into_iter().filter(|(_, _, s)| !is_dependency(s)).collect()
}

fn all_sources(core: &Core) -> Vec<(String, String, Source)> {
    let titles: HashMap<String, String> = core.library.read().unwrap_or_else(|e| e.into_inner()).iter().map(|i| (i.id.clone(), i.title.clone())).collect();
    let mut out = vec![];
    for r in core.native.registered() {
        let Some(e) = core.native.entry(&r.file_id) else { continue };
        let path = core.native.desktop_path(&r.file_id);
        let argv = crate::providers::native::parse_exec(&e.exec);
        let src = source_from(&argv, path.as_deref(), &r.file_id).or_else(|| {
            let p = path.as_ref()?;
            let (_, o, _) = run(&bin("NEXUS_DPKG_QUERY_BIN", "dpkg-query"), &["-S", &p.to_string_lossy()]);
            parse_dpkg_owner(&o).map(|pkg| Source::Apt { pkg })
        });
        if let Some(s) = src {
            let title = titles.get(&r.item_id).cloned().or(r.name.clone()).unwrap_or(e.name.clone());
            out.push((r.item_id, title, s));
        }
    }
    out
}

/// Procura atualizações de todos os apps (bloqueante: rode em `spawn_blocking`).
pub fn check(core: &Core) -> Vec<AppUpdate> {
    let srcs = sources(core);
    let flatpak = bin("NEXUS_FLATPAK_BIN", "flatpak");
    let need = |f: fn(&Source) -> bool| srcs.iter().any(|(_, _, s)| f(s));
    let (fp_new, fp_cur) = if need(|s| matches!(s, Source::Flatpak { .. })) {
        let (_, up, _) = run(&flatpak, &["remote-ls", "--updates", "--app", "--columns=application,version"]);
        let (_, cur, _) = run(&flatpak, &["list", "--app", "--columns=application,version"]);
        (parse_flatpak_updates(&up), parse_flatpak_list(&cur))
    } else {
        Default::default()
    };
    let (snap_new, snap_cur) = if need(|s| matches!(s, Source::Snap { .. })) {
        let snap = bin("NEXUS_SNAP_BIN", "snap");
        (parse_snap_table(&run(&snap, &["refresh", "--list"]).1), parse_snap_table(&run(&snap, &["list"]).1))
    } else {
        Default::default()
    };
    let apt = if need(|s| matches!(s, Source::Apt { .. })) { parse_apt_upgradable(&run(&bin("NEXUS_APT_BIN", "apt"), &["list", "--upgradable"]).1) } else { Default::default() };
    let mut out = vec![];
    for (id, title, s) in srcs {
        let (current, available) = match &s {
            Source::Flatpak { app, .. } => match fp_new.get(app) {
                Some(v) => (fp_cur.get(app).cloned().unwrap_or_default(), v.clone()),
                None => continue,
            },
            Source::Snap { name } => match snap_new.get(name) {
                Some(v) => (snap_cur.get(name).cloned().unwrap_or_default(), v.clone()),
                None => continue,
            },
            Source::Apt { pkg } => match apt.get(pkg) {
                Some((c, n)) => (c.clone(), n.clone()),
                None => continue,
            },
        };
        out.push(AppUpdate { id, title, source: s.label().into(), current, available, can_update: s.can_update() });
    }
    out
}

/// Guarda o resultado, marca os itens com a etiqueta "update", avisa (uma vez por versão) e emite `apps.updates`.
pub fn publish(core: &Arc<Core>, list: Vec<AppUpdate>) {
    for u in &list {
        let key = format!("appupd:notified:{}", u.id);
        let tag = if u.available.is_empty() { "nova".to_string() } else { u.available.clone() };
        if core.db.kv_get(&key).as_deref() != Some(tag.as_str()) {
            let _ = core.db.kv_set(&key, &tag);
            let body = if u.available.is_empty() { u.title.clone() } else { format!("{} · versão {}", u.title, u.available) };
            core.notify("download", "Atualização disponível", &body, Some(&format!("hub:{}", u.id)));
        }
    }
    *core.app_updates.lock().unwrap_or_else(|e| e.into_inner()) = list.iter().map(|u| (u.id.clone(), u.clone())).collect();
    core.emit("apps.updates", json!(list));
    core.kick.notify_one(); // a biblioteca ganha/perde a etiqueta "update"
}

/// Procura agora e publica.
pub async fn check_now(core: &Arc<Core>) -> Vec<AppUpdate> {
    let c = core.clone();
    let list = tokio::task::spawn_blocking(move || check(&c)).await.unwrap_or_default();
    publish(core, list.clone());
    list
}

/// Procura atualizações pouco depois de ligar e a cada 6 horas.
pub async fn update_loop(core: Arc<Core>) {
    if std::env::var("NEXUS_OFFLINE").is_ok_and(|v| v == "1") || core.cfg.demo {
        return;
    }
    tokio::time::sleep(Duration::from_secs(90)).await;
    loop {
        check_now(&core).await;
        tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
    }
}

/// O item é uma dependência do Nexus (RetroArch, Steam, navegador)?
pub fn is_dependency_item(core: &Core, id: &str) -> bool {
    all_sources(core).iter().any(|(i, _, s)| i == id && is_dependency(s))
}

fn source_of(core: &Core, id: &str) -> Result<Source, String> {
    if is_dependency_item(core, id) {
        return Err(DEP_MSG.into());
    }
    sources(core).into_iter().find(|(i, _, _)| i == id).map(|(_, _, s)| s).ok_or_else(|| "Não sei de onde este app veio".into())
}

/// Atualiza um app. Flatpak: direto (liberado sem senha), guardando a versão atual para poder voltar (instalação do
/// usuário). Snap/apt: pelo ajudante.
pub fn update(core: &Core, id: &str) -> Result<String, String> {
    let s = source_of(core, id)?;
    let Source::Flatpak { app, system } = &s else { return via_helper("update", &s) };
    let flatpak = bin("NEXUS_FLATPAK_BIN", "flatpak");
    let (_, info, _) = run(&flatpak, &["info", inst_flag(*system), app]);
    if let Some(c) = info_field(&info, "Commit") {
        let _ = core.db.kv_set(&format!("appupd:prev:{id}"), &c);
    }
    let (ok, _, err) = run(&flatpak, &["update", "-y", "--noninteractive", inst_flag(*system), app]);
    if !ok {
        return Err(short_err(&err));
    }
    let (_, info, _) = run(&flatpak, &["info", inst_flag(*system), app]);
    Ok(info_field(&info, "Version").unwrap_or_default())
}

/// Volta para a versão anterior. Flatpak do usuário: direto (a de antes da última atualização feita pelo Nexus, ou o
/// commit pai). Flatpak do sistema, Snap e apt: pelo ajudante root, que escolhe a anterior.
pub fn restore(core: &Core, id: &str) -> Result<String, String> {
    let s = source_of(core, id)?;
    let Source::Flatpak { app, system: false } = &s else { return via_helper("restore", &s) };
    let flatpak = bin("NEXUS_FLATPAK_BIN", "flatpak");
    let (_, info, _) = run(&flatpak, &["info", "--user", app]);
    let cur = info_field(&info, "Commit");
    let key = format!("appupd:prev:{id}");
    let target = core.db.kv_get(&key).filter(|c| Some(c) != cur.as_ref()).or_else(|| info_field(&info, "Parent")).ok_or("Não há versão anterior para este app")?;
    let commit = format!("--commit={target}");
    let (ok, _, err) = run(&flatpak, &["update", "-y", "--noninteractive", "--user", &commit, app]);
    if !ok {
        return Err(short_err(&err));
    }
    let _ = core.db.kv_del(&key); // a próxima "versão anterior" passa a ser o pai desta
    let (_, info, _) = run(&flatpak, &["info", "--user", app]);
    Ok(info_field(&info, "Version").unwrap_or_default())
}

/// Os apps que podem voltar para a versão anterior (para a Shell mostrar a opção).
pub fn restorable(core: &Core) -> Vec<String> {
    sources(core).into_iter().filter(|(_, _, s)| s.can_restore()).map(|(i, _, _)| i).collect()
}

fn short_err(err: &str) -> String {
    let l = err.lines().map(str::trim).filter(|l| !l.is_empty()).last().unwrap_or("falhou");
    if l.contains("Not authorized") || l.contains("not allowed") {
        return "O sistema pediu senha de administrador".into();
    }
    l.trim_start_matches("error: ").trim_start_matches("Error: ").chars().take(160).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn v(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn nexus_dependencies_are_left_out() {
        assert!(is_dependency(&Source::Flatpak { app: "org.libretro.RetroArch".into(), system: true }));
        assert!(is_dependency(&Source::Apt { pkg: "steam-installer".into() }));
        assert!(is_dependency(&Source::Apt { pkg: "libretro-snes9x".into() }));
        assert!(is_dependency(&Source::Snap { name: "chromium".into() }));
        assert!(!is_dependency(&Source::Flatpak { app: "org.mozilla.firefox".into(), system: true }));
        assert!(!is_dependency(&Source::Apt { pkg: "vlc".into() }));
    }

    #[test]
    fn detects_the_source() {
        let fp = source_from(&v("/usr/bin/flatpak run --branch=stable --arch=x86_64 --command=retroarch org.libretro.RetroArch @@u @@"), Some(Path::new("/var/lib/flatpak/exports/share/applications/org.libretro.RetroArch.desktop")), "org.libretro.RetroArch.desktop");
        assert_eq!(fp, Some(Source::Flatpak { app: "org.libretro.RetroArch".into(), system: true }));
        let user = source_from(&v("flatpak run org.x.App"), Some(Path::new("/home/a/.local/share/flatpak/exports/share/applications/org.x.App.desktop")), "org.x.App.desktop");
        assert_eq!(user, Some(Source::Flatpak { app: "org.x.App".into(), system: false }));
        assert_eq!(source_from(&v("/snap/bin/firefox %u"), Some(Path::new("/var/lib/snapd/desktop/applications/firefox_firefox.desktop")), "firefox_firefox.desktop"), Some(Source::Snap { name: "firefox".into() }));
        assert_eq!(source_from(&v("env BAMF=1 /snap/bin/x"), Some(Path::new("/var/lib/snapd/desktop/applications/kodi_kodi.desktop")), "kodi_kodi.desktop"), Some(Source::Snap { name: "kodi".into() }));
        assert_eq!(source_from(&v("gnome-calculator"), Some(Path::new("/usr/share/applications/org.gnome.Calculator.desktop")), "org.gnome.Calculator.desktop"), None);
        assert_eq!(parse_dpkg_owner("gnome-calculator: /usr/share/applications/org.gnome.Calculator.desktop\n").as_deref(), Some("gnome-calculator"));
        assert_eq!(parse_dpkg_owner("steam-installer:amd64: /usr/share/applications/steam.desktop\n").as_deref(), Some("steam-installer"));
        assert_eq!(parse_dpkg_owner("dpkg-query: no path found matching pattern /x\n"), None);
    }

    /// O ajudante root (system/sysd/nexus-app-helper) com snap/apt falsos: recusa nomes estranhos, guarda a versão do
    /// apt antes de atualizar e volta para ela; Snap usa `snap revert`.
    #[test]
    fn root_helper_script() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        let exe = |n: &str, body: &str| {
            std::fs::write(p.join(n), body).unwrap();
            std::fs::set_permissions(p.join(n), std::fs::Permissions::from_mode(0o755)).unwrap();
        };
        std::fs::write(p.join("ver"), "1.0").unwrap();
        exe("dpkg-query", &format!("#!/bin/sh\ncat {}/ver\n", p.display()));
        exe("apt-get", &format!("#!/bin/sh\necho \"$@\" >> {0}/calls\ncase \"$*\" in *kodi=*) echo \"${{4#kodi=}}\" > {0}/ver ;; *) echo 2.0 > {0}/ver ;; esac\n", p.display()));
        exe("snap", &format!("#!/bin/sh\necho \"snap $@\" >> {}/calls\n[ \"$1\" = list ] && printf 'Name Version\\nfirefox 159.0\\n'\nexit 0\n", p.display()));
        let helper = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../system/sysd/nexus-app-helper");
        let go = |arg: &str| {
            let o = Command::new("sh").arg(&helper).arg(arg).env("PATH", format!("{}:/usr/bin:/bin", p.display())).env("NEXUS_APP_OUT", p.join("out")).env("NEXUS_APP_STATE", p.join("state")).output().unwrap();
            (o.status.code(), std::fs::read_to_string(p.join("out").join(arg.splitn(2, ':').nth(1).unwrap_or("").replacen(':', "-", 1))).unwrap_or_default())
        };
        assert_eq!(go("update:apt:../../etc").0, Some(2), "nome com barra recusado");
        assert_eq!(go("update:apt:a b").0, Some(2), "nome com espaço recusado");
        assert_eq!(go("rm:apt:kodi").0, Some(2), "ação desconhecida recusada");
        assert_eq!(go("update:apt:kodi"), (Some(0), "ok 2.0\n".into()));
        assert_eq!(std::fs::read_to_string(p.join("state/kodi.prev")).unwrap().trim(), "1.0", "versão anterior guardada");
        assert_eq!(go("restore:apt:kodi"), (Some(0), "ok 1.0\n".into()));
        assert!(std::fs::read_to_string(p.join("calls")).unwrap().contains("install -y --allow-downgrades kodi=1.0"));
        let (code, msg) = go("restore:apt:kodi");
        assert_eq!(code, Some(1));
        assert!(msg.starts_with("err Não há versão anterior"), "{msg}");
        assert_eq!(go("restore:snap:firefox"), (Some(0), "ok 159.0\n".into()));
        assert!(std::fs::read_to_string(p.join("calls")).unwrap().contains("snap revert firefox"));
    }

    #[test]
    fn helper_names() {
        assert!(helper_name_ok("org.libretro.RetroArch") && helper_name_ok("gnome-calculator") && helper_name_ok("firefox"));
        assert!(!helper_name_ok("g++") && !helper_name_ok("../x") && !helper_name_ok("-x") && !helper_name_ok(""));
    }

    #[test]
    fn parses_update_lists() {
        let fp = parse_flatpak_updates("org.libretro.RetroArch\t1.22.3\norg.x.App\t\n");
        assert_eq!(fp["org.libretro.RetroArch"], "1.22.3");
        assert_eq!(fp["org.x.App"], "");
        let snap = parse_snap_table("Name     Version  Rev   Size   Publisher  Notes\nfirefox  160.0    9200  300MB  mozilla**  -\n");
        assert_eq!(snap["firefox"], "160.0");
        assert!(parse_snap_table("All snaps up to date.\n").is_empty());
        let apt = parse_apt_upgradable("Listing...\nkodi/noble-updates 2:21.2 amd64 [upgradable from: 2:21.1]\n");
        assert_eq!(apt["kodi"], ("2:21.1".to_string(), "2:21.2".to_string()));
        let info = "    Version: 1.22.2\n     Commit: bb2d\n     Parent: 9c51\n";
        assert_eq!(info_field(info, "Parent").as_deref(), Some("9c51"));
        assert_eq!(info_field(info, "Version").as_deref(), Some("1.22.2"));
        assert_eq!(info_field(info, "Origin"), None);
    }
}
