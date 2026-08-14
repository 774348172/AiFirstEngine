use editor_core::EngineBuiltInFontPack;
use std::path::PathBuf;

fn main() {
    let pack_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../resources/runtime/font-packs/aife-default-zh-cn-common-v1");
    match EngineBuiltInFontPack::produce(&pack_root) {
        Ok(manifest) => println!(
            "built {}: codepoints={}, han={}, bitmapPages={}, msdfPages={}, rawBytes={}, digest={}",
            manifest.pack_id,
            manifest.codepoint_count,
            manifest.han_codepoint_count,
            manifest.bitmap_page_count,
            manifest.msdf_page_count,
            manifest.raw_page_bytes,
            manifest.bundle_digest
        ),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
