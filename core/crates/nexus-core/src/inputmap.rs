//! Mapeamento de caracteres/teclas/botões para códigos do kernel (linux/input-event-codes.h), layout US.
//! Puro (sem uinput) para ser testado em qualquer máquina. Herdado do RetroPad.

pub const BTN_LEFT: u16 = 0x110;
pub const BTN_RIGHT: u16 = 0x111;
pub const BTN_MIDDLE: u16 = 0x112;
pub const KEY_LEFTSHIFT: u16 = 42;

const LETTERS: [u16; 26] = [30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17, 45, 21, 44];
const DIGITS: [u16; 10] = [11, 2, 3, 4, 5, 6, 7, 8, 9, 10];

fn punct(c: char) -> Option<u16> {
    Some(match c {
        ' ' => 57,
        '\n' => 28,
        '\t' => 15,
        '-' => 12,
        '=' => 13,
        '[' => 26,
        ']' => 27,
        '\\' => 43,
        ';' => 39,
        '\'' => 40,
        '`' => 41,
        ',' => 51,
        '.' => 52,
        '/' => 53,
        _ => return None,
    })
}

fn base(c: char) -> Option<u16> {
    match c {
        'a'..='z' => Some(LETTERS[(c as u8 - b'a') as usize]),
        '0'..='9' => Some(DIGITS[(c as u8 - b'0') as usize]),
        c => punct(c),
    }
}

fn unshifted(c: char) -> Option<char> {
    Some(match c {
        '!' => '1',
        '@' => '2',
        '#' => '3',
        '$' => '4',
        '%' => '5',
        '^' => '6',
        '&' => '7',
        '*' => '8',
        '(' => '9',
        ')' => '0',
        '_' => '-',
        '+' => '=',
        '{' => '[',
        '}' => ']',
        '|' => '\\',
        ':' => ';',
        '"' => '\'',
        '~' => '`',
        '<' => ',',
        '>' => '.',
        '?' => '/',
        _ => return None,
    })
}

/// (código, precisa_shift); None para caracteres que não existem no layout US (ç, ã, é...).
pub fn char_to_key(c: char) -> Option<(u16, bool)> {
    if let Some(k) = base(c) {
        return Some((k, false));
    }
    if c.is_ascii_uppercase() {
        return base(c.to_ascii_lowercase()).map(|k| (k, true));
    }
    unshifted(c).and_then(base).map(|k| (k, true))
}

pub fn named_key(name: &str) -> Option<u16> {
    Some(match name {
        "esc" => 1,
        "tab" => 15,
        "enter" => 28,
        "backspace" => 14,
        "delete" => 111,
        "insert" => 110,
        "space" => 57,
        "home" => 102,
        "end" => 107,
        "pageup" => 104,
        "pagedown" => 109,
        "up" => 103,
        "down" => 108,
        "left" => 105,
        "right" => 106,
        "capslock" => 58,
        "printscreen" => 99,
        "menu" => 127,
        n if n.starts_with('f') => match n[1..].parse::<u16>().ok()? {
            k @ 1..=10 => 58 + k,
            11 => 87,
            12 => 88,
            _ => return None,
        },
        _ => return None,
    })
}

pub fn modifier(name: &str) -> Option<u16> {
    Some(match name {
        "ctrl" => 29,
        "alt" => 56,
        "shift" => 42,
        "meta" => 125,
        _ => return None,
    })
}

pub fn mouse_button(name: &str) -> Option<u16> {
    match name {
        "left" => Some(BTN_LEFT),
        "right" => Some(BTN_RIGHT),
        "middle" => Some(BTN_MIDDLE),
        _ => None,
    }
}

/// Botões do gamepad virtual (layout Xbox 360/xpad). Retorna o código evdev.
pub fn pad_button(name: &str) -> Option<u16> {
    Some(match name {
        "a" => 0x130,
        "b" => 0x131,
        "x" => 0x133,
        "y" => 0x134,
        "l" => 0x136,
        "r" => 0x137,
        "l2" => 0x138,
        "r2" => 0x139,
        "select" => 0x13a,
        "start" => 0x13b,
        "mode" => 0x13c,
        "l3" => 0x13d,
        "r3" => 0x13e,
        _ => return None,
    })
}

pub const PAD_BUTTONS: [&str; 13] = ["a", "b", "x", "y", "l", "r", "l2", "r2", "select", "start", "mode", "l3", "r3"];

pub fn all_keyboard_codes() -> Vec<u16> {
    let mut v: Vec<u16> = (1..=127).collect();
    v.retain(|c| !(0x100..0x200).contains(c));
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chars() {
        assert_eq!(char_to_key('a'), Some((30, false)));
        assert_eq!(char_to_key('A'), Some((30, true)));
        assert_eq!(char_to_key('1'), Some((2, false)));
        assert_eq!(char_to_key('0'), Some((11, false)));
        assert_eq!(char_to_key('!'), Some((2, true)));
        assert_eq!(char_to_key('?'), Some((53, true)));
        assert_eq!(char_to_key(' '), Some((57, false)));
        assert_eq!(char_to_key('ç'), None);
        assert_eq!(char_to_key('ã'), None);
    }
    #[test]
    fn named() {
        assert_eq!(named_key("enter"), Some(28));
        assert_eq!(named_key("f1"), Some(59));
        assert_eq!(named_key("f10"), Some(68));
        assert_eq!(named_key("f12"), Some(88));
        assert_eq!(named_key("f13"), None);
        assert_eq!(named_key("nada"), None);
        assert_eq!(modifier("ctrl"), Some(29));
        assert_eq!(pad_button("x"), Some(0x133));
        assert_eq!(mouse_button("right"), Some(BTN_RIGHT));
    }
    #[test]
    fn every_letter_distinct() {
        let mut s = std::collections::HashSet::new();
        for c in 'a'..='z' {
            assert!(s.insert(char_to_key(c).unwrap().0));
        }
    }
}
