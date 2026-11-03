use std::{env, fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-changed=native");
    println!("cargo:rerun-if-changed=vendor");
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .opt_level(2)
        .debug(false)
        .warnings(false)
        .define("DT_POLYREF64", None)
        .file("native/nav.cpp");
    for domain in ["Recast", "Detour"] {
        let base = PathBuf::from("vendor").join(domain);
        build.include(base.join("Include"));
        let mut files = fs::read_dir(base.join("Source"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "cpp"))
            .collect::<Vec<_>>();
        files.sort();
        build.files(files);
    }
    if env::var("TARGET").unwrap().contains("msvc") {
        build.static_crt(false);
        if let Some(vc) = env::var_os("VCToolsInstallDir") {
            let cl = PathBuf::from(vc).join("bin/Hostx64/x64/cl.exe");
            if cl.is_file() {
                build.compiler(cl);
            }
        }
    }
    build.compile("aife_recast_detour64");
}
