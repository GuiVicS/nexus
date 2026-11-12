//! Fuso horário, hora automática e teclado. Fuso/NTP pelo systemd-timedated (`timedatectl`, polkit do Nexus);
//! teclado aplicado na hora no compositor (`swaymsg input type:keyboard xkb_layout …`) e guardado pelo Core, que
//! reaplica ao iniciar; também vai para o sistema (`localectl`) quando o polkit deixa.
//! `NEXUS_TIMEDATECTL`, `NEXUS_LOCALECTL` e `NEXUS_SWAYMSG` trocam os binários (testes).
use crate::db::Db;
use serde_json::{json, Value};
use std::process::{Command, Stdio};

/// Fusos mais usados por quem fala português, com nome amigável. O resto continua disponível pela lista completa.
pub const TIMEZONES: &[(&str, &str)] = &[
    ("America/Sao_Paulo", "Brasília (SP, RJ, MG, Sul, GO, DF, BA…)"),
    ("America/Manaus", "Amazonas (Manaus)"),
    ("America/Cuiaba", "Mato Grosso (Cuiabá)"),
    ("America/Campo_Grande", "Mato Grosso do Sul (Campo Grande)"),
    ("America/Porto_Velho", "Rondônia (Porto Velho)"),
    ("America/Boa_Vista", "Roraima (Boa Vista)"),
    ("America/Rio_Branco", "Acre (Rio Branco)"),
    ("America/Belem", "Pará e Amapá (Belém)"),
    ("America/Fortaleza", "Nordeste (Fortaleza, Natal, São Luís…)"),
    ("America/Recife", "Pernambuco (Recife)"),
    ("America/Noronha", "Fernando de Noronha"),
    ("Europe/Lisbon", "Portugal (Lisboa)"),
    ("Atlantic/Azores", "Açores"),
    ("Africa/Luanda", "Angola (Luanda)"),
    ("Africa/Maputo", "Moçambique (Maputo)"),
    ("America/New_York", "EUA – Leste (Nova York)"),
    ("America/Los_Angeles", "EUA – Pacífico (Los Angeles)"),
    ("Europe/London", "Reino Unido (Londres)"),
    ("UTC", "UTC (tempo universal)"),
];

/// Layouts de teclado: (id, layout xkb, variante, nome).
pub const KEYBOARDS: &[(&str, &str, &str, &str)] = &[
    ("br", "br", "", "Português (Brasil, ABNT2)"),
    ("us", "us", "", "Inglês (EUA)"),
    ("us-intl", "us", "intl", "Inglês (EUA, internacional com acentos)"),
    ("pt", "pt", "", "Português (Portugal)"),
    ("es", "es", "", "Espanhol"),
    ("latam", "latam", "", "Espanhol (América Latina)"),
    ("gb", "gb", "", "Inglês (Reino Unido)"),
    ("fr", "fr", "", "Francês"),
    ("de", "de", "", "Alemão"),
];

fn bin(env: &str, def: &str) -> String {
    std::env::var(env).unwrap_or_else(|_| def.into())
}

fn run(prog: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new("timeout").arg("10").arg(prog).args(args).env("LC_ALL", "C").stdin(Stdio::null()).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        let e = String::from_utf8_lossy(&out.stderr).to_lowercase();
        Err(if e.contains("access denied") || e.contains("not authorized") || e.contains("interactive authentication required") {
            "Sem permissão para mudar o relógio do sistema (falta a regra do polkit do Nexus)".into()
        } else if e.trim().is_empty() {
            "Não foi possível concluir".into()
        } else {
            e.trim().to_string()
        })
    }
}

pub fn parse_timedatectl(out: &str) -> (String, bool, bool) {
    let get = |k: &str| out.lines().find_map(|l| l.strip_prefix(k).and_then(|r| r.strip_prefix('='))).unwrap_or("").to_string();
    (get("Timezone"), get("NTP") == "yes", get("NTPSynchronized") == "yes")
}

pub fn keyboard_id(db: &Db) -> String {
    db.kv_get("keyboard").filter(|k| KEYBOARDS.iter().any(|x| x.0 == k)).unwrap_or_else(|| "br".into())
}

pub fn status(db: &Db) -> Value {
    let (tz, ntp, synced) = run(&bin("NEXUS_TIMEDATECTL", "timedatectl"), &["show"]).map(|o| parse_timedatectl(&o)).unwrap_or_default();
    json!({
        "timezone": tz,
        "ntp": ntp,
        "synced": synced,
        "keyboard": keyboard_id(db),
        "language": "pt-BR",
        "timezones": TIMEZONES.iter().map(|(id, name)| json!({ "id": id, "name": name })).collect::<Vec<_>>(),
        "keyboards": KEYBOARDS.iter().map(|(id, _, _, name)| json!({ "id": id, "name": name })).collect::<Vec<_>>(),
    })
}

/// Fuso no formato Região/Cidade (só letras, números, '_', '-', '+' e '/'), sem "..".
pub fn valid_tz(tz: &str) -> bool {
    !tz.is_empty() && tz.len() <= 64 && !tz.contains("..") && !tz.starts_with('/') && tz.chars().all(|c| c.is_ascii_alphanumeric() || "_-+/".contains(c))
}

pub fn set_timezone(tz: &str) -> Result<(), String> {
    if !valid_tz(tz) {
        return Err("Fuso inválido".into());
    }
    run(&bin("NEXUS_TIMEDATECTL", "timedatectl"), &["set-timezone", tz]).map(|_| ())
}

pub fn set_ntp(on: bool) -> Result<(), String> {
    run(&bin("NEXUS_TIMEDATECTL", "timedatectl"), &["set-ntp", if on { "true" } else { "false" }]).map(|_| ())
}

/// Aplica o layout no compositor (vale na hora, para a Shell e os jogos) e no sistema (melhor esforço).
pub fn apply_keyboard(id: &str) -> Result<(), String> {
    let (_, layout, variant, _) = KEYBOARDS.iter().find(|k| k.0 == id).ok_or("Teclado inválido")?;
    if std::env::var_os("SWAYSOCK").is_some() || std::env::var_os("NEXUS_SWAYMSG").is_some() {
        run(&bin("NEXUS_SWAYMSG", "swaymsg"), &["input", "type:keyboard", "xkb_layout", layout])?;
        run(&bin("NEXUS_SWAYMSG", "swaymsg"), &["input", "type:keyboard", "xkb_variant", if variant.is_empty() { "\"\"" } else { variant }])?;
    }
    let mut a = vec!["set-x11-keymap", layout, "pc105"];
    if !variant.is_empty() {
        a.push(variant);
    }
    let _ = run(&bin("NEXUS_LOCALECTL", "localectl"), &a); // sem permissão: fica só na sessão do Nexus
    Ok(())
}

pub fn set_keyboard(db: &Db, id: &str) -> Result<(), String> {
    apply_keyboard(id)?;
    db.kv_set("keyboard", id).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_validates() {
        let (tz, ntp, sync) = parse_timedatectl("Timezone=America/Sao_Paulo\nLocalRTC=no\nCanNTP=yes\nNTP=yes\nNTPSynchronized=no\n");
        assert_eq!((tz.as_str(), ntp, sync), ("America/Sao_Paulo", true, false));
        assert!(valid_tz("America/Sao_Paulo") && valid_tz("UTC") && valid_tz("Etc/GMT+3"));
        assert!(!valid_tz("../etc/passwd") && !valid_tz("/etc") && !valid_tz("a b") && !valid_tz(""));
        assert!(TIMEZONES.iter().all(|t| valid_tz(t.0)));
    }

    #[test]
    fn keyboard_is_remembered_and_validated() {
        let db = Db::memory().unwrap();
        assert_eq!(keyboard_id(&db), "br", "padrão ABNT2");
        std::env::set_var("NEXUS_LOCALECTL", "true");
        std::env::set_var("NEXUS_SWAYMSG", "true");
        set_keyboard(&db, "us-intl").unwrap();
        assert_eq!(keyboard_id(&db), "us-intl");
        assert!(set_keyboard(&db, "klingon").is_err());
        assert_eq!(keyboard_id(&db), "us-intl");
    }
}
