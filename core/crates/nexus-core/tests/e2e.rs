//! Testes de ponta a ponta: sobem o Core de verdade e falam JSON-RPC pelo socket Unix.
use nexus_core::config::Config;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

struct Client {
    w: tokio::net::unix::OwnedWriteHalf,
    rx: mpsc::UnboundedReceiver<Value>,
    events: Vec<Value>,
    id: u64,
}

impl Client {
    async fn connect(sock: &Path) -> Client {
        let s = UnixStream::connect(sock).await.unwrap();
        let (r, w) = s.into_split();
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut l = BufReader::new(r).lines();
            while let Ok(Some(line)) = l.next_line().await {
                let _ = tx.send(serde_json::from_str::<Value>(&line).unwrap());
            }
        });
        Client { w, rx, events: vec![], id: 0 }
    }

    async fn call(&mut self, method: &str, params: Value) -> Result<Value, Value> {
        self.id += 1;
        let id = self.id;
        let req = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        self.w.write_all(format!("{req}\n").as_bytes()).await.unwrap();
        loop {
            let m = tokio::time::timeout(Duration::from_secs(10), self.rx.recv()).await.expect("timeout esperando resposta").expect("conexão fechou");
            if m.get("id").and_then(|i| i.as_u64()) == Some(id) {
                return match m.get("error") {
                    Some(e) => Err(e.clone()),
                    None => Ok(m["result"].clone()),
                };
            }
            self.events.push(m);
        }
    }

    async fn ok(&mut self, method: &str, params: Value) -> Value {
        self.call(method, params).await.unwrap_or_else(|e| panic!("{method} falhou: {e}"))
    }

    async fn code(&mut self, method: &str, params: Value) -> i64 {
        self.call(method, params).await.expect_err(&format!("{method} deveria falhar"))["code"].as_i64().unwrap()
    }

    async fn event(&mut self, method: &str, secs: u64) -> Value {
        if let Some(i) = self.events.iter().position(|e| e["method"] == method) {
            return self.events.remove(i)["params"].clone();
        }
        let end = tokio::time::Instant::now() + Duration::from_secs(secs);
        loop {
            let left = end.saturating_duration_since(tokio::time::Instant::now());
            let m = tokio::time::timeout(left, self.rx.recv()).await.unwrap_or_else(|_| panic!("evento {method} não chegou; recebidos: {:?}", self.events.iter().map(|e| e["method"].clone()).collect::<Vec<_>>())).unwrap();
            if m["method"] == method {
                return m["params"].clone();
            }
            self.events.push(m);
        }
    }

    fn clear(&mut self) {
        self.events.clear();
        while self.rx.try_recv().is_ok() {}
    }
}

async fn boot(dir: &Path, tweak: impl FnOnce(&mut Config)) -> (nexus_core::Running, Client) {
    let mut cfg = Config::for_test(dir);
    tweak(&mut cfg);
    let r = nexus_core::run(cfg).await.unwrap();
    let c = Client::connect(&r.core.cfg.rpc_socket).await;
    (r, c)
}

fn write_exec(p: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(p, body).unwrap();
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn desktop_dir(dir: &Path, entries: &[(&str, &str, &str)]) -> std::path::PathBuf {
    let apps = dir.join("apps/applications");
    std::fs::create_dir_all(&apps).unwrap();
    for (file, name, exec) in entries {
        std::fs::write(apps.join(file), format!("[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\nCategories=Game;\n")).unwrap();
    }
    apps
}

async fn first_adult(c: &mut Client) -> String {
    let p = c.ok("profiles.create", json!({ "name": "Lia", "avatar": "lia", "hue": 180 })).await;
    let id = p["id"].as_str().unwrap().to_string();
    c.ok("profiles.select", json!({ "id": id })).await;
    id
}

#[tokio::test]
async fn profiles_parental_and_kid_rules() {
    let d = tempfile::tempdir().unwrap();
    let apps = desktop_dir(d.path(), &[("jogo.desktop", "Jogo Adulto", "sleep 30")]);
    let (_r, mut c) = boot(d.path(), |cfg| cfg.apps_dirs = vec![apps]).await;

    assert!(c.ok("profiles.list", json!({})).await.as_array().unwrap().is_empty());
    assert_eq!(c.code("library.list", json!({})).await, 1009, "sem perfil ativo");
    let lia = first_adult(&mut c).await;
    assert_eq!(c.code("profiles.create", json!({ "name": "Pipo", "kind": "kid" })).await, 1010, "perfil infantil exige PIN dos responsáveis");
    c.ok("parental.set", json!({ "newPin": "4321" })).await;
    assert_eq!(c.code("profiles.create", json!({ "name": "Pipo", "kind": "kid" })).await, 1001, "criar perfil agora pede o PIN");
    assert_eq!(c.code("profiles.create", json!({ "name": "Pipo", "kind": "kid", "parentPin": "0000" })).await, 1002);
    let kid = c.ok("profiles.create", json!({ "name": "Pipo", "kind": "kid", "parentPin": "4321", "ageLimit": 10, "dailyLimitMin": 45 })).await;
    assert_eq!(kid["kind"], "kid");
    assert_eq!(kid["ageLimit"], 10);
    let kid_id = kid["id"].as_str().unwrap().to_string();

    // PIN de perfil adulto
    let teo = c.ok("profiles.create", json!({ "name": "Teo", "parentPin": "4321", "pin": "2468" })).await;
    let teo_id = teo["id"].as_str().unwrap().to_string();
    assert_eq!(c.code("profiles.select", json!({ "id": teo_id })).await, 1001);
    assert_eq!(c.code("profiles.select", json!({ "id": teo_id, "pin": "1111" })).await, 1002);
    c.ok("profiles.select", json!({ "id": teo_id, "pin": "2468" })).await;
    c.ok("profiles.select", json!({ "id": lia })).await;

    // entra como criança: biblioteca filtrada, sem loja, sem instalar
    let s = c.ok("profiles.select", json!({ "id": kid_id })).await;
    assert_eq!(s["remainingMin"], 45);
    c.ok("library.list", json!({})).await;
    c.clear();
    assert_eq!(c.code("store.list", json!({ "category": "jogos" })).await, 1004);
    assert_eq!(c.code("item.install", json!({ "id": "native:jogo.desktop" })).await, 1004);
    assert_eq!(c.code("item.launch", json!({ "id": "native:jogo.desktop" })).await, 1004, "app não liberado");
    assert_eq!(c.code("remote.rotate", json!({})).await, 1004);
    // sair do perfil infantil exige o PIN dos responsáveis
    assert_eq!(c.code("profiles.select", json!({ "id": lia })).await, 1001);
    assert_eq!(c.code("session.logout", json!({})).await, 1001);
    assert_eq!(c.code("profiles.update", json!({ "id": kid_id, "dailyLimitMin": 600 })).await, 1001);
    let g = c.ok("parental.verify", json!({ "pin": "4321" })).await["grant"].as_str().unwrap().to_string();
    // liberar um item específico e abrir
    c.ok("parental.allow", json!({ "profileId": kid_id, "itemId": "native:jogo.desktop", "allow": true, "grant": g })).await;
    let lib = c.ok("library.list", json!({})).await;
    assert_eq!(lib["total"], 1, "só o item liberado aparece");
    c.ok("item.launch", json!({ "id": "native:jogo.desktop" })).await;
    c.event("game.started", 5).await;
    c.ok("game.close", json!({})).await;
    c.event("game.exited", 8).await;
    c.ok("session.logout", json!({ "grant": g })).await;

    // regras de exclusão
    assert_eq!(c.code("profiles.delete", json!({ "id": kid_id })).await, 1001, "exige PIN dos responsáveis");
    c.ok("profiles.delete", json!({ "id": kid_id, "parentPin": "4321" })).await;
    let n = c.ok("profiles.list", json!({})).await.as_array().unwrap().len();
    assert_eq!(n, 2);
    c.ok("profiles.select", json!({ "id": lia })).await;
    assert_eq!(c.code("profiles.delete", json!({ "id": lia, "parentPin": "4321" })).await, 1005, "não exclui o perfil ativo");
    // prefs
    let p = c.ok("prefs.set", json!({ "patch": { "theme": "light", "vol": 250, "lixo": 1 } })).await;
    assert_eq!(p["theme"], "light");
    assert_eq!(p["vol"], 100);
    assert_eq!(c.ok("session.current", json!({})).await["prefs"]["theme"], "light");
}

#[tokio::test]
async fn native_launch_roundtrip_and_playtime() {
    let d = tempfile::tempdir().unwrap();
    let apps = desktop_dir(d.path(), &[("jogo.desktop", "Jogo", r#"sh -c "sleep 30; true""#)]);
    let (r, mut c) = boot(d.path(), |cfg| cfg.apps_dirs = vec![apps]).await;
    let lia = first_adult(&mut c).await;
    let it = c.ok("library.get", json!({ "id": "native:jogo.desktop" })).await;
    assert_eq!(it["title"], "Jogo");
    assert_eq!(it["kind"], "game");
    let s = c.ok("item.launch", json!({ "id": "native:jogo.desktop" })).await;
    assert_eq!(s["state"], "launching");
    let started = c.event("game.started", 5).await;
    assert_eq!(started["session"]["state"], "running");
    assert_eq!(c.code("item.launch", json!({ "id": "native:jogo.desktop" })).await, 1005, "um jogo por vez");
    assert_eq!(c.ok("game.current", json!({})).await["itemId"], "native:jogo.desktop");
    assert_eq!(c.ok("library.get", json!({ "id": "native:jogo.desktop" })).await["state"], "running");
    tokio::time::sleep(Duration::from_millis(1200)).await;
    let pid = s["pid"].as_u64().unwrap() as u32;
    assert!(nexus_core::procs::alive(pid));
    c.ok("game.close", json!({})).await;
    let ex = c.event("game.exited", 10).await;
    assert!(ex["playSec"].as_u64().unwrap() >= 1);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(nexus_core::procs::session_members(pid).is_empty(), "nenhum processo sobrou na sessão");
    assert!(c.ok("game.current", json!({})).await.is_null());
    let it = c.ok("library.get", json!({ "id": "native:jogo.desktop" })).await;
    assert!(it["playtimeSec"].as_u64().unwrap() >= 1, "tempo de jogo registrado: {it}");
    assert_eq!(it["state"], "installed");
    assert!(r.core.accounts.playtime(&lia, "native:jogo.desktop").0 >= 1);
}

#[tokio::test]
async fn failing_command_reports_failure_and_returns() {
    let d = tempfile::tempdir().unwrap();
    let apps = desktop_dir(d.path(), &[("quebrado.desktop", "Quebrado", r#"sh -c "exit 3""#), ("inexistente.desktop", "Inexistente", "/nao/existe/mesmo")]);
    let (_r, mut c) = boot(d.path(), |cfg| cfg.apps_dirs = vec![apps]).await;
    first_adult(&mut c).await;
    c.ok("item.launch", json!({ "id": "native:quebrado.desktop" })).await;
    let f = c.event("game.failed", 8).await;
    assert!(f["reason"].as_str().unwrap().contains('3'), "{f}");
    c.event("game.exited", 5).await;
    assert!(c.ok("game.current", json!({})).await.is_null());
    assert_eq!(c.code("item.launch", json!({ "id": "native:inexistente.desktop" })).await, 1011);
    assert!(c.ok("game.current", json!({})).await.is_null(), "falha ao executar não deixa sessão presa");
    assert_eq!(c.code("item.launch", json!({ "id": "native:nao-tem.desktop" })).await, 1006);
}

#[tokio::test]
async fn steam_install_progress_launch_and_exit() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("Steam");
    std::fs::create_dir_all(root.join("steamapps")).unwrap();
    std::fs::write(root.join("steamapps/appmanifest_570.acf"), "\"AppState\"\n{\n\"appid\" \"570\"\n\"name\" \"Dota Falso\"\n\"StateFlags\" \"4\"\n\"SizeOnDisk\" \"1000\"\n}\n").unwrap();
    std::fs::create_dir_all(root.join("config")).unwrap();
    std::fs::write(root.join("config/loginusers.vdf"), "\"users\"\n{\n\"765\"\n{\n\"AccountName\" \"lia\"\n\"PersonaName\" \"Lia Gamer\"\n\"MostRecent\" \"1\"\n}\n}\n").unwrap();
    // Steam falsa: roda o jogo num "reaper" separado e baixa 4 passos para o ACF
    let fake = d.path().join("steam");
    write_exec(&fake, &format!(r#"#!/bin/sh
URI="$1"
ROOT="{root}"
case "$URI" in
  steam://rungameid/*)
    N="${{URI##*/}}"
    setsid nohup sh -c 'sleep 40; true' SteamLaunch AppId=$N >/dev/null 2>&1 &
    ;;
  steam://install/*)
    N="${{URI##*/}}"
    (for P in 250 500 750; do
       printf '"AppState"\n{{\n"appid" "%s"\n"name" "Novo Jogo"\n"StateFlags" "1026"\n"BytesToDownload" "1000"\n"BytesDownloaded" "%s"\n}}\n' "$N" "$P" > "$ROOT/steamapps/appmanifest_$N.acf"
       sleep 1
     done
     printf '"AppState"\n{{\n"appid" "%s"\n"name" "Novo Jogo"\n"StateFlags" "4"\n"SizeOnDisk" "1000"\n}}\n' "$N" > "$ROOT/steamapps/appmanifest_$N.acf") >/dev/null 2>&1 &
    ;;
esac
exit 0
"#, root = root.display()));
    let (r, mut c) = boot(d.path(), |cfg| {
        cfg.steam_root = Some(root.clone());
        cfg.steam_bin = fake.display().to_string();
    })
    .await;
    first_adult(&mut c).await;
    let acc = c.ok("steam.account", json!({})).await;
    assert_eq!((acc["installed"].as_bool(), acc["persona"].as_str()), (Some(true), Some("Lia Gamer")));
    r.core.db.kv_set("steam_owned", r#"{"response":{"games":[{"appid":99,"name":"Novo Jogo","playtime_forever":0}]}}"#).unwrap();
    r.core.refresh_library().await;
    let lib = c.ok("library.list", json!({})).await;
    let by = |id: &str| lib["items"].as_array().unwrap().iter().find(|i| i["id"] == id).cloned();
    assert_eq!(by("steam:570").unwrap()["state"], "installed");
    assert_eq!(by("steam:99").unwrap()["state"], "not_installed");
    assert_eq!(by("steam:99").unwrap()["caps"]["install"], true);

    // instalar: progresso vem dos ACF
    c.clear();
    c.ok("item.install", json!({ "id": "steam:99" })).await;
    let mut seen = vec![];
    let end = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let ev = c.event("library.changed", 10).await;
        if ev["ids"].as_array().unwrap().iter().any(|i| i == "steam:99") {
            let it = c.ok("library.get_many", json!({ "ids": ["steam:99"] })).await[0].clone();
            seen.push((it["state"].as_str().unwrap().to_string(), it["install"]["percent"].as_f64()));
        }
        if seen.iter().any(|(s, _)| s == "installed") || tokio::time::Instant::now() > end {
            break;
        }
    }
    assert!(seen.iter().any(|(s, p)| s == "installing" && p.is_some_and(|p| p >= 25.0)), "progresso observado: {seen:?}");
    assert_eq!(seen.last().unwrap().0, "installed", "{seen:?}");
    let n = c.ok("notifications.list", json!({})).await;
    assert!(n.as_array().unwrap().iter().any(|x| x["title"] == "Pronto para jogar"));

    // jogar: Steam repassa ao reaper; o Core acompanha até o reaper sumir
    let s = c.ok("item.launch", json!({ "id": "steam:570" })).await;
    assert_eq!(s["itemId"], "steam:570");
    let started = c.event("game.started", 10).await;
    assert_eq!(started["session"]["state"], "running");
    assert!(!nexus_core::procs::steam_game_pids(570).is_empty());
    c.ok("game.close", json!({})).await;
    c.event("game.exited", 12).await;
    assert!(nexus_core::procs::steam_game_pids(570).is_empty());
    assert!(c.ok("game.current", json!({})).await.is_null());
}

#[tokio::test]
async fn steam_launch_that_never_appears_fails_cleanly() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("Steam");
    std::fs::create_dir_all(root.join("steamapps")).unwrap();
    std::fs::write(root.join("steamapps/appmanifest_7.acf"), "\"AppState\"\n{\n\"appid\" \"7\"\n\"name\" \"Sem Reaper\"\n\"StateFlags\" \"4\"\n}\n").unwrap();
    let fake = d.path().join("steam");
    write_exec(&fake, "#!/bin/sh\nexit 0\n");
    let (r, mut c) = boot(d.path(), |cfg| {
        cfg.steam_root = Some(root.clone());
        cfg.steam_bin = fake.display().to_string();
    })
    .await;
    first_adult(&mut c).await;
    c.ok("item.launch", json!({ "id": "steam:7" })).await;
    // sem cliente real o reaper nunca aparece: fecha manualmente em vez de esperar os 120 s
    assert!(r.core.game_info().is_some());
    assert!(c.ok("game.close", json!({})).await["closing"].as_bool().unwrap());
}

#[tokio::test]
async fn kid_time_limit_blocks_launch() {
    let d = tempfile::tempdir().unwrap();
    let apps = desktop_dir(d.path(), &[("a.desktop", "A", "sleep 30")]);
    let (_r, mut c) = boot(d.path(), |cfg| cfg.apps_dirs = vec![apps]).await;
    first_adult(&mut c).await;
    c.ok("parental.set", json!({ "newPin": "1111" })).await;
    let kid = c.ok("profiles.create", json!({ "name": "Zeca", "kind": "kid", "parentPin": "1111", "dailyLimitMin": 5 })).await["id"].as_str().unwrap().to_string();
    let g = c.ok("parental.verify", json!({ "pin": "1111" })).await["grant"].as_str().unwrap().to_string();
    c.ok("profiles.select", json!({ "id": kid, "grant": g })).await;
    c.ok("parental.allow", json!({ "profileId": kid, "itemId": "native:a.desktop", "grant": g })).await;
    // gasta o tempo todo
    _r.core.accounts.add_usage(&kid, 5 * 60);
    assert_eq!(c.code("item.launch", json!({ "id": "native:a.desktop" })).await, 1007);
    let r = c.ok("parental.extend", json!({ "profileId": kid, "minutes": 10, "grant": g })).await;
    assert_eq!(r["remainingMin"], 10);
    c.ok("item.launch", json!({ "id": "native:a.desktop" })).await;
    c.event("game.started", 5).await;
    c.ok("game.close", json!({ "force": true })).await;
    c.event("game.exited", 8).await;
}

#[tokio::test]
async fn demo_store_flow_with_wallet() {
    std::env::set_var("NEXUS_DEMO_DL_SECS", "1");
    let d = tempfile::tempdir().unwrap();
    let (_r, mut c) = boot(d.path(), |cfg| cfg.demo = true).await;
    let profs = c.ok("profiles.list", json!({})).await;
    assert_eq!(profs.as_array().unwrap().len(), 4);
    let lia = profs[0]["id"].as_str().unwrap().to_string();
    c.ok("profiles.select", json!({ "id": lia })).await;
    let lib = c.ok("library.list", json!({})).await;
    assert!(lib["total"].as_u64().unwrap() >= 20);
    let wallet = c.ok("store.wallet", json!({})).await["balance"].as_f64().unwrap();
    assert_eq!(wallet, 400.0);
    let store = c.ok("store.list", json!({ "category": "jogos" })).await;
    assert_eq!(store.as_array().unwrap().len(), 4);
    c.ok("item.install", json!({ "id": "demo:s4" })).await;
    let end = tokio::time::Instant::now() + Duration::from_secs(8);
    loop {
        let it = c.ok("library.get", json!({ "id": "demo:s4" })).await;
        if it["state"] == "installed" {
            break;
        }
        assert!(tokio::time::Instant::now() < end, "download demo não terminou");
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    assert!((c.ok("store.wallet", json!({})).await["balance"].as_f64().unwrap() - 310.1).abs() < 0.01);
    let found = c.ok("library.search", json!({ "q": "forja estelar" })).await;
    assert_eq!(found[0]["id"], "demo:s4");
    let friends = c.ok("social.friends", json!({})).await;
    assert_eq!(friends.as_array().unwrap().len(), 4);
    // criança no demo: só jogos com classificação até o limite; sem apps; sem loja
    let pipo = profs.as_array().unwrap().iter().find(|p| p["kind"] == "kid").unwrap()["id"].as_str().unwrap().to_string();
    let g = c.ok("parental.verify", json!({ "pin": "1234" })).await["grant"].as_str().unwrap().to_string();
    c.ok("profiles.select", json!({ "id": pipo, "grant": g })).await;
    let lib = c.ok("library.list", json!({})).await;
    for it in lib["items"].as_array().unwrap() {
        assert_eq!(it["kind"], "game");
        assert!(it["rating"].as_u64().unwrap() <= 10, "{it}");
    }
    assert!(lib["total"].as_u64().unwrap() >= 3);
}

#[tokio::test]
async fn dev_http_art_rpc_ws_and_host_check() {
    let d = tempfile::tempdir().unwrap();
    let demo = d.path().join("demo");
    std::fs::create_dir_all(demo.join("th")).unwrap();
    std::fs::write(demo.join("th/g1.jpg"), [0xff, 0xd8, 0xff, 0xe0, 1, 2, 3]).unwrap();
    let (r, _c) = boot(d.path(), |cfg| {
        cfg.demo = true;
        cfg.demo_assets = demo.clone();
    })
    .await;
    let addr = r.started.dev_addr.unwrap();
    let get = move |path: &str, host: Option<&str>| {
        let url = format!("http://{addr}{path}");
        let host = host.map(String::from);
        tokio::task::spawn_blocking(move || {
            let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
            let mut rq = agent.get(&url);
            if let Some(h) = host {
                rq = rq.header("Host", &h);
            }
            let mut res = rq.call().unwrap();
            (res.status().as_u16(), res.headers().get("content-type").and_then(|v| v.to_str().ok()).map(String::from), res.body_mut().read_to_vec().unwrap())
        })
    };
    assert_eq!(get("/healthz", None).await.unwrap().0, 200);
    let (st, ct, body) = get("/art/demo:g1/cover", None).await.unwrap();
    assert_eq!((st, ct.as_deref()), (200, Some("image/jpeg")));
    assert_eq!(body.len(), 7);
    assert_eq!(get("/art/demo:g1/hero", None).await.unwrap().0, 404);
    assert_eq!(get("/art/nada:1/cover", None).await.unwrap().0, 404);
    assert_eq!(get("/art/demo:g1/cover", Some("evil.example.com")).await.unwrap().0, 403, "DNS rebinding");
    assert_eq!(get("/art/..%2F..%2Fetc/cover", None).await.unwrap().0, 404);

    use futures_util::{SinkExt, StreamExt};
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/rpc")).await.unwrap();
    ws.send(tokio_tungstenite::tungstenite::Message::text(json!({ "jsonrpc": "2.0", "id": 1, "method": "core.hello" }).to_string())).await.unwrap();
    let m = tokio::time::timeout(Duration::from_secs(5), ws.next()).await.unwrap().unwrap().unwrap();
    let v: Value = serde_json::from_str(m.to_text().unwrap()).unwrap();
    assert_eq!(v["result"]["demo"], true);
    ws.send(tokio_tungstenite::tungstenite::Message::text("isso não é json")).await.unwrap();
    let m = tokio::time::timeout(Duration::from_secs(5), ws.next()).await.unwrap().unwrap().unwrap();
    assert_eq!(serde_json::from_str::<Value>(m.to_text().unwrap()).unwrap()["error"]["code"], -32700);
}

#[tokio::test]
async fn remote_pairing_websocket_to_shell_events() {
    let d = tempfile::tempdir().unwrap();
    let (r, mut c) = boot(d.path(), |_| {}).await;
    first_adult(&mut c).await;
    let p = c.ok("remote.pairing", json!({})).await;
    let url = p["url"].as_str().unwrap().to_string();
    assert!(p["qrSvg"].as_str().unwrap().contains("<svg"));
    assert_eq!(p["uinput"], false);
    let token = url.split("t=").nth(1).unwrap().to_string();
    let addr = r.started.remote_addr.unwrap();
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;
    assert!(tokio_tungstenite::connect_async(format!("ws://{addr}/remote/ws?t=errado")).await.is_err(), "token inválido é recusado");
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/remote/ws?t={token}")).await.unwrap();
    ws.send(Message::text(json!({ "t": "join", "mode": "gamepad", "player": 1, "name": "Moto G" }).to_string())).await.unwrap();
    let m: Value = serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
    assert_eq!((m["t"].as_str(), m["player"].as_u64()), (Some("joined"), Some(1)));
    let dv = c.event("remote.devices", 3).await;
    assert_eq!(dv["devices"][0]["name"], "Moto G");
    // segundo aparelho no mesmo jogador é recusado
    let (mut ws2, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/remote/ws?t={token}")).await.unwrap();
    ws2.send(Message::text(json!({ "t": "join", "mode": "gamepad", "player": 1 }).to_string())).await.unwrap();
    let m: Value = serde_json::from_str(ws2.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(m["t"], "error");
    // sem uinput, o botão A vira tecla de navegação para a Shell
    ws.send(Message::text(json!({ "t": "state", "buttons": ["a"], "hat": [0, 1], "axes": {} }).to_string())).await.unwrap();
    let k1 = c.event("remote.input", 3).await;
    let k2 = c.event("remote.input", 3).await;
    let keys: Vec<_> = [k1, k2].iter().map(|k| (k["key"].as_str().unwrap().to_string(), k["down"].as_bool().unwrap())).collect();
    assert!(keys.contains(&("A".into(), true)) && keys.contains(&("d".into(), true)), "{keys:?}");
    // touchpad
    let (mut tp, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/remote/ws?t={token}")).await.unwrap();
    tp.send(Message::text(json!({ "t": "join", "mode": "touchpad", "name": "iPhone" }).to_string())).await.unwrap();
    tp.next().await.unwrap().unwrap();
    tp.send(Message::text(json!({ "t": "move", "dx": 12, "dy": -4 }).to_string())).await.unwrap();
    let mv = c.event("remote.pointer", 3).await;
    assert_eq!((mv["dx"].as_i64(), mv["dy"].as_i64()), (Some(12), Some(-4)));
    tp.send(Message::text(json!({ "t": "click", "b": "left", "down": true }).to_string())).await.unwrap();
    assert_eq!(c.event("remote.button", 3).await["b"], "left");
    // trocar o QR derruba os aparelhos
    let before = c.ok("remote.pairing", json!({})).await["devices"].as_array().unwrap().len();
    assert_eq!(before, 2);
    c.ok("remote.rotate", json!({})).await;
    tokio::time::sleep(Duration::from_millis(2600)).await;
    let after = c.ok("remote.pairing", json!({})).await;
    assert!(after["devices"].as_array().unwrap().is_empty());
    assert_ne!(after["url"].as_str().unwrap(), url);
    // /remote/hello
    let hello = tokio::task::spawn_blocking(move || {
        let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
        agent.get(&format!("http://{addr}/remote/hello?t=nada")).call().unwrap().status().as_u16()
    })
    .await
    .unwrap();
    assert_eq!(hello, 403);
}

#[tokio::test]
async fn unix_socket_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    let (r, mut c) = boot(d.path(), |_| {}).await;
    assert_eq!(std::fs::metadata(&r.core.cfg.rpc_socket).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(c.code("nada.existe", json!({})).await, -32601);
    let h = c.ok("hardware.profile", json!({})).await;
    assert!(h["memoryMb"].as_u64().unwrap() > 0);
}
