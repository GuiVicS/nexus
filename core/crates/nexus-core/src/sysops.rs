//! Ações de sistema que precisam de root (atualizar o Nexus e o sistema, ligar/desligar o SSH), no mesmo modelo dos
//! apps (`appupdates.rs`): o Core inicia `nexus-sys@<ação>.service` com `systemctl start` e o polkit libera só essa
//! unidade; quem faz o trabalho é o ajudante `/usr/lib/nexus/nexus-sys-helper`, que valida a ação.
//! Variáveis de teste: `NEXUS_SYSTEMCTL` (programa), `NEXUS_SYS_OUT` (onde o ajudante deixa o resultado),
//! `NEXUS_SYS_HELPER` (roda o ajudante direto, sem systemd).
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub fn bin(env: &str, def: &str) -> String {
    std::env::var(env).unwrap_or_else(|_| def.into())
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Roda um programa com tempo limite (argv fixo, `LC_ALL=C`): (sucesso, stdout, stderr).
pub fn run(prog: &str, args: &[&str], secs: u32) -> (bool, String, String) {
    match Command::new("timeout").arg(secs.to_string()).arg(prog).args(args).env("LC_ALL", "C").stdin(Stdio::null()).output() {
        Ok(o) => (o.status.success(), String::from_utf8_lossy(&o.stdout).to_string(), String::from_utf8_lossy(&o.stderr).to_string()),
        Err(e) => (false, String::new(), e.to_string()),
    }
}

pub fn out_dir() -> PathBuf {
    PathBuf::from(bin("NEXUS_SYS_OUT", "/run/nexus-sys"))
}

/// Só estas ações existem (iguais às do ajudante e da regra do polkit).
pub fn action_ok(a: &str) -> bool {
    match a.split_once(':') {
        Some(("update-nexus", v)) => v == "latest" || version_ok(v),
        Some(_) => false,
        None => matches!(a, "upgrade-system" | "ssh-on" | "ssh-off"),
    }
}

pub fn version_ok(v: &str) -> bool {
    let p: Vec<&str> = v.split('.').collect();
    p.len() == 3 && p.iter().all(|x| !x.is_empty() && x.len() <= 6 && x.chars().all(|c| c.is_ascii_digit()))
}

fn verb(a: &str) -> &str {
    a.split(':').next().unwrap_or(a)
}

/// Última linha do andamento (log do ajudante), para mostrar "Baixando…".
pub fn progress(action: &str) -> String {
    let t = std::fs::read_to_string(out_dir().join(format!("{}.log", verb(action)))).unwrap_or_default();
    t.lines().rev().find(|l| !l.trim().is_empty()).map(|l| l.trim().chars().take(140).collect()).unwrap_or_default()
}

/// Pede ao ajudante root para executar a ação e espera o fim. Ok(detalhe) ou Err(mensagem em português).
pub fn run_sys(action: &str) -> Result<String, String> {
    if !action_ok(action) {
        return Err("Ação de sistema inválida".into());
    }
    let res = out_dir().join(verb(action));
    let _ = std::fs::remove_file(&res);
    let (ok, _, err) = match std::env::var("NEXUS_SYS_HELPER") {
        Ok(h) => run(&h, &[action], 3600),
        Err(_) => {
            let unit = format!("nexus-sys@{action}.service");
            match Command::new(bin("NEXUS_SYSTEMCTL", "systemctl")).args(["start", "--no-ask-password", &unit]).stdin(Stdio::null()).output() {
                Ok(o) => (o.status.success(), String::new(), String::from_utf8_lossy(&o.stderr).to_string()),
                Err(e) => (false, String::new(), e.to_string()),
            }
        }
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
    let e = err.to_lowercase();
    Err(if e.contains("not found") || e.contains("not-found") || e.contains("could not be found") {
        "O ajudante do Nexus não está instalado (reinstale o pacote do Nexus)".into()
    } else if e.contains("authentication") || e.contains("access denied") || e.contains("interactive") {
        "Sem permissão para o ajudante do Nexus (reinstale o pacote)".into()
    } else {
        "Não foi possível executar a ação no sistema".into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actions_are_whitelisted() {
        for a in ["ssh-on", "ssh-off", "upgrade-system", "update-nexus:latest", "update-nexus:0.5.0"] {
            assert!(action_ok(a), "{a}");
        }
        for a in ["", "ssh", "ssh-on:x", "update-nexus:", "update-nexus:1.2", "update-nexus:1.2.3;rm", "update-nexus:../1.2.3", "reboot", "update-nexus:1.2.3.4"] {
            assert!(!action_ok(a), "{a}");
        }
    }
    #[test]
    fn invalid_action_never_runs() {
        assert!(run_sys("rm -rf /").is_err());
    }
}
