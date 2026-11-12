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

/// Navegar pelas imagens do disco: perfil adulto ativo, primeiro uso (sem perfis) ou, na tela de perfis, os responsáveis.
fn may_browse(core: &Core, v: &Value) -> Result<(), RpcError> {
    match core.current_profile() {
        Some(p) if p.kind == ProfileKind::Kid => Err(RpcError::new(E_FORBIDDEN, "Indisponível neste perfil")),
        Some(_) => Ok(()),
        None if core.accounts.list()?.is_empty() => Ok(()),
        None => parental(core, v),
    }
}

/// Avatar recebido da Shell: um dos prontos, a inicial ou uma foto já importada pelo Core.
fn avatar_arg(core: &Core, v: &Value) -> Result<Option<String>, RpcError> {
    match os(v, "avatar") {
        None => Ok(None),
        Some(a) if crate::avatars::valid(core, &a) => Ok(Some(a)),
        Some(_) => Err(RpcError::new(E_FAILED, "avatar inválido")),
    }
}

fn kid_session(core: &Core) -> bool {
    core.current_profile().is_some_and(|p| p.kind == ProfileKind::Kid)
}

pub async fn dispatch(core: &Arc<Core>, method: &str, v: Value) -> R {
    match method {
        "core.hello" => {
            // a Shell acabou de subir (ou reconectou): sem jogo aberto, ela é a única coisa a mostrar. Evita a TV
            // presa na área dos jogos (tela preta) se a Shell reiniciou enquanto outra janela tinha o foco.
            if core.game_info().is_none() && !core.steam_ui.load(std::sync::atomic::Ordering::SeqCst) {
                core.compositor.show_shell();
            }
            ok(core.nexus_hello())
        }
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
            // instalada = há dados (já foi aberta) ou o programa existe (primeira abertura pelo Nexus)
            ok(SteamAccount { installed: root.is_some() || st.client_available(), persona: u.as_ref().map(|u| u.persona.clone()), account: u.map(|u| u.account), library_games: n })
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
                avatar: avatar_arg(core, &v)?.unwrap_or_else(|| "initial".into()),
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
                avatar: avatar_arg(core, &v)?,
                hue: v.get("hue").and_then(|h| h.as_u64()).map(|h| h as u16 % 360),
                age_limit: v.get("ageLimit").map(|a| a.as_u64().map(|a| a.min(18) as u8)).filter(|_| target.kind == ProfileKind::Kid),
                daily_limit_min: v.get("dailyLimitMin").map(|a| a.as_u64().map(|a| a.clamp(5, 720) as u32)).filter(|_| target.kind == ProfileKind::Kid),
                pin: v.get("pin").map(|p| p.as_str().map(String::from)),
            };
            let p = core.accounts.update(&id, patch)?;
            if p.avatar != target.avatar {
                crate::avatars::forget(core, &target.avatar);
            }
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
            crate::avatars::forget(core, &target.avatar);
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
            core.set_session(Some(id.clone()));
            let info = core.session_info(core.accounts.get(&id).unwrap());
            core.emit("session.changed", serde_json::to_value(&info)?);
            core.adopt_running_game();
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
            core.set_session(None);
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
            let id = s(&v, "id")?;
            let found = match core.provider_for(&id) {
                Some(prov) => prov.get(&id).await.ok().flatten(),
                None => None,
            };
            let mut it = match found {
                Some(i) => i,
                None => core.find_item(&id).await.ok_or_else(|| RpcError::new(E_NOT_FOUND, "Item não encontrado"))?,
            };
            core.tag_update(&mut it);
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
            // jogos da loja da Steam pelo nome (adultos; os resultados já filtrados pelo termo)
            let mut res_store: Vec<Item> = vec![];
            if p.kind == ProfileKind::Adult {
                if let Some(st) = &core.steam {
                    res_store = st.search_store(&os(&v, "q").unwrap_or_default()).await;
                }
            }
            let mut res: Vec<Item> = pool
                .into_iter()
                .filter(|i| core.visible_to(Some(&p), i))
                .filter(|i| q.is_empty() || norm_text(&format!("{} {} {} {}", i.title, i.subtitle, i.category.clone().unwrap_or_default(), i.studio.clone().unwrap_or_default())).contains(&q))
                .map(|i| core.with_profile_data(&p, i))
                .take(60)
                .collect();
            for it in res_store {
                if res.len() >= 60 {
                    break;
                }
                if !res.iter().any(|x| x.id == it.id) && core.visible_to(Some(&p), &it) {
                    res.push(core.with_profile_data(&p, it));
                }
            }
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
            if !std::env::var("NEXUS_OFFLINE").is_ok_and(|v| v == "1") {
                // pré-carga das capas (a tela pede na ordem de exibição; a pré-carga começa pelo fim)
                crate::art::prefetch(core, out.iter().rev().map(|x| x.id.clone()).collect());
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
                    // A Steam pede confirmação num diálogo dela ("Install"). O usuário não deve ver a Steam: o overlay do
                    // Nexus cobre a tela (deixando os cliques passarem), o diálogo vem à frente por baixo da capa, o Core
                    // clica em "Install" e volta ao Nexus. Se o download não começa (ex.: falta espaço, botão desativado),
                    // clica em "Cancel" e avisa no Nexus.
                    if let (Some(st), Some(n)) = (core.steam.clone(), id.strip_prefix("steam:").map(String::from)) {
                        if core.compositor.name() == "sway" {
                            let c = core.clone();
                            let title = core.find_item(&id).await.map(|i| i.title).unwrap_or_default();
                            let item_id = id.clone();
                            std::thread::spawn(move || steam_confirm_install(c, st, n, item_id, title));
                        }
                    }
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
        "controllers.list" => {
            need_session(core)?;
            let db = core.db.clone();
            ok(tokio::task::spawn_blocking(move || crate::controllers::list(&db)).await.unwrap_or_default())
        }
        "controllers.actions" => Ok(crate::controllers::actions_json()),
        "apps.updates" => {
            need_session(core)?;
            let list: Vec<crate::appupdates::AppUpdate> = core.app_updates.lock().unwrap_or_else(|e| e.into_inner()).values().cloned().collect();
            let c = core.clone();
            let restorable = tokio::task::spawn_blocking(move || crate::appupdates::restorable(&c)).await.unwrap_or_default();
            let jobs: Vec<String> = core.app_jobs.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect();
            Ok(json!({ "updates": list, "restorable": restorable, "jobs": jobs }))
        }
        "apps.check_updates" => {
            let p = need_session(core)?;
            if p.kind == ProfileKind::Kid {
                return Err(RpcError::new(E_FORBIDDEN, "Indisponível neste perfil"));
            }
            ok(crate::appupdates::check_now(core).await)
        }
        "apps.upgrade" | "apps.restore" => {
            let p = need_session(core)?;
            if p.kind == ProfileKind::Kid {
                return Err(RpcError::new(E_FORBIDDEN, "Indisponível neste perfil"));
            }
            let id = s(&v, "id")?;
            let (c0, id0) = (core.clone(), id.clone());
            if tokio::task::spawn_blocking(move || crate::appupdates::is_dependency_item(&c0, &id0)).await.unwrap_or(false) {
                return Err(RpcError::new(E_FORBIDDEN, crate::appupdates::DEP_MSG));
            }
            if core.game.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|g| g.item_id == id) {
                return Err(RpcError::new(E_BUSY, "Feche o app antes de mudar a versão"));
            }
            if !core.app_jobs.lock().unwrap_or_else(|e| e.into_inner()).insert(id.clone()) {
                return Err(RpcError::new(E_BUSY, "Este app já está sendo atualizado"));
            }
            let restore = method == "apps.restore";
            let title = core.find_item(&id).await.map(|i| i.title).unwrap_or_else(|| "App".into());
            core.emit("apps.job", json!({ "id": id, "op": if restore { "restore" } else { "update" }, "state": "running" }));
            let c = core.clone();
            tokio::spawn(async move {
                let (c2, id2) = (c.clone(), id.clone());
                let r = tokio::task::spawn_blocking(move || if restore { crate::appupdates::restore(&c2, &id2) } else { crate::appupdates::update(&c2, &id2) }).await.unwrap_or_else(|e| Err(e.to_string()));
                c.app_jobs.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
                match &r {
                    Ok(ver) => {
                        let body = if ver.is_empty() { title.clone() } else { format!("{title} · versão {ver}") };
                        c.notify("check", if restore { "Versão anterior restaurada" } else { "App atualizado" }, &body, Some(&format!("hub:{id}")));
                    }
                    Err(e) => {
                        tracing::warn!("{} de {id}: {e}", if restore { "restauração" } else { "atualização" });
                        c.notify("alert", if restore { "Não foi possível restaurar" } else { "Não foi possível atualizar" }, &format!("{title} · {e}"), Some(&format!("hub:{id}")));
                    }
                }
                c.emit("apps.job", json!({ "id": id, "op": if restore { "restore" } else { "update" }, "state": if r.is_ok() { "done" } else { "failed" }, "error": r.as_ref().err() }));
                crate::appupdates::check_now(&c).await;
            });
            ok(json!({ "ok": true }))
        }
        "emu.status" => {
            need_session(core)?;
            let mut st = tokio::task::spawn_blocking(crate::providers::emu::systems_status).await.unwrap_or(Value::Null);
            // emulação desligada (NEXUS_EMULATION=0): como se o RetroArch não existisse (sem aba Emuladores)
            if !core.cfg.emulation {
                if let Some(o) = st.as_object_mut() {
                    o.insert("retroarch".into(), Value::Null);
                }
            }
            Ok(st)
        }
        "emu.fetch_cores" => {
            let p = need_session(core)?;
            if p.kind == ProfileKind::Kid {
                return Err(RpcError::new(E_FORBIDDEN, "Indisponível neste perfil"));
            }
            let all = v.get("all").and_then(|a| a.as_bool()).unwrap_or(false);
            Ok(tokio::task::spawn_blocking(move || crate::providers::emu::fetch_cores(all)).await.unwrap_or(Value::Null))
        }
        "controllers.save" | "controllers.delete" => {
            let p = need_session(core)?;
            if p.kind == ProfileKind::Kid {
                return Err(RpcError::new(E_FORBIDDEN, "Indisponível neste perfil"));
            }
            let id = s(&v, "id")?;
            let r = if method == "controllers.save" {
                let player = v.get("player").map(|p| p.as_u64().map(|n| n as u8));
                crate::controllers::save(&core.db, &id, os(&v, "nickname"), player).map(|r| serde_json::to_value(r).unwrap())
            } else {
                crate::controllers::delete(&core.db, &id).map(|_| json!({ "ok": true }))
            };
            r.map_err(|e| RpcError::new(E_FAILED, &e))
        }
        "controllers.capture" | "controllers.get_map" | "controllers.set_map" | "bt.status" | "bt.scan" | "bt.pair" | "bt.remove" => {
            let p = need_session(core)?;
            if p.kind == ProfileKind::Kid {
                return Err(RpcError::new(E_FORBIDDEN, "Indisponível neste perfil"));
            }
            let fail = |e: String| RpcError::new(E_FAILED, &e);
            match method {
                "controllers.get_map" => Ok(Value::Object(crate::controllers::get_map(&core.db, &s(&v, "id")?))),
                "controllers.capture" => {
                    let id = s(&v, "id")?;
                    let ms = v.get("timeoutMs").and_then(|t| t.as_u64()).unwrap_or(10_000).clamp(1000, 30_000);
                    let r = tokio::task::spawn_blocking(move || crate::controllers::capture(&id, std::time::Duration::from_millis(ms))).await.map_err(|e| fail(e.to_string()))?.map_err(fail)?;
                    Ok(json!({ "binding": r }))
                }
                "controllers.set_map" => {
                    let id = s(&v, "id")?;
                    let map = v.get("map").and_then(|m| m.as_object()).cloned().ok_or_else(|| RpcError::new(E_FAILED, "mapa ausente"))?;
                    let db = core.db.clone();
                    let files = tokio::task::spawn_blocking(move || crate::controllers::set_map(&db, &id, &map)).await.map_err(|e| fail(e.to_string()))?.map_err(fail)?;
                    Ok(json!({ "ok": true, "retroarch": files }))
                }
                "bt.status" => Ok(tokio::task::spawn_blocking(crate::controllers::bt_status).await.unwrap_or(Value::Null)),
                "bt.scan" => {
                    let secs = v.get("secs").and_then(|t| t.as_u64()).unwrap_or(10).clamp(3, 20);
                    Ok(tokio::task::spawn_blocking(move || crate::controllers::bt_scan(secs)).await.unwrap_or(Value::Null))
                }
                _ => {
                    parental(core, &v)?;
                    let mac = s(&v, "mac")?;
                    let pair = method == "bt.pair";
                    tokio::task::spawn_blocking(move || if pair { crate::controllers::bt_pair(&mac) } else { crate::controllers::bt_remove(&mac) }).await.map_err(|e| fail(e.to_string()))?.map_err(fail)?;
                    Ok(json!({ "ok": true }))
                }
            }
        }
        "roms.media" | "roms.mount" | "roms.browse" | "roms.import" | "roms.upload_link" => {
            let p = need_session(core)?;
            if p.kind == ProfileKind::Kid {
                return Err(RpcError::new(E_FORBIDDEN, "Peça a um responsável para adicionar jogos"));
            }
            let fail = |e: String| RpcError::new(E_FAILED, &e);
            match method {
                "roms.media" => Ok(tokio::task::spawn_blocking(crate::roms::media).await.unwrap_or(Value::Null)),
                "roms.mount" => {
                    let dev = s(&v, "dev")?;
                    let path = tokio::task::spawn_blocking(move || crate::roms::mount(&dev)).await.map_err(|e| fail(e.to_string()))?.map_err(fail)?;
                    Ok(json!({ "path": path }))
                }
                "roms.browse" => {
                    let path = s(&v, "path")?;
                    Ok(tokio::task::spawn_blocking(move || crate::roms::browse(&path)).await.map_err(|e| fail(e.to_string()))?.map_err(fail)?)
                }
                "roms.import" => {
                    let items: Vec<(std::path::PathBuf, String)> = v["items"].as_array().cloned().unwrap_or_default().iter().filter_map(|i| Some((std::path::PathBuf::from(i["path"].as_str()?), i["system"].as_str()?.to_string()))).collect();
                    if items.is_empty() {
                        return Err(RpcError::invalid("nada para copiar"));
                    }
                    let c = core.clone();
                    let r = tokio::task::spawn_blocking(move || crate::roms::import(&items, &|done, total, name| c.emit("roms.progress", json!({ "done": done, "total": total, "name": name })))).await.map_err(|e| fail(e.to_string()))?;
                    core.refresh_library().await;
                    Ok(r)
                }
                _ => {
                    // página de envio do celular: mesmo endereço/token do QR do controle, em /roms
                    let url = core.hub.url().replacen("/?t=", "/roms?t=", 1);
                    Ok(json!({ "url": url, "qrSvg": crate::remote::qr_svg(&url) }))
                }
            }
        }
        "steam.eula_answer" => {
            need_session(core)?;
            let accept = v.get("accept").and_then(|a| a.as_bool()).unwrap_or(false);
            *core.eula.lock().unwrap_or_else(|e| e.into_inner()) = Some(accept);
            Ok(json!({ "ok": true }))
        }
        "guide.closed" => {
            // a Shell fechou o menu rápido sem voltar ao jogo (ex.: "Ir ao Início")
            core.guide_open.store(false, std::sync::atomic::Ordering::SeqCst);
            Ok(json!({ "ok": true }))
        }
        "guide.press" => {
            crate::guide::pressed(core);
            Ok(json!({ "ok": true }))
        }
        "game.show_nexus" => {
            core.guide_open.store(false, std::sync::atomic::Ordering::SeqCst);
            core.compositor.show_shell();
            Ok(json!({ "ok": true }))
        }
        "game.resume" => {
            core.guide_open.store(false, std::sync::atomic::Ordering::SeqCst);
            if core.game_info().is_some() {
                core.compositor.show_game();
            }
            Ok(json!({ "ok": true }))
        }

        // ---------- apps e jogos do Linux adicionados pelo usuário ----------
        "fs.list" | "profiles.avatar_import" => {
            may_browse(core, &v)?;
            if method == "fs.list" {
                return ok(crate::customart::list(core.native.roots(), os(&v, "path").as_deref()).map_err(|e| RpcError::new(E_FAILED, &e))?);
            }
            let avatar = crate::avatars::import(core, &s(&v, "path")?).map_err(|e| RpcError::new(E_FAILED, &e))?;
            Ok(json!({ "avatar": avatar }))
        }
        "apps.system" | "apps.registered" | "apps.add" | "apps.update" | "apps.remove" => {
            let p = need_session(core)?;
            if p.kind == ProfileKind::Kid {
                return Err(RpcError::new(E_FORBIDDEN, "Indisponível neste perfil"));
            }
            match method {
                "apps.system" => ok(core.native.system_apps()),
                "apps.registered" => ok(core.native.custom_apps()),
                _ => {
                    parental(core, &v)?;
                    let opt = |k: &str| -> Option<Option<String>> { v.get(k).map(|x| x.as_str().filter(|s| !s.is_empty()).map(String::from)) };
                    let id = match method {
                        "apps.add" => core.native.add(&s(&v, "fileId")?, os(&v, "kind").as_deref())?,
                        "apps.update" => {
                            let id = s(&v, "id")?;
                            core.native.update(&id, crate::providers::native::RegPatch { name: opt("name"), kind: os(&v, "kind"), icon: opt("icon"), cover: opt("cover"), hero: opt("hero") })?;
                            id
                        }
                        _ => {
                            let id = s(&v, "id")?;
                            core.native.remove(&id)?;
                            let mut prefs = core.accounts.prefs(&p.id);
                            prefs.pins.retain(|x| x != &id);
                            prefs.favs.retain(|x| x != &id);
                            core.accounts.set_prefs(&p.id, &prefs)?;
                            id
                        }
                    };
                    core.refresh_library().await;
                    if method == "apps.remove" {
                        return Ok(json!({ "ok": true }));
                    }
                    ok(core.native.custom_apps().into_iter().find(|a| a.id == id))
                }
            }
        }

        // ---------- Steam: abrir o cliente e a página da loja ----------
        "steam.open" => {
            need_session(core)?;
            let st = core.steam.as_ref().expect("steam provider");
            st.open_client()?;
            core.show_steam_ui();
            Ok(json!({ "ok": true }))
        }
        "steam.store_page" => {
            let p = need_session(core)?;
            if p.kind == ProfileKind::Kid {
                return Err(RpcError::new(E_FORBIDDEN, "Indisponível neste perfil"));
            }
            let id = s(&v, "id")?;
            let n: u32 = id.strip_prefix("steam:").and_then(|x| x.parse().ok()).ok_or_else(|| RpcError::invalid("id da Steam inválido"))?;
            core.steam.as_ref().expect("steam provider").open_store(n)?;
            core.show_steam_ui();
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


/// Instala sem mostrar a Steam: confirma o diálogo "Install" dela por trás da capa do Nexus (ver `item.install`).
/// Se o jogo tem contrato de licença (EULA), quem decide é o usuário: o Nexus pergunta na própria tela e só então
/// clica em "Accept" (ou "Cancel").
fn steam_confirm_install(c: Arc<Core>, st: Arc<crate::providers::steam::SteamProvider>, n: String, id: String, title: String) {
    use crate::compositor::{click_at, hide_window, peek_window, steam_dialog, steam_dialog_points, SteamDialog};
    use std::time::{Duration, Instant};
    let started = || st.root().is_some_and(|r| st.steamapps_dirs(&r).iter().any(|d| d.join(format!("appmanifest_{n}.acf")).is_file()));
    let wait = |secs: u64, done: &dyn Fn() -> bool| {
        let t = Instant::now();
        while !done() && t.elapsed() < Duration::from_secs(secs) {
            std::thread::sleep(Duration::from_millis(300));
        }
        done()
    };
    let cover = |open: bool| {
        c.emit("steam.dialog", json!({ "open": open, "title": title }));
        if open {
            std::thread::sleep(Duration::from_millis(500)); // a capa aparece antes da Steam
        }
    };
    // clica num botão do diálogo (tirado do scratchpad, por baixo da capa)
    let press = |kind: SteamDialog, ok: bool| -> bool {
        let Some((k, con, _)) = steam_dialog().filter(|d| d.0 == kind) else { return false };
        c.compositor.show_game();
        peek_window(con);
        std::thread::sleep(Duration::from_millis(1200)); // o diálogo (CEF) termina de desenhar
        let Some((_, _, rect)) = steam_dialog().filter(|d| d.1 == con) else { return false };
        let (yes, no) = steam_dialog_points(k, rect);
        let (x, y) = if ok { yes } else { no };
        tracing::info!("instalação {n}: diálogo {k:?} em {rect:?} → clique {} em ({x},{y})", if ok { "confirmar" } else { "cancelar" });
        click_at(x, y);
        true
    };
    let back = || {
        for (_, con, _) in steam_dialog().into_iter() {
            hide_window(con);
        }
        crate::compositor::steam_hide_all();
        if c.game_info().is_none() {
            c.compositor.show_shell();
        }
    };
    // a Steam pode estar abrindo/atualizando: espera o diálogo até 90 s
    if !wait(90, &|| started() || steam_dialog().is_some()) || started() {
        return;
    }
    cover(true);
    let mut outcome = "fail"; // ok | declined | fail
    'tries: for _ in 0..3 {
        if !press(SteamDialog::Install, true) && steam_dialog().is_none_or(|d| d.0 != SteamDialog::Eula) {
            break;
        }
        // depois do "Install": começa o download, ou aparece o contrato de licença
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(10) {
            if started() {
                outcome = "ok";
                break 'tries;
            }
            if steam_dialog().is_some_and(|d| d.0 == SteamDialog::Eula) {
                tracing::info!("instalação {n}: contrato de licença — perguntando ao usuário");
                back();
                cover(false);
                *c.eula.lock().unwrap_or_else(|e| e.into_inner()) = None;
                c.emit("steam.eula", json!({ "itemId": id, "title": title }));
                let answer = (0..600).find_map(|_| {
                    std::thread::sleep(Duration::from_millis(500));
                    c.eula.lock().unwrap_or_else(|e| e.into_inner()).take()
                });
                cover(true);
                let accept = answer == Some(true);
                press(SteamDialog::Eula, accept);
                if !accept {
                    std::thread::sleep(Duration::from_millis(800));
                    press(SteamDialog::Install, false);
                    outcome = "declined";
                    break 'tries;
                }
                if wait(15, &|| started()) {
                    outcome = "ok";
                }
                break 'tries;
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        if steam_dialog().is_none() {
            break;
        }
    }
    tracing::info!("instalação {n}: resultado {outcome} (diálogos abertos: {:?})", steam_dialog());
    if outcome != "ok" && !started() {
        // desiste: fecha o que sobrou (Cancel) e avisa — salvo se o usuário recusou o contrato
        for kind in [SteamDialog::Eula, SteamDialog::Install] {
            press(kind, false);
        }
        c.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
        if outcome == "fail" {
            c.notify("alert", "A Steam não começou a instalação", &format!("{title} · confira o espaço livre no disco"), None);
        }
    }
    back();
    std::thread::sleep(Duration::from_millis(400));
    cover(false);
    c.kick.notify_one();
}
