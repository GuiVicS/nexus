//! Atualizações do Nexus e do sistema (B11): versão instalada (dpkg), versão mais nova da release do GitHub (como o
//! `tools/update.sh`), pacotes do sistema com atualização (apt), instalar, histórico e voltar à versão anterior.
//! O trabalho de root é do ajudante (`sysops.rs`). Programas trocáveis: `NEXUS_DPKGQ`, `NEXUS_APT`, `NEXUS_CURL`;
//! `NEXUS_UPDATE_REPO` (padrão GuiVicS/nexus), `NEXUS_REBOOT_REQUIRED` (padrão /run/reboot-required).
use crate::db::Db;
use crate::sysops::{bin, now, progress, run, run_sys};
use serde_json::{json, Value};
use std::sync::Mutex;

#[derive(Default, Clone)]
struct Job {
    running: bool,
    kind: String,
    action: String,
    started: u64,
    /// (ok, mensagem) do último trabalho terminado
    done: Option<(bool, String)>,
}
static JOB: Mutex<Job> = Mutex::new(Job { running: false, kind: String::new(), action: String::new(), started: 0, done: None });

fn job() -> Job {
    JOB.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// "0.5.0-1", "1.2.3+b1" → (0, 5, 0)
pub fn semver(v: &str) -> Option<(u32, u32, u32)> {
    let core = v.trim().trim_start_matches('v').split(['-', '+', '~']).next()?;
    let mut p = core.split('.').map(|x| x.parse::<u32>());
    let t = (p.next()?.ok()?, p.next()?.ok()?, p.next()?.ok()?);
    p.next().is_none().then_some(t)
}

/// Versão do pacote instalado; `None` fora de um pacote .deb (desenvolvimento).
pub fn installed_version() -> Option<String> {
    let (ok, out, _) = run(&bin("NEXUS_DPKGQ", "dpkg-query"), &["-W", "-f=${Version}", "nexus"], 5);
    let v = out.trim().to_string();
    (ok && !v.is_empty()).then_some(v)
}

pub fn running_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Versão da release mais nova no GitHub: o redirecionamento de /releases/latest (sem a API, que tem limite).
pub fn latest_version() -> Result<String, String> {
    let repo = bin("NEXUS_UPDATE_REPO", "GuiVicS/nexus");
    let url = format!("https://github.com/{repo}/releases/latest");
    let (ok, out, _) = run(&bin("NEXUS_CURL", "curl"), &["-fsSLI", "-o", "/dev/null", "-w", "%{url_effective}", "--max-time", "15", &url], 20);
    parse_latest(&out).ok_or_else(|| if ok { "Não achei a versão mais nova (a página de releases mudou)".into() } else { "Sem internet ou o GitHub não respondeu".into() })
}

pub fn parse_latest(url: &str) -> Option<String> {
    let v = url.trim().rsplit_once("/v")?.1;
    semver(v).map(|_| v.to_string())
}

/// Pacotes do sistema com atualização (lista local do apt; quem atualiza a lista é o `apt update` do sistema).
pub fn parse_upgradable(out: &str) -> Vec<String> {
    out.lines().filter(|l| l.contains("upgradable from")).filter_map(|l| l.split('/').next()).map(String::from).collect()
}

pub fn system_upgradable() -> Vec<String> {
    let (_, out, _) = run(&bin("NEXUS_APT", "apt"), &["list", "--upgradable"], 30);
    parse_upgradable(&out)
}

fn history(db: &Db) -> Vec<Value> {
    db.kv_get("upd_history").and_then(|s| serde_json::from_str::<Vec<Value>>(&s).ok()).unwrap_or_default()
}

fn push_history(db: &Db, e: Value) {
    let mut h = history(db);
    h.push(e);
    let n = h.len();
    if n > 30 {
        h.drain(0..n - 30);
    }
    let _ = db.kv_set("upd_history", &Value::Array(h).to_string());
}

/// Versão para a qual dá para voltar: a de antes da última atualização do Nexus que deu certo (e ainda é a atual).
fn revert_target(db: &Db, current: &str) -> Option<String> {
    history(db).iter().rev().find(|e| e["kind"] == "nexus" && e["ok"] == true && e["to"] == current && e["from"].as_str().is_some_and(|f| semver(f).is_some())).and_then(|e| e["from"].as_str().map(String::from))
}

fn os_name() -> String {
    crate::hardware::os_pretty(&std::fs::read_to_string("/etc/os-release").unwrap_or_default())
}

pub fn status(db: &Db, game_running: bool) -> Value {
    let inst = installed_version();
    let current = inst.clone().unwrap_or_else(|| running_version().to_string());
    let latest = db.kv_get("upd_latest").filter(|v| semver(v).is_some());
    let has = match (&latest, semver(&current)) {
        (Some(l), Some(c)) => semver(l).is_some_and(|l| l > c),
        _ => false,
    };
    let sys: Vec<String> = db.kv_get("upd_system").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let j = job();
    let reboot = std::path::Path::new(&bin("NEXUS_REBOOT_REQUIRED", "/run/reboot-required")).exists();
    json!({
        "nexus": {
            "version": current,
            "packaged": inst.is_some(),
            "running": running_version(),
            "latest": latest,
            "hasUpdate": has,
            "needsRestart": inst.as_deref().is_some_and(|i| semver(i) != semver(running_version())),
            "revertTo": revert_target(db, &current),
        },
        "system": {
            "os": os_name(),
            "kernel": std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default().trim(),
            "upgradable": sys.len(),
            "packages": sys.iter().take(8).collect::<Vec<_>>(),
            "needsReboot": reboot,
        },
        "checkedAt": db.kv_get("upd_checked_at").and_then(|s| s.parse::<u64>().ok()),
        "checkError": db.kv_get("upd_check_error"),
        "history": history(db).into_iter().rev().take(10).collect::<Vec<_>>(),
        "gameRunning": game_running,
        "job": { "running": j.running, "kind": j.kind, "progress": if j.running { progress(&j.action) } else { String::new() }, "startedAt": j.started, "done": j.done.as_ref().map(|(ok, m)| json!({ "ok": ok, "message": m })) },
    })
}

/// Procura a versão mais nova e os pacotes do sistema. A falha de rede não apaga o que já se sabia.
pub fn check(db: &Db) {
    match latest_version() {
        Ok(v) => {
            let _ = db.kv_set("upd_latest", &v);
            let _ = db.kv_del("upd_check_error");
        }
        Err(e) => {
            let _ = db.kv_set("upd_check_error", &e);
        }
    }
    let sys = system_upgradable();
    let _ = db.kv_set("upd_system", &serde_json::to_string(&sys).unwrap_or_else(|_| "[]".into()));
    let _ = db.kv_set("upd_checked_at", &now().to_string());
}

/// Inicia a instalação em segundo plano (uma por vez). `version`: só para o Nexus (voltar a uma versão específica).
pub fn start_install(db: std::sync::Arc<Db>, kind: &str, version: Option<String>, on_done: impl FnOnce(bool, String) + Send + 'static) -> Result<(), String> {
    let action = match (kind, &version) {
        ("nexus", Some(v)) => format!("update-nexus:{v}"),
        ("nexus", None) => "update-nexus:latest".to_string(),
        ("system", _) => "upgrade-system".to_string(),
        _ => return Err("Tipo de atualização inválido".into()),
    };
    if !crate::sysops::action_ok(&action) {
        return Err("Versão inválida".into());
    }
    {
        let mut j = JOB.lock().unwrap_or_else(|e| e.into_inner());
        if j.running {
            return Err("Já existe uma atualização em andamento".into());
        }
        *j = Job { running: true, kind: kind.into(), action: action.clone(), started: now(), done: None };
    }
    let kind = kind.to_string();
    std::thread::spawn(move || {
        let from = installed_version().unwrap_or_default();
        let r = run_sys(&action);
        let to = installed_version().unwrap_or_default();
        let (ok, msg) = match &r {
            Ok(_) => (true, if kind == "nexus" { format!("Nexus {to} instalado") } else { "Sistema atualizado".to_string() }),
            Err(e) => (false, e.clone()),
        };
        push_history(&db, json!({ "ts": now(), "kind": kind, "from": from, "to": to, "ok": ok, "message": msg }));
        if kind == "system" && ok {
            let _ = db.kv_set("upd_system", "[]");
        }
        {
            let mut j = JOB.lock().unwrap_or_else(|e| e.into_inner());
            j.running = false;
            j.done = Some((ok, msg.clone()));
        }
        on_done(ok, msg);
    });
    Ok(())
}

/// Versão para a qual o "Voltar para a versão anterior" vai.
pub fn revert_version(db: &Db) -> Option<String> {
    let cur = installed_version()?;
    revert_target(db, &cur)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions() {
        assert_eq!(semver("0.5.0-1"), Some((0, 5, 0)));
        assert_eq!(semver("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(semver("1.2"), None);
        assert_eq!(semver("a.b.c"), None);
        assert!(semver("0.10.0") > semver("0.9.9"));
        assert_eq!(parse_latest("https://github.com/GuiVicS/nexus/releases/tag/v0.6.1\n"), Some("0.6.1".into()));
        assert_eq!(parse_latest("https://github.com/GuiVicS/nexus/releases"), None);
        assert_eq!(parse_latest(""), None);
    }
    #[test]
    fn upgradable() {
        let o = "Listing...\nlibfoo/noble-updates 1.2 amd64 [upgradable from: 1.1]\nbar/noble 2 all [upgradable from: 1]\n";
        assert_eq!(parse_upgradable(o), vec!["libfoo", "bar"]);
    }
    #[test]
    fn revert_target_is_previous_of_current() {
        let db = Db::memory().unwrap();
        push_history(&db, json!({ "kind": "nexus", "ok": true, "from": "0.4.0-1", "to": "0.5.0-1" }));
        push_history(&db, json!({ "kind": "system", "ok": true, "from": "", "to": "" }));
        assert_eq!(revert_target(&db, "0.5.0-1"), Some("0.4.0-1".into()));
        assert_eq!(revert_target(&db, "0.6.0-1"), None, "a atualização não é a atual");
        push_history(&db, json!({ "kind": "nexus", "ok": false, "from": "0.5.0-1", "to": "0.5.0-1" }));
        assert_eq!(revert_target(&db, "0.5.0-1"), Some("0.4.0-1".into()));
    }
    #[test]
    fn history_is_capped() {
        let db = Db::memory().unwrap();
        for i in 0..40 {
            push_history(&db, json!({ "i": i }));
        }
        assert_eq!(history(&db).len(), 30);
    }
}
