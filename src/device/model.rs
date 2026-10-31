//! Formato do arquivo de aparelho e a validação dele. Só dados: nada aqui vira comando cru ao aparelho.
use serde::{Deserialize, Deserializer};

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DeviceModel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub note: String,
    pub usb: Vec<UsbId>,
    pub protocol: Protocol,
    pub image: ImageSpec,
    pub controls: Vec<Control>,
    pub status: Status,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UsbId {
    #[serde(deserialize_with = "hex16")]
    pub vid: u16,
    #[serde(deserialize_with = "hex16")]
    pub pid: u16,
    #[serde(deserialize_with = "hex16")]
    pub usage_page: u16,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Family {
    /// AJAZZ / Mirabox e clones (comandos "CRT", imagem "BAT").
    Mirabox,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Protocol {
    pub family: Family,
    pub version: u8,
    /// Tamanhos de pacote a tentar, em ordem.
    pub packets: Vec<usize>,
    /// O aparelho informa a soltura da tecla (não só o aperto).
    pub release_events: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ImageSpec {
    /// Lado da imagem de cada tecla, em pixels.
    pub px: u32,
    pub format: ImageFormat,
    /// Maior imagem aceita (acima disso a tela do AKP153E cai para o logo).
    pub max_bytes: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ImageFormat {
    Jpeg,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Alguém provou no aparelho (checklist).
    Verified,
    Experimental,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub id: String,
    pub kind: ControlKind,
    /// Posição na grade do desenho do aparelho (coluna, linha) e tamanho em células.
    pub x: u32,
    pub y: u32,
    #[serde(default = "one")]
    pub w: u32,
    #[serde(default = "one")]
    pub h: u32,
    /// Índice no protocolo (0-based; no fio vai +1). Teclas.
    #[serde(default)]
    pub hw: Option<u8>,
    /// Telas: tamanho em pixels.
    #[serde(default)]
    pub px: Option<[u32; 2]>,
    /// Faixa lateral: as partes que o aparelho desenha (índice e linha de início), de cima para baixo.
    #[serde(default)]
    pub slots: Vec<Slot>,
}

fn one() -> u32 {
    1
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Slot {
    pub hw: u8,
    pub top: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ControlKind {
    Key,
    /// Tela sem toque (a faixa lateral do AKP153E).
    Strip,
}

fn hex16<'de, D: Deserializer<'de>>(d: D) -> Result<u16, D::Error> {
    let text = String::deserialize(d)?;
    u16::from_str_radix(&text, 16).map_err(serde::de::Error::custom)
}

/// Limites de um arquivo válido (um arquivo da comunidade não pode pedir o absurdo).
const MAX_CONTROLS: usize = 128;
const MAX_FILE_BYTES: usize = 64 * 1024;
const PACKET_SIZES: [usize; 2] = [512, 1024];
const IMAGE_PX: std::ops::RangeInclusive<u32> = 32..=512;
const MAX_IMAGE_BYTES: usize = 64 * 1024;
const MAX_GRID: u32 = 16;

impl DeviceModel {
    /// Lê e valida um arquivo de aparelho.
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > MAX_FILE_BYTES {
            return Err("arquivo grande demais".into());
        }
        let model: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        model.validate().map_err(|e| format!("{}: {e}", model.id))?;
        Ok(model)
    }

    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() || self.usb.is_empty() || self.controls.is_empty() {
            return Err("faltam id, usb ou controles".into());
        }
        if self.controls.len() > MAX_CONTROLS {
            return Err("controles demais".into());
        }
        let p = &self.protocol;
        if p.packets.is_empty() || p.packets.iter().any(|n| !PACKET_SIZES.contains(n)) {
            return Err(format!("pacote deve ser 512 ou 1024: {:?}", p.packets));
        }
        if !IMAGE_PX.contains(&self.image.px) || self.image.max_bytes > MAX_IMAGE_BYTES {
            return Err("tamanho de imagem fora do limite".into());
        }
        let mut ids = std::collections::HashSet::new();
        let mut hws = std::collections::HashSet::new();
        let mut cells = std::collections::HashSet::new();
        for c in &self.controls {
            if !ids.insert(c.id.as_str()) {
                return Err(format!("controle repetido: {}", c.id));
            }
            if c.x + c.w > MAX_GRID || c.y + c.h > MAX_GRID || c.w == 0 || c.h == 0 {
                return Err(format!("{} fora da grade", c.id));
            }
            for x in c.x..c.x + c.w {
                for y in c.y..c.y + c.h {
                    if !cells.insert((x, y)) {
                        return Err(format!("{} sobrepõe outro controle", c.id));
                    }
                }
            }
            let used: Vec<u8> = match c.kind {
                ControlKind::Key => vec![c.hw.ok_or(format!("{} sem hw", c.id))?],
                ControlKind::Strip => {
                    if c.slots.is_empty() || c.px.is_none() {
                        return Err(format!("{} sem slots ou px", c.id));
                    }
                    c.slots.iter().map(|s| s.hw).collect()
                }
            };
            for hw in used {
                // 255 é "todas as teclas" no protocolo Mirabox.
                if hw == u8::MAX || !hws.insert(hw) {
                    return Err(format!("hw {hw} repetido ou reservado ({})", c.id));
                }
            }
        }
        Ok(())
    }

    pub fn matches(&self, vid: u16, pid: u16) -> bool {
        self.usb.iter().any(|u| (u.vid, u.pid) == (vid, pid))
    }

    /// Teclas em ordem de leitura (linha, depois coluna): a ordem dos botões de uma página.
    pub fn keys(&self) -> Vec<&Control> {
        let mut keys: Vec<&Control> = self
            .controls
            .iter()
            .filter(|c| c.kind == ControlKind::Key)
            .collect();
        keys.sort_by_key(|c| (c.y, c.x));
        keys
    }

    /// Índice no protocolo da tecla `i` da página.
    pub fn key_hw(&self, i: usize) -> Option<u8> {
        self.keys().get(i).and_then(|c| c.hw)
    }

    /// Tecla da página para um índice do protocolo; `None` se não for tecla (ex.: parte da lateral).
    pub fn key_of_hw(&self, hw: u8) -> Option<usize> {
        self.keys().iter().position(|c| c.hw == Some(hw))
    }

    /// A faixa lateral, se o aparelho tem uma.
    pub fn strip(&self) -> Option<&Control> {
        self.controls.iter().find(|c| c.kind == ControlKind::Strip)
    }
}
