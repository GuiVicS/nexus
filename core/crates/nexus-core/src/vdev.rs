//! Dispositivos virtuais de entrada (gamepad P1/P2, mouse, teclado) via /dev/uinput.
use crate::inputmap;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PadState {
    pub buttons: HashSet<String>,
    pub hat: (i32, i32),
    pub axes: HashMap<String, f32>,
}

pub trait InputSink: Send + Sync {
    /// true se os eventos chegam ao sistema (uinput). Se false, o Core repassa à Shell.
    fn available(&self) -> bool;
    fn gamepad(&self, player: u8, st: &PadState);
    fn release(&self, player: u8);
    fn mouse_move(&self, dx: i32, dy: i32);
    fn scroll(&self, v: i32, h: i32);
    fn button(&self, name: &str, down: bool);
    fn key(&self, name: &str, mods: &[String]);
    fn text(&self, back: u32, text: &str, mods: &[String]);
}

/// Sem uinput: nada é escrito no sistema.
pub struct Unavailable;
impl InputSink for Unavailable {
    fn available(&self) -> bool {
        false
    }
    fn gamepad(&self, _: u8, _: &PadState) {}
    fn release(&self, _: u8) {}
    fn mouse_move(&self, _: i32, _: i32) {}
    fn scroll(&self, _: i32, _: i32) {}
    fn button(&self, _: &str, _: bool) {}
    fn key(&self, _: &str, _: &[String]) {}
    fn text(&self, _: u32, _: &str, _: &[String]) {}
}

/// Sequência de (tipo, código, valor) que o Uinput escreveria; testável sem dispositivo.
pub type Ev = (u16, u16, i32);
const KEY: u16 = 1;
const REL: u16 = 2;
const ABS: u16 = 3;

pub fn pad_events(prev: &PadState, cur: &PadState) -> Vec<Ev> {
    let mut out = vec![];
    for name in inputmap::PAD_BUTTONS {
        let (a, b) = (prev.buttons.contains(name), cur.buttons.contains(name));
        if a != b {
            out.push((KEY, inputmap::pad_button(name).unwrap(), b as i32));
        }
    }
    if prev.hat.0 != cur.hat.0 {
        out.push((ABS, 0x10, cur.hat.0));
    }
    if prev.hat.1 != cur.hat.1 {
        out.push((ABS, 0x11, cur.hat.1));
    }
    for (name, code) in [("x", 0u16), ("y", 1), ("rx", 3), ("ry", 4)] {
        let f = |s: &PadState| (s.axes.get(name).copied().unwrap_or(0.0).clamp(-1.0, 1.0) * 32767.0) as i32;
        if f(prev) != f(cur) {
            out.push((ABS, code, f(cur)));
        }
    }
    out
}

pub fn text_events(back: u32, text: &str, mods: &[String]) -> Vec<Ev> {
    let mods: Vec<u16> = mods.iter().filter_map(|m| inputmap::modifier(m)).collect();
    let mut out = vec![];
    let tap = |out: &mut Vec<Ev>, code: u16, extra_shift: bool| {
        let mut m = mods.clone();
        if extra_shift && !m.contains(&inputmap::KEY_LEFTSHIFT) {
            m.push(inputmap::KEY_LEFTSHIFT);
        }
        for k in &m {
            out.push((KEY, *k, 1));
        }
        out.push((KEY, code, 1));
        out.push((KEY, code, 0));
        for k in m.iter().rev() {
            out.push((KEY, *k, 0));
        }
    };
    for _ in 0..back.min(200) {
        out.push((KEY, 14, 1));
        out.push((KEY, 14, 0));
    }
    for c in text.chars().take(200) {
        if let Some((code, shift)) = inputmap::char_to_key(c) {
            tap(&mut out, code, shift);
        }
    }
    out
}

pub fn key_events(name: &str, mods: &[String]) -> Vec<Ev> {
    let Some(code) = inputmap::named_key(name) else { return vec![] };
    let mods: Vec<u16> = mods.iter().filter_map(|m| inputmap::modifier(m)).collect();
    let mut out: Vec<Ev> = mods.iter().map(|k| (KEY, *k, 1)).collect();
    out.push((KEY, code, 1));
    out.push((KEY, code, 0));
    out.extend(mods.iter().rev().map(|k| (KEY, *k, 0)));
    out
}

/// Implementação real. Cada dispositivo é criado na primeira vez que alguém o usa.
pub struct Uinput {
    inner: Mutex<UinputInner>,
}

#[derive(Default)]
struct UinputInner {
    pads: HashMap<u8, (evdev::uinput::VirtualDevice, PadState)>,
    mouse: Option<evdev::uinput::VirtualDevice>,
    kbd: Option<evdev::uinput::VirtualDevice>,
}

fn emit(dev: &mut evdev::uinput::VirtualDevice, evs: &[Ev]) {
    if evs.is_empty() {
        return;
    }
    let v: Vec<evdev::InputEvent> = evs.iter().map(|(t, c, v)| evdev::InputEvent::new(*t, *c, *v)).collect();
    if let Err(e) = dev.emit(&v) {
        tracing::warn!("uinput: {e}");
    }
}

impl Uinput {
    pub fn probe() -> Option<Uinput> {
        std::fs::OpenOptions::new().write(true).open("/dev/uinput").ok()?;
        Some(Uinput { inner: Mutex::new(UinputInner::default()) })
    }

    fn new_pad(player: u8) -> std::io::Result<evdev::uinput::VirtualDevice> {
        use evdev::*;
        let mut keys = AttributeSet::<KeyCode>::new();
        for n in inputmap::PAD_BUTTONS {
            keys.insert(KeyCode::new(inputmap::pad_button(n).unwrap()));
        }
        let stick = AbsInfo::new(0, -32768, 32767, 16, 128, 0);
        let hat = AbsInfo::new(0, -1, 1, 0, 0, 0);
        let trig = AbsInfo::new(0, 0, 255, 0, 0, 0);
        uinput::VirtualDevice::builder()?
            .name(&format!("Nexus Remote P{player}"))
            .input_id(InputId::new(BusType::BUS_USB, 0x045e, 0x028e, 0x0110))
            .with_keys(&keys)?
            .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode(0), stick))?
            .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode(1), stick))?
            .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode(2), trig))?
            .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode(3), stick))?
            .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode(4), stick))?
            .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode(5), trig))?
            .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode(0x10), hat))?
            .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode(0x11), hat))?
            .build()
    }

    fn new_mouse() -> std::io::Result<evdev::uinput::VirtualDevice> {
        use evdev::*;
        let mut keys = AttributeSet::<KeyCode>::new();
        for c in [inputmap::BTN_LEFT, inputmap::BTN_RIGHT, inputmap::BTN_MIDDLE] {
            keys.insert(KeyCode::new(c));
        }
        let mut rel = AttributeSet::<RelativeAxisCode>::new();
        for c in [0u16, 1, 6, 8] {
            rel.insert(RelativeAxisCode(c));
        }
        uinput::VirtualDevice::builder()?.name("Nexus Remote Mouse").input_id(InputId::new(BusType::BUS_USB, 0x1209, 0x0001, 0x0100)).with_keys(&keys)?.with_relative_axes(&rel)?.build()
    }

    fn new_kbd() -> std::io::Result<evdev::uinput::VirtualDevice> {
        use evdev::*;
        let mut keys = AttributeSet::<KeyCode>::new();
        for c in inputmap::all_keyboard_codes() {
            keys.insert(KeyCode::new(c));
        }
        uinput::VirtualDevice::builder()?.name("Nexus Remote Keyboard").input_id(InputId::new(BusType::BUS_USB, 0x1209, 0x0002, 0x0100)).with_keys(&keys)?.build()
    }
}

impl InputSink for Uinput {
    fn available(&self) -> bool {
        true
    }
    fn gamepad(&self, player: u8, st: &PadState) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if !g.pads.contains_key(&player) {
            match Self::new_pad(player) {
                Ok(d) => {
                    g.pads.insert(player, (d, PadState::default()));
                }
                Err(e) => return tracing::warn!("uinput gamepad: {e}"),
            }
        }
        let (dev, prev) = g.pads.get_mut(&player).unwrap();
        emit(dev, &pad_events(prev, st));
        *prev = st.clone();
    }
    fn release(&self, player: u8) {
        self.gamepad(player, &PadState::default());
    }
    fn mouse_move(&self, dx: i32, dy: i32) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if g.mouse.is_none() {
            g.mouse = Self::new_mouse().ok();
        }
        if let Some(m) = g.mouse.as_mut() {
            let mut v = vec![];
            if dx != 0 {
                v.push((REL, 0, dx.clamp(-200, 200)));
            }
            if dy != 0 {
                v.push((REL, 1, dy.clamp(-200, 200)));
            }
            emit(m, &v);
        }
    }
    fn scroll(&self, v: i32, h: i32) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if g.mouse.is_none() {
            g.mouse = Self::new_mouse().ok();
        }
        if let Some(m) = g.mouse.as_mut() {
            let mut e = vec![];
            if v != 0 {
                e.push((REL, 8, v.clamp(-20, 20)));
            }
            if h != 0 {
                e.push((REL, 6, h.clamp(-20, 20)));
            }
            emit(m, &e);
        }
    }
    fn button(&self, name: &str, down: bool) {
        let Some(code) = inputmap::mouse_button(name) else { return };
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if g.mouse.is_none() {
            g.mouse = Self::new_mouse().ok();
        }
        if let Some(m) = g.mouse.as_mut() {
            emit(m, &[(KEY, code, down as i32)]);
        }
    }
    fn key(&self, name: &str, mods: &[String]) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if g.kbd.is_none() {
            g.kbd = Self::new_kbd().ok();
        }
        if let Some(k) = g.kbd.as_mut() {
            emit(k, &key_events(name, mods));
        }
    }
    fn text(&self, back: u32, text: &str, mods: &[String]) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if g.kbd.is_none() {
            g.kbd = Self::new_kbd().ok();
        }
        if let Some(k) = g.kbd.as_mut() {
            emit(k, &text_events(back, text, mods));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(b: &[&str], hat: (i32, i32), x: f32) -> PadState {
        PadState { buttons: b.iter().map(|s| s.to_string()).collect(), hat, axes: [("x".to_string(), x)].into() }
    }

    #[test]
    fn pad_diff_only_changes() {
        let a = PadState::default();
        let b = st(&["a", "start"], (1, 0), 0.5);
        let ev = pad_events(&a, &b);
        assert!(ev.contains(&(KEY, 0x130, 1)) && ev.contains(&(KEY, 0x13b, 1)));
        assert!(ev.contains(&(ABS, 0x10, 1)));
        assert!(ev.contains(&(ABS, 0, 16383)));
        assert_eq!(pad_events(&b, &b), vec![]);
        let rel = pad_events(&b, &PadState::default());
        assert!(rel.contains(&(KEY, 0x130, 0)) && rel.contains(&(ABS, 0x10, 0)) && rel.contains(&(ABS, 0, 0)));
    }

    #[test]
    fn typing_uses_shift_and_skips_unknown() {
        let ev = text_events(1, "Aç", &[]);
        assert_eq!(&ev[..2], &[(KEY, 14, 1), (KEY, 14, 0)]);
        assert_eq!(&ev[2..], &[(KEY, 42, 1), (KEY, 30, 1), (KEY, 30, 0), (KEY, 42, 0)]);
    }

    #[test]
    fn named_with_modifiers() {
        let ev = key_events("f4", &["alt".to_string()]);
        assert_eq!(ev, vec![(KEY, 56, 1), (KEY, 62, 1), (KEY, 62, 0), (KEY, 56, 0)]);
        assert!(key_events("nada", &[]).is_empty());
    }
}
