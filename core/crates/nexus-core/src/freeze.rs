//! Shell congelada durante o jogo: com um jogo ou app na frente, a Shell (Electron) não aparece, mas continuaria
//! gastando CPU (timers, animações, coleta de lixo) e memória. O Core congela o cgroup dela (`cgroup.freeze`, o mesmo
//! que `systemctl --user freeze`) pouco depois de mostrar o jogo e descongela antes de mostrá-la de novo (F1, fim do
//! jogo). Congelar em vez de encerrar mantém o menu rápido instantâneo; a memória dela pode ser devolvida ao sistema
//! com `memory.reclaim` (com zram/swap, vai comprimida; sem, só as páginas de arquivo, que voltam do disco ao
//! descongelar).
//!
//! Só age quando a Shell roda como unit `nexus-shell.service` ao lado do Core (sessão real). `NEXUS_FREEZE_SHELL=0`
//! desliga; `NEXUS_SHELL_CGROUP` aponta outro cgroup (testes).
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

/// Espera depois de mostrar o jogo antes de congelar (a Shell termina a animação e grava o que precisa).
pub const FREEZE_DELAY: Duration = Duration::from_millis(1500);

static GEN: AtomicU64 = AtomicU64::new(0);
static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

fn enabled() -> bool {
    !std::env::var("NEXUS_FREEZE_SHELL").is_ok_and(|v| v == "0")
}

/// cgroup da Shell: irmão do cgroup do Core (`.../app.slice/nexus-core.service` → `.../app.slice/nexus-shell.service`).
pub fn shell_cgroup_from(self_cgroup: &str, root: &Path) -> Option<PathBuf> {
    let rel = self_cgroup.lines().find_map(|l| l.strip_prefix("0::"))?.trim();
    let parent = Path::new(rel).parent()?;
    let p = root.join(parent.strip_prefix("/").unwrap_or(parent)).join("nexus-shell.service");
    p.join("cgroup.freeze").exists().then_some(p)
}

fn path() -> Option<&'static Path> {
    PATH.get_or_init(|| {
        if let Some(p) = std::env::var_os("NEXUS_SHELL_CGROUP") {
            return Some(PathBuf::from(p));
        }
        let me = std::fs::read_to_string("/proc/self/cgroup").ok()?;
        shell_cgroup_from(&me, Path::new("/sys/fs/cgroup"))
    })
    .as_deref()
}

fn write(p: &Path, file: &str, v: &str) -> bool {
    std::fs::write(p.join(file), v).is_ok()
}

/// A Shell está congelada agora?
pub fn is_frozen() -> bool {
    path().and_then(|p| std::fs::read_to_string(p.join("cgroup.freeze")).ok()).is_some_and(|v| v.trim() == "1")
}

/// Descongela já (antes de mostrar a Shell) e cancela um congelamento agendado.
pub fn thaw() {
    GEN.fetch_add(1, Ordering::SeqCst);
    if let Some(p) = path() {
        if is_frozen() {
            write(p, "cgroup.freeze", "0");
            tracing::debug!("Shell descongelada");
        }
    }
}

/// Congela a Shell daqui a `delay`, se nada mostrar a Shell antes disso.
pub fn freeze_later(delay: Duration) {
    if !enabled() || path().is_none() {
        return;
    }
    let g = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        if GEN.load(Ordering::SeqCst) != g {
            return; // a Shell voltou para a frente nesse meio-tempo
        }
        let Some(p) = path() else { return };
        if write(p, "cgroup.freeze", "1") {
            tracing::debug!("Shell congelada durante o jogo");
            // devolve a memória dela ao sistema (limite alto: o kernel tira o que conseguir)
            if !std::env::var("NEXUS_RECLAIM_SHELL").is_ok_and(|v| v == "0") {
                let _ = write(p, "memory.reclaim", "2G");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_shell_next_to_the_core() {
        let root = tempfile::tempdir().unwrap();
        let app = root.path().join("user.slice/user-1000.slice/user@1000.service/app.slice");
        std::fs::create_dir_all(app.join("nexus-shell.service")).unwrap();
        std::fs::write(app.join("nexus-shell.service/cgroup.freeze"), "0").unwrap();
        let me = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/nexus-core.service\n";
        assert_eq!(shell_cgroup_from(me, root.path()), Some(app.join("nexus-shell.service")));
        // fora da sessão (Core num terminal): sem Shell ao lado, não faz nada
        assert_eq!(shell_cgroup_from("0::/user.slice/user-1000.slice/session-2.scope\n", root.path()), None);
    }
}
