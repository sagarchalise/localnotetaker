//! Parse hotkey strings like "Ctrl+Alt+N" into a `global_hotkey::HotKey`.

use std::str::FromStr;

use anyhow::{anyhow, bail};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};

pub fn parse(spec: &str) -> anyhow::Result<HotKey> {
    let mut mods = Modifiers::empty();
    let mut key = None;
    for part in spec.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "alt" => mods |= Modifiers::ALT,
            "shift" => mods |= Modifiers::SHIFT,
            "win" | "super" | "meta" => mods |= Modifiers::SUPER,
            _ => {
                if key.is_some() {
                    bail!("hotkey '{spec}' has more than one non-modifier key");
                }
                key = Some(parse_key(part)?);
            }
        }
    }
    let key = key.ok_or_else(|| anyhow!("hotkey '{spec}' has no key"))?;
    anyhow::ensure!(!mods.is_empty(), "hotkey '{spec}' needs at least one modifier");
    Ok(HotKey::new(Some(mods), key))
}

fn parse_key(part: &str) -> anyhow::Result<Code> {
    let name = match part.chars().collect::<Vec<_>>().as_slice() {
        [c] if c.is_ascii_alphabetic() => format!("Key{}", c.to_ascii_uppercase()),
        [c] if c.is_ascii_digit() => format!("Digit{c}"),
        _ => part.to_string(), // F1..F12, Space, ... use the W3C code names
    };
    Code::from_str(&name).map_err(|_| anyhow!("unknown key '{part}'"))
}
