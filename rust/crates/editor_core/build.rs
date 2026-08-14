use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let crate_root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let pack_root =
        crate_root.join("../../resources/runtime/font-packs/aife-default-zh-cn-common-v1");
    let cooked_root = pack_root.join("cooked");
    println!("cargo:rerun-if-changed={}", cooked_root.display());
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("out dir"))
        .join("engine_builtin_font_pack_embedded.rs");
    let source = generate_embedded_source(&cooked_root).unwrap_or_else(|error| {
        println!("cargo:warning=built-in Chinese FontPack is not sealed yet: {error}");
        empty_embedded_source()
    });
    fs::write(output, source).expect("write generated built-in FontPack source");
}

fn generate_embedded_source(cooked_root: &Path) -> Result<String, String> {
    let manifest_path = cooked_root.join("manifest.json");
    let manifest_bytes = fs::read(&manifest_path).map_err(|error| error.to_string())?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest_bytes).map_err(|error| error.to_string())?;
    let metadata_path = manifest
        .get("bundleMetadataPath")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "manifest bundleMetadataPath is missing".to_string())?;
    let page_paths = manifest
        .get("pagePaths")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "manifest pagePaths is missing".to_string())?;
    let mut source = String::new();
    source.push_str(&format!(
        "pub const BUILTIN_FONT_PACK_MANIFEST_BYTES: &[u8] = include_bytes!({:?});\n",
        manifest_path
    ));
    source.push_str(&format!(
        "pub const BUILTIN_FONT_BUNDLE_BYTES: &[u8] = include_bytes!({:?});\n",
        cooked_root.join(metadata_path)
    ));
    source.push_str("pub const BUILTIN_FONT_PAGE_BYTES: &[&[u8]] = &[\n");
    for path in page_paths {
        let path = path
            .as_str()
            .ok_or_else(|| "manifest page path is not a string".to_string())?;
        let absolute = cooked_root.join(path);
        if !absolute.is_file() {
            return Err(format!("missing sealed page {}", absolute.display()));
        }
        source.push_str(&format!("    include_bytes!({:?}),\n", absolute));
    }
    source.push_str("];\n");
    Ok(source)
}

fn empty_embedded_source() -> String {
    "pub const BUILTIN_FONT_PACK_MANIFEST_BYTES: &[u8] = &[];\n\
     pub const BUILTIN_FONT_BUNDLE_BYTES: &[u8] = &[];\n\
     pub const BUILTIN_FONT_PAGE_BYTES: &[&[u8]] = &[];\n"
        .to_string()
}
