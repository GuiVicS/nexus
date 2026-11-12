//! Google Drive de ponta a ponta com o `rclone` de verdade: o "Drive" é um remoto `alias` numa pasta local e o
//! `rclone authorize` é simulado (sem conta Google). Testa login → pasta `Nexus` criada → montagem FUSE →
//! ROM da nuvem na biblioteca → download prévio → sair da conta.
//! Arquivo próprio (processo próprio): o módulo `drive` guarda as pastas num estático.
//! Precisa do rclone e do FUSE: `NEXUS_TEST_RCLONE=/caminho/rclone` (sem ele, o teste é pulado).
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
            let m = tokio::time::timeout(Duration::from_secs(30), self.rx.recv()).await.expect("timeout esperando resposta").expect("conexão fechou");
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

    async fn event(&mut self, method: &str, secs: u64) -> Value {
        if let Some(i) = self.events.iter().position(|e| e["method"] == method) {
            return self.events.remove(i)["params"].clone();
        }
        let end = tokio::time::Instant::now() + Duration::from_secs(secs);
        loop {
            let left = end.saturating_duration_since(tokio::time::Instant::now());
            let m = tokio::time::timeout(left, self.rx.recv()).await.unwrap_or_else(|_| panic!("evento {method} não chegou")).unwrap();
            if m["method"] == method {
                return m["params"].clone();
            }
            self.events.push(m);
        }
    }
}

fn write_exec(p: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(p, body).unwrap();
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[tokio::test]
async fn drive_login_folder_mount_cloud_rom_and_disconnect() {
    let Some(real) = std::env::var_os("NEXUS_TEST_RCLONE").map(std::path::PathBuf::from).filter(|p| p.is_file()) else {
        eprintln!("NEXUS_TEST_RCLONE não definido: teste do Google Drive pulado");
        return;
    };
    // caminho curto: o socket Unix tem limite de tamanho
    let base = std::path::PathBuf::from(std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into()));
    let tmp = tempfile::Builder::new().prefix("nxd").tempdir_in(&base).unwrap();
    let d = tmp.path();
    let cloud = d.join("cloud");
    std::fs::create_dir_all(&cloud).unwrap();
    let fake_conf = d.join("fake.conf");
    std::fs::write(&fake_conf, format!("[fakedrive]\ntype = alias\nremote = {}\n", cloud.display())).unwrap();
    // rclone falso: "authorize" imprime o endereço e, 1 s depois, o token; o resto vai para o rclone real com a config do teste
    let wrap = d.join("rclone");
    write_exec(
        &wrap,
        &format!(
            r#"#!/bin/sh
if [ "$1" = authorize ]; then
  echo 'NOTICE: Please go to the following link: http://127.0.0.1:53682/auth?state=abc' >&2
  echo 'NOTICE: Waiting for code...' >&2
  sleep 1
  echo 'Paste the following into your remote machine --->'
  echo '{{"access_token":"ya29.fake","token_type":"Bearer","refresh_token":"1//f","expiry":"2030-01-01T00:00:00Z"}}'
  echo '<---End paste'
  exit 0
fi
skip=0
for a; do shift; if [ $skip = 1 ]; then set -- "$@" "{conf}"; skip=0; elif [ "$a" = --config ]; then set -- "$@" "$a"; skip=1; else set -- "$@" "$a"; fi; done
exec "{real}" "$@"
"#,
            conf = fake_conf.display(),
            real = real.display()
        ),
    );
    let browser = d.join("chromium-falso");
    write_exec(&browser, "#!/bin/sh\nexec sleep 60\n");
    std::env::set_var("NEXUS_RCLONE", &wrap);
    std::env::set_var("NEXUS_DRIVE_REMOTE", "fakedrive");
    std::env::set_var("NEXUS_BROWSER", &browser);
    std::env::set_var("NEXUS_ROMS_DIR", d.join("roms"));
    std::env::set_var("NEXUS_OFFLINE", "1");
    let mut cfg = Config::for_test(d);
    cfg.emulation = true;
    let r = nexus_core::run(cfg).await.unwrap();
    let mut c = Client::connect(&r.core.cfg.rpc_socket).await;
    let p = c.ok("profiles.create", json!({ "name": "Lia", "avatar": "lia", "hue": 180 })).await;
    c.ok("profiles.select", json!({ "id": p["id"] })).await;

    let st = c.ok("drive.status", json!({})).await;
    assert_eq!((st["available"].as_bool(), st["connected"].as_bool()), (Some(true), Some(false)), "{st}");

    // login: abre a "janela" (item drive:login) e, autorizado, cria a pasta Nexus, monta e fecha a janela
    let l = c.ok("drive.login", json!({})).await;
    assert!(l["url"].as_str().unwrap().starts_with("http://127.0.0.1:53682/auth"));
    assert_eq!(c.ok("game.current", json!({})).await["itemId"], "drive:login", "janela do login aberta como app");
    let ev = c.event("drive.changed", 30).await;
    assert_eq!((ev["connected"].as_bool(), ev["created"].as_bool()), (Some(true), Some(true)), "{ev}");
    assert!(cloud.join("Nexus").is_dir(), "pasta Nexus criada na raiz do Drive");
    let conf = std::fs::read_to_string(d.join("data/rclone.conf")).unwrap();
    assert!(conf.contains("[nexus-drive]") && conf.contains("ya29.fake"));
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(d.join("data/rclone.conf")).unwrap().permissions().mode() & 0o777, 0o600, "token só legível pelo dono");
    for _ in 0..50 {
        if c.ok("game.current", json!({})).await.is_null() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert!(c.ok("game.current", json!({})).await.is_null(), "janela do login fechada no fim");
    let st = c.ok("drive.status", json!({})).await;
    assert_eq!(st["mounted"].as_bool(), Some(true), "{st}");

    // subpastas criadas pela montagem e uma ROM de 3 MB "na nuvem"
    let mnt = r.core.cfg.runtime_dir.join("drive");
    for s in ["ROMs/gb", "Filmes", "Séries", "Músicas", "Saves"] {
        assert!(mnt.join(s).is_dir(), "{s} na montagem");
    }
    let rom = mnt.join("ROMs/gb/Jogo da Nuvem.gb");
    std::fs::write(&rom, vec![7u8; 3 << 20]).unwrap();
    r.core.refresh_library().await;
    let lib = c.ok("library.list", json!({})).await;
    let it = lib["items"].as_array().unwrap().iter().find(|i| i["title"].as_str().unwrap_or("").contains("Jogo da Nuvem")).cloned().unwrap_or_else(|| panic!("ROM da nuvem na biblioteca: {lib}"));
    assert!(it["tags"].as_array().unwrap().iter().any(|t| t == "cloud"), "{it}");
    assert!(it["subtitle"].as_str().unwrap().ends_with("Google Drive"), "{it}");

    // download prévio com progresso até 100%
    let seen = std::sync::Mutex::new(vec![]);
    nexus_core::drive::prefetch(&rom, &|done, total| seen.lock().unwrap().push((done, total))).unwrap();
    let seen = seen.into_inner().unwrap();
    assert_eq!(seen.first(), Some(&(0, 3 << 20)));
    assert_eq!(seen.last(), Some(&(3 << 20, 3 << 20)));

    // sair da conta: desmonta, apaga token e cache; a ROM some da biblioteca (continua no Drive)
    c.ok("drive.disconnect", json!({})).await;
    assert!(!d.join("data/rclone.conf").exists());
    let st = c.ok("drive.status", json!({})).await;
    assert_eq!((st["connected"].as_bool(), st["mounted"].as_bool()), (Some(false), Some(false)), "{st}");
    let lib = c.ok("library.list", json!({})).await;
    assert!(!lib["items"].as_array().unwrap().iter().any(|i| i["tags"].as_array().map(|t| t.iter().any(|x| x == "cloud")).unwrap_or(false)));
    drop(r);
}
