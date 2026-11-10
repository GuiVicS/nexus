//! Leitura do /proc para acompanhar processos de jogos (sessão, linha de comando, ambiente).
use std::fs;

#[derive(Debug, Clone)]
pub struct Proc {
    pub pid: u32,
    pub ppid: u32,
    pub sid: u32,
    pub state: char,
}

/// Interpreta /proc/<pid>/stat (o nome do processo pode conter espaços e parênteses).
pub fn parse_stat(stat: &str) -> Option<Proc> {
    let pid: u32 = stat.split_whitespace().next()?.parse().ok()?;
    let rest = &stat[stat.rfind(')')? + 1..];
    let mut f = rest.split_whitespace();
    let state = f.next()?.chars().next()?;
    let ppid = f.next()?.parse().ok()?;
    let _pgrp = f.next()?;
    let sid = f.next()?.parse().ok()?;
    Some(Proc { pid, ppid, sid, state })
}

pub fn list() -> Vec<Proc> {
    let Ok(rd) = fs::read_dir("/proc") else { return vec![] };
    rd.flatten()
        .filter(|e| e.file_name().to_string_lossy().bytes().all(|b| b.is_ascii_digit()))
        .filter_map(|e| parse_stat(&fs::read_to_string(e.path().join("stat")).ok()?))
        .collect()
}

pub fn alive(pid: u32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat")).ok().and_then(|s| parse_stat(&s)).is_some_and(|p| p.state != 'Z' && p.state != 'X')
}

pub fn cmdline(pid: u32) -> Vec<String> {
    fs::read(format!("/proc/{pid}/cmdline")).map(|b| b.split(|c| *c == 0).filter(|s| !s.is_empty()).map(|s| String::from_utf8_lossy(s).to_string()).collect()).unwrap_or_default()
}

pub fn environ_has(pid: u32, needle: &str) -> bool {
    fs::read(format!("/proc/{pid}/environ")).map(|b| b.split(|c| *c == 0).any(|e| e == needle.as_bytes())).unwrap_or(false)
}

/// Processos vivos da sessão (setsid) iniciada por `leader`.
pub fn session_members(leader: u32) -> Vec<u32> {
    let all: Vec<Proc> = list().into_iter().filter(|p| p.state != 'Z' && p.state != 'X').collect();
    // a sessão do jogo e todos os descendentes: Flatpak (bwrap) e navegadores abrem sessões próprias
    // lá dentro, e o SIGTERM precisa chegar ao processo de verdade (para o emulador gravar o save)
    let mut out: Vec<u32> = all.iter().filter(|p| p.sid == leader).map(|p| p.pid).collect();
    let mut i = 0;
    while i < out.len() {
        let parent = out[i];
        let kids: Vec<u32> = all.iter().filter(|p| p.ppid == parent && !out.contains(&p.pid)).map(|p| p.pid).collect();
        out.extend(kids);
        i += 1;
    }
    out
}

/// Processos da Steam que pertencem ao jogo `appid` (reaper `SteamLaunch AppId=N` ou ambiente SteamGameId=N).
pub fn steam_game_pids(appid: u32) -> Vec<u32> {
    let want = format!("AppId={appid}");
    let env = format!("SteamGameId={appid}");
    let me = std::process::id();
    list()
        .into_iter()
        .filter(|p| p.pid != me && p.state != 'Z' && p.state != 'X')
        .filter(|p| {
            let c = cmdline(p.pid);
            (c.iter().any(|a| a == &want) && c.iter().any(|a| a == "SteamLaunch")) || environ_has(p.pid, &env)
        })
        .map(|p| p.pid)
        .collect()
}

/// Jogo da Steam rodando (reaper "SteamLaunch AppId=N"), para o Core reencontrar após reiniciar.
pub fn running_steam_appid() -> Option<u32> {
    let me = std::process::id();
    list().into_iter().filter(|p| p.pid != me && p.state != 'Z').find_map(|p| {
        let c = cmdline(p.pid);
        if !c.iter().any(|a| a == "SteamLaunch") {
            return None;
        }
        c.iter().find_map(|a| a.strip_prefix("AppId=")?.parse::<u32>().ok())
    })
}

pub fn signal(pids: &[u32], sig: i32) {
    for p in pids {
        unsafe {
            libc::kill(*p as i32, sig);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stat_with_weird_name() {
        let p = parse_stat("1234 (my (weird) game) S 1 1234 1234 0 -1 4194560").unwrap();
        assert_eq!((p.pid, p.ppid, p.sid, p.state), (1234, 1, 1234, 'S'));
    }
    #[test]
    fn sees_itself() {
        let me = std::process::id();
        assert!(alive(me));
        assert!(list().iter().any(|p| p.pid == me));
        assert!(!cmdline(me).is_empty());
        assert!(!alive(4_000_000));
    }
}
