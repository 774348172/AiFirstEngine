pub use project_authoring_execution::{
    BuildProfile, BuildProfileApplication, BuildProfileIconRef, BuildProfileRelease,
    BuildProfileValidationIssue, PrefabRuntimeBakeInstanceEntry, PrefabRuntimeBakeReport,
    ProjectRuntimePackageAssembler, ProjectRuntimePackageAssemblyDiagnostic,
    ProjectRuntimePackageAssemblyDomain, ProjectRuntimePackageAssemblyReport,
    ProjectRuntimePackageAssemblyRequest, ProjectRuntimePackageAssemblyResult,
    ProjectRuntimePackageAssemblySeverity, ProjectRuntimePackageAssemblyStatus,
    ProjectRuntimeSourceMapping, BUILD_PROFILE_SCHEMA_VERSION, BUILD_PROFILE_SCHEMA_VERSION_V1,
    PREFAB_RUNTIME_BAKE_REPORT_SCHEMA_VERSION,
    PROJECT_RUNTIME_PACKAGE_ASSEMBLY_REPORT_SCHEMA_VERSION,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommandStatus, EditorSession, EditorVec3, SceneEditCommand};
    use project_authoring_execution::{
        GameProjectCompiler, ProjectAuthoringSession, TargetProfile,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn editor_compiler_adapter_matches_neutral_owner_and_ignores_unsaved_draft() {
        let source = workspace_root().join("samples/complex_shooter_project");
        let project = unique_temp_dir("editor-compiler-adapter");
        let _guard = TestDirectoryGuard(project.clone());
        copy_source_tree(&source, &project);

        let adapter = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&project),
        );
        assert_eq!(
            adapter.status,
            ProjectRuntimePackageAssemblyStatus::Success,
            "{:#?}",
            adapter.report.diagnostics
        );
        let adapter_input = serde_json::to_vec(adapter.build_input.as_ref().unwrap()).unwrap();

        let mut owner_session = ProjectAuthoringSession::open(&project).unwrap();
        let paths = owner_session
            .source_inventory()
            .unwrap()
            .entries
            .into_iter()
            .map(|entry| entry.relative_path)
            .collect();
        let lease = owner_session
            .acquire_snapshot_lease("f-d-editor-owner-conformance", paths)
            .unwrap();
        let compiler = GameProjectCompiler::bind(&lease).unwrap();
        let first = compiler.prepare(&lease, TargetProfile::WindowsDev).unwrap();
        let second = compiler.prepare(&lease, TargetProfile::WindowsDev).unwrap();
        assert_eq!(first.preparation_identity(), second.preparation_identity());
        assert_eq!(
            adapter_input,
            serde_json::to_vec(first.runtime_package_build_input()).unwrap()
        );

        let scene_path = project.join("Scenes/Main.scene.json");
        let canonical_scene = fs::read(&scene_path).unwrap();
        let mut editor = EditorSession::new();
        assert_eq!(
            editor.open_scene_document_for_test(&scene_path).status,
            CommandStatus::Committed
        );
        assert_eq!(
            editor
                .execute_scene_edit_for_test(SceneEditCommand::SetTransform {
                    entity_id: "entity-player".to_string(),
                    local_position: Some(EditorVec3 {
                        x: 99.0,
                        y: -4.5,
                        z: 0.0,
                    }),
                    local_rotation: None,
                    local_scale: None,
                })
                .status,
            CommandStatus::Committed
        );
        assert_eq!(fs::read(&scene_path).unwrap(), canonical_scene);

        let after_unsaved_draft = ProjectRuntimePackageAssembler::assemble(
            ProjectRuntimePackageAssemblyRequest::new(&project),
        );
        assert_eq!(
            adapter_input,
            serde_json::to_vec(after_unsaved_draft.build_input.as_ref().unwrap()).unwrap()
        );
    }

    fn copy_source_tree(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            if matches!(
                name.to_str(),
                Some("Build" | "Library" | "target" | ".aife" | ".git")
            ) {
                continue;
            }
            let source_path = entry.path();
            let destination_path = destination.join(name);
            if entry.file_type().unwrap().is_dir() {
                copy_source_tree(&source_path, &destination_path);
            } else {
                fs::copy(source_path, destination_path).unwrap();
            }
        }
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("aife-{label}-{}-{nonce}", std::process::id()))
    }

    struct TestDirectoryGuard(PathBuf);

    impl Drop for TestDirectoryGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
