//! Mídia (filmes, séries e músicas) tocada pelo `mpv` em tela cheia.
//!
//! Fontes lidas como uma só: a pasta `Nexus/` do Google Drive montada (`Filmes`, `Séries`, `Músicas`) e as
//! pastas da TV (`~/Vídeos`/`~/Videos`, `~/Música`/`~/Music`). Mesmo nome em duas fontes aparece uma vez e a cópia da TV
//! vence. Da nuvem o mpv toca em streaming pela montagem (sem baixar antes).
//!
//! `NEXUS_MPV` troca o binário; `NEXUS_VIDEOS_DIR` / `NEXUS_MUSIC_DIR` trocam as pastas da TV (testes).
use super::*;
use nexus_protocol::*;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const VIDEO_EXTS: &[&str] = &["mkv", "mp4", "avi", "mov", "webm", "m4v", "ts", "m2ts", "mpg", "mpeg", "wmv", "flv", "ogv"];
pub const AUDIO_EXTS: &[&str] = &["mp3", "flac", "ogg", "oga", "opus", "m4a", "aac", "wav", "wma", "ape"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Movie,
    Series,
    Music,
}

impl Kind {
    fn tag(self) -> &'static str {
        match self {
            Kind::Movie => "filme",
            Kind::Series => "serie",
            Kind::Music => "musica",
        }
    }
    fn category(self) -> &'static str {
        match self {
            Kind::Movie => "Filmes",
            Kind::Series => "Séries",
            Kind::Music => "Músicas",
        }
    }
}

/// Um arquivo de mídia encontrado.
#[derive(Debug, Clone)]
pub struct Found {
    pub kind: Kind,
    /// série (ou pasta/álbum da música); vazio para filme solto
    pub show: String,
    /// temporada/subpasta, se houver
    pub season: String,
    pub path: PathBuf,
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

/// Pastas de vídeo da TV.
pub fn video_dirs() -> Vec<PathBuf> {
    if let Some(d) = std::env::var_os("NEXUS_VIDEOS_DIR") {
        return vec![PathBuf::from(d)];
    }
    ["Vídeos", "Videos"].iter().map(|n| home().join(n)).filter(|p| p.is_dir()).collect()
}

pub fn music_dirs() -> Vec<PathBuf> {
    if let Some(d) = std::env::var_os("NEXUS_MUSIC_DIR") {
        return vec![PathBuf::from(d)];
    }
    ["Música", "Music"].iter().map(|n| home().join(n)).filter(|p| p.is_dir()).collect()
}

fn mpv_bin() -> String {
    std::env::var("NEXUS_MPV").unwrap_or_else(|_| "mpv".into())
}

fn is_series_dir(name: &str) -> bool {
    let n = name.to_lowercase();
    n == "séries" || n == "series"
}

fn ext_of(p: &Path) -> String {
    p.extension().and_then(|x| x.to_str()).map(|x| x.to_lowercase()).unwrap_or_default()
}

fn walk(dir: &Path, depth: usize, max: usize, exts: &[&str], out: &mut Vec<PathBuf>, skip_series: bool) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            if depth < max && !(skip_series && depth == 0 && is_series_dir(&name)) {
                walk(&p, depth + 1, max, exts, out, skip_series);
            }
        } else if exts.contains(&ext_of(&p).as_str()) {
            out.push(p);
        }
    }
}

/// Filmes: todos os vídeos da pasta (até 3 níveis); em `~/Vídeos` a subpasta `Séries` fica de fora (é lida como séries).
pub fn scan_movies(dir: &Path, skip_series: bool) -> Vec<Found> {
    let mut v = vec![];
    walk(dir, 0, 3, VIDEO_EXTS, &mut v, skip_series);
    v.into_iter().map(|path| Found { kind: Kind::Movie, show: String::new(), season: String::new(), path }).collect()
}

/// Séries: `<série>/[<temporada>/]<episódio>`; arquivo solto vai para "Outros".
pub fn scan_series(dir: &Path) -> Vec<Found> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            let Ok(sub) = std::fs::read_dir(&p) else { continue };
            for s in sub.flatten() {
                let sp = s.path();
                let sname = s.file_name().to_string_lossy().to_string();
                if sname.starts_with('.') {
                    continue;
                }
                if sp.is_dir() {
                    let mut files = vec![];
                    walk(&sp, 0, 2, VIDEO_EXTS, &mut files, false);
                    out.extend(files.into_iter().map(|path| Found { kind: Kind::Series, show: name.clone(), season: sname.clone(), path }));
                } else if VIDEO_EXTS.contains(&ext_of(&sp).as_str()) {
                    out.push(Found { kind: Kind::Series, show: name.clone(), season: String::new(), path: sp });
                }
            }
        } else if VIDEO_EXTS.contains(&ext_of(&p).as_str()) {
            out.push(Found { kind: Kind::Series, show: "Outros".into(), season: String::new(), path: p });
        }
    }
    out
}

/// Músicas: arquivos de áudio; a pasta de cada faixa vira o "álbum".
pub fn scan_music(dir: &Path) -> Vec<Found> {
    let mut v = vec![];
    walk(dir, 0, 3, AUDIO_EXTS, &mut v, false);
    v.into_iter()
        .map(|path| {
            let album = path.parent().filter(|p| *p != dir).and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            Found { kind: Kind::Music, show: album, season: String::new(), path }
        })
        .collect()
}

/// Subpasta do Drive montado (a nuvem só é lida montada: senão seria uma pasta vazia ou uma montagem caída).
fn cloud_dir(sub: &str) -> Option<PathBuf> {
    if !crate::drive::mounted() {
        return None;
    }
    crate::drive::mount_dir().map(|m| m.join(sub))
}

/// Tudo junto: TV primeiro, nuvem depois; repetido (mesmo tipo, série e nome de arquivo) some e a TV vence.
pub fn scan_all() -> Vec<Found> {
    let mut all = vec![];
    for d in video_dirs() {
        all.extend(scan_movies(&d, true));
        for s in ["Séries", "Series"] {
            all.extend(scan_series(&d.join(s)));
        }
    }
    for d in music_dirs() {
        all.extend(scan_music(&d));
    }
    if let Some(d) = cloud_dir("Filmes") {
        all.extend(scan_movies(&d, false));
    }
    if let Some(d) = cloud_dir("Séries") {
        all.extend(scan_series(&d));
    }
    if let Some(d) = cloud_dir("Músicas") {
        all.extend(scan_music(&d));
    }
    dedupe(all)
}

pub fn dedupe(all: Vec<Found>) -> Vec<Found> {
    let mut seen = HashSet::new();
    let mut out = vec![];
    for f in all {
        let name = f.path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        if seen.insert((f.kind, f.show.to_lowercase(), name)) {
            out.push(f);
        }
    }
    out
}

type Nat = Vec<(u8, String, u64)>;

/// Ordem "natural": episódio 2 antes do 10.
pub fn natural_key(s: &str) -> Nat {
    let mut v: Nat = vec![];
    let mut cur = String::new();
    let mut digit = false;
    fn flush(cur: &mut String, digit: bool, v: &mut Nat) {
        if cur.is_empty() {
            return;
        }
        if digit {
            v.push((0, String::new(), cur.parse().unwrap_or(0)));
        } else {
            v.push((1, cur.to_lowercase(), 0));
        }
        cur.clear();
    }
    for c in s.chars() {
        if c.is_ascii_digit() != digit {
            flush(&mut cur, digit, &mut v);
            digit = c.is_ascii_digit();
        }
        cur.push(c);
    }
    flush(&mut cur, digit, &mut v);
    v
}

/// Título limpo do arquivo: sem extensão, sem [tags] (qualidade etc.) e com espaços no lugar de `_` e `.`.
pub fn title_of(stem: &str) -> String {
    let mut t = String::new();
    let mut depth = 0;
    for c in stem.chars() {
        match c {
            '[' => depth += 1,
            ']' => depth = (depth - 1).max(0),
            _ if depth == 0 => t.push(c),
            _ => {}
        }
    }
    let t = if t.trim().contains(' ') { t } else { t.replace('.', " ") };
    let t = t.replace('_', " ").split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() { stem.to_string() } else { t }
}

fn item_id(f: &Found) -> String {
    let h = Sha256::digest(f.path.to_string_lossy().as_bytes());
    format!("media:{}-{}", f.kind.tag(), h.iter().take(6).map(|b| format!("{b:02x}")).collect::<String>())
}

/// Argumentos do mpv. `playlist` são as faixas seguintes (música).
pub fn mpv_argv(bin: &str, file: &Path, kind: Kind, cloud: bool, watch_later: &Path, ipc: &Path, playlist: &[PathBuf]) -> Vec<String> {
    let mut a = vec![
        bin.to_string(),
        "--fs".into(),
        "--hwdec=auto-safe".into(),
        "--save-position-on-quit".into(),
        format!("--watch-later-directory={}", watch_later.display()),
        "--sub-auto=fuzzy".into(),
        format!("--input-ipc-server={}", ipc.display()),
        "--no-terminal".into(),
        "--force-window=immediate".into(),
        "--keep-open=no".into(),
    ];
    if kind == Kind::Music {
        a.push("--audio-display=embedded-first".into());
    }
    if cloud {
        // streaming pela montagem: um pouco de adiantamento, sem encher a memória
        a.extend(["--cache=yes".into(), "--demuxer-max-bytes=48MiB".into(), "--demuxer-readahead-secs=60".into()]);
    }
    a.push("--".into());
    a.push(file.display().to_string());
    a.extend(playlist.iter().map(|p| p.display().to_string()));
    a
}

pub struct MediaProvider {
    data_dir: PathBuf,
    runtime_dir: PathBuf,
    index: Mutex<HashMap<String, Found>>,
}

fn build_items(found: Vec<Found>) -> (Vec<Item>, HashMap<String, Found>) {
    let mut found = found;
    let key = |f: &Found| (f.kind as u8, natural_key(&f.show), natural_key(&f.season), natural_key(&f.path.file_name().unwrap_or_default().to_string_lossy()));
    found.sort_by_key(key);
    let mut idx = HashMap::new();
    let mut items = vec![];
    for f in found {
        let id = item_id(&f);
        let stem = f.path.file_stem().and_then(|x| x.to_str()).unwrap_or("Mídia");
        let mut it = Item::new(id.clone(), "media", ItemKind::App, title_of(stem));
        it.state = ItemState::Installed;
        it.category = Some(f.kind.category().into());
        it.tags = vec!["media".into(), f.kind.tag().into()];
        it.subtitle = match f.kind {
            Kind::Series => [f.show.as_str(), f.season.as_str()].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" · "),
            Kind::Music => f.show.clone(),
            Kind::Movie => String::new(),
        };
        if !f.show.is_empty() {
            it.tags.push(format!("show:{}", f.show));
        }
        if !f.season.is_empty() {
            it.tags.push(format!("season:{}", f.season));
        }
        it.hue = match f.kind {
            Kind::Movie => (350, 20),
            Kind::Series => (230, 280),
            Kind::Music => (150, 190),
        };
        if crate::drive::is_cloud(&f.path) {
            it.tags.push("cloud".into());
            it.subtitle = if it.subtitle.is_empty() { "Google Drive".into() } else { format!("{} · Google Drive", it.subtitle) };
        }
        it.size_bytes = std::fs::metadata(&f.path).ok().map(|m| m.len());
        idx.insert(id, f);
        items.push(it);
    }
    (items, idx)
}

impl MediaProvider {
    pub fn new(data_dir: PathBuf, runtime_dir: PathBuf) -> MediaProvider {
        MediaProvider { data_dir, runtime_dir, index: Mutex::new(HashMap::new()) }
    }
}

#[async_trait]
impl Provider for MediaProvider {
    fn id(&self) -> &'static str {
        "media"
    }

    async fn library(&self) -> anyhow::Result<Vec<Item>> {
        // a nuvem é lida pela montagem (pode demorar): fora da thread assíncrona
        let (items, idx) = tokio::task::spawn_blocking(|| build_items(scan_all())).await?;
        *self.index.lock().unwrap_or_else(|e| e.into_inner()) = idx;
        Ok(items)
    }

    async fn launch(&self, id: &str) -> anyhow::Result<LaunchPlan> {
        let mut f = self.index.lock().unwrap_or_else(|e| e.into_inner()).get(id).cloned();
        if f.is_none() {
            self.library().await?;
            f = self.index.lock().unwrap_or_else(|e| e.into_inner()).get(id).cloned();
        }
        let f = f.ok_or(ProviderError::NotFound)?;
        if !f.path.is_file() {
            return Err(ProviderError::Msg("o arquivo não está mais lá (o Google Drive está conectado?)".into()).into());
        }
        let wl = self.data_dir.join("mpv-watch-later");
        let _ = std::fs::create_dir_all(&wl);
        let ipc = self.runtime_dir.join("mpv.sock");
        let _ = std::fs::remove_file(&ipc);
        // música: depois da faixa escolhida, toca as seguintes da mesma pasta
        let playlist = if f.kind == Kind::Music {
            let mut sibs: Vec<PathBuf> = self.index.lock().unwrap_or_else(|e| e.into_inner()).values().filter(|o| o.kind == Kind::Music && o.path.parent() == f.path.parent()).map(|o| o.path.clone()).collect();
            sibs.sort_by_key(|p| natural_key(&p.file_name().unwrap_or_default().to_string_lossy()));
            sibs.into_iter().skip_while(|p| *p != f.path).skip(1).collect()
        } else {
            vec![]
        };
        let argv = mpv_argv(&mpv_bin(), &f.path, f.kind, crate::drive::is_cloud(&f.path), &wl, &ipc, &playlist);
        Ok(LaunchPlan { argv, env: vec![], cwd: None, tracker: Tracker::Process })
    }

    fn art(&self, _id: &str, _kind: ArtKind) -> ArtSource {
        ArtSource::Missing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"x").unwrap();
    }

    #[test]
    fn lists_and_dedupes_local_first() {
        let t = tempfile::tempdir().unwrap();
        let tv = t.path().join("tv");
        let cloud = t.path().join("cloud");
        touch(&tv.join("Matrix.mkv"));
        touch(&tv.join("Séries/Dark/Temporada 1/e01.mkv"));
        touch(&cloud.join("Filmes/matrix.MKV"));
        touch(&cloud.join("Filmes/Alien.mp4"));
        touch(&cloud.join("Séries/Dark/Temporada 1/e01.mkv"));
        touch(&cloud.join("Séries/Dark/Temporada 1/e02.mkv"));
        let mut all = scan_movies(&tv, true);
        all.extend(scan_series(&tv.join("Séries")));
        all.extend(scan_movies(&cloud.join("Filmes"), false));
        all.extend(scan_series(&cloud.join("Séries")));
        let all = dedupe(all);
        let movies: Vec<_> = all.iter().filter(|f| f.kind == Kind::Movie).collect();
        assert_eq!(movies.len(), 2, "Matrix aparece uma vez");
        assert!(movies.iter().any(|f| f.path.starts_with(&tv)), "a cópia da TV vence");
        let eps: Vec<_> = all.iter().filter(|f| f.kind == Kind::Series).collect();
        assert_eq!(eps.len(), 2);
        assert!(eps.iter().all(|f| f.show == "Dark" && f.season == "Temporada 1"));
        assert!(!scan_movies(&tv, true).iter().any(|f| f.path.to_string_lossy().contains("Dark")), "Séries fora dos filmes");
    }

    #[test]
    fn natural_order_and_titles() {
        assert!(natural_key("ep2.mkv") < natural_key("ep10.mkv"));
        assert_eq!(title_of("Meu_Filme.2020 [1080p]"), "Meu Filme 2020");
        assert_eq!(title_of("Blade Runner (1982)"), "Blade Runner (1982)");
    }

    #[test]
    fn mpv_argv_has_resume_subs_and_streaming() {
        let a = mpv_argv("mpv", Path::new("/mnt/drive/Filmes/a.mkv"), Kind::Movie, true, Path::new("/d/wl"), Path::new("/r/mpv.sock"), &[]);
        assert_eq!(a[0], "mpv");
        for want in ["--fs", "--hwdec=auto-safe", "--save-position-on-quit", "--watch-later-directory=/d/wl", "--sub-auto=fuzzy", "--input-ipc-server=/r/mpv.sock"] {
            assert!(a.iter().any(|x| x == want), "falta {want}");
        }
        assert!(a.iter().any(|x| x.starts_with("--demuxer-max-bytes")));
        assert_eq!(a.last().unwrap(), "/mnt/drive/Filmes/a.mkv");
        let m = mpv_argv("mpv", Path::new("/m/1.mp3"), Kind::Music, false, Path::new("/d"), Path::new("/r"), &[PathBuf::from("/m/2.mp3")]);
        assert!(m.iter().any(|x| x == "--audio-display=embedded-first") && !m.iter().any(|x| x.starts_with("--demuxer")));
        assert_eq!(&m[m.len() - 2..], ["/m/1.mp3", "/m/2.mp3"]);
    }
}
