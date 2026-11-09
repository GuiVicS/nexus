//! Despacho JSON-RPC 2.0. A Shell envia apenas IDs e intenções; toda permissão é decidida aqui.
use crate::accounts::{NewProfile, PinError, ProfilePatch};
use crate::core::Core;
use crate::game::LaunchError;
use crate::util::norm_text;
use nexus_protocol::*;
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Debug)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

pub const E_PIN_REQUIRED: i64 = 1001;
pub const E_PIN_INVALID: i64 = 1002;
pub const E_LOCKED: i64 = 1003;
pub const E_FORBIDDEN: i64 = 1004;
pub const E_BUSY: i64 = 1005;
pub const E_NOT_FOUND: i64 = 1006;
pub const E_LIMIT: i64 = 1007;
pub const E_UNSUPPORTED: i64 = 1008;
pub const E_SESSION: i64 = 1009;
pub const E_PARENTAL_PIN: i64 = 1010;
pub const E_FAILED: i64 = 1011;

impl RpcError {
    fn new(code: i64, msg: &str) -> RpcError {
        RpcError { code, message: msg.into(), data: None }
    }
    fn invalid(msg: &str) -> RpcError {
        RpcError::new(-32602, msg)
    }
}

impl From<PinError> for RpcError {
    fn from(e: PinError) -> RpcError {
        match e {
            PinError::Required => RpcError::new(E_PIN_REQUIRED, "PIN necessário"),
            PinError::Malformed => RpcError::new(E_PIN_INVALID, "PIN deve ter de 4 a 6 números"),
            PinError::Invalid { remaining } => RpcError { code: E_PIN_INVALID, message: "PIN incorreto".into(), data: Some(json!({ "remaining": remaining })) },
            PinError::Locked { secs } => RpcError { code: E_LOCKED, message: "Muitas tentativas".into(), data: Some(json!({ "secs": secs })) },
        }
    }
}

/// Erro de PIN com o escopo (profile = PIN do perfil, parental = PIN dos responsáveis) para a Shell saber o que pedir.
fn scoped(e: PinError, scope: &str) -> RpcError {
    let mut r = RpcError::from(e);
    let mut d = r.data.take().unwrap_or_else(|| json!({}));
    d["scope"] = json!(scope);
    r.data = Some(d);
    r
}

impl From<LaunchError> for RpcError {
    fn from(e: LaunchError) -> RpcError {
        match e {
            LaunchError::NoSession => RpcError::new(E_SESSION, "Nenhum perfil ativo"),
            LaunchError::Busy => RpcError::new(E_BUSY, "Já existe um jogo em execução"),
            LaunchError::NotFound => RpcError::new(E_NOT_FOUND, "Item não encontrado"),
            LaunchError::Forbidden => RpcError::new(E_FORBIDDEN, "Este perfil não pode abrir este item"),
            LaunchError::LimitReached => RpcError::new(E_LIMIT, "O tempo de hoje acabou"),
            LaunchError::Failed(m) => RpcError::new(E_FAILED, &m),
        }
    }
}

impl From<serde_json::Error> for RpcError {
    fn from(e: serde_json::Error) -> RpcError {
        RpcError::new(E_FAILED, &e.to_string())
    }
}

impl From<anyhow::Error> for RpcError {
    fn from(e: anyhow::Error) -> RpcError {
        use crate::providers::ProviderError;
        match e.downcast_ref::<ProviderError>() {
            Some(ProviderError::Unsupported) => RpcError::new(E_UNSUPPORTED, "Não suportado"),
            Some(ProviderError::NotFound) => RpcError::new(E_NOT_FOUND, "Item não encontrado"),
            _ => RpcError::new(E_FAILED, &e.to_string()),
        }
    }
}

type R = Result<Value, RpcError>;

fn s(v: &Value, k: &str) -> Result<String, RpcError> {
    v.get(k).and_then(|x| x.as_str()).map(String::from).ok_or_else(|| RpcError::invalid(&format!("falta '{k}'")))
}
fn os(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).map(String::from)
}
fn ok<T: serde::Serialize>(v: T) -> R {
    Ok(serde_json::to_value(v).unwrap_or(Value::Null))
}

fn need_session(core: &Core) -> Result<Profile, RpcError> {
    core.current_profile().ok_or_else(|| RpcError::new(E_SESSION, "Nenhum perfil ativo"))
}

/// Ação sensível: perfil infantil ativo ou alvo infantil exige o PIN dos responsáveis (quando configurado).
fn parental(core: &Core, v: &Value) -> Result<(), RpcError> {
    let pin = os(v, "parentPin");
    let grant = os(v, "grant");
    core.accounts.parental_ok(pin.as_deref(), grant.as_deref()).map_err(|e| scoped(e, "parental"))?;
    Ok(())
}

fn kid_session(core: &Core) -> bool {
    core.current_profile().is_some_and(|p| p.kind == ProfileKind::Kid)
}

pub async fn dispatch(core: &Arc<Core>, method: &str, v: Value) -> R {
    match method {
        "core.hello" => ok(core.nexus_hello()),
        "hardware.profile" => ok(crate::hardware::detect(core.provider_names(), &core.cfg.data_dir)),
        "system.power" => {
            let a = s(&v, "action")?;
            let cmd = match a.as_str() {
                "off" => "poweroff",
                "restart" => "reboot",
                "suspend" => "suspend",
                _ => return Err(RpcError::invalid("ação inválida")),
            };
            if core.cfg.dev {
                core.emit("system.power", json!({ "action": a, "simulated": true }));
                return Ok(json!({ "simulated": true }));
            }
            std::process::Command::new("systemctl").arg(cmd).spawn().map_err(|e| RpcError::new(E_FAILED, &e.to_string()))?;
            Ok(json!({ "simulated": false }))
        }

        // ---------- steam ----------
        "steam.account" => {
            let st = core.steam.as_ref().expect("steam provider");
            let root = st.root();
            let u = st.user();
            let n = core.library.read().unwrap().iter().filter(|i| i.provider == "steam").count() as u32;
            ok(SteamAccount { installed: root.is_some(), persona: u.as_ref().map(|u| u.persona.clone()), account: u.map(|u| u.account), library_games: n })
        }
        "steam.set_key" => {
            let p = need_session(core)?;
            if p.kind == ProfileKind::Kid {
                return Err(RpcError::new(E_FORBIDDEN, "Indisponível neste perfil"));
            }
            let key = s(&v, "key")?;
            if key.len() != 32 || !key.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(RpcError::invalid("a chave da Steam tem 32 caracteres (0-9, A-F)"));
            }
            core.db.kv_set("steam_api_key", &key)?;
            Ok(json!({ "ok": true }))
        }
        "steam.sync" => {
            need_session(core)?;
            let st = core.steam.as_ref().expect("steam provider");
            let n = st.sync_owned().await?;
            core.refresh_library().await;
            Ok(json!({ "count": n, "hasKey": st.api_key().is_some() }))
        }

        // ---------- perfis ----------
        "profiles.list" => ok(core.accounts.list()?),
        "parental.status" => ok(core.accounts.parental()),
        "parental.verify" => {
            let g = core.accounts.grant(os(&v, "pin").as_deref()).map_err(|e| scoped(e, "parental"))?;
            Ok(json!({ "grant": g }))
        }
        "parental.set" => {
            let new = os(&v, "newPin");
            core.accounts.set_parental(new.as_deref(), os(&v, "currentPin").as_deref()).map_err(|e| scoped(e, "parental"))?;
            ok(core.accounts.parental())
        }
        "parental.extend" => {
            parental(core, &v)?;
            let id = s(&v, "profileId")?;
            let min = v.get("minutes").and_then(|m| m.as_u64()).unwrap_or(15).clamp(1, 240) as u32;
            core.accounts.extend_today(&id, min);
            let p = core.accounts.get(&id).ok_or_else(|| RpcError::new(E_NOT_FOUND, "perfil não existe"))?;
            if core.current_profile().is_some_and(|c| c.id == id) {
                core.emit("session.changed", serde_json::to_value(core.session_info(p.clone()))?);
            }
            Ok(json!({ "remainingMin": core.accounts.remaining_min(&p) }))
        }
        "parental.allowed" => {
            if kid_session(core) {
                return Err(RpcError::new(E_FORBIDDEN, "Indisponível neste perfil"));
            }
            need_session(core)?;
            ok(core.accounts.allowed(&s(&v, "profileId")?))
        }
        "parental.allow" => {
            parental(core, &v)?;
            let pid = s(&v, "profileId")?;
            let item = s(&v, "itemId")?;
            let allow = v.get("allow").and_then(|b| b.as_bool()).unwrap_or(true);
            let list = core.accounts.set_allowed(&pid, &item, allow)?;
            if core.current_profile().is_some_and(|c| c.id == pid) {
                if let Some(p) = core.accounts.get(&pid) {
                    core.emit("session.changed", serde_json::to_value(core.session_info(p))?);
                }
            }
            ok(list)
        }
        "profiles.create" => {
            let kid = os(&v, "kind").as_deref() == Some("kid");
            let has_any = !core.accounts.list()?.is_empty();
            if kid && !core.accounts.parental().has_pin {
                return Err(RpcError::new(E_PARENTAL_PIN, "Defina o PIN dos responsáveis antes de criar um perfil infantil"));
            }
            if has_any {
                parental(core, &v)?;
            }
            let p = core.accounts.create(NewProfile {
                name: s(&v, "name")?,
                avatar: os(&v, "avatar").unwrap_or_else(|| "initial".into()),
                hue: v.get("hue").and_then(|h| h.as_u64()).unwrap_or(180) as u16 % 360,
                kid,
                pin: os(&v, "pin"),
                age_limit: v.get("ageLimit").and_then(|a| a.as_u64()).map(|a| a.min(18) as u8),
                daily_limit_min: v.get("dailyLimitMin").and_then(|a| a.as_u64()).map(|a| a.clamp(5, 720) as u32),
            })?;
            core.emit("profiles.changed", json!({}));
            ok(p)
        }
        "profiles.update" => {
            let id = s(&v, "id")?;
            let target = core.accounts.get(&id).ok_or_else(|| RpcError::new(E_NOT_FOUND, "perfil não existe"))?;
            if target.kind == ProfileKind::Kid || kid_session(core) {
                parental(core, &v)?;
            } else if target.has_pin {
                if let Err(e) = core.accounts.verify_profile_pin(&id, os(&v, "currentPin").as_deref()) {
                    parental(core, &v).map_err(|_| scoped(e, "profile"))?;
                }
            }
            let patch = ProfilePatch {
                name: os(&v, "name"),
                avatar: os(&v, "avatar"),
                hue: v.get("hue").and_then(|h| h.as_u64()).map(|h| h as u16 % 360),
                age_limit: v.get("ageLimit").map(|a| a.as_u64().map(|a| a.min(18) as u8)).filter(|_| target.kind == ProfileKind::Kid),
                daily_limit_min: v.get("dailyLimitMin").map(|a| a.as_u64().map(|a| a.clamp(5, 720) as u32)).filter(|_| target.kind == ProfileKind::Kid),
                pin: v.get("pin").map(|p| p.as_str().map(String::from)),
            };
            let p = core.accounts.update(&id, patch)?;
            core.emit("profiles.changed", json!({}));
            if core.current_profile().is_some_and(|c| c.id == id) {
                core.emit("session.changed", serde_json::to_value(core.session_info(p.clone()))?);
            }
            ok(p)
        }
        "profiles.delete" => {
            let id = s(&v, "id")?;
            let target = core.accounts.get(&id).ok_or_else(|| RpcError::new(E_NOT_FOUND, "perfil não existe"))?;
            if core.accounts.list()?.len() <= 1 {
                return Err(RpcError::new(E_FORBIDDEN, "É preciso manter ao menos um perfil"));
            }
            if core.current_profile().is_some_and(|c| c.id == id) {
                return Err(RpcError::new(E_BUSY, "Troque de perfil antes de excluir este"));
            }
            if core.accounts.parental().has_pin {
                parental(core, &v)?;
            } else if target.has_pin {
                core.accounts.verify_profile_pin(&id, os(&v, "pin").as_deref()).map_err(|e| scoped(e, "profile"))?;
            }
            core.accounts.delete(&id)?;
            core.emit("profiles.changed", json!({}));
            Ok(json!({ "ok": true }))
        }
        "profiles.select" => {
            let id = s(&v, "id")?;
            if kid_session(core) && core.current_profile().is_some_and(|c| c.id != id) {
                parental(core, &v)?;
            }
            core.accounts.get(&id).ok_or_else(|| RpcError::new(E_NOT_FOUND, "perfil não existe"))?;
            core.accounts.verify_profile_pin(&id, os(&v, "pin").as_deref()).map_err(|e| scoped(e, "profile"))?;
            *core.session.lock().unwrap() = Some(id.clone());
            let info = core.session_info(core.accounts.get(&id).unwrap());
            core.emit("session.changed", serde_json::to_value(&info)?);
            ok(info)
        }
        "session.current" => match core.current_profile() {
            Some(p) => ok(core.session_info(p)),
            None => Ok(Value::Null),
        },
        "session.logout" => {
            if kid_session(core) {
                parental(core, &v)?;
            }
            if core.game_info().is_some() {
                return Err(RpcError::new(E_BUSY, "Feche o jogo antes de trocar de perfil"));
            }
            *core.session.lock().unwrap() = None;
            core.emit("session.changed", Value::Null);
            Ok(json!({ "ok": true }))
        }
        "prefs.set" => {
            let p = need_session(core)?;
            let mut cur = serde_json::to_value(core.accounts.prefs(&p.id))?;
            if let (Some(c), Some(patch)) = (cur.as_object_mut(), v.get("patch").and_then(|p| p.as_object())) {
                for (k, val) in patch {
                    if c.contains_key(k) {
                        c.insert(k.clone(), val.clone());
                    }
                }
            }
            let prefs: Prefs = serde_json::from_value(cur).map_err(|e| RpcError::invalid(&e.to_string()))?;
            core.accounts.set_prefs(&p.id, &Prefs { vol: prefs.vol.min(100), ..prefs.clone() })?;
            ok(core.accounts.prefs(&p.id))
        }

        // ---------- biblioteca / loja ----------
        "library.list" => {
            let p = need_session(core)?;
            let items: Vec<Item> = core.library.read().unwrap().iter().filter(|i| core.visible_to(Some(&p), i)).cloned().map(|i| core.with_profile_data(&p, i)).collect();
            let total = items.len() as u32;
            ok(LibraryPage { items, total })
        }
        "library.get_many" => {
            let p = need_session(core)?;
            let ids: Vec<String> = v.get("ids").and_then(|i| i.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            let items: Vec<Item> = core.library.read().unwrap().iter().filter(|i| ids.contains(&i.id) && core.visible_to(Some(&p), i)).cloned().map(|i| core.with_profile_data(&p, i)).collect();
            ok(items)
        }
        "library.get" => {
            let p = need_session(core)?;
            let it = core.find_item(&s(&v, "id")?).await.ok_or_else(|| RpcError::new(E_NOT_FOUND, "Item não encontrado"))?;
            if !core.visible_to(Some(&p), &it) {
                return Err(RpcError::new(E_FORBIDDEN, "Este perfil não pode ver este item"));
            }
            ok(core.with_profile_data(&p, it))
        }
        "library.search" => {
            let p = need_session(core)?;
            let q = norm_text(os(&v, "q").unwrap_or_default().trim());
            let mut pool: Vec<Item> = core.library.read().unwrap().clone();
            if p.kind == ProfileKind::Adult {
                for prov in &core.providers {
                    for c in ["jogos", "classicos", "apps"] {
                        if let Ok(list) = prov.store(c).await {
                            for it in list {
                                if !pool.iter().any(|x| x.id == it.id) {
                                    pool.push(it);
                                }
                            }
                        }
                    }
                }
            }
            let res: Vec<Item> = pool
                .into_iter()
                .filter(|i| core.visible_to(Some(&p), i))
                .filter(|i| q.is_empty() || norm_text(&format!("{} {} {} {}", i.title, i.subtitle, i.category.clone().unwrap_or_default(), i.studio.clone().unwrap_or_default())).contains(&q))
                .map(|i| core.with_profile_data(&p, i))
                .take(60)
                .collect();
            ok(res)
        }
        "store.list" => {
            let p = need_session(core)?;
            if p.kind == ProfileKind::Kid {
                return Err(RpcError::new(E_FORBIDDEN, "A loja não está disponível neste perfil"));
            }
            let cat = os(&v, "category").unwrap_or_else(|| "jogos".into());
            let mut out = vec![];
            for prov in &core.providers {
                out.extend(prov.store(&cat).await.unwrap_or_default());
            }
            ok(out)
        }
        "store.wallet" => {
            need_session(core)?;
            match &core.demo {
                Some(d) => Ok(json!({ "balance": d.wallet(), "currency": "BRL" })),
                None => Ok(Value::Null),
            }
        }
        "social.friends" => {
            need_session(core)?;
            ok(core.demo.as_ref().map(|d| d.friends()).unwrap_or_default())
        }

        // ---------- ações em itens ----------
        "item.launch" => ok(core.launch_item(&s(&v, "id")?).await?),
        "item.install" | "item.cancel" | "item.uninstall" => {
            let p = need_session(core)?;
            if p.kind == ProfileKind::Kid {
                return Err(RpcError::new(E_FORBIDDEN, "Peça a um responsável para instalar"));
            }
            let id = s(&v, "id")?;
            let prov = core.provider_for(&id).ok_or_else(|| RpcError::new(E_NOT_FOUND, "Item não encontrado"))?;
            match method {
                "item.install" => {
                    prov.install(&id).await?;
                    core.pending.lock().unwrap().insert(id.clone(), std::time::Instant::now());
                    core.kick.notify_one();
                }
                "item.cancel" => prov.cancel(&id).await?,
                _ => {
                    prov.uninstall(&id).await?;
                    let mut prefs = core.accounts.prefs(&p.id);
                    prefs.pins.retain(|x| x != &id);
                    core.accounts.set_prefs(&p.id, &prefs)?;
                }
            }
            core.refresh_library().await;
            Ok(json!({ "ok": true }))
        }
        "game.current" => ok(core.game_info()),
        "game.close" => {
            need_session(core)?;
            let force = v.get("force").and_then(|f| f.as_bool()).unwrap_or(false);
            Ok(json!({ "closing": core.close_game(force).await }))
        }
        "game.show_nexus" => {
            core.compositor.show_shell();
            Ok(json!({ "ok": true }))
        }
        "game.resume" => {
            if core.game_info().is_some() {
                core.compositor.show_game();
            }
            Ok(json!({ "ok": true }))
        }

        // ---------- notificações ----------
        "notifications.list" => {
            if kid_session(core) {
                return ok(Vec::<Notification>::new());
            }
            ok(core.notifs.lock().unwrap().clone())
        }
        "notifications.clear" => {
            core.notifs.lock().unwrap().clear();
            Ok(json!({ "ok": true }))
        }

        // ---------- controle no celular ----------
        "remote.pairing" => {
            need_session(core)?;
            ok(core.hub.pairing())
        }
        "remote.rotate" => {
            need_session(core)?;
            if kid_session(core) {
                return Err(RpcError::new(E_FORBIDDEN, "Indisponível neste perfil"));
            }
            core.hub.rotate_token();
            ok(core.hub.pairing())
        }
        "remote.disconnect" => {
            need_session(core)?;
            core.hub.disconnect(&s(&v, "id")?);
            ok(core.hub.pairing())
        }
        _ => Err(RpcError::new(-32601, &format!("método desconhecido: {method}"))),
    }
}
