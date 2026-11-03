fn main() {
    let mut files: Vec<_> = std::fs::read_dir("vendor/spine-cpp/src/spine")
        .expect("vendored official Spine sources")
        .map(|entry| entry.expect("source entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "cpp"))
        .collect();
    files.sort();
    let mut native = cc::Build::new();
    native
        .cpp(true)
        .std("c++17")
        .include("vendor/spine-cpp/include")
        .files(files)
        .file("native/spine374.cpp")
        .warnings(true);
    // Match Rust's normal Release assertion policy. Debug native owners retain
    // their assertions; source admission and explicit frame bounds stay active.
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        native.define("NDEBUG", None);
        let compiler = native.get_compiler();
        if compiler.is_like_msvc() && !compiler.is_like_clang() {
            // Keep MSVC IR in its native archive until the final MSVC link.
            // Rust's default rlib bundling cannot index these /GL objects.
            native.flag("/GL").flag("/Ob3").link_lib_modifier("-bundle");
        }
    }
    native.compile("spine374");
    println!("cargo:rerun-if-changed=vendor/spine-cpp");
    println!("cargo:rerun-if-changed=native");
}
