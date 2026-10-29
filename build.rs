//! Windows only: generates `mangue.ico` (16, 32, 48, 256) from the app icon
//! export and embeds it in `mangue.exe`; a no-op on other hosts.

/// Source artwork shared with the macOS and Linux package scripts.
#[cfg(windows)]
const ICON_SRC: &str = "assets/app-icon/mangue Exports/mangue-iOS-Default-1024@1x.png";

/// Embeds the generated icon resource when building on Windows.
fn main() {
    #[cfg(windows)]
    embed_icon();
}

/// Writes `mangue.ico` and `mangue.rc` to `OUT_DIR` and compiles the resource.
#[cfg(windows)]
fn embed_icon() {
    use image::codecs::ico::{IcoEncoder, IcoFrame};
    use image::{ExtendedColorType, imageops::FilterType};

    println!("cargo:rerun-if-changed={ICON_SRC}");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let source = image::open(ICON_SRC).expect("app icon export").to_rgba8();

    let frames: Vec<IcoFrame> = [16, 32, 48, 256]
        .into_iter()
        .map(|size| {
            let resized = image::imageops::resize(&source, size, size, FilterType::Lanczos3);
            IcoFrame::as_png(resized.as_raw(), size, size, ExtendedColorType::Rgba8).unwrap()
        })
        .collect();
    let ico = out.join("mangue.ico");
    IcoEncoder::new(std::fs::File::create(&ico).unwrap())
        .encode_images(&frames)
        .unwrap();

    let rc = out.join("mangue.rc");
    let ico_path = ico.display().to_string().replace('\\', "/");
    std::fs::write(&rc, format!("1 ICON \"{ico_path}\"\n")).unwrap();
    embed_resource::compile(&rc, embed_resource::NONE)
        .manifest_optional()
        .unwrap();
}
