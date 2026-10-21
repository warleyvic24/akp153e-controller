//! Embute no `.exe` o mesmo ícone da janela e da bandeja (`src/appicon.rs`).
//! O `.ico` é gerado aqui, em `OUT_DIR`, para não manter um binário no repositório.
#[path = "src/appicon.rs"]
mod appicon;

use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
use std::{env, fs, path::PathBuf};

/// Tamanhos do `.ico`: o Explorer escolhe o mais próximo do que precisa mostrar.
const ICON_SIZES: [u32; 4] = [16, 32, 48, 256];
const ICO_HEADER_LEN: usize = 6;
const ICO_ENTRY_LEN: usize = 16;

fn png(px: u32) -> Vec<u8> {
    let mut out = Vec::new();
    PngEncoder::new(&mut out)
        .write_image(&appicon::rgba(px), px, px, ExtendedColorType::Rgba8)
        .expect("codificar PNG do ícone");
    out
}

/// Contêiner ICO com imagens PNG (suportado desde o Windows Vista).
fn ico() -> Vec<u8> {
    let images: Vec<(u32, Vec<u8>)> = ICON_SIZES.iter().map(|&px| (px, png(px))).collect();
    let mut out = Vec::new();
    out.extend([0, 0, 1, 0]);
    out.extend((images.len() as u16).to_le_bytes());
    let mut offset = ICO_HEADER_LEN + ICO_ENTRY_LEN * images.len();
    for (px, data) in &images {
        // Largura/altura 0 significa 256 no formato ICO.
        let dim = if *px >= 256 { 0 } else { *px as u8 };
        out.extend([dim, dim, 0, 0, 1, 0, 32, 0]);
        out.extend((data.len() as u32).to_le_bytes());
        out.extend((offset as u32).to_le_bytes());
        offset += data.len();
    }
    for (_, data) in images {
        out.extend(data);
    }
    out
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/appicon.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let path = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("akp153e.ico");
    fs::write(&path, ico()).expect("gravar akp153e.ico");
    let mut res = winresource::WindowsResource::new();
    res.set_icon(path.to_str().expect("caminho UTF-8"));
    res.set("ProductName", "AKP153E Open Controller");
    res.set(
        "FileDescription",
        "Unofficial controller for the AKP153E stream controller",
    );
    res.compile().expect("embutir recursos do Windows");
}
