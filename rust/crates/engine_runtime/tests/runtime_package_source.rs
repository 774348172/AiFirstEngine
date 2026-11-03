//! Same authoritative package decoder, backed by disk or downloaded bytes.
use engine_content::runtime_package_source::RuntimePackageSource;
use engine_runtime::runtime_package::{
    load_runtime_package, load_runtime_package_from_source, RuntimeProjectInfo,
};
use engine_runtime::runtime_package_builder::*;
use std::path::{Path, PathBuf};

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        // Exact test-owned child, not a shared root.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn fixture(name: &str) -> Fixture {
    let root =
        std::env::temp_dir().join(format!("375-package-source-{}-{name}", std::process::id()));
    assert!(!root.exists());
    let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
        "source-fixture",
        "Source fixture",
        "0.1.0",
    ));
    input.scenes.push(
        serde_json::from_value(serde_json::json!({
            "schemaVersion":"runtime-scene.v1", "id":"main", "name":"Main", "gravity":0,
            "background":"#142035", "skyColor":"#142035", "entities":[]
        }))
        .unwrap(),
    );
    input.assets.push(RuntimePackageSourceAsset::new(
        "main",
        "Main",
        "scene",
        "Scenes/Main.scene.json",
        "scenes/main.json",
    ));
    let mapping = engine_input::InputMappingAsset::gameplay_default();
    input.input_mappings.push(RuntimePackageSourceJson {
        id: mapping.asset_id.clone(),
        document: serde_json::to_value(mapping).unwrap(),
    });
    use engine_runtime::aui::{AuiCanvas, AuiDocument, AuiNode, AuiNodeKind, AuiRect};
    let document = AuiDocument::new(
        "hud",
        vec![AuiCanvas::screen_overlay("canvas", 1280., 720., "root")],
        vec![AuiNode::new(
            "root",
            AuiNodeKind::Panel,
            AuiRect::stretch_full(),
        )],
    );
    input.aui_documents.push(RuntimePackageSourceJson {
        id: "hud".into(),
        document: serde_json::to_value(document).unwrap(),
    });
    input.aui_manifest = Some(engine_runtime::runtime_package::RuntimeAuiManifest {
        schema_version: engine_runtime::runtime_package::RUNTIME_AUI_MANIFEST_SCHEMA_VERSION.into(),
        documents: vec![engine_runtime::runtime_package::RuntimeAuiManifestEntry {
            document_id: "hud".into(),
            path: "aui/documents/hud.aui.json".into(),
            canvas_count: 1,
            node_count: 1,
            binding_count: 0,
            action_count: 0,
            asset_refs: vec![],
        }],
    });
    let mut path = project_runtime_sdk::aui_draw2d::PathBuilder::new();
    path.move_to([0., 0.]).unwrap();
    path.line_to([10., 0.]).unwrap();
    path.line_to([0., 10.]).unwrap();
    path.close().unwrap();
    let mut draw = project_runtime_sdk::aui_draw2d::Draw2D::new();
    draw.fill(
        path.finish(),
        [0., 1., 0., 1.],
        project_runtime_sdk::aui_draw2d::FillRule::NonZero,
    )
    .unwrap();
    let graphics = project_runtime_sdk::aui_draw2d::Graphics2DAsset {
        schema_version: project_runtime_sdk::aui_draw2d::GRAPHICS_ASSET_SCHEMA.into(),
        asset_id: "shape".into(),
        asset_guid: "guid-shape".into(),
        graphics: draw.finish(),
    };
    for (id, kind, guid, value) in [
        (
            "shape",
            "graphics2d",
            "guid-shape",
            serde_json::to_value(graphics).unwrap(),
        ),
        (
            "theme",
            "aui-theme",
            "guid-theme",
            serde_json::json!({
                "schemaVersion":"aui-theme-asset.v1", "assetId":"theme", "tokens":{}, "styles":{}
            }),
        ),
    ] {
        let bytes = serde_json::to_vec(&value).unwrap();
        let mut asset = RuntimePackageSourceAsset::new(
            id,
            id,
            kind,
            format!("Assets/{id}.json"),
            format!("cooked/{id}.json"),
        )
        .with_runtime_payload(bytes.clone());
        asset.asset_guid = Some(guid.into());
        asset.hash = Some(engine_content::canonical_digest::sha256_prefixed(&bytes));
        input.assets.push(asset);
    }
    let report = RuntimePackageBuilder::build(
        &RuntimePackageBuildRequest::dev_desktop(&root, "main"),
        &input,
    );
    assert_eq!(
        report.status,
        RuntimePackageBuildStatus::Success,
        "{:?}",
        report.diagnostics
    );
    Fixture(root)
}
fn collect(root: &Path, dir: &Path, files: &mut Vec<(String, Vec<u8>)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(root, &path, files);
        } else {
            files.push((
                path.strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replace('\\', "/"),
                std::fs::read(&path).unwrap(),
            ));
        }
    }
}
fn files(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut values = vec![];
    collect(root, root, &mut values);
    values
}

#[test]
fn downloaded_package_preserves_native_validation_and_hydration() {
    let fixture = fixture("parity");
    let native = load_runtime_package(&fixture.0).value.unwrap();
    let downloaded =
        load_runtime_package_from_source(RuntimePackageSource::memory(files(&fixture.0)).unwrap())
            .value
            .unwrap();
    assert_eq!(
        native.manifest.content_hash,
        downloaded.manifest.content_hash
    );
    assert_eq!(
        serde_json::to_value(&native.active_scene).unwrap(),
        serde_json::to_value(&downloaded.active_scene).unwrap()
    );
    assert_eq!(
        native.default_input_mapping.unwrap().asset_id,
        downloaded.default_input_mapping.as_ref().unwrap().asset_id
    );
    let native_graphics = &native.aui_documents.documents_by_id["hud"].graphics_assets;
    let downloaded_graphics = &downloaded.aui_documents.documents_by_id["hud"].graphics_assets;
    assert_eq!(native_graphics, downloaded_graphics);
    assert!(downloaded_graphics.resolve("shape", "guid-shape").is_ok());
    assert_eq!(
        native.aui_themes.get("theme"),
        downloaded.aui_themes.get("theme")
    );
    assert!(downloaded.aui_themes.get("theme").is_some());
    let hydration =
        engine_runtime::runtime_scene_hydration::hydrate_active_scene_into_world(&downloaded);
    assert!(hydration.value.is_some(), "{:?}", hydration.diagnostics);
}

#[test]
fn downloaded_graphics_and_theme_keep_the_existing_parser_rejections() {
    let fixture = fixture("bad-ui-assets");
    for target in ["cooked/shape.json", "cooked/theme.json"] {
        let mut values = files(&fixture.0);
        values.iter_mut().find(|(key, _)| key == target).unwrap().1 = b"{".to_vec();
        let result =
            load_runtime_package_from_source(RuntimePackageSource::memory(values).unwrap());
        assert!(result.value.is_none(), "{target} was accepted");
        assert!(result.diagnostics.has_errors());
    }
}

#[test]
fn missing_and_malformed_downloads_fail_the_existing_decoder() {
    let fixture = fixture("bad-files");
    let mut values = files(&fixture.0);
    values.retain(|(key, _)| key != "manifest.json");
    let result = load_runtime_package_from_source(RuntimePackageSource::memory(values).unwrap());
    assert!(result.value.is_none());
    assert!(result.diagnostics.has_errors());
    let mut values = files(&fixture.0);
    values
        .iter_mut()
        .find(|(key, _)| key == "manifest.json")
        .unwrap()
        .1 = b"{".to_vec();
    assert!(
        load_runtime_package_from_source(RuntimePackageSource::memory(values).unwrap())
            .value
            .is_none()
    );
}

#[test]
fn downloaded_manifest_cannot_escape_its_logical_root() {
    let fixture = fixture("bad-path");
    let mut values = files(&fixture.0);
    let (_, bytes) = values
        .iter_mut()
        .find(|(key, _)| key == "manifest.json")
        .unwrap();
    let mut manifest: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    manifest["scenes"][0]["path"] = serde_json::json!("../escape.json");
    *bytes = serde_json::to_vec(&manifest).unwrap();
    let result = load_runtime_package_from_source(RuntimePackageSource::memory(values).unwrap());
    assert!(result.value.is_none());
    assert!(result.diagnostics.has_errors());
}

#[test]
fn downloaded_texture_preserves_pixels_and_native_error_contracts() {
    use engine_runtime::runtime_asset::{CookedAssetRecord, RuntimeAssetIndex, RuntimeAssetRecord};
    use engine_runtime::runtime_package::{CookedTextureAsset, RuntimeAssetRef};
    use engine_runtime::runtime_texture::{
        load_runtime_texture_payload, load_runtime_texture_payload_from_source,
        RuntimeTextureLoadErrorCode,
    };
    let root = std::env::temp_dir().join(format!("375-texture-source-{}", std::process::id()));
    assert!(!root.exists());
    std::fs::create_dir_all(root.join("cooked")).unwrap();
    let fixture = Fixture(root);
    let metadata: CookedTextureAsset = serde_json::from_value(serde_json::json!({
        "schemaVersion":"cooked-texture.v1", "assetId":"texture", "cookedAssetId":"cooked-texture",
        "sourceHash":"source", "width":1, "height":1, "format":"rgba8UnormSrgb", "colorSpace":"srgb",
        "mipCount":1, "byteLength":4, "pixelDataPath":"cooked/pixels.rgba8", "sampler":"linearClamp"
    })).unwrap();
    let record: RuntimeAssetRecord = serde_json::from_value(serde_json::json!({
        "assetGuid":"guid-texture", "assetId":"texture", "type":"texture",
        "cookedAssetId":"cooked-texture", "bundleId":"startup", "loaderKind":"texture"
    }))
    .unwrap();
    let cooked: CookedAssetRecord = serde_json::from_value(serde_json::json!({
        "cookedAssetId":"cooked-texture", "bundleId":"startup", "path":"cooked/texture.json"
    }))
    .unwrap();
    let index = RuntimeAssetIndex::new(vec![record], vec![cooked]);
    let reference = RuntimeAssetRef {
        id: "texture".into(),
        asset_type: "texture".into(),
        guid: Some("guid-texture".into()),
        sub_asset: None,
    };
    std::fs::write(
        fixture.0.join("cooked/texture.json"),
        serde_json::to_vec(&metadata).unwrap(),
    )
    .unwrap();
    std::fs::write(fixture.0.join("cooked/pixels.rgba8"), [255, 64, 0, 255]).unwrap();
    let native = load_runtime_texture_payload(&fixture.0, &index, &reference).unwrap();
    let source = RuntimePackageSource::memory(files(&fixture.0)).unwrap();
    let downloaded = load_runtime_texture_payload_from_source(&source, &index, &reference).unwrap();
    assert_eq!(native, downloaded);
    assert_eq!(downloaded.rgba8.as_slice(), &[255, 64, 0, 255]);
    for (payload, expected) in [
        (None, RuntimeTextureLoadErrorCode::MissingPixelPayload),
        (
            Some(vec![255]),
            RuntimeTextureLoadErrorCode::ByteLengthMismatch,
        ),
    ] {
        let mut values = files(&fixture.0);
        values.retain(|(key, _)| key != "cooked/pixels.rgba8");
        if let Some(bytes) = payload {
            values.push(("cooked/pixels.rgba8".into(), bytes));
        }
        let source = RuntimePackageSource::memory(values).unwrap();
        assert_eq!(
            load_runtime_texture_payload_from_source(&source, &index, &reference)
                .unwrap_err()
                .code,
            expected
        );
    }
}
