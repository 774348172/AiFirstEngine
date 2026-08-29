use super::*;
use engine_runtime::project_runtime_module::{
    LinkedProjectRuntimeSet, ProjectRuntimeError, ProjectRuntimeModule,
    ProjectRuntimeModuleDescriptor, ProjectRuntimeRegistration,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct TestLinkedProjectRuntime {
    descriptor: ProjectRuntimeModuleDescriptor,
}

impl ProjectRuntimeModule for TestLinkedProjectRuntime {
    fn descriptor(&self) -> &ProjectRuntimeModuleDescriptor {
        &self.descriptor
    }

    fn install(
        &self,
        _registration: &mut ProjectRuntimeRegistration,
    ) -> Result<(), ProjectRuntimeError> {
        Ok(())
    }
}

pub(super) fn session_with_linked_project_runtime(module_id: &str) -> EditorSession {
    let linked = LinkedProjectRuntimeSet::singleton(Arc::new(TestLinkedProjectRuntime {
        descriptor: ProjectRuntimeModuleDescriptor::new(module_id, "sha256:test-linked-runtime"),
    }))
    .expect("test linked project runtime must form a singleton composition");
    EditorSession::with_linked_project_runtimes(Arc::new(linked))
}

pub(super) fn opened_session(package_dir: &Path) -> EditorSession {
    let mut session = EditorSession::new();
    let result = session.execute_command(command_for_test(UiCommandPayload::OpenRuntimePackage {
        path: package_dir.display().to_string(),
    }));
    assert_eq!(result.status, CommandStatus::Committed);
    session
}

pub(super) fn opened_editor_scene_session(scene_path: &Path) -> EditorSession {
    let mut session = EditorSession::new();
    let result = session.open_scene_document_for_test(scene_path);
    assert_eq!(result.status, CommandStatus::Committed);
    session
}

pub(super) fn opened_project_editor_scene_session(scene_fixture_path: &Path) -> EditorSession {
    let project_root = unique_editor_project_temp_dir();
    let mut session = EditorSession::new();
    let create = session.execute_command(command_for_test(UiCommandPayload::CreateProject {
        path: project_root.display().to_string(),
        name: "Project Scene Fixture".to_string(),
    }));
    assert_eq!(create.status, CommandStatus::Committed);
    let scene_path = project_root.join("Scenes/Main.scene.json");
    fs::copy(scene_fixture_path, &scene_path).unwrap();
    let open = session.open_scene_document_for_test(&scene_path);
    assert_eq!(open.status, CommandStatus::Committed);
    session
}

pub(super) fn player_view_x(session: &EditorSession) -> f32 {
    session
        .build_ui_model()
        .viewport
        .renderables
        .iter()
        .find(|renderable| renderable.entity_id == "entity-player")
        .expect("player renderable")
        .local_position
        .x
}

pub(super) fn make_scene_dirty(session: &mut EditorSession) {
    let result = session.execute_command(command_for_test(UiCommandPayload::SetSceneTransform {
        entity_id: "entity-player".to_string(),
        local_position: Some(Vec3 {
            x: 3.0,
            y: 0.0,
            z: 0.0,
        }),
        local_rotation: None,
        local_scale: None,
    }));
    assert_eq!(result.status, CommandStatus::Committed);
    assert!(
        session
            .editor_scene_document
            .as_ref()
            .expect("scene document")
            .dirty_state
            .dirty
    );
}

pub(super) fn write_runtime_package_fixture() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let fixture_root =
        std::env::temp_dir().join(format!("native-editor-runtime-fixture-{stamp}-{sequence}"));
    let package_dir = fixture_root.join("runtime-package");
    fs::create_dir_all(package_dir.join("scenes")).unwrap();
    fs::create_dir_all(package_dir.join("assets")).unwrap();
    fs::create_dir_all(package_dir.join("rules")).unwrap();
    fs::create_dir_all(package_dir.join("input")).unwrap();
    fs::create_dir_all(package_dir.join("cooked")).unwrap();
    fs::write(
            package_dir.join("manifest.json"),
            r##"{
  "schemaVersion": "runtime-package.v2",
  "packageMode": "debug-readable",
  "project": {
    "projectId": "native-editor-fixture",
    "name": "Native Editor Fixture",
    "version": "0.0.3",
    "runtimeModule": {
      "moduleId": "engine.empty.runtime",
      "interfaceVersion": "project-runtime-module.v2",
      "aotContentDigest": "sha256:engine-empty-runtime-v2"
    }
  },
  "activeSceneId": "scene-main",
  "scenes": [{ "id": "scene-main", "name": "Main", "path": "scenes/scene-main.json", "entityCount": 2 }],
  "assets": { "path": "assets/asset-manifest.json", "assetCount": 4 },
  "rules": { "path": "rules/rule-manifest.json", "mode": "rust-aot" },
  "input": { "path": "input/input-manifest.json", "defaultMappingId": "input.none", "mappingCount": 1 },
  "contentHash": null
}"##,
        )
        .unwrap();
    fs::write(
        package_dir.join("scenes").join("scene-main.json"),
        r##"{
  "schemaVersion": "runtime-scene.v1",
  "id": "scene-main",
  "name": "Main",
  "gravity": 0,
  "background": "#000",
  "skyColor": "#111",
  "entities": [
    {
      "schemaVersion": "runtime-entity.v1",
      "id": "entity-player",
      "name": "Player",
      "kind": "player",
      "enabled": true,
      "parentId": null,
      "siblingOrder": 0,
      "transform": {
        "localPosition": { "x": 0, "y": 0, "z": 0 },
        "localRotation": { "x": 0, "y": 0, "z": 0 },
        "localScale": { "x": 1, "y": 1, "z": 1 }
      },
      "mesh": {
        "primitive": "model",
        "assetRef": { "id": "model-player", "type": "model" },
        "materialRef": { "id": "mat-player", "type": "material" },
        "visible": true,
        "layer": "default"
      }
    },
    {
      "schemaVersion": "runtime-entity.v1",
      "id": "entity-gun",
      "name": "Gun",
      "kind": "weapon",
      "enabled": true,
      "parentId": "entity-player",
      "siblingOrder": 0,
      "transform": {
        "localPosition": { "x": 1, "y": 0, "z": 0 },
        "localRotation": { "x": 0, "y": 0, "z": 0 },
        "localScale": { "x": 1, "y": 1, "z": 1 }
      },
      "mesh": {
        "primitive": "model",
        "assetRef": { "id": "model-gun", "type": "model" },
        "visible": true,
        "layer": "default"
      }
    }
  ]
}"##,
    )
    .unwrap();
    fs::write(
            package_dir.join("assets").join("asset-manifest.json"),
            r#"{
  "schemaVersion": "runtime-asset-manifest.v1",
  "assets": [
    { "id": "scene-main", "name": "Main", "type": "scene", "source": "scenes/scene-main.json", "state": "available", "bundleId": "startup" },
    { "id": "model-player", "name": "Player", "type": "model", "source": "player.glb", "state": "available", "bundleId": "startup" },
    { "id": "model-gun", "name": "Gun", "type": "model", "source": "gun.glb", "state": "available", "bundleId": "startup" },
    { "id": "mat-player", "name": "Player Material", "type": "material", "source": "player.mat", "state": "available", "bundleId": "startup" }
  ],
  "runtimeAssetIndex": [
    {
      "assetGuid": "scene-main",
      "assetId": "scene-main",
      "assetType": "scene",
      "subAssetId": null,
      "version": "1",
      "cookedAssetId": "cooked-scene-main",
      "bundleId": "startup",
      "loaderKind": "scene",
      "dependencies": [],
      "hash": null,
      "size": null,
      "flags": ["test"]
    },
    {
      "assetGuid": "model-player",
      "assetId": "model-player",
      "assetType": "model",
      "subAssetId": null,
      "version": "1",
      "cookedAssetId": "cooked-model-player",
      "bundleId": "startup",
      "loaderKind": "model",
      "dependencies": [],
      "hash": null,
      "size": 0,
      "flags": ["test"]
    },
    {
      "assetGuid": "model-gun",
      "assetId": "model-gun",
      "assetType": "model",
      "subAssetId": null,
      "version": "1",
      "cookedAssetId": "cooked-model-gun",
      "bundleId": "startup",
      "loaderKind": "model",
      "dependencies": [],
      "hash": null,
      "size": 0,
      "flags": ["test"]
    },
    {
      "assetGuid": "mat-player",
      "assetId": "mat-player",
      "assetType": "material",
      "subAssetId": null,
      "version": "1",
      "cookedAssetId": "cooked-mat-player",
      "bundleId": "startup",
      "loaderKind": "material",
      "dependencies": [],
      "hash": null,
      "size": 2,
      "flags": ["test"]
    }
  ],
  "bundleTable": [{
    "bundleId": "startup",
    "mountId": null,
    "uri": "bundles/startup",
    "hash": null,
    "version": null,
    "mounted": false
  }],
  "cookedAssetTable": [
    {
      "cookedAssetId": "cooked-scene-main",
      "bundleId": "startup",
      "path": "scenes/scene-main.json",
      "offset": null,
      "size": null,
      "compression": "none",
      "hash": null
    },
    {
      "cookedAssetId": "cooked-model-player",
      "bundleId": "startup",
      "path": "cooked/player.glb.bin",
      "offset": null,
      "size": 0,
      "compression": "none",
      "hash": null
    },
    {
      "cookedAssetId": "cooked-model-gun",
      "bundleId": "startup",
      "path": "cooked/gun.glb.bin",
      "offset": null,
      "size": 0,
      "compression": "none",
      "hash": null
    },
    {
      "cookedAssetId": "cooked-mat-player",
      "bundleId": "startup",
      "path": "cooked/player.mat.json",
      "offset": null,
      "size": 2,
      "compression": "none",
      "hash": null
    }
  ],
  "dependencyTable": []
}"#,
        )
        .unwrap();
    fs::write(package_dir.join("cooked").join("player.glb.bin"), []).unwrap();
    fs::write(package_dir.join("cooked").join("gun.glb.bin"), []).unwrap();
    fs::write(package_dir.join("cooked").join("player.mat.json"), b"{}").unwrap();
    fs::write(
            package_dir.join("rules").join("rule-manifest.json"),
            r#"{ "schemaVersion": "runtime-rule-manifest.v1", "mode": "rust-aot", "rules": [], "modules": [] }"#,
        )
        .unwrap();
    write_empty_input_fixture(&package_dir);
    package_dir
}

#[test]
fn runtime_package_fixtures_use_distinct_report_ownership_roots() {
    let first = write_runtime_package_fixture();
    let second = write_runtime_package_fixture();

    assert_ne!(first.parent(), second.parent());
}

fn write_empty_input_fixture(package_dir: &Path) {
    fs::write(
        package_dir.join("input").join("input-manifest.json"),
        r#"{
  "schemaVersion": "runtime-input-manifest.v1",
  "defaultMappingId": "input.none",
  "mappings": [{ "id": "input.none", "path": "input/input.none.json", "enabled": true }]
}"#,
    )
    .unwrap();
    fs::write(
        package_dir.join("input").join("input.none.json"),
        r#"{
  "schema_version": "input-mapping.v2",
  "asset_id": "input.none",
  "actions": [],
  "contexts": [],
  "bindings": [],
  "platform_overrides": []
}"#,
    )
    .unwrap();
}

pub(super) fn write_editor_scene_fixture() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("native-editor-scene-{stamp}-{sequence}"));
    fs::create_dir_all(root.join("scenes")).unwrap();
    let scene_path = root.join("scenes").join("main.scene.json");
    fs::write(
        &scene_path,
        r##"{
  "schemaVersion": "editor-scene-document.v1",
  "id": "scene-main",
  "name": "Main",
  "gravity": 0,
  "background": "#000",
  "skyColor": "#111",
  "entities": [{
    "schemaVersion": "runtime-entity.v1",
    "id": "entity-player",
    "name": "Player",
    "kind": "player",
    "enabled": true,
    "parentId": null,
    "siblingOrder": 0,
    "transform": {
      "localPosition": { "x": 0, "y": 0, "z": 0 },
      "localRotation": { "x": 0, "y": 0, "z": 0 },
      "localScale": { "x": 1, "y": 1, "z": 1 }
    },
    "mesh": {
      "primitive": "model",
      "assetRef": { "id": "model-player", "type": "model" },
      "materialRef": { "id": "mat-player", "type": "material" },
      "visible": true,
      "layer": "default"
    },
    "components": [{
      "componentType": "game.health",
      "fields": { "hp": 10, "maxHp": 10 }
    }]
  }]
}"##,
    )
    .unwrap();
    scene_path
}

pub(super) fn unique_editor_project_temp_dir() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp_root = canonical_test_temp_dir();
    temp_root.join(format!("native-editor-project-{stamp}-{sequence}"))
}

fn canonical_test_temp_dir() -> PathBuf {
    let canonical =
        std::fs::canonicalize(std::env::temp_dir()).unwrap_or_else(|_| std::env::temp_dir());
    #[cfg(windows)]
    {
        let display = canonical.to_string_lossy();
        if let Some(unc) = display.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{unc}"));
        }
        if let Some(local) = display.strip_prefix(r"\\?\") {
            return PathBuf::from(local);
        }
    }
    canonical
}
