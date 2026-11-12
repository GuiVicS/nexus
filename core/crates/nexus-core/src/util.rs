use std::io::Read;

pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut b);
    }
    b
}

pub fn random_hex(n: usize) -> String {
    let mut s = String::new();
    for b in random_bytes::<32>().iter().take(n.div_ceil(2)) {
        s.push_str(&format!("{b:02x}"));
    }
    s.truncate(n);
    s
}

pub fn today() -> String {
    let secs = nexus_protocol::now_unix();
    let days = secs.div_euclid(86400);
    let (y, m, d) = civil(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// dias desde 1970-01-01 -> (ano, mês, dia) no calendário gregoriano (UTC)
fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn norm_text(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn civil_dates() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(19723), (2024, 1, 1));
    }
    #[test]
    fn normalizes() {
        assert_eq!(norm_text("Ação Épica"), "acao epica");
    }
    #[test]
    fn random_is_hex() {
        let s = random_hex(16);
        assert_eq!(s.len(), 16);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

/// Roda um comando e espera no máximo `limit`; passou disso, mata e devolve `false`. Para ferramentas que podem
/// travar (ex.: `grim` com a tela bloqueada) em caminhos que o usuário espera resposta na hora (menu rápido).
pub fn run_with_timeout(cmd: &mut std::process::Command, limit: std::time::Duration) -> bool {
    let Ok(mut child) = cmd.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn() else { return false };
    let t = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(st)) => return st.success(),
            Ok(None) if t.elapsed() < limit => std::thread::sleep(std::time::Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

#[cfg(test)]
mod timeout_tests {
    #[test]
    fn kills_what_hangs() {
        let t = std::time::Instant::now();
        assert!(!super::run_with_timeout(std::process::Command::new("sleep").arg("5"), std::time::Duration::from_millis(200)));
        assert!(t.elapsed() < std::time::Duration::from_secs(2));
        assert!(super::run_with_timeout(&mut std::process::Command::new("true"), std::time::Duration::from_secs(2)));
    }
}
