//! Google Drive como armazenamento na nuvem, pelo `rclone` (dependência do pacote).
//!
//! - Login: `rclone authorize drive` abre um endereço local (127.0.0.1:53682); a Core mostra esse endereço numa
//!   janela de navegador na TV (item escondido `drive:login`) e, quando o Google devolve a autorização, grava o
//!   token num `rclone.conf` só do Nexus (pasta de dados, permissão 600) e fecha a janela.
//! - Pasta do sistema: `Nexus/` na raiz do Drive (procura; se não houver, cria) com `ROMs/<console>`, `Filmes`,
//!   `Séries`, `Músicas` e `Saves`.
//! - Montagem: `rclone mount` com cache em disco (`--vfs-cache-mode full`, limite configurável): ROMs e mídia são
//!   lidas sob demanda e o que já foi lido fica na TV. Antes de abrir uma ROM da nuvem, `prefetch` lê o arquivo
//!   inteiro (com progresso): o emulador então lê do disco, sem travar no meio do jogo.
//!
//! `NEXUS_RCLONE` troca o binário e `NEXUS_DRIVE_REMOTE` o remoto (testes usam um remoto `local`).
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const FOLDER: &str = "Nexus";
pub const SUBDIRS: &[&str] = &["ROMs", "Filmes", "Séries", "Músicas", "Saves"];
const REMOTE: &str = "nexus-drive";
/// Limite padrão do cache local (GB): o que passar disso sai do cache, os menos usados primeiro.
const CACHE_GB_DEFAULT: u64 = 20;

struct Paths {
    conf: PathBuf,
    mnt: PathBuf,
    cache: PathBuf,
    log: PathBuf,
}

static PATHS: OnceLock<Paths> = OnceLock::new();
type Emit = Box<dyn Fn(&str, Value) + Send + Sync>;
static EMIT: OnceLock<Emit> = OnceLock::new();

pub fn set_emitter(f: Emit) {
    let _ = EMIT.set(f);
}

pub fn emit(method: &str, v: Value) {
    if let Some(f) = EMIT.get() {
        f(method, v);
    }
}
static MOUNT: Mutex<Option<Child>> = Mutex::new(None);
static LOGIN: Mutex<Option<Login>> = Mutex::new(None);
/// "Cancelar" na tela de Abrindo… enquanto a ROM baixa da nuvem.
static FETCH_CANCEL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn cancel_fetch() {
    FETCH_CANCEL.store(true, std::sync::atomic::Ordering::SeqCst);
}

struct Login {
    child: Child,
    url: String,
}

pub fn init(data_dir: &Path, cache_dir: &Path, runtime_dir: &Path) {
    let _ = PATHS.set(Paths { conf: data_dir.join("rclone.conf"), mnt: runtime_dir.join("drive"), cache: cache_dir.join("drive"), log: cache_dir.join("logs").join("drive.log") });
}

fn p() -> &'static Paths {
    PATHS.get().expect("drive::init")
}

/// Pasta montada (a raiz `Nexus/` do Drive).
pub fn mount_dir() -> Option<PathBuf> {
    PATHS.get().map(|p| p.mnt.clone())
}

pub fn roms_root() -> Option<PathBuf> {
    mount_dir().map(|m| m.join("ROMs"))
}

fn bin() -> String {
    std::env::var("NEXUS_RCLONE").unwrap_or_else(|_| "rclone".into())
}

fn remote() -> String {
    std::env::var("NEXUS_DRIVE_REMOTE").unwrap_or_else(|_| REMOTE.into())
}

pub fn available() -> bool {
    Command::new(bin()).arg("version").stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

fn rclone(args: &[&str], secs: u64) -> Result<String, String> {
    let out = Command::new("timeout")
        .arg(secs.to_string())
        .arg(bin())
        .args(args)
        .arg("--config")
        .arg(&p().conf)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("rclone indisponível: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(friendly(&String::from_utf8_lossy(&out.stderr)))
    }
}

fn friendly(err: &str) -> String {
    let e = err.to_lowercase();
    if e.contains("invalid_grant") || e.contains("token expired") || e.contains("unauthorized") {
        "O acesso ao Google Drive expirou: entre de novo em Ajustes → Armazenamento".into()
    } else if e.contains("no such host") || e.contains("network is unreachable") || e.contains("dial tcp") || e.contains("i/o timeout") {
        "Sem internet para falar com o Google Drive".into()
    } else if e.contains("ratelimit") || e.contains("rate limit") || e.contains("403") {
        "O Google Drive limitou o acesso agora: tente de novo em alguns minutos".into()
    } else {
        let l = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("erro do rclone");
        format!("Google Drive: {}", l.trim().chars().take(160).collect::<String>())
    }
}

pub fn connected() -> bool {
    std::fs::read_to_string(&p().conf).map(|t| t.contains(&format!("[{REMOTE}]")) && t.contains("token")).unwrap_or(false)
}

pub fn mounted() -> bool {
    let m = p().mnt.display().to_string();
    std::fs::read_to_string("/proc/self/mounts").map(|t| t.lines().any(|l| l.split_whitespace().nth(1) == Some(m.as_str()))).unwrap_or(false)
}

// ---------- login ----------

/// Extrai o token do que o `rclone authorize` imprime entre "--->" e "<---End paste".
pub fn parse_token(out: &str) -> Option<String> {
    let a = out.find("--->")? + 4;
    let b = out[a..].find("<---")? + a;
    let t = out[a..b].trim();
    serde_json::from_str::<Value>(t).ok().filter(|v| v.get("access_token").is_some()).map(|_| t.to_string())
}

pub fn conf_text(token: &str) -> String {
    format!("[{REMOTE}]\ntype = drive\nscope = drive\ntoken = {token}\n")
}

/// Começa o login: devolve o endereço que a janela do navegador deve abrir. `done` é chamado no fim
/// (Ok = conectado; Err = mensagem).
pub fn login_start(done: impl FnOnce(Result<(), String>) + Send + 'static) -> Result<String, String> {
    login_cancel();
    let mut child = Command::new(bin())
        .args(["authorize", "drive", "--auth-no-open-browser"])
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("rclone indisponível: {e}"))?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let all = std::sync::Arc::new(Mutex::new(String::new()));
    for (r, tx) in [(Box::new(stdout) as Box<dyn Read + Send>, tx.clone()), (Box::new(stderr) as Box<dyn Read + Send>, tx)] {
        let all = all.clone();
        std::thread::spawn(move || {
            for l in BufReader::new(r).lines().map_while(Result::ok) {
                all.lock().unwrap_or_else(|e| e.into_inner()).push_str(&(l.clone() + "\n"));
                let _ = tx.send(l);
            }
        });
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    let url = loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(l) => {
                if let Some(i) = l.find("http://127.0.0.1:") {
                    break l[i..].split_whitespace().next().unwrap_or("").to_string();
                }
            }
            Err(_) => {
                let _ = child.kill();
                return Err("O rclone não respondeu ao começar o login".into());
            }
        }
    };
    *LOGIN.lock().unwrap_or_else(|e| e.into_inner()) = Some(Login { child, url: url.clone() });
    std::thread::spawn(move || {
        // espera o fim (o usuário autoriza ou cancela); o processo é do LOGIN para o cancelamento poder matá-lo
        let code = loop {
            std::thread::sleep(Duration::from_millis(300));
            let mut g = LOGIN.lock().unwrap_or_else(|e| e.into_inner());
            let Some(l) = g.as_mut() else { break None };
            if let Ok(Some(st)) = l.child.try_wait() {
                *g = None;
                break Some(st.success());
            }
        };
        std::thread::sleep(Duration::from_millis(200)); // últimas linhas
        let out = all.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let r = match (code, parse_token(&out)) {
            (Some(true), Some(tok)) => save_token(&tok),
            (None, _) => Err("Login cancelado".into()),
            _ => Err("O Google não autorizou o acesso".into()),
        };
        done(r);
    });
    Ok(url)
}

pub fn login_url() -> Option<String> {
    LOGIN.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|l| l.url.clone())
}

pub fn login_cancel() {
    if let Some(mut l) = LOGIN.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = l.child.kill();
        let _ = l.child.wait();
    }
}

fn save_token(tok: &str) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new().create(true).write(true).truncate(true).mode(0o600).open(&p().conf).map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut f, conf_text(tok).as_bytes()).map_err(|e| e.to_string())
}

// ---------- pasta e montagem ----------

/// Procura a pasta `Nexus` na raiz; sem ela, cria. Devolve true se acabou de criar.
pub fn ensure_folder() -> Result<bool, String> {
    let r = remote();
    let list = rclone(&["lsjson", &format!("{r}:"), "--dirs-only"], 60)?;
    let found = serde_json::from_str::<Vec<Value>>(&list).unwrap_or_default().iter().any(|d| d["Name"].as_str() == Some(FOLDER));
    if !found {
        rclone(&["mkdir", &format!("{r}:{FOLDER}")], 60)?;
    }
    Ok(!found)
}

pub fn cache_gb(db: &crate::db::Db) -> u64 {
    db.kv_get("drive_cache_gb").and_then(|v| v.parse().ok()).filter(|g: &u64| (2..=500).contains(g)).unwrap_or(CACHE_GB_DEFAULT)
}

pub fn mount_args(cache_gb: u64) -> Vec<String> {
    let pp = p();
    vec![
        "mount".into(),
        format!("{}:{FOLDER}", remote()),
        pp.mnt.display().to_string(),
        "--config".into(),
        pp.conf.display().to_string(),
        "--vfs-cache-mode".into(),
        "full".into(),
        "--vfs-cache-max-size".into(),
        format!("{cache_gb}G"),
        "--vfs-cache-max-age".into(),
        "2160h".into(),
        "--cache-dir".into(),
        pp.cache.display().to_string(),
        "--dir-cache-time".into(),
        "10m".into(),
        "--poll-interval".into(),
        "1m".into(),
        "--vfs-read-ahead".into(),
        "64M".into(),
        "--buffer-size".into(),
        "8M".into(),
        "--low-level-retries".into(),
        "3".into(),
        "--log-file".into(),
        pp.log.display().to_string(),
        "--log-level".into(),
        "NOTICE".into(),
    ]
}

fn fusermount_u() {
    for b in ["fusermount3", "fusermount"] {
        if Command::new(b).args(["-uz"]).arg(&p().mnt).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok() {
            return;
        }
    }
}

/// Monta (se conectado e ainda não montado) e cria as subpastas. Espera até 15 s a montagem aparecer.
pub fn mount(cache_gb: u64) -> Result<(), String> {
    if !connected() {
        return Err("Google Drive não conectado".into());
    }
    if mounted() {
        return Ok(());
    }
    let pp = p();
    fusermount_u(); // montagem velha ("Transport endpoint is not connected")
    std::fs::create_dir_all(&pp.mnt).map_err(|e| e.to_string())?;
    let _ = std::fs::create_dir_all(&pp.cache);
    if let Some(d) = pp.log.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let mut cmd = Command::new(bin());
    // GOGC=50 + buffer de 8 MB: ~67 MB de memória em vez de ~88 MB (medido lendo 50 MB); quem segura a
    // velocidade pela rede é o --vfs-read-ahead
    cmd.args(mount_args(cache_gb)).env("LC_ALL", "C").env("GOGC", "50").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    // a montagem morre junto com a Core (sem ficar uma pasta montada órfã)
    unsafe {
        std::os::unix::process::CommandExt::pre_exec(&mut cmd, || {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            Ok(())
        });
    }
    let child = cmd.spawn().map_err(|e| format!("rclone indisponível: {e}"))?;
    *MOUNT.lock().unwrap_or_else(|e| e.into_inner()) = Some(child);
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(15) {
        if mounted() {
            for d in SUBDIRS {
                let _ = std::fs::create_dir_all(pp.mnt.join(d));
            }
            let roms = pp.mnt.join("ROMs");
            for s in crate::providers::emu::SYSTEMS {
                let _ = std::fs::create_dir(roms.join(s.dir));
            }
            return Ok(());
        }
        if let Some(c) = MOUNT.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            if let Ok(Some(_)) = c.try_wait() {
                let log = std::fs::read_to_string(&pp.log).unwrap_or_default();
                return Err(friendly(&log.lines().rev().take(3).collect::<Vec<_>>().join("\n")));
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err("O Google Drive não montou a tempo".into())
}

pub fn unmount() {
    if let Some(mut c) = MOUNT.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = c.kill();
        let _ = c.wait();
    }
    if PATHS.get().is_some() {
        fusermount_u();
    }
}

/// Sai da conta: desmonta, apaga o token e o cache local.
pub fn disconnect() {
    login_cancel();
    unmount();
    let pp = p();
    let _ = std::fs::remove_file(&pp.conf);
    let _ = std::fs::remove_dir_all(&pp.cache);
}

fn dir_size(d: &Path) -> u64 {
    let mut n = 0;
    let mut st = vec![d.to_path_buf()];
    while let Some(x) = st.pop() {
        for e in std::fs::read_dir(&x).into_iter().flatten().flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => st.push(e.path()),
                Ok(t) if t.is_file() => n += e.metadata().map(|m| m.len()).unwrap_or(0),
                _ => {}
            }
        }
    }
    n
}

/// Espaço da conta (`rclone about`): {total, used, free} em bytes, quando o Drive informa.
pub fn about() -> Option<Value> {
    let out = rclone(&["about", &format!("{}:", remote()), "--json"], 20).ok()?;
    serde_json::from_str(&out).ok()
}

pub fn status(db: &crate::db::Db, with_about: bool) -> Value {
    let av = available();
    let conn = av && connected();
    let mnt = conn && mounted();
    let cache = PATHS.get().map(|pp| dir_size(&pp.cache.join("vfs"))).unwrap_or(0);
    json!({
        "available": av,
        "connected": conn,
        "mounted": mnt,
        "folder": FOLDER,
        "loggingIn": login_url().is_some(),
        "cacheBytes": cache,
        "cacheGb": cache_gb(db),
        "about": if conn && with_about { about() } else { None },
    })
}

/// A ROM está na nuvem (pasta montada)?
pub fn is_cloud(path: &Path) -> bool {
    mount_dir().map(|m| path.starts_with(m)).unwrap_or(false)
}

/// Lê o arquivo inteiro pela montagem (o rclone guarda no cache local), avisando o progresso a cada ~1%.
pub fn prefetch(path: &Path, progress: &dyn Fn(u64, u64)) -> Result<(), String> {
    let total = std::fs::metadata(path).map(|m| m.len()).map_err(|e| friendly(&e.to_string()))?;
    let mut f = std::fs::File::open(path).map_err(|e| friendly(&e.to_string()))?;
    let mut buf = vec![0u8; 4 << 20];
    let mut done = 0u64;
    let mut last = 0u64;
    FETCH_CANCEL.store(false, std::sync::atomic::Ordering::SeqCst);
    progress(0, total);
    loop {
        if FETCH_CANCEL.swap(false, std::sync::atomic::Ordering::SeqCst) {
            return Err("Download cancelado".into());
        }
        let n = f.read(&mut buf).map_err(|e| friendly(&e.to_string()))?;
        if n == 0 {
            break;
        }
        done += n as u64;
        if done - last >= (total / 100).max(1 << 20) || done == total {
            last = done;
            progress(done, total);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_parsing_and_conf() {
        let out = "NOTICE: Waiting for code...\nNOTICE: Got code\nPaste the following into your remote machine --->\n{\"access_token\":\"ya29.x\",\"token_type\":\"Bearer\",\"refresh_token\":\"1//y\",\"expiry\":\"2026-11-12T21:00:00Z\"}\n<---End paste\n";
        let t = parse_token(out).unwrap();
        assert!(t.starts_with('{') && t.contains("refresh_token"));
        assert!(parse_token("Paste ---> {\"error\":1} <---End").is_none());
        assert!(parse_token("nada").is_none());
        let c = conf_text(&t);
        assert!(c.starts_with("[nexus-drive]\ntype = drive\nscope = drive\ntoken = {"));
    }

    #[test]
    fn friendly_errors() {
        assert!(friendly("oauth2: cannot fetch token: 400 Bad Request\nResponse: {\"error\": \"invalid_grant\"}").contains("expirou"));
        assert!(friendly("dial tcp: lookup www.googleapis.com: no such host").contains("internet"));
        assert!(friendly("x\nalgo deu errado\n").contains("algo deu errado"));
    }
}
