//! Catálogo fictício do protótipo (jogos, apps e loja) para desenvolvimento, testes e demonstração.
//! Só é registrado com `--demo`. Instalação simulada com progresso e carteira virtual.
use super::*;
use crate::db::Db;
use nexus_protocol::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Entry {
    #[serde(flatten)]
    item: Item,
    icon_file: Option<String>,
}

#[derive(Deserialize)]
struct Catalog {
    items: Vec<Entry>,
    friends: Vec<Friend>,
}

pub struct DemoProvider {
    entries: Vec<Entry>,
    friends: Vec<Friend>,
    assets: PathBuf,
    db: Arc<Db>,
    downloads: Mutex<HashMap<String, (Instant, Duration, f64)>>, // id -> (início, duração, preço)
}

const CATALOG: &str = include_str!("../../../../../assets/demo/catalog.json");

pub fn price_num(s: &str) -> f64 {
    if s == "Grátis" {
        return 0.0;
    }
    s.chars().filter(|c| c.is_ascii_digit() || *c == ',').collect::<String>().replace(',', ".").parse().unwrap_or(0.0)
}

impl DemoProvider {
    pub fn new(assets: PathBuf, db: Arc<Db>) -> DemoProvider {
        let c: Catalog = serde_json::from_str(CATALOG).expect("catálogo demo válido");
        DemoProvider { entries: c.items, friends: c.friends, assets, db, downloads: Mutex::new(HashMap::new()) }
    }

    pub fn friends(&self) -> Vec<Friend> {
        self.friends.clone()
    }

    pub fn wallet(&self) -> f64 {
        self.db.kv_get("demo_wallet").and_then(|s| s.parse().ok()).unwrap_or(400.0)
    }

    fn installed(&self, id: &str) -> bool {
        self.db.with(|c| c.query_row("SELECT 1 FROM installed WHERE item_id=?1", [id], |_| Ok(()))).is_ok()
    }

    /// Conclui downloads cujo tempo acabou (cobra da carteira e marca como instalado).
    fn settle(&self) {
        let mut d = self.downloads.lock().unwrap_or_else(|e| e.into_inner());
        let done: Vec<String> = d.iter().filter(|(_, (t, dur, _))| t.elapsed() >= *dur).map(|(k, _)| k.clone()).collect();
        for id in done {
            let (_, _, price) = d.remove(&id).unwrap();
            let _ = self.db.with(|c| c.execute("INSERT OR IGNORE INTO installed(item_id,ts) VALUES(?1,?2)", rusqlite::params![id, now_unix()]).map(|_| ()));
            let _ = self.db.kv_set("demo_wallet", &format!("{:.2}", self.wallet() - price));
        }
    }

    fn decorate(&self, e: &Entry) -> Item {
        let mut it = e.item.clone();
        let store = it.tags.iter().any(|t| t.starts_with("store:"));
        let dl = self.downloads.lock().unwrap_or_else(|e| e.into_inner()).get(&it.id).map(|(t, d, _)| (t.elapsed(), *d));
        if store && self.installed(&it.id) {
            it.state = ItemState::Installed;
            it.tags.retain(|t| t != "lib");
            it.tags.push("lib".into());
            it.caps = Caps { install: false, pause: false, cancel: false, uninstall: true };
            it.price = None;
            it.offer = None;
        } else if let Some((el, dur)) = dl {
            it.state = ItemState::Installing;
            let pct = (el.as_secs_f64() / dur.as_secs_f64()).min(1.0);
            let total = it.size_bytes.unwrap_or(1_000_000_000);
            it.install = Some(InstallProgress {
                bytes_done: (total as f64 * pct) as u64,
                bytes_total: total,
                bps: (total as f64 / dur.as_secs_f64()) as u64,
                eta_sec: Some((dur.as_secs_f64() - el.as_secs_f64()).max(0.0) as u64),
                percent: (pct * 100.0) as f32,
                paused: false,
            });
            it.caps = Caps { install: false, pause: false, cancel: true, uninstall: false };
        }
        it
    }
}

#[async_trait]
impl Provider for DemoProvider {
    fn id(&self) -> &'static str {
        "demo"
    }

    async fn library(&self) -> anyhow::Result<Vec<Item>> {
        self.settle();
        Ok(self.entries.iter().map(|e| self.decorate(e)).filter(|i| i.tags.iter().any(|t| t == "lib") || i.state == ItemState::Installing).collect())
    }

    async fn store(&self, category: &str) -> anyhow::Result<Vec<Item>> {
        self.settle();
        let want = format!("store:{category}");
        Ok(self
            .entries
            .iter()
            .filter(|e| match category {
                "ofertas" => e.item.offer.is_some(),
                c => e.item.tags.contains(&format!("store:{c}")) || e.item.tags.contains(&want),
            })
            .map(|e| self.decorate(e))
            .collect())
    }

    async fn install(&self, id: &str) -> anyhow::Result<()> {
        self.settle();
        let e = self.entries.iter().find(|e| e.item.id == id).ok_or(ProviderError::NotFound)?;
        if self.installed(id) || self.downloads.lock().unwrap().contains_key(id) {
            return Ok(());
        }
        let price = price_num(e.item.offer.as_ref().map(|o| o.new.as_str()).or(e.item.price.as_deref()).unwrap_or("Grátis"));
        if price > self.wallet() {
            return Err(ProviderError::Msg("saldo insuficiente".into()).into());
        }
        let secs = std::env::var("NEXUS_DEMO_DL_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(5.0);
        self.downloads.lock().unwrap().insert(id.to_string(), (Instant::now(), Duration::from_secs_f64(secs), price));
        Ok(())
    }

    async fn get(&self, id: &str) -> anyhow::Result<Option<Item>> {
        self.settle();
        Ok(self.entries.iter().find(|e| e.item.id == id).map(|e| self.decorate(e)))
    }

    async fn cancel(&self, id: &str) -> anyhow::Result<()> {
        self.downloads.lock().unwrap().remove(id);
        Ok(())
    }

    async fn uninstall(&self, id: &str) -> anyhow::Result<()> {
        let _ = self.db.with(|c| c.execute("DELETE FROM installed WHERE item_id=?1", [id]).map(|_| ()));
        Ok(())
    }

    async fn launch(&self, id: &str) -> anyhow::Result<LaunchPlan> {
        self.settle();
        let e = self.entries.iter().find(|e| e.item.id == id).ok_or(ProviderError::NotFound)?;
        let ok = e.item.tags.iter().any(|t| t == "lib") && !e.item.tags.iter().any(|t| t.starts_with("store:")) || self.installed(id);
        if !ok {
            return Err(ProviderError::Msg("item não instalado".into()).into());
        }
        Ok(LaunchPlan { argv: vec!["sleep".into(), "86400".into()], env: vec![], cwd: None, tracker: Tracker::Process })
    }

    fn art(&self, id: &str, kind: ArtKind) -> ArtSource {
        let Some((_, local)) = split_id(id) else { return ArtSource::Missing };
        let local = local.replace(['/', '\\', '.'], "");
        let p = match kind {
            ArtKind::Cover => self.assets.join("th").join(format!("{local}.jpg")),
            ArtKind::Hero => self.assets.join("img").join(format!("{local}.jpg")),
            ArtKind::Icon => match self.entries.iter().find(|e| e.item.id == id).and_then(|e| e.icon_file.clone()) {
                Some(f) => self.assets.join("icons").join(f.replace(['/', '\\'], "")),
                None => return ArtSource::Missing,
            },
        };
        if p.is_file() { ArtSource::File(p) } else { ArtSource::Missing }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prices() {
        assert_eq!(price_num("R$ 89,90"), 89.9);
        assert_eq!(price_num("Grátis"), 0.0);
    }

    #[tokio::test]
    async fn catalog_loads_and_store_flow() {
        std::env::set_var("NEXUS_DEMO_DL_SECS", "0.3");
        let p = DemoProvider::new(PathBuf::from("/nonexistent"), Arc::new(Db::memory().unwrap()));
        let lib = p.library().await.unwrap();
        assert!(lib.iter().any(|i| i.id == "demo:g1" && i.state == ItemState::Installed));
        assert!(!lib.iter().any(|i| i.id == "demo:s1"), "item da loja não está na biblioteca");
        let store = p.store("jogos").await.unwrap();
        assert_eq!(store.len(), 4);
        assert!(p.launch("demo:s1").await.is_err(), "não instalado não abre");
        p.install("demo:s4").await.unwrap();
        let lib = p.library().await.unwrap();
        assert_eq!(lib.iter().find(|i| i.id == "demo:s4").unwrap().state, ItemState::Installing);
        tokio::time::sleep(Duration::from_millis(400)).await;
        let lib = p.library().await.unwrap();
        assert_eq!(lib.iter().find(|i| i.id == "demo:s4").unwrap().state, ItemState::Installed);
        assert!((p.wallet() - 310.1).abs() < 0.01, "cobrou o preço em oferta: {}", p.wallet());
        assert!(p.launch("demo:s4").await.is_ok());
        p.uninstall("demo:s4").await.unwrap();
        assert!(p.launch("demo:s4").await.is_err());
        assert!(!p.store("ofertas").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn insufficient_funds() {
        let db = Arc::new(Db::memory().unwrap());
        db.kv_set("demo_wallet", "10.00").unwrap();
        let p = DemoProvider::new(PathBuf::from("/x"), db);
        let e = p.install("demo:s2").await.unwrap_err();
        assert!(e.to_string().contains("saldo"));
    }
}
