//! Web export preparation. Every project goes through the neutral assembler.
use crate::{
    ProjectAuthoringSession, ProjectRuntimePackageAssembler, ProjectRuntimePackageAssemblyRequest,
    ProjectRuntimeProductionStaging,
};
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::runtime_package::load_runtime_package;
use engine_runtime::runtime_package_builder::{
    RuntimePackageBuildRequest, RuntimePackageBuildStatus, RuntimePackageBuilder,
};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebExportPreparation {
    pub project_id: String,
    pub package_content_hash: String,
    pub player_crate: PathBuf,
    pub output_directory: PathBuf,
    pub generated_glue_digest: String,
    pub source_digest: String,
}

/// The build directory and output directory must be fresh, operation-owned roots.
pub fn prepare_web_export(
    project: &Path,
    build: &Path,
    output: &Path,
    engine_rust: &Path,
) -> Result<WebExportPreparation, String> {
    if build.exists() || output.exists() {
        return Err("web.export.destination_exists: use fresh build/output directories".into());
    }
    let session = ProjectAuthoringSession::open(project)
        .map_err(|e| format!("web.export.project_open:{e:?}"))?;
    let source_digest = session
        .source_inventory()
        .map_err(|e| format!("web.export.inventory:{e:?}"))?
        .source_digest;
    let plan = ProjectRuntimeProductionStaging::plan(project, engine_rust)
        .map_err(|e| format!("{}:{}", e.code, e.message))?;
    fs::create_dir_all(build).map_err(|e| e.to_string())?;
    let staged = build.join("Source");
    ProjectRuntimeProductionStaging::stage(project, &staged, &plan)
        .map_err(|e| format!("{}:{}", e.code, e.message))?;
    if session
        .source_inventory()
        .map_err(|e| format!("web.export.inventory:{e:?}"))?
        .source_digest
        != source_digest
    {
        return Err("web.export.source_drift: prepare a fresh export".into());
    }
    let assembly = ProjectRuntimePackageAssembler::assemble(
        ProjectRuntimePackageAssemblyRequest::new(&staged)
            .with_build_profile_path("BuildProfiles/web.dev.json"),
    );
    let input = assembly
        .build_input
        .ok_or_else(|| format!("web.export.assembly:{:?}", assembly.report.diagnostics))?;
    let glue = assembly
        .prepared_runtime_glue
        .ok_or("web.export.rust_sdk_module_required")?;
    if input.spine_animator.is_some()
        || input
            .scenes
            .iter()
            .any(|s| s.entities.iter().any(|e| e.spatial.is_some()))
    {
        return Err("web.export.unsupported_capability: Web v1 supports 2D without Spine".into());
    }
    let package = build.join("package");
    let scene = assembly
        .active_scene_id
        .ok_or("web.export.active_scene_missing")?;
    let report = RuntimePackageBuilder::build(
        &RuntimePackageBuildRequest::dev_desktop(&package, scene),
        &input,
    );
    if report.status != RuntimePackageBuildStatus::Success {
        return Err(format!("web.export.package:{:?}", report.diagnostics));
    }
    let loaded = load_runtime_package(&package)
        .value
        .ok_or("web.export.package_validation_failed")?;
    fs::create_dir_all(output.join("package")).map_err(|e| e.to_string())?;
    let mut entries = Vec::new();
    copy_package(&package, &package, &output.join("package"), &mut entries)?;
    entries.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    let package_hash = loaded
        .manifest
        .content_hash
        .clone()
        .ok_or("web.export.package_hash_missing")?;
    let manifest = serde_json::json!({"schemaVersion":"web-package.v1","projectId":input.project.project_id,"moduleId":input.project.runtime_module.module_id,"packageContentHash":package_hash,"files":entries});
    fs::write(
        output.join("web-package.json"),
        serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let glue_root = build.join("glue");
    glue.materialize(&glue_root, engine_rust, &staged.join("RuntimeModuleBuild"))
        .map_err(|e| format!("{}:{}", e.code, e.message))?;
    let player = build.join("player");
    fs::create_dir_all(player.join("src")).map_err(|e| e.to_string())?;
    let normalize = |p: &Path| p.to_string_lossy().replace('\\', "/");
    let cargo = serde_json::json!({"package":{"name":"aife_web_game","version":"0.1.0","edition":"2021","publish":false},"lib":{"crate-type":["cdylib"]},"workspace":{},"profile":{"release":{"opt-level":2,"debug":false,"strip":"debuginfo"}},"dependencies":{
        "runtime_player_web":{"path":normalize(&engine_rust.join("crates/runtime_player_web"))},
        "aife_generated_runtime_glue":{"path":normalize(&glue_root),"features":["static-link"]},
        "wasm-bindgen":"=0.2.125","wasm-bindgen-futures":"0.4","js-sys":"0.3","web-sys":{"version":"0.3","features":["HtmlCanvasElement"]}
    },"patch":{"crates-io":{"wgpu":{"path":normalize(&engine_rust.join("crates/engine_render_native/vendor/wgpu"))}}}});
    let cargo: toml::Value = serde_json::from_value(cargo).map_err(|e| e.to_string())?;
    fs::write(player.join("Cargo.lock"), &plan.dependency_lock_bytes).map_err(|e| e.to_string())?;
    fs::write(
        player.join("Cargo.toml"),
        toml::to_string_pretty(&cargo).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::write(player.join("src/lib.rs"), WEB_GAME_ENTRY).map_err(|e| e.to_string())?;
    fs::write(
        build.join("assembly-report.json"),
        serde_json::to_vec_pretty(&assembly.report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let preparation = WebExportPreparation {
        project_id: input.project.project_id,
        package_content_hash: package_hash,
        player_crate: player,
        output_directory: output.into(),
        generated_glue_digest: glue.generation_digest().into(),
        source_digest,
    };
    fs::write(
        build.join("preparation.json"),
        serde_json::to_vec_pretty(&preparation).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(preparation)
}

fn copy_package(
    root: &Path,
    current: &Path,
    destination: &Path,
    entries: &mut Vec<serde_json::Value>,
) -> Result<(), String> {
    for entry in fs::read_dir(current).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let relative = path.strip_prefix(root).map_err(|e| e.to_string())?;
        if relative.starts_with("reports") {
            continue;
        }
        if fs::symlink_metadata(&path)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("web.export.symlink_not_allowed".into());
        }
        if path.is_dir() {
            fs::create_dir_all(destination.join(relative)).map_err(|e| e.to_string())?;
            copy_package(root, &path, destination, entries)?;
        } else {
            let bytes = fs::read(&path).map_err(|e| e.to_string())?;
            fs::write(destination.join(relative), &bytes).map_err(|e| e.to_string())?;
            entries.push(serde_json::json!({"path":relative.to_string_lossy().replace('\\',"/"),"bytes":bytes.len(),"sha256":sha256_prefixed(&bytes)}));
        }
    }
    Ok(())
}

const WEB_GAME_ENTRY: &str = r#"use wasm_bindgen::prelude::*;
#[wasm_bindgen]
pub async fn create_player(canvas: web_sys::HtmlCanvasElement, manifest: String, files: js_sys::Map) -> Result<runtime_player_web::WebPlayer, JsValue> {
    // Compiler-generated static ABI entry, also used by the native Player.
    let api = unsafe { *aife_generated_runtime_glue::aife_project_runtime_entry_v1() };
    runtime_player_web::create_player(canvas, &manifest, files, api).await
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn web_export_rejects_existing_destinations_before_reading_the_project() {
        let absent = std::env::temp_dir().join(format!("375-unused-export-{}", std::process::id()));
        assert!(!absent.exists());
        for (build, output) in [
            (Path::new("."), absent.as_path()),
            (absent.as_path(), Path::new(".")),
        ] {
            let result = prepare_web_export(
                Path::new("missing-project"),
                build,
                output,
                Path::new("missing-sdk"),
            );
            assert!(result
                .unwrap_err()
                .starts_with("web.export.destination_exists"));
        }
        assert!(!absent.exists());
    }
}
