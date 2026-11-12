//! Recuperação (B15), atrás do PIN dos responsáveis: reiniciar a sessão, reparar a interface, voltar ao desktop no
//! próximo login (`force-desktop`), restaurar padrões, apagar os dados de um perfil e reset de fábrica.
//! Só o que o usuário da sessão pode fazer (systemd --user e o banco do Nexus): nada de root aqui.
//! `NEXUS_SYSTEMCTL` troca o programa (testes); `NEXUS_FORCE_DESKTOP` troca o arquivo da marca.
use crate::db::Db;
use crate::sysops::{bin, run};
use rusqlite::params;
use std::path::{Path, PathBuf};

pub fn force_desktop_file() -> PathBuf {
    match std::env::var("NEXUS_FORCE_DESKTOP") {
        Ok(p) => PathBuf::from(p),
        Err(_) => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config/nexus/force-desktop"),
    }
}

/// O próximo login volta ao desktop normal? (marca do usuário ou a de todo o sistema)
pub fn desktop_mode() -> bool {
    force_desktop_file().exists() || Path::new("/etc/nexus/force-desktop").exists()
}

pub fn set_desktop_mode(on: bool) -> Result<(), String> {
    let f = force_desktop_file();
    if on {
        if let Some(d) = f.parent() {
            std::fs::create_dir_all(d).map_err(|_| "Não foi possível gravar a marca de recuperação")?;
        }
        std::fs::write(&f, "voltar ao desktop no próximo login (Ajustes → Recuperação)\n").map_err(|_| "Não foi possível gravar a marca de recuperação")?;
        Ok(())
    } else {
        match std::fs::remove_file(&f) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err("Não foi possível remover a marca de recuperação".into()),
        }
    }
}

fn systemctl_user(args: &[&str]) -> Result<(), String> {
    let mut a = vec!["--user"];
    a.extend_from_slice(args);
    let (ok, _, _) = run(&bin("NEXUS_SYSTEMCTL", "systemctl"), &a, 20);
    if ok { Ok(()) } else { Err("O serviço do sistema não respondeu".into()) }
}

/// Reinicia toda a sessão do Nexus (compositor, Core e interface). O próprio Core cai junto.
pub fn restart_session() -> Result<(), String> {
    systemctl_user(&["restart", "--no-block", "nexus-session.target"])
}

/// Reinicia só a interface (Shell): corrige tela travada ou preta sem derrubar o Core nem o jogo.
pub fn repair_shell() -> Result<(), String> {
    let _ = systemctl_user(&["reset-failed", "nexus-shell.service"]);
    systemctl_user(&["restart", "--no-block", "nexus-shell.service"])
}

/// Volta as preferências de todos os perfis e os ajustes do aparelho (tela, margem, teclado, saída de som lembrada)
/// ao padrão. Não apaga perfis, jogos, PIN nem contas.
pub fn restore_defaults(db: &Db) -> Result<(), String> {
    db.with(|c| {
        c.execute("UPDATE profiles SET prefs='{}'", [])?;
        c.execute("DELETE FROM kv WHERE key IN ('display_margin','keyboard') OR key LIKE 'display_%' OR key LIKE 'audio_%'", [])?;
        Ok(())
    })
    .map_err(|_| "Não foi possível restaurar os padrões".to_string())
}

/// Apaga o histórico e as escolhas de UM perfil (tempo jogado, uso diário, preferências, favoritos, liberados),
/// mas mantém o perfil, o nome, o avatar e o PIN.
pub fn erase_profile_data(db: &Db, id: &str) -> Result<(), String> {
    db.with(|c| {
        c.execute("DELETE FROM playtime WHERE profile_id=?1", params![id])?;
        c.execute("DELETE FROM usage WHERE profile_id=?1", params![id])?;
        c.execute("UPDATE profiles SET prefs='{}', allowed='[]' WHERE id=?1", params![id])?;
        Ok(())
    })
    .map_err(|_| "Não foi possível apagar os dados do perfil".to_string())
}

/// Reset de fábrica: apaga perfis, PIN dos responsáveis, histórico, apps adicionados, controles, chave da Steam e
/// pareamento do celular. Mantém os jogos, as ROMs e a Steam (arquivos fora do banco do Nexus). O Nexus volta ao
/// assistente de primeiro uso.
pub fn factory_reset(db: &Db, data_dir: &Path) -> Result<(), String> {
    db.with(|c| {
        for t in ["profiles", "playtime", "usage", "installed", "custom_apps", "kv"] {
            c.execute(&format!("DELETE FROM {t}"), [])?;
        }
        Ok(())
    })
    .map_err(|_| "Não foi possível apagar os dados".to_string())?;
    for d in ["avatars", "reports"] {
        let _ = std::fs::remove_dir_all(data_dir.join(d));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn db_resets() {
        let db = Db::memory().unwrap();
        db.with(|c| {
            c.execute("INSERT INTO profiles(id,name,avatar,hue,kind,prefs,allowed,created_at) VALUES('p1','Ana','a',1,'adult','{\"vol\":9}','[\"x\"]',0)", [])?;
            c.execute("INSERT INTO playtime(profile_id,item_id,seconds) VALUES('p1','g',50)", [])?;
            c.execute("INSERT INTO usage(profile_id,day,seconds) VALUES('p1','2026-11-12',50)", []).map(|_| ())
        })
        .unwrap();
        db.kv_set("display_margin", "5").unwrap();
        db.kv_set("parental_pin", "x").unwrap();
        restore_defaults(&db).unwrap();
        assert!(db.kv_get("display_margin").is_none() && db.kv_get("parental_pin").is_some(), "só os ajustes, nunca o PIN");
        let prefs: String = db.with(|c| c.query_row("SELECT prefs FROM profiles", [], |r| r.get(0))).unwrap();
        assert_eq!(prefs, "{}");
        erase_profile_data(&db, "p1").unwrap();
        let n: i64 = db.with(|c| c.query_row("SELECT (SELECT COUNT(*) FROM playtime)+(SELECT COUNT(*) FROM usage)", [], |r| r.get(0))).unwrap();
        assert_eq!(n, 0);
        let still: i64 = db.with(|c| c.query_row("SELECT COUNT(*) FROM profiles", [], |r| r.get(0))).unwrap();
        assert_eq!(still, 1, "o perfil continua");
        factory_reset(&db, Path::new("/nonexistent")).unwrap();
        let all: i64 = db.with(|c| c.query_row("SELECT (SELECT COUNT(*) FROM profiles)+(SELECT COUNT(*) FROM kv)", [], |r| r.get(0))).unwrap();
        assert_eq!(all, 0);
    }
}
