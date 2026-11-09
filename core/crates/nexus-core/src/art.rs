//! Arte dos itens: arquivos locais dos provedores ou imagens baixadas uma vez e guardadas em cache.
use crate::core::Core;
use crate::providers::{ArtKind, ArtSource};
use axum::body::Body;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

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

async fn fetch(url: String, dest: PathBuf, miss_mark: PathBuf) -> Option<PathBuf> {
    tokio::task::spawn_blocking(move || {
        let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(8))).build().into();
        let res = agent.get(&url).call().ok();
        let ok = res.and_then(|mut r| {
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
    let Some(kind_e) = ArtKind::parse(kind) else { return miss() };
    let Some(provider) = core.provider_for(id) else { return miss() };
    let path = match provider.art(id, kind_e) {
        ArtSource::File(p) => p,
        ArtSource::Missing => return miss(),
        ArtSource::Url(url) => {
            let dir = core.cfg.cache_dir.join("art");
            let _ = std::fs::create_dir_all(&dir);
            let base = dir.join(cache_name(id, kind));
            let hit = ["jpg", "png", "webp"].iter().map(|e| base.with_extension(e)).find(|p| p.is_file());
            let mark = base.with_extension("miss");
            let fresh_miss = mark.metadata().and_then(|m| m.modified()).ok().is_some_and(|t| t.elapsed().unwrap_or_default() < Duration::from_secs(3600));
            match hit {
                Some(p) => p,
                None if fresh_miss => return miss(),
                None => match fetch(url, base, mark).await {
                    Some(p) => p,
                    None => return miss(),
                },
            }
        }
    };
    let Some(ct) = ctype(&path) else { return miss() };
    match tokio::fs::read(&path).await {
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
