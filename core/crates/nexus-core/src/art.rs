//! Arte dos itens: arquivos locais dos provedores ou imagens baixadas uma vez e guardadas em cache.
use crate::core::Core;
use crate::providers::{ArtKind, ArtSource};
use axum::body::Body;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// Um só cliente HTTP para todas as capas: reaproveita as conexões TLS com a CDN (a 1ª carga da loja
/// era lenta porque cada imagem abria uma conexão nova).
fn agent() -> &'static ureq::Agent {
    static A: OnceLock<ureq::Agent> = OnceLock::new();
    A.get_or_init(|| ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(8))).build().into())
}

/// Um download por arquivo: pedidos simultâneos da mesma capa (tela + pré-carga) esperam o primeiro.
fn inflight(key: &str) -> Arc<tokio::sync::Mutex<()>> {
    static M: OnceLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> = OnceLock::new();
    let mut m = M.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if m.len() > 512 {
        m.retain(|_, v| Arc::strong_count(v) > 1);
    }
    m.entry(key.to_string()).or_default().clone()
}

fn ctype(p: &Path) -> Option<&'static str> {
    match p.extension()?.to_str()?.to_lowercase().as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "svg" => Some("image/svg+xml"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

fn miss() -> Response {
    Response::builder().status(StatusCode::NOT_FOUND).body(Body::empty()).unwrap()
}

fn cache_name(id: &str, kind: &str) -> String {
    let safe: String = id.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    format!("{safe}-{kind}")
}

async fn fetch(urls: Vec<String>, dest: PathBuf, miss_mark: PathBuf) -> Option<PathBuf> {
    tokio::task::spawn_blocking(move || {
        let agent = agent();
        let mut ok = None;
        for url in &urls {
            let res = agent.get(url).call().ok();
            ok = res.and_then(|mut r| {
            let ct = r.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
            if !ct.starts_with("image/") {
                return None;
            }
            let bytes = r.body_mut().with_config().limit(12 * 1024 * 1024).read_to_vec().ok()?;
            let ext = if ct.contains("png") { "png" } else if ct.contains("webp") { "webp" } else { "jpg" };
            let p = dest.with_extension(ext);
            std::fs::write(&p, bytes).ok()?;
            Some(p)
            });
            if ok.is_some() {
                break;
            }
        }
        if ok.is_none() {
            let _ = std::fs::write(&miss_mark, b"");
        }
        ok
    })
    .await
    .ok()
    .flatten()
}

pub async fn serve(core: &Arc<Core>, id: &str, kind: &str) -> Response {
    if kind == "frame" {
        // quadro do jogo capturado ao abrir a Central (só este arquivo)
        return send(&core.cfg.runtime_dir.join("frame.png")).await;
    }
    if kind == "photo" {
        // fotos de perfil: /art/photo:<arquivo>/photo
        return match crate::avatars::path(core, id) {
            Some(p) => send(&p).await,
            None => miss(),
        };
    }
    match ensure(core, id, kind).await {
        Some(p) => send(&p).await,
        None => miss(),
    }
}

/// Caminho local da arte (baixa e guarda em cache se for remota).
async fn ensure(core: &Arc<Core>, id: &str, kind: &str) -> Option<PathBuf> {
    let kind_e = ArtKind::parse(kind)?;
    let provider = core.provider_for(id)?;
    let urls = match provider.art(id, kind_e) {
        ArtSource::File(p) => return Some(p),
        ArtSource::Missing => return None,
        ArtSource::Url(u) => vec![u],
        ArtSource::Urls(v) => v,
    };
    let dir = core.cfg.cache_dir.join("art");
    let _ = std::fs::create_dir_all(&dir);
    let name = cache_name(id, kind);
    let base = dir.join(&name);
    let lookup = || {
        let hit = ["jpg", "png", "webp"].iter().map(|e| base.with_extension(e)).find(|p| p.is_file());
        let mark = base.with_extension("miss");
        let fresh_miss = mark.metadata().and_then(|m| m.modified()).ok().is_some_and(|t| t.elapsed().unwrap_or_default() < Duration::from_secs(3600));
        (hit, fresh_miss)
    };
    if let (Some(p), _) = lookup() {
        return Some(p);
    }
    let lock = inflight(&name);
    let _g = lock.lock().await;
    match lookup() {
        (Some(p), _) => Some(p),
        (None, true) => None,
        (None, false) => fetch(urls, base.clone(), base.with_extension("miss")).await,
    }
}

/// Baixa em segundo plano as capas de uma lista (ex.: a vitrine da loja), 6 de cada vez,
/// para que a tela já as encontre em cache.
pub fn prefetch(core: &Arc<Core>, ids: Vec<String>) {
    let core = core.clone();
    tokio::spawn(async move {
        let q = Arc::new(Mutex::new(ids));
        let mut tasks = vec![];
        for _ in 0..6 {
            let (core, q) = (core.clone(), q.clone());
            tasks.push(tokio::spawn(async move {
                loop {
                    let next = q.lock().unwrap_or_else(|e| e.into_inner()).pop();
                    let Some(id) = next else { break };
                    let _ = ensure(&core, &id, "cover").await;
                }
            }));
        }
        for t in tasks {
            let _ = t.await;
        }
    });
}

async fn send(path: &Path) -> Response {
    let Some(ct) = ctype(path) else { return miss() };
    match tokio::fs::read(path).await {
        Ok(bytes) if bytes.len() <= 12 * 1024 * 1024 => Response::builder()
            .header(header::CONTENT_TYPE, ct)
            .header(header::CACHE_CONTROL, "max-age=86400")
            .header("X-Content-Type-Options", "nosniff")
            .header(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("sandbox; default-src 'none'; style-src 'unsafe-inline'"))
            .body(Body::from(bytes))
            .unwrap(),
        _ => miss(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_are_filesystem_safe() {
        assert_eq!(cache_name("steam:570", "cover"), "steam_570-cover");
        assert_eq!(cache_name("../../etc/passwd", "x"), "______etc_passwd-x");
    }
    #[test]
    fn content_types() {
        assert_eq!(ctype(Path::new("a.JPG")), Some("image/jpeg"));
        assert_eq!(ctype(Path::new("a.xpm")), None);
        assert_eq!(ctype(Path::new("a")), None);
    }
}
