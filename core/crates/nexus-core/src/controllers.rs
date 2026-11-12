//! Controles físicos: lista (USB, Bluetooth, virtual), captura de botões para o mapeamento,
//! mapa salvo por controle e exportação para o RetroArch (autoconfig udev), além do pareamento
//! Bluetooth: ver `bluetooth.rs`.
use crate::db::Db;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Ações do mapeamento, na ordem em que o assistente pede (id, rótulo).
pub const ACTIONS: &[(&str, &str)] = &[
    ("up", "D-PAD UP"),
    ("down", "D-PAD DOWN"),
    ("left", "D-PAD LEFT"),
    ("right", "D-PAD RIGHT"),
    ("start", "START"),
    ("select", "SELECT"),
    ("a", "BUTTON A"),
    ("b", "BUTTON B"),
    ("x", "BUTTON X"),
    ("y", "BUTTON Y"),
    ("l", "LEFT SHOULDER"),
    ("r", "RIGHT SHOULDER"),
    ("l2", "LEFT TRIGGER"),
    ("r2", "RIGHT TRIGGER"),
    ("l3", "LEFT THUMB"),
    ("r3", "RIGHT THUMB"),
    ("ls_up", "LEFT ANALOG UP"),
    ("ls_down", "LEFT ANALOG DOWN"),
    ("ls_left", "LEFT ANALOG LEFT"),
    ("ls_right", "LEFT ANALOG RIGHT"),
    ("rs_up", "RIGHT ANALOG UP"),
    ("rs_down", "RIGHT ANALOG DOWN"),
    ("rs_left", "RIGHT ANALOG LEFT"),
    ("rs_right", "RIGHT ANALOG RIGHT"),
    ("hotkey", "HOTKEY ENABLE"),
];

const BTN_MISC: u16 = 0x100;
const BTN_JOYSTICK: u16 = 0x120;
const BTN_SOUTH: u16 = 0x130;
const KEY_MAX: u16 = 0x2ff;
const ABS_HAT0X: u16 = 0x10;
const ABS_HAT3Y: u16 = 0x17;
const ABS_MISC: u16 = 0x28;

/// Um botão, um eixo (com sentido) ou um direcional digital (hat).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Binding {
    Button { code: u16 },
    Axis { code: u16, dir: i8 },
    Hat { code: u16, dir: i8 },
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Pad {
    /// estável entre reconexões: fabricante:produto:nome
    pub id: String,
    pub name: String,
    /// "usb", "bluetooth", "virtual" ou "outro"
    pub bus: String,
    pub vendor: u16,
    pub product: u16,
    pub mapped: bool,
    /// conectado agora
    pub connected: bool,
    /// cadastrado no Nexus (aparece mesmo desconectado)
    pub registered: bool,
    /// nome dado pelo usuário (vazio = nome do aparelho)
    pub nickname: String,
    /// jogador 1–4 (None = livre: entra na ordem em que conectar)
    pub player: Option<u8>,
}

/// Cadastro de um controle (fica salvo mesmo desconectado).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Reg {
    pub id: String,
    pub name: String,
    pub bus: String,
    pub vendor: u16,
    pub product: u16,
    pub nickname: String,
    pub player: Option<u8>,
}

pub fn regs(db: &Db) -> Vec<Reg> {
    db.kv_get("pads").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save_regs(db: &Db, v: &[Reg]) -> Result<(), String> {
    db.kv_set("pads", &serde_json::to_string(v).unwrap()).map_err(|e| e.to_string())?;
    write_players(v);
    Ok(())
}

/// Cria/atualiza o cadastro. Um jogador só pode ter um controle: quem já tinha o número fica livre.
pub fn save(db: &Db, id: &str, nickname: Option<String>, player: Option<Option<u8>>) -> Result<Reg, String> {
    if let Some(Some(p)) = player {
        if !(1..=4).contains(&p) {
            return Err("jogador deve ser de 1 a 4".into());
        }
    }
    let mut v = regs(db);
    if !v.iter().any(|r| r.id == id) {
        let pad = list_connected(db).into_iter().find(|p| p.id == id).ok_or("controle não encontrado — conecte-o para cadastrar")?;
        v.push(Reg { id: id.into(), name: pad.name, bus: pad.bus, vendor: pad.vendor, product: pad.product, nickname: String::new(), player: None });
    }
    if let Some(Some(p)) = player {
        for r in v.iter_mut().filter(|r| r.id != id && r.player == Some(p)) {
            r.player = None;
        }
    }
    let r = v.iter_mut().find(|r| r.id == id).unwrap();
    if let Some(n) = nickname {
        r.nickname = n.trim().chars().take(24).collect();
    }
    if let Some(p) = player {
        r.player = p;
    }
    let out = r.clone();
    save_regs(db, &v)?;
    Ok(out)
}

/// Exclui o cadastro, o mapa e o perfil gerado no RetroArch.
pub fn delete(db: &Db, id: &str) -> Result<(), String> {
    let v: Vec<Reg> = regs(db).into_iter().filter(|r| r.id != id).collect();
    if let Some(name) = id.splitn(3, ':').nth(2) {
        for dir in retroarch_dirs() {
            let _ = std::fs::remove_file(dir.join(format!("Nexus-{name}.cfg")));
        }
    }
    let _ = db.kv_del(&format!("padmap:{id}"));
    save_regs(db, &v)
}

fn slug(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect::<String>().split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-")
}

fn is_gamepad(d: &evdev::Device) -> bool {
    let keys = d.supported_keys();
    let has = |c: u16| keys.is_some_and(|k| k.contains(evdev::KeyCode::new(c)));
    has(BTN_SOUTH) || has(BTN_JOYSTICK) || has(0x121)
}

fn devices() -> Vec<(PathBuf, evdev::Device)> {
    let Ok(rd) = std::fs::read_dir("/dev/input") else { return vec![] };
    let mut v: Vec<(PathBuf, evdev::Device)> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("event")))
        .filter_map(|p| evdev::Device::open(&p).ok().map(|d| (p, d)))
        .filter(|(_, d)| is_gamepad(d))
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

fn pad_id(d: &evdev::Device) -> String {
    let i = d.input_id();
    format!("{:04x}:{:04x}:{}", i.vendor(), i.product(), slug(d.name().unwrap_or("controle")))
}

pub fn list(db: &Db) -> Vec<Pad> {
    let regs = regs(db);
    let mut out = list_connected(db);
    for p in out.iter_mut() {
        if let Some(r) = regs.iter().find(|r| r.id == p.id) {
            p.registered = true;
            p.nickname = r.nickname.clone();
            p.player = r.player;
        }
    }
    let connected: Vec<String> = out.iter().map(|p| p.id.clone()).collect();
    for r in regs.iter().filter(|r| !connected.contains(&r.id)) {
        out.push(Pad { id: r.id.clone(), name: r.name.clone(), bus: r.bus.clone(), vendor: r.vendor, product: r.product, mapped: db.kv_get(&format!("padmap:{}", r.id)).is_some(), connected: false, registered: true, nickname: r.nickname.clone(), player: r.player });
    }
    out.sort_by_key(|p| (p.player.unwrap_or(9), !p.connected));
    out
}

fn list_connected(db: &Db) -> Vec<Pad> {
    let mut out: Vec<Pad> = vec![];
    for (_, d) in devices() {
        let id = pad_id(&d);
        if out.iter().any(|p| p.id == id) {
            continue;
        }
        let i = d.input_id();
        let bus = match i.bus_type().0 {
            0x03 => "usb",
            0x05 => "bluetooth",
            0x06 => "virtual",
            _ => "outro",
        };
        out.push(Pad { mapped: db.kv_get(&format!("padmap:{id}")).is_some(), id, name: d.name().unwrap_or("Controle").to_string(), bus: bus.into(), vendor: i.vendor(), product: i.product(), connected: true, registered: false, nickname: String::new(), player: None });
    }
    out
}

fn find(id: &str) -> Option<(PathBuf, evdev::Device)> {
    devices().into_iter().find(|(_, d)| pad_id(d) == id)
}

/// Espera o próximo botão/eixo/direcional acionado no controle (ignora pequenos movimentos dos analógicos).
pub fn capture(id: &str, timeout: Duration) -> Result<Option<Binding>, String> {
    let (_, mut d) = find(id).ok_or("controle não encontrado (desconectado?)")?;
    let info: BTreeMap<u16, (i32, i32)> = d.get_absinfo().map(|it| it.map(|(a, i)| (a.0, (i.minimum(), i.maximum()))).collect()).unwrap_or_default();
    let fd = d.as_raw_fd();
    let end = Instant::now() + timeout;
    while Instant::now() < end {
        let mut pfd = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
        let ms = end.saturating_duration_since(Instant::now()).as_millis().min(500) as i32;
        if unsafe { libc::poll(&mut pfd, 1, ms) } <= 0 {
            continue;
        }
        let evs: Vec<evdev::InputEvent> = d.fetch_events().map_err(|e| e.to_string())?.collect();
        for ev in evs {
            match ev.event_type() {
                evdev::EventType::KEY if ev.value() == 1 => return Ok(Some(Binding::Button { code: ev.code() })),
                evdev::EventType::ABSOLUTE => {
                    let code = ev.code();
                    if (ABS_HAT0X..=ABS_HAT3Y).contains(&code) {
                        if ev.value() != 0 {
                            return Ok(Some(Binding::Hat { code, dir: ev.value().signum() as i8 }));
                        }
                        continue;
                    }
                    let (min, max) = info.get(&code).copied().unwrap_or((-32768, 32767));
                    let center = (min + max) / 2;
                    let half = ((max - min) / 2).max(1);
                    let off = ev.value() - center;
                    // gatilhos analógicos (0..max) e alavancas: precisa passar de 60% do curso
                    if off.abs() * 10 > half * 6 {
                        return Ok(Some(Binding::Axis { code, dir: off.signum() as i8 }));
                    }
                }
                _ => {}
            }
        }
    }
    Ok(None)
}

pub fn get_map(db: &Db, id: &str) -> Map<String, Value> {
    db.kv_get(&format!("padmap:{id}")).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Valida e guarda o mapa (só ações conhecidas e ligações bem formadas) e exporta para o RetroArch.
pub fn set_map(db: &Db, id: &str, map: &Map<String, Value>) -> Result<Vec<PathBuf>, String> {
    let mut clean = Map::new();
    for (k, v) in map {
        if !ACTIONS.iter().any(|(a, _)| a == k) {
            return Err(format!("ação desconhecida: {k}"));
        }
        let b: Binding = serde_json::from_value(v.clone()).map_err(|_| format!("ligação inválida em {k}"))?;
        clean.insert(k.clone(), serde_json::to_value(b).unwrap());
    }
    db.kv_set(&format!("padmap:{id}"), &Value::Object(clean.clone()).to_string()).map_err(|e| e.to_string())?;
    // mapear cadastra o controle (se ainda não estava)
    if !regs(db).iter().any(|r| r.id == id) {
        let _ = save(db, id, None, None);
    }
    Ok(match find(id) {
        Some((_, d)) => export_retroarch(&d, &clean),
        None => vec![],
    })
}

/// Índices que o driver udev do RetroArch dá aos botões (BTN_JOYSTICK…KEY_MAX, depois BTN_MISC…BTN_JOYSTICK)
/// e aos eixos (códigos < ABS_MISC, sem os direcionais digitais).
fn retro_indices(d: &evdev::Device) -> (BTreeMap<u16, usize>, BTreeMap<u16, usize>) {
    let keys: Vec<u16> = d.supported_keys().map(|k| k.iter().map(|c| c.code()).collect()).unwrap_or_default();
    let mut btn = BTreeMap::new();
    for c in keys.iter().filter(|c| (BTN_JOYSTICK..KEY_MAX).contains(*c)).chain(keys.iter().filter(|c| (BTN_MISC..BTN_JOYSTICK).contains(*c))) {
        let n = btn.len();
        btn.insert(*c, n);
    }
    let axes: Vec<u16> = d.supported_absolute_axes().map(|a| a.iter().map(|c| c.0).collect()).unwrap_or_default();
    let mut ax = BTreeMap::new();
    for c in axes.into_iter().filter(|c| *c < ABS_MISC && !(ABS_HAT0X..=ABS_HAT3Y).contains(c)) {
        let n = ax.len();
        ax.insert(c, n);
    }
    (btn, ax)
}

fn retro_key(action: &str) -> &'static str {
    match action {
        "up" => "up",
        "down" => "down",
        "left" => "left",
        "right" => "right",
        "start" => "start",
        "select" => "select",
        "a" => "a",
        "b" => "b",
        "x" => "x",
        "y" => "y",
        "l" => "l",
        "r" => "r",
        "l2" => "l2",
        "r2" => "r2",
        "l3" => "l3",
        "r3" => "r3",
        "ls_up" => "l_y_minus",
        "ls_down" => "l_y_plus",
        "ls_left" => "l_x_minus",
        "ls_right" => "l_x_plus",
        "rs_up" => "r_y_minus",
        "rs_down" => "r_y_plus",
        "rs_left" => "r_x_minus",
        "rs_right" => "r_x_plus",
        _ => "enable_hotkey",
    }
}

pub fn retroarch_cfg(name: &str, vendor: u16, product: u16, map: &Map<String, Value>, btn: &BTreeMap<u16, usize>, ax: &BTreeMap<u16, usize>) -> String {
    let mut s = format!("# Gerado pelo Nexus (Ajustes → Controles → Mapear botões)\ninput_driver = \"udev\"\ninput_device = \"{}\"\ninput_vendor_id = \"{vendor}\"\ninput_product_id = \"{product}\"\n", name.replace('"', ""));
    for (action, _) in ACTIONS {
        let Some(b) = map.get(*action).and_then(|v| serde_json::from_value::<Binding>(v.clone()).ok()) else { continue };
        let k = retro_key(action);
        match b {
            Binding::Button { code } => {
                if let Some(i) = btn.get(&code) {
                    s += &format!("input_{k}_btn = \"{i}\"\n");
                }
            }
            Binding::Hat { code, dir } => {
                let hat = (code - ABS_HAT0X) / 2;
                let dirn = match (code % 2 == 0, dir < 0) {
                    (true, true) => "left",
                    (true, false) => "right",
                    (false, true) => "up",
                    (false, false) => "down",
                };
                s += &format!("input_{k}_btn = \"h{hat}{dirn}\"\n");
            }
            Binding::Axis { code, dir } => {
                if let Some(i) = ax.get(&code) {
                    s += &format!("input_{k}_axis = \"{}{i}\"\n", if dir < 0 { '-' } else { '+' });
                }
            }
        }
    }
    s
}

fn retroarch_dirs() -> Vec<PathBuf> {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let mut v = vec![];
    for base in [home.join(".var/app/org.libretro.RetroArch/config/retroarch"), home.join(".config/retroarch")] {
        // só onde o RetroArch existe (Flatpak instalado ou configuração nativa)
        let exists = base.is_dir() || (base.starts_with(home.join(".var")) && Path::new("/var/lib/flatpak/app/org.libretro.RetroArch").exists());
        if exists {
            v.push(base.join("autoconfig/udev"));
        }
    }
    v
}

fn export_retroarch(d: &evdev::Device, map: &Map<String, Value>) -> Vec<PathBuf> {
    let (btn, ax) = retro_indices(d);
    let name = d.name().unwrap_or("Controle");
    let i = d.input_id();
    let cfg = retroarch_cfg(name, i.vendor(), i.product(), map, &btn, &ax);
    let file = format!("Nexus-{}.cfg", slug(name));
    retroarch_dirs()
        .into_iter()
        .filter_map(|dir| {
            std::fs::create_dir_all(&dir).ok()?;
            let p = dir.join(&file);
            std::fs::write(&p, &cfg).ok()?;
            Some(p)
        })
        .collect()
}

/// Reservas de jogador para o RetroArch (arquivo próprio, carregado com --appendconfig pelo Nexus):
/// o controle cadastrado como Jogador N vai para a porta N, não importa a ordem em que conectou.
pub fn players_cfg(regs: &[Reg]) -> String {
    let mut s = String::from("# Gerado pelo Nexus (Ajustes → Controles): jogador de cada controle\n");
    for r in regs.iter().filter(|r| r.player.is_some()) {
        let n = r.player.unwrap();
        s += &format!("input_player{n}_reserved_device = \"{:04x}:{:04x} {}\"\ninput_player{n}_device_reservation_type = \"2\"\n", r.vendor, r.product, r.name.replace('"', ""));
    }
    s
}

pub fn players_file() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config/nexus/retroarch-players.cfg")
}

fn write_players(regs: &[Reg]) {
    let p = players_file();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(p, players_cfg(regs));
}

pub fn actions_json() -> Value {
    Value::Array(ACTIONS.iter().map(|(id, label)| json!({ "id": id, "label": label })).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retroarch_config_uses_udev_indices() {
        let mut map = Map::new();
        map.insert("a".into(), json!({ "kind": "button", "code": 0x131 }));
        map.insert("up".into(), json!({ "kind": "hat", "code": 0x11, "dir": -1 }));
        map.insert("right".into(), json!({ "kind": "hat", "code": 0x10, "dir": 1 }));
        map.insert("ls_left".into(), json!({ "kind": "axis", "code": 0, "dir": -1 }));
        map.insert("r2".into(), json!({ "kind": "axis", "code": 5, "dir": 1 }));
        map.insert("hotkey".into(), json!({ "kind": "button", "code": 0x13c }));
        let btn: BTreeMap<u16, usize> = [(0x130, 0), (0x131, 1), (0x13c, 8)].into_iter().collect();
        let ax: BTreeMap<u16, usize> = [(0, 0), (1, 1), (5, 5)].into_iter().collect();
        let s = retroarch_cfg("Pad \"X\"", 1118, 654, &map, &btn, &ax);
        for want in ["input_device = \"Pad X\"", "input_vendor_id = \"1118\"", "input_a_btn = \"1\"", "input_up_btn = \"h0up\"", "input_right_btn = \"h0right\"", "input_l_x_minus_axis = \"-0\"", "input_r2_axis = \"+5\"", "input_enable_hotkey_btn = \"8\""] {
            assert!(s.contains(want), "faltou {want} em:\n{s}");
        }
        assert_eq!(ACTIONS.len(), 25);
    }

    #[test]
    fn players_are_unique_and_exported() {
        let db = Db::memory().unwrap();
        let a = Reg { id: "045e:028e:xbox".into(), name: "Xbox 360 Controller".into(), bus: "usb".into(), vendor: 0x45e, product: 0x28e, nickname: String::new(), player: Some(1) };
        let mut b = a.clone();
        b.id = "054c:0ce6:dualsense".into();
        b.name = "DualSense".into();
        b.vendor = 0x54c;
        b.product = 0xce6;
        b.player = None;
        db.kv_set("pads", &serde_json::to_string(&vec![a, b]).unwrap()).unwrap();
        let r = save(&db, "054c:0ce6:dualsense", Some("  Controle da Lia  ".into()), Some(Some(1))).unwrap();
        assert_eq!((r.nickname.as_str(), r.player), ("Controle da Lia", Some(1)));
        let all = regs(&db);
        assert_eq!(all.iter().find(|r| r.id.starts_with("045e")).unwrap().player, None, "o jogador 1 saiu do outro controle");
        assert!(save(&db, "054c:0ce6:dualsense", None, Some(Some(7))).is_err());
        let cfg = players_cfg(&all);
        assert!(cfg.contains("input_player1_reserved_device = \"054c:0ce6 DualSense\""), "{cfg}");
        delete(&db, "045e:028e:xbox").unwrap();
        assert_eq!(regs(&db).len(), 1);
    }
}
