//! Privacidade (B19): o que o Nexus manda pela rede, o que guarda e como limpar. O Nexus não tem telemetria: nada é
//! enviado sobre o uso, os jogos ou o aparelho. A tela mostra a lista real de conexões (as que dependem de uma
//! escolha do usuário aparecem como ligadas ou desligadas).
use crate::core::Core;
use rusqlite::params;
use serde_json::{json, Value};

pub fn status(core: &Core, profile: &str) -> Value {
    let drive = crate::drive::connected();
    let (plays, recent): (i64, i64) = core
        .db
        .with(|c| c.query_row("SELECT COUNT(*), COALESCE(SUM(last_played IS NOT NULL),0) FROM playtime WHERE profile_id=?1", params![profile], |r| Ok((r.get(0)?, r.get(1)?))))
        .unwrap_or((0, 0));
    let art = dir_files(&core.cfg.cache_dir.join("art")) + dir_files(&core.cfg.cache_dir.join("web-icons"));
    let net = |name: &str, what: &str, when: &str, on: bool| json!({ "name": name, "what": what, "when": when, "on": on });
    json!({
        "telemetry": false,
        "connections": [
            net("Steam (loja e capas)", "Pede à Steam a vitrine da loja, descrições e imagens dos jogos. Não envia seu nome nem sua lista de jogos.", "Quando você abre a Loja ou um jogo", core.steam.is_some()),
            net("Sua biblioteca da Steam", "Só se você informou a chave da Web API: pede a lista dos seus jogos à Steam.", "Ao sincronizar a biblioteca", core.db.kv_get("steam_api_key").is_some()),
            net("GitHub (atualizações do Nexus)", "Pergunta qual é a versão mais nova do Nexus. Não envia nada sobre você.", "Só quando você procura atualizações", true),
            net("Atualizações de apps (apt, Flatpak, Snap)", "Os próprios gerenciadores consultam os servidores deles.", "A cada 6 horas", !core.cfg.demo),
            net("libretro (emuladores e capas de ROMs)", "Baixa o emulador do console e as capas dos jogos de console.", "Na primeira vez que um console é usado", core.cfg.emulation),
            net("Google Drive", "Só se você conectou: lê e grava a pasta Nexus do seu Drive.", "Enquanto o Drive estiver conectado", drive),
            net("Relógio da internet (NTP)", "O sistema acerta a hora sozinho.", "Se o ajuste automático estiver ligado", true),
            net("Celular como controle", "Fica só na sua rede local (Wi-Fi de casa); não passa pela internet.", "Enquanto houver celular conectado", true),
        ],
        "stores": [
            { "name": "Tempo de jogo e jogos recentes", "count": plays, "detail": format!("{recent} jogo(s) na lista de recentes deste perfil") },
            { "name": "Imagens baixadas (capas e ícones)", "count": art, "detail": "Guardadas só neste aparelho" },
            { "name": "Buscas", "count": 0, "detail": "O Nexus não guarda o que você busca" },
        ],
    })
}

fn dir_files(d: &std::path::Path) -> i64 {
    std::fs::read_dir(d).map(|r| r.flatten().count() as i64).unwrap_or(0)
}

/// Limpa o histórico do perfil (`recent`: tira os jogos da lista de recentes mantendo o tempo jogado; `playtime`: apaga
/// também o tempo jogado) ou as imagens baixadas (`cache`, voltam a ser baixadas quando precisar).
pub fn clear(core: &Core, profile: &str, what: &str) -> Result<u64, String> {
    match what {
        "recent" => core.db.with(|c| c.execute("UPDATE playtime SET last_played=NULL WHERE profile_id=?1", params![profile])).map(|n| n as u64).map_err(|_| "Não foi possível limpar".into()),
        "playtime" => core.db.with(|c| c.execute("DELETE FROM playtime WHERE profile_id=?1", params![profile])).map(|n| n as u64).map_err(|_| "Não foi possível limpar".into()),
        "cache" => {
            let mut n = 0;
            for d in ["art", "web-icons"] {
                let p = core.cfg.cache_dir.join(d);
                n += dir_files(&p) as u64;
                let _ = std::fs::remove_dir_all(&p);
            }
            Ok(n)
        }
        _ => Err("Opção inválida".into()),
    }
}
