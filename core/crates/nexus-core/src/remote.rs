//! Controle no celular: QR de pareamento, gamepad P1/P2, mouse e teclado. O celular abre uma página
//! (PWA) servida pelo Core; sem /dev/uinput as entradas são repassadas à Shell como eventos.
use crate::config::Config;
use crate::core::Event;
use crate::db::Db;
use crate::util;
use crate::vdev::{InputSink, PadState, Uinput, Unavailable};
use nexus_protocol::{RemoteDevice, RemotePairing};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

pub struct Hub {
    db: Arc<Db>,
    events: broadcast::Sender<Event>,
    host: Option<String>,
    bind: SocketAddr,
    port: AtomicU16,
    pub sink: Arc<dyn InputSink>,
    devices: Mutex<Vec<RemoteDevice>>,
    fails: Mutex<HashMap<IpAddr, (u32, Instant)>>,
}

pub fn qr_svg(data: &str) -> String {
    use qrcode::{render::svg, QrCode};
    QrCode::new(data.as_bytes())
        .map(|c| c.render::<svg::Color>().min_dimensions(260, 260).dark_color(svg::Color("#04201c")).light_color(svg::Color("#ffffff")).quiet_zone(true).build())
        .unwrap_or_default()
}

pub fn lan_ip() -> Option<IpAddr> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    let ip = s.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

/// Bordas do estado do gamepad virtual → teclas de navegação da Shell (`remote.input`).
pub fn nav_edges(prev: &PadState, cur: &PadState) -> Vec<(&'static str, bool)> {
    let held = |s: &PadState| -> HashSet<&'static str> {
        let mut h = HashSet::new();
        for (b, k) in [("a", "A"), ("b", "B"), ("x", "OPT"), ("y", "Y"), ("start", "OPT"), ("select", "Y"), ("l", "LB"), ("r", "RB"), ("l2", "LT"), ("r2", "RT"), ("mode", "G")] {
            if s.buttons.contains(b) {
                h.insert(k);
            }
        }
        let (mut hx, mut hy) = s.hat;
        let ax = |n: &str| s.axes.get(n).copied().unwrap_or(0.0);
        if ax("x") > 0.55 {
            hx = 1
        } else if ax("x") < -0.55 {
            hx = -1
        }
        if ax("y") > 0.55 {
            hy = 1
        } else if ax("y") < -0.55 {
            hy = -1
        }
        match hx {
            1 => h.insert("r"),
            -1 => h.insert("l"),
            _ => false,
        };
        match hy {
            1 => h.insert("d"),
            -1 => h.insert("u"),
            _ => false,
        };
        h
    };
    let (a, b) = (held(prev), held(cur));
    let mut out: Vec<(&'static str, bool)> = b.difference(&a).map(|k| (*k, true)).collect();
    out.extend(a.difference(&b).map(|k| (*k, false)));
    out
}

impl Hub {
    pub fn new(cfg: &Config, db: Arc<Db>, events: broadcast::Sender<Event>) -> Hub {
        let sink: Arc<dyn InputSink> = if cfg.uinput {
            match Uinput::probe() {
                Some(u) => Arc::new(u),
                None => Arc::new(Unavailable),
            }
        } else {
            Arc::new(Unavailable)
        };
        let hub = Hub { db, events, host: cfg.remote_host.clone(), bind: cfg.remote_bind, port: AtomicU16::new(cfg.remote_bind.port()), sink, devices: Mutex::new(vec![]), fails: Mutex::new(HashMap::new()) };
        if hub.db.kv_get("remote_token").is_none() {
            hub.rotate_token();
        }
        hub
    }

    pub fn set_port(&self, p: u16) {
        self.port.store(p, Ordering::SeqCst);
    }

    pub fn bind(&self) -> SocketAddr {
        self.bind
    }

    pub fn token(&self) -> String {
        self.db.kv_get("remote_token").unwrap_or_default()
    }

    pub fn rotate_token(&self) {
        let _ = self.db.kv_set("remote_token", &util::random_hex(24));
        let ids: Vec<String> = self.devices.lock().unwrap_or_else(|e| e.into_inner()).iter().map(|d| d.id.clone()).collect();
        for id in ids {
            self.disconnect(&id);
        }
    }

    pub fn url(&self) -> String {
        let host = self.host.clone().or_else(|| lan_ip().map(|i| i.to_string())).unwrap_or_else(|| "127.0.0.1".into());
        format!("http://{host}:{}/?t={}", self.port.load(Ordering::SeqCst), self.token())
    }

    pub fn pairing(&self) -> RemotePairing {
        let url = self.url();
        RemotePairing { qr_svg: qr_svg(&url), url, devices: self.devices.lock().unwrap_or_else(|e| e.into_inner()).clone(), uinput: self.sink.available() }
    }

    pub fn token_ok(&self, ip: IpAddr, t: &str) -> Result<(), &'static str> {
        let mut f = self.fails.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((n, at)) = f.get(&ip) {
            if *n >= 8 && at.elapsed() < Duration::from_secs(60) {
                return Err("Muitas tentativas. Aguarde um minuto.");
            }
        }
        let good = {
            let tok = self.token();
            tok.len() == t.len() && tok.bytes().zip(t.bytes()).fold(0u8, |a, (x, y)| a | (x ^ y)) == 0
        };
        if good {
            f.remove(&ip);
            Ok(())
        } else {
            let e = f.entry(ip).or_insert((0, Instant::now()));
            if e.1.elapsed() > Duration::from_secs(60) {
                *e = (0, Instant::now());
            }
            e.0 += 1;
            e.1 = Instant::now();
            Err("Código de pareamento inválido. Escaneie o QR code de novo.")
        }
    }

    fn broadcast_devices(&self) {
        let d = self.devices.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let _ = self.events.send(Event { method: "remote.devices".into(), params: json!({ "devices": d }) });
    }

    pub fn join(&self, name: &str, mode: &str, player: Option<u8>) -> Result<String, String> {
        let mut d = self.devices.lock().unwrap_or_else(|e| e.into_inner());
        if d.len() >= 6 {
            return Err("Limite de aparelhos conectados".into());
        }
        if mode == "gamepad" {
            let p = player.filter(|p| (1..=2).contains(p)).ok_or("Jogador inválido")?;
            if d.iter().any(|x| x.player == Some(p)) {
                return Err(format!("Jogador {p} já está em uso"));
            }
        } else if mode != "touchpad" {
            return Err("Modo inválido".into());
        }
        let id = util::random_hex(8);
        d.push(RemoteDevice { id: id.clone(), name: name.chars().take(24).collect(), mode: mode.into(), player: if mode == "gamepad" { player } else { None } });
        drop(d);
        self.broadcast_devices();
        Ok(id)
    }

    pub fn leave(&self, id: &str) {
        let removed = {
            let mut d = self.devices.lock().unwrap_or_else(|e| e.into_inner());
            let i = d.iter().position(|x| x.id == id);
            i.map(|i| d.remove(i))
        };
        if let Some(r) = removed {
            if let Some(p) = r.player {
                self.sink.release(p);
                let _ = self.events.send(Event { method: "remote.input".into(), params: json!({ "reset": true }) });
            }
            self.broadcast_devices();
        }
    }

    pub fn disconnect(&self, id: &str) {
        // a conexão WebSocket nota que o aparelho saiu da lista e se encerra
        self.leave(id);
    }

    pub fn is_joined(&self, id: &str) -> bool {
        self.devices.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|d| d.id == id)
    }

    fn emit(&self, method: &str, params: Value) {
        let _ = self.events.send(Event { method: method.into(), params });
    }

    /// Trata uma mensagem do celular já associada a um aparelho. `pad` guarda o último estado.
    pub fn handle(&self, dev: &RemoteDevice, pad: &mut PadState, msg: &Value) {
        let i = |k: &str| msg.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0) as i32;
        let mods: Vec<String> = msg.get("mods").and_then(|m| m.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
        let avail = self.sink.available();
        match (dev.mode.as_str(), msg.get("t").and_then(|t| t.as_str()).unwrap_or("")) {
            ("gamepad", "state") => {
                let buttons: HashSet<String> = msg.get("buttons").and_then(|b| b.as_array()).map(|a| a.iter().filter_map(|x| x.as_str()).filter(|n| crate::inputmap::pad_button(n).is_some()).map(String::from).collect()).unwrap_or_default();
                let hat = msg.get("hat").and_then(|h| h.as_array()).map(|h| (h.first().and_then(|v| v.as_i64()).unwrap_or(0).clamp(-1, 1) as i32, h.get(1).and_then(|v| v.as_i64()).unwrap_or(0).clamp(-1, 1) as i32)).unwrap_or((0, 0));
                let mut axes = HashMap::new();
                if let Some(a) = msg.get("axes").and_then(|a| a.as_object()) {
                    for n in ["x", "y", "rx", "ry"] {
                        if let Some(v) = a.get(n).and_then(|v| v.as_f64()) {
                            axes.insert(n.to_string(), (v as f32).clamp(-1.0, 1.0));
                        }
                    }
                }
                let cur = PadState { buttons, hat, axes };
                if avail {
                    self.sink.gamepad(dev.player.unwrap_or(1), &cur);
                    self.emit("remote.activity", json!({ "mode": "gamepad" }));
                } else if dev.player == Some(1) {
                    for (k, down) in nav_edges(pad, &cur) {
                        self.emit("remote.input", json!({ "key": k, "down": down }));
                    }
                }
                *pad = cur;
            }
            ("touchpad", "move") if avail => self.sink.mouse_move(i("dx").clamp(-200, 200), i("dy").clamp(-200, 200)),
            ("touchpad", "move") => self.emit("remote.pointer", json!({ "dx": i("dx").clamp(-200, 200), "dy": i("dy").clamp(-200, 200) })),
            ("touchpad", "scroll") if avail => self.sink.scroll(i("v"), i("h")),
            ("touchpad", "scroll") => self.emit("remote.scroll", json!({ "v": i("v").clamp(-20, 20) })),
            ("touchpad", "click") => {
                let b = msg.get("b").and_then(|b| b.as_str()).unwrap_or("left");
                let down = msg.get("down").and_then(|d| d.as_bool()).unwrap_or(true);
                if avail {
                    self.sink.button(b, down)
                } else if crate::inputmap::mouse_button(b).is_some() {
                    self.emit("remote.button", json!({ "b": b, "down": down }))
                }
            }
            ("touchpad", "key") => {
                let k = msg.get("key").and_then(|k| k.as_str()).unwrap_or("");
                if avail {
                    self.sink.key(k, &mods)
                } else {
                    self.emit("remote.key", json!({ "key": k, "mods": mods }))
                }
            }
            ("touchpad", "text") => {
                let t = msg.get("text").and_then(|t| t.as_str()).unwrap_or("");
                let back = i("back").clamp(0, 200) as u32;
                if avail {
                    self.sink.text(back, t, &mods)
                } else {
                    self.emit("remote.text", json!({ "back": back, "text": t.chars().take(200).collect::<String>() }))
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hub() -> Hub {
        let d = tempfile::tempdir().unwrap();
        let cfg = Config::for_test(d.path());
        let (tx, _) = broadcast::channel(16);
        Hub::new(&cfg, Arc::new(Db::memory().unwrap()), tx)
    }

    #[test]
    fn pairing_has_token_url_and_svg_qr() {
        let h = hub();
        let p = h.pairing();
        assert!(p.url.starts_with("http://127.0.0.1:") && p.url.contains("/?t="));
        assert!(p.url.ends_with(&h.token()));
        assert!(p.qr_svg.starts_with("<?xml") || p.qr_svg.contains("<svg"));
        assert!(!p.uinput);
    }

    #[test]
    fn token_validation_and_lockout() {
        let h = hub();
        let ip: IpAddr = "10.0.0.5".parse().unwrap();
        assert!(h.token_ok(ip, &h.token()).is_ok());
        for _ in 0..8 {
            assert!(h.token_ok(ip, "errado").is_err());
        }
        assert!(h.token_ok(ip, &h.token()).unwrap_err().contains("Muitas"), "bloqueado mesmo com token certo");
        let other: IpAddr = "10.0.0.6".parse().unwrap();
        assert!(h.token_ok(other, &h.token()).is_ok());
    }

    #[test]
    fn rotate_invalidates_and_disconnects() {
        let h = hub();
        let old = h.token();
        let id = h.join("Moto", "touchpad", None).unwrap();
        h.rotate_token();
        assert_ne!(old, h.token());
        assert!(!h.is_joined(&id));
    }

    #[test]
    fn join_rules() {
        let h = hub();
        assert!(h.join("a", "gamepad", Some(3)).is_err());
        let a = h.join("a", "gamepad", Some(1)).unwrap();
        assert!(h.join("b", "gamepad", Some(1)).is_err(), "jogador ocupado");
        assert!(h.join("b", "gamepad", Some(2)).is_ok());
        assert!(h.join("c", "touchpad", None).is_ok());
        assert!(h.join("c", "xyz", None).is_err());
        h.leave(&a);
        assert!(h.join("d", "gamepad", Some(1)).is_ok(), "vaga liberada");
    }

    #[test]
    fn nav_edges_from_pad() {
        let none = PadState::default();
        let a = PadState { buttons: ["a".to_string()].into(), hat: (0, 1), ..Default::default() };
        let mut e = nav_edges(&none, &a);
        e.sort();
        assert_eq!(e, vec![("A", true), ("d", true)]);
        let mut e = nav_edges(&a, &none);
        e.sort();
        assert_eq!(e, vec![("A", false), ("d", false)]);
        let stick = PadState { axes: [("x".to_string(), -0.9)].into(), ..Default::default() };
        assert_eq!(nav_edges(&none, &stick), vec![("l", true)]);
    }

    #[test]
    fn fallback_forwards_events_to_shell() {
        let h = hub();
        let mut rx = h.events.subscribe();
        let id = h.join("tel", "touchpad", None).unwrap();
        let dev = h.devices.lock().unwrap().iter().find(|d| d.id == id).cloned().unwrap();
        let mut pad = PadState::default();
        h.handle(&dev, &mut pad, &json!({ "t": "move", "dx": 500, "dy": -3 }));
        h.handle(&dev, &mut pad, &json!({ "t": "text", "back": 1, "text": "oi" }));
        let mut seen = vec![];
        while let Ok(e) = rx.try_recv() {
            seen.push((e.method, e.params));
        }
        assert!(seen.iter().any(|(m, p)| m == "remote.pointer" && p["dx"] == 200 && p["dy"] == -3));
        assert!(seen.iter().any(|(m, p)| m == "remote.text" && p["text"] == "oi"));
    }
}
