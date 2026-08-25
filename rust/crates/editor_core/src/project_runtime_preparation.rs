use crate::{ProjectNativeModuleIdentity, ProjectRuntimeNativeModuleDiagnostic};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRuntimePreparationTicket {
    pub generation: u64,
    pub project_id: String,
    pub module_id: String,
    pub interface_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRuntimePreparationBlocker {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectRuntimePreparationState {
    Inactive,
    AwaitingTrust(ProjectRuntimePreparationTicket),
    Preparing(ProjectRuntimePreparationTicket),
    Ready {
        ticket: ProjectRuntimePreparationTicket,
        identity_digest: String,
    },
    Failed {
        ticket: ProjectRuntimePreparationTicket,
        diagnostic: ProjectRuntimeNativeModuleDiagnostic,
    },
}

pub struct ProjectRuntimePreparationModule {
    next_generation: u64,
    state: ProjectRuntimePreparationState,
}

impl Default for ProjectRuntimePreparationModule {
    fn default() -> Self {
        Self {
            next_generation: 1,
            state: ProjectRuntimePreparationState::Inactive,
        }
    }
}

impl ProjectRuntimePreparationModule {
    pub fn await_trust(
        &mut self,
        project_id: impl Into<String>,
        module_id: impl Into<String>,
        interface_version: impl Into<String>,
    ) -> ProjectRuntimePreparationTicket {
        let ticket = self.new_ticket(project_id, module_id, interface_version);
        self.state = ProjectRuntimePreparationState::AwaitingTrust(ticket.clone());
        ticket
    }

    pub fn begin(
        &mut self,
        project_id: impl Into<String>,
        module_id: impl Into<String>,
        interface_version: impl Into<String>,
    ) -> ProjectRuntimePreparationTicket {
        let ticket = self.new_ticket(project_id, module_id, interface_version);
        self.state = ProjectRuntimePreparationState::Preparing(ticket.clone());
        ticket
    }

    pub fn complete(
        &mut self,
        ticket: &ProjectRuntimePreparationTicket,
        identity: &ProjectNativeModuleIdentity,
    ) -> bool {
        if !self.is_current_preparing(ticket)
            || identity.project_id != ticket.project_id
            || identity.module_id != ticket.module_id
            || identity.logical_interface_version != ticket.interface_version
        {
            return false;
        }
        let Ok(identity_digest) = identity.digest() else {
            return false;
        };
        self.state = ProjectRuntimePreparationState::Ready {
            ticket: ticket.clone(),
            identity_digest,
        };
        true
    }

    pub fn fail(
        &mut self,
        ticket: &ProjectRuntimePreparationTicket,
        diagnostic: ProjectRuntimeNativeModuleDiagnostic,
    ) -> bool {
        if !self.is_current(ticket) {
            return false;
        }
        self.state = ProjectRuntimePreparationState::Failed {
            ticket: ticket.clone(),
            diagnostic,
        };
        true
    }

    pub fn cancel(&mut self) {
        self.next_generation = self.next_generation.saturating_add(1);
        self.state = ProjectRuntimePreparationState::Inactive;
    }

    pub fn state(&self) -> &ProjectRuntimePreparationState {
        &self.state
    }

    pub fn play_blocker(
        &self,
        project_id: &str,
        module_id: &str,
        interface_version: &str,
    ) -> Option<ProjectRuntimePreparationBlocker> {
        let exact = |ticket: &ProjectRuntimePreparationTicket| {
            ticket.project_id == project_id
                && ticket.module_id == module_id
                && ticket.interface_version == interface_version
        };
        match &self.state {
            ProjectRuntimePreparationState::Ready { ticket, .. } if exact(ticket) => None,
            ProjectRuntimePreparationState::AwaitingTrust(ticket) if exact(ticket) => {
                Some(ProjectRuntimePreparationBlocker {
                    code: "project_runtime.trust_required".to_string(),
                    message: "Approve this ProjectRust identity before starting Play.".to_string(),
                })
            }
            ProjectRuntimePreparationState::Preparing(ticket) if exact(ticket) => {
                Some(ProjectRuntimePreparationBlocker {
                    code: "project_runtime.preparation_pending".to_string(),
                    message: "The project runtime module is still being prepared.".to_string(),
                })
            }
            ProjectRuntimePreparationState::Failed { ticket, diagnostic } if exact(ticket) => {
                Some(ProjectRuntimePreparationBlocker {
                    code: diagnostic.code.clone(),
                    message: diagnostic.message.clone(),
                })
            }
            _ => Some(ProjectRuntimePreparationBlocker {
                code: "project_runtime.preparation_unavailable".to_string(),
                message: "No ready native runtime module matches the active project.".to_string(),
            }),
        }
    }

    fn new_ticket(
        &mut self,
        project_id: impl Into<String>,
        module_id: impl Into<String>,
        interface_version: impl Into<String>,
    ) -> ProjectRuntimePreparationTicket {
        let ticket = ProjectRuntimePreparationTicket {
            generation: self.next_generation,
            project_id: project_id.into(),
            module_id: module_id.into(),
            interface_version: interface_version.into(),
        };
        self.next_generation = self.next_generation.saturating_add(1);
        ticket
    }

    fn is_current(&self, ticket: &ProjectRuntimePreparationTicket) -> bool {
        match &self.state {
            ProjectRuntimePreparationState::AwaitingTrust(current)
            | ProjectRuntimePreparationState::Preparing(current)
            | ProjectRuntimePreparationState::Ready {
                ticket: current, ..
            }
            | ProjectRuntimePreparationState::Failed {
                ticket: current, ..
            } => current == ticket,
            ProjectRuntimePreparationState::Inactive => false,
        }
    }

    fn is_current_preparing(&self, ticket: &ProjectRuntimePreparationTicket) -> bool {
        matches!(
            &self.state,
            ProjectRuntimePreparationState::Preparing(current) if current == ticket
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        PROJECT_RUNTIME_NATIVE_MODULE_BUILDER_SCHEMA_VERSION,
        PROJECT_RUNTIME_NATIVE_MODULE_IDENTITY_SCHEMA_VERSION,
    };

    fn identity(project_id: &str) -> ProjectNativeModuleIdentity {
        let digest = |value: char| format!("sha256:{}", value.to_string().repeat(64));
        ProjectNativeModuleIdentity {
            schema_version: PROJECT_RUNTIME_NATIVE_MODULE_IDENTITY_SCHEMA_VERSION.to_string(),
            project_runtime_abi_digest: digest('1'),
            project_runtime_sdk_digest: digest('2'),
            project_id: project_id.to_string(),
            module_id: "fixture.runtime".to_string(),
            logical_interface_version: "project-runtime-module.v2".to_string(),
            aot_content_digest: digest('3'),
            normalized_manifest_digest: digest('4'),
            normalized_dependency_digest: digest('5'),
            dependency_lock_digest: digest('6'),
            toolchain_identity: "rustc-test".to_string(),
            target_triple: "host".to_string(),
            profile: "release".to_string(),
            features: Vec::new(),
            builder_schema_version: PROJECT_RUNTIME_NATIVE_MODULE_BUILDER_SCHEMA_VERSION
                .to_string(),
        }
    }

    #[test]
    fn project_runtime_preparation_fail_closes_until_exact_ready() {
        let mut module = ProjectRuntimePreparationModule::default();
        let trust = module.await_trust(
            "fixture.project",
            "fixture.runtime",
            "project-runtime-module.v2",
        );
        assert_eq!(
            module
                .play_blocker(
                    "fixture.project",
                    "fixture.runtime",
                    "project-runtime-module.v2"
                )
                .unwrap()
                .code,
            "project_runtime.trust_required"
        );
        assert!(!module.complete(&trust, &identity("fixture.project")));

        let preparing = module.begin(
            "fixture.project",
            "fixture.runtime",
            "project-runtime-module.v2",
        );
        assert_eq!(
            module
                .play_blocker(
                    "fixture.project",
                    "fixture.runtime",
                    "project-runtime-module.v2"
                )
                .unwrap()
                .code,
            "project_runtime.preparation_pending"
        );
        assert!(module.complete(&preparing, &identity("fixture.project")));
        assert!(module
            .play_blocker(
                "fixture.project",
                "fixture.runtime",
                "project-runtime-module.v2"
            )
            .is_none());
    }

    #[test]
    fn project_runtime_preparation_rejects_stale_completion_after_switch() {
        let mut module = ProjectRuntimePreparationModule::default();
        let stale = module.begin(
            "first.project",
            "fixture.runtime",
            "project-runtime-module.v2",
        );
        let current = module.begin(
            "second.project",
            "fixture.runtime",
            "project-runtime-module.v2",
        );
        assert!(!module.complete(&stale, &identity("first.project")));
        assert!(module.complete(&current, &identity("second.project")));
    }

    #[test]
    fn project_open_authoring_ready_before_runtime_module() {
        use editor_ui_model::UiCommandPayload;
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("aife-authoring-first-{stamp}"));
        let mut creator = crate::EditorSession::new();
        let created =
            creator.execute_command(crate::command_for_test(UiCommandPayload::CreateProject {
                path: root.display().to_string(),
                name: "AuthoringFirst".to_string(),
            }));
        assert_eq!(created.status, crate::CommandStatus::Committed);
        drop(creator);

        let mut stable = crate::EditorSession::new();
        let opened =
            stable.execute_command(crate::command_for_test(UiCommandPayload::OpenProject {
                path: root.display().to_string(),
            }));
        assert_eq!(opened.status, crate::CommandStatus::Committed);
        let project = stable.active_project_session().unwrap();
        let project_id = project.manifest.project_id.clone();
        let module_id = project.manifest.runtime_module.module_id.clone();
        let interface_version = project.manifest.runtime_module.interface_version.clone();
        let ticket = stable.begin_project_runtime_preparation(
            project_id.clone(),
            module_id.clone(),
            interface_version.clone(),
        );
        assert!(stable.active_project_session().is_some());
        assert!(matches!(
            stable.project_runtime_preparation_state(),
            ProjectRuntimePreparationState::Preparing(current)
                if current == &ticket
                    && current.project_id == project_id
                    && current.module_id == module_id
                    && current.interface_version == interface_version
        ));
        drop(stable);
        fs::remove_dir_all(root).unwrap();
    }
}
