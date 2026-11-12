//! Sobre e diagnóstico (B16): modelo, versões, CPU, memória, GPU, temperatura, throttling (Raspberry Pi), uso agora,
//! tempo ligado, quem mais consome e o "relatório de suporte" (um arquivo de texto com logs e informações, sem
//! segredos). Programas trocáveis: `NEXUS_VCGENCMD`, `NEXUS_PS`, `NEXUS_JOURNALCTL`.
use crate::sysops::{bin, run};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn rd(p: &str) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

/// Modelo do aparelho: device tree (Raspberry Pi e outras placas ARM) ou DMI (PC).
pub fn model() -> String {
    let dt = rd("/sys/firmware/devicetree/base/model").trim_matches(|c: char| c == '\0' || c.is_whitespace()).to_string();
    if !dt.is_empty() {
        return dt;
    }
    let v = rd("/sys/class/dmi/id/sys_vendor").trim().to_string();
    let n = rd("/sys/class/dmi/id/product_name").trim().to_string();
    [v, n].iter().filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("to be filled by o.e.m.")).cloned().collect::<Vec<_>>().join(" ")
}

/// (nome da CPU, núcleos) do /proc/cpuinfo.
pub fn parse_cpu(info: &str) -> (String, usize) {
    let cores = info.lines().filter(|l| l.starts_with("processor")).count();
    let get = |k: &str| info.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_once(':')).map(|(_, v)| v.trim().to_string()).filter(|v| !v.is_empty());
    // x86: "model name"; ARM: "Model"/"Hardware" (nomes de placa) ou "CPU part" sem nome legível
    (get("model name").or_else(|| get("Hardware")).or_else(|| get("Model")).unwrap_or_else(|| "Processador".into()), cores.max(1))
}

/// (total, disponível) em MB.
pub fn parse_mem(m: &str) -> (u64, u64) {
    let kb = |k: &str| m.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0) / 1024;
    (kb("MemTotal:"), kb("MemAvailable:"))
}

fn cpu_times() -> (u64, u64) {
    let s = rd("/proc/stat");
    let v: Vec<u64> = s.lines().next().unwrap_or("").split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
    let idle = v.get(3).copied().unwrap_or(0) + v.get(4).copied().unwrap_or(0);
    (v.iter().sum(), idle)
}

/// Uso de CPU agora (%), medindo por um instante.
fn cpu_usage() -> u8 {
    let (t0, i0) = cpu_times();
    std::thread::sleep(std::time::Duration::from_millis(250));
    let (t1, i1) = cpu_times();
    let dt = t1.saturating_sub(t0);
    if dt == 0 { 0 } else { (100 * dt.saturating_sub(i1.saturating_sub(i0)) / dt).min(100) as u8 }
}

/// Maior temperatura entre as zonas térmicas (°C).
fn temperature() -> Option<f32> {
    let mut best: Option<f32> = None;
    for e in std::fs::read_dir("/sys/class/thermal").ok()?.flatten() {
        if !e.file_name().to_string_lossy().starts_with("thermal_zone") {
            continue;
        }
        if let Some(t) = std::fs::read_to_string(e.path().join("temp")).ok().and_then(|s| s.trim().parse::<f32>().ok()) {
            let c = t / 1000.0;
            if c > 0.0 && c < 150.0 && best.is_none_or(|b| c > b) {
                best = Some(c);
            }
        }
    }
    best
}

/// `throttled=0x50005` → (problemas agora, problemas desde que ligou), em português.
pub fn parse_throttled(out: &str) -> Option<(Vec<&'static str>, Vec<&'static str>)> {
    let hex = out.trim().strip_prefix("throttled=0x")?;
    let v = u32::from_str_radix(hex, 16).ok()?;
    let names = ["Tensão baixa (fonte fraca)", "Frequência limitada", "Desempenho reduzido pelo calor ou pela energia", "Limite de temperatura"];
    let pick = |shift: u32| names.iter().enumerate().filter(|(i, _)| v & (1 << (*i as u32 + shift)) != 0).map(|(_, n)| *n).collect::<Vec<_>>();
    Some((pick(0), pick(16)))
}

/// `ps -eo comm=,pcpu=,rss=` ordenado → [{name, cpu, memMb}]
pub fn parse_ps(out: &str, n: usize) -> Vec<Value> {
    out.lines()
        .filter_map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            let (rss, cpu) = (w.last()?.parse::<u64>().ok()?, w.get(w.len().checked_sub(2)?)?.parse::<f32>().ok()?);
            let name = w[..w.len() - 2].join(" ");
            (!name.is_empty()).then(|| json!({ "name": name, "cpu": (cpu as f64 * 10.0).round() / 10.0, "memMb": rss / 1024 }))
        })
        .take(n)
        .collect()
}

fn top_processes() -> Vec<Value> {
    let (_, out, _) = run(&bin("NEXUS_PS", "ps"), &["-eo", "comm=,pcpu=,rss=", "--sort=-pcpu"], 5);
    parse_ps(&out, 6)
}

pub fn status(data_dir: &Path) -> Value {
    let (cpu, cores) = parse_cpu(&rd("/proc/cpuinfo"));
    let (mem, avail) = parse_mem(&rd("/proc/meminfo"));
    let hw = crate::hardware::detect(vec![], data_dir);
    let load: Vec<String> = rd("/proc/loadavg").split_whitespace().take(3).map(String::from).collect();
    let up = rd("/proc/uptime").split_whitespace().next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) as u64;
    let (_, thr, _) = run(&bin("NEXUS_VCGENCMD", "vcgencmd"), &["get_throttled"], 3);
    let throttle = parse_throttled(&thr);
    json!({
        "model": model(),
        "cpu": cpu,
        "cores": cores,
        "arch": hw.architecture,
        "memoryMb": mem,
        "memoryUsedMb": mem.saturating_sub(avail),
        "gpu": hw.gpu_vendor,
        "tier": hw.performance_tier,
        "os": hw.os,
        "kernel": rd("/proc/sys/kernel/osrelease").trim(),
        "nexus": crate::updates::installed_version().unwrap_or_else(|| crate::updates::running_version().to_string()),
        "diskFreeBytes": hw.disk_free_bytes,
        "diskTotalBytes": hw.disk_total_bytes,
        "tempC": temperature(),
        "throttle": throttle.map(|(now, past)| json!({ "now": now, "past": past })),
        "cpuPct": cpu_usage(),
        "load": load,
        "uptimeSecs": up,
        "top": top_processes(),
    })
}

/// Tira códigos de pareamento e chaves de textos de log.
pub fn redact(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    'outer: while !rest.is_empty() {
        for key in ["?t=", "&t=", "token=", "key=", "pin="] {
            if let Some(r) = rest.strip_prefix(key) {
                out.push_str(key);
                out.push_str("***");
                rest = r.trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-');
                continue 'outer;
            }
        }
        let c = rest.chars().next().unwrap();
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

fn tail(path: &Path, lines: usize) -> String {
    let t = std::fs::read_to_string(path).unwrap_or_default();
    let v: Vec<&str> = t.lines().collect();
    v[v.len().saturating_sub(lines)..].join("\n")
}

/// Data e hora (UTC) como AAAAMMDD-HHMMSS, sem depender de crates de calendário.
pub fn stamp(secs: u64) -> String {
    let (days, rem) = ((secs / 86400) as i64, secs % 86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

pub fn reports_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("reports")
}

/// O relatório mais recente (nome, caminho, tamanho).
pub fn latest_report(data_dir: &Path) -> Option<(String, PathBuf, u64)> {
    let mut v: Vec<_> = std::fs::read_dir(reports_dir(data_dir)).ok()?.flatten().filter(|e| e.file_name().to_string_lossy().starts_with("nexus-relatorio-")).collect();
    v.sort_by_key(|e| e.file_name());
    let e = v.pop()?;
    Some((e.file_name().to_string_lossy().to_string(), e.path(), e.metadata().map(|m| m.len()).unwrap_or(0)))
}

/// Gera o relatório de suporte e guarda os 5 mais recentes. Devolve (nome, caminho, tamanho).
pub fn make_report(data_dir: &Path, extra: &Value) -> Result<(String, PathBuf, u64), String> {
    let dir = reports_dir(data_dir);
    std::fs::create_dir_all(&dir).map_err(|_| "Não foi possível criar a pasta de relatórios")?;
    let name = format!("nexus-relatorio-{}.txt", stamp(crate::sysops::now()));
    let st = status(data_dir);
    let mut t = String::new();
    t += "RELATÓRIO DE SUPORTE DO NEXUS\n(sem senhas, PINs nem códigos de pareamento)\n\n== Aparelho ==\n";
    for (label, k) in [("Modelo", "model"), ("Processador", "cpu"), ("Arquitetura", "arch"), ("Sistema", "os"), ("Kernel", "kernel"), ("Nexus", "nexus"), ("GPU", "gpu"), ("Desempenho", "tier")] {
        t += &format!("{label}: {}\n", st[k].as_str().unwrap_or("-"));
    }
    t += &format!("Memória: {} MB (em uso {} MB)\nTempo ligado: {} min\nTemperatura: {}\nCarga: {}\n", st["memoryMb"], st["memoryUsedMb"], st["uptimeSecs"].as_u64().unwrap_or(0) / 60, st["tempC"], st["load"]);
    if !st["throttle"].is_null() {
        t += &format!("Throttling: agora {} · desde que ligou {}\n", st["throttle"]["now"], st["throttle"]["past"]);
    }
    t += &format!("\n== Mais consomem ==\n{}\n", st["top"].as_array().map(|a| a.iter().map(|p| format!("{} · cpu {}% · {} MB", p["name"].as_str().unwrap_or("?"), p["cpu"], p["memMb"])).collect::<Vec<_>>().join("\n")).unwrap_or_default());
    if !extra.is_null() {
        t += &format!("\n== Outros ==\n{}\n", serde_json::to_string_pretty(extra).unwrap_or_default());
    }
    let jc = bin("NEXUS_JOURNALCTL", "journalctl");
    let (_, j, _) = run(&jc, &["--user", "-u", "nexus-core", "-u", "nexus-shell", "-n", "300", "--no-pager"], 15);
    t += &format!("\n== Registro do Nexus (últimas 300 linhas) ==\n{}\n", j.trim());
    let state = std::env::var("XDG_STATE_HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".local/state"));
    t += &format!("\n== Sessão (session.log, últimas 200 linhas) ==\n{}\n", tail(&state.join("nexus/session.log"), 200));
    let (_, k, _) = run(&jc, &["-k", "-n", "100", "--no-pager"], 15);
    t += &format!("\n== Kernel (últimas 100 linhas) ==\n{}\n", k.trim());
    std::fs::write(dir.join(&name), redact(&t)).map_err(|_| "Não foi possível gravar o relatório")?;
    // guarda só os 5 mais recentes
    let mut all: Vec<_> = std::fs::read_dir(&dir).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    all.sort();
    while all.len() > 5 {
        let _ = std::fs::remove_file(all.remove(0));
    }
    let path = dir.join(&name);
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    Ok((name, path, size))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses() {
        let (n, c) = parse_cpu("processor\t: 0\nmodel name\t: Intel(R) Core(TM) i5\nprocessor\t: 1\nmodel name\t: Intel(R) Core(TM) i5\n");
        assert_eq!((n.as_str(), c), ("Intel(R) Core(TM) i5", 2));
        let (n, c) = parse_cpu("processor : 0\nprocessor : 1\nprocessor : 2\nprocessor : 3\nHardware : BCM2712\nModel : Raspberry Pi 5\n");
        assert_eq!((n.as_str(), c), ("BCM2712", 4));
        assert_eq!(parse_mem("MemTotal: 8192000 kB\nMemFree: 1 kB\nMemAvailable: 4096000 kB\n"), (8000, 4000));
        let (now, past) = parse_throttled("throttled=0x50005\n").unwrap();
        assert_eq!(now, vec!["Tensão baixa (fonte fraca)", "Desempenho reduzido pelo calor ou pela energia"]);
        assert_eq!(past.len(), 2);
        assert_eq!(parse_throttled("throttled=0x0").unwrap().0.len(), 0);
        assert!(parse_throttled("comando não encontrado").is_none());
        let ps = parse_ps("sway 12.5 90000\nnexus-shell 3.0 200000\nEmulation Station 1.0 1024\nbad\n", 2);
        assert_eq!(ps[0]["name"], "sway");
        assert_eq!(ps[1]["memMb"], 195);
        assert_eq!(parse_ps("Web Content 1.0 2048\n", 5)[0]["name"], "Web Content");
    }
    #[test]
    fn stamp_is_utc_date() {
        assert_eq!(stamp(0), "19700101-000000");
        assert_eq!(stamp(1_700_000_000), "20231114-221320");
        assert_eq!(stamp(951_782_400 + 86399), "20000229-235959");
    }
    #[test]
    fn redacts_tokens() {
        assert_eq!(redact("GET /?t=abc123_x-Y ok"), "GET /?t=*** ok");
        assert_eq!(redact("a&t=zzz&b=1 token=deadbeef fim"), "a&t=***&b=1 token=*** fim");
        assert_eq!(redact("sem segredo ação"), "sem segredo ação");
    }
    #[test]
    fn report_is_written_and_capped() {
        let d = tempfile::tempdir().unwrap();
        let (name, p, size) = make_report(d.path(), &Value::Null).unwrap();
        assert!(name.starts_with("nexus-relatorio-") && p.is_file() && size > 100);
        let body = std::fs::read_to_string(&p).unwrap();
        assert!(body.contains("RELATÓRIO DE SUPORTE") && body.contains("== Aparelho =="));
        for i in 0..7 {
            std::fs::write(reports_dir(d.path()).join(format!("nexus-relatorio-2000010{i}-000000.txt")), "x").unwrap();
        }
        make_report(d.path(), &Value::Null).unwrap();
        assert!(std::fs::read_dir(reports_dir(d.path())).unwrap().count() <= 5);
        assert!(latest_report(d.path()).is_some());
    }
}
