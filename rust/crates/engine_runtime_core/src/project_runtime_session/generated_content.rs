//! Async content admission stays in the original callback mutation path.
use super::*;
use crate::runtime_instance_loader::RuntimeInstanceLoader;
use project_runtime_sdk::generated_content::{
    ContentError, ContentErrorCode, ContentTicket, GeneratedContentRequest,
};
use project_runtime_sdk::project_storage::ProjectStorageRequest;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Intent {
    Submit(Box<GeneratedContentRequest>),
    Cancel(ContentTicket),
    Storage(Box<ProjectStorageRequest>),
    Abandon(ContentTicket),
}

impl ProjectRuntimeMutationBuffer {
    pub(crate) fn generated_content_request(&mut self, request: GeneratedContentRequest) {
        self.generated_content
            .push(Intent::Submit(Box::new(request)));
    }

    pub(crate) fn generated_content_cancel(&mut self, ticket: ContentTicket) {
        self.generated_content.push(Intent::Cancel(ticket));
    }

    pub(crate) fn project_storage_request(&mut self, request: ProjectStorageRequest) {
        self.generated_content
            .push(Intent::Storage(Box::new(request)));
    }

    pub(crate) fn project_storage_abandon(&mut self, ticket: ContentTicket) {
        self.generated_content.push(Intent::Abandon(ticket));
    }
}

pub(super) fn admit(
    intents: Vec<Intent>,
    loader: &mut RuntimeInstanceLoader,
    world: &World,
    report: &mut ProjectRuntimeMutationCommitReport,
) {
    report.staged_count += intents.len();
    for intent in intents {
        let result = match intent {
            Intent::Submit(request) if cfg!(feature = "real-wgpu") => {
                loader.accept_generated_content(*request, world).map(|_| ())
            }
            // CPU-only hosts have no native publication consumer.
            Intent::Submit(request) => Err(ContentError {
                code: ContentErrorCode::CapabilityGap,
                request_id: request.request_id,
                field_path: "capability.generatedContent.nativePublication".into(),
                expected: None,
                actual: None,
                next_action:
                    "Use packaged content until generated-content publication is available.".into(),
            }),
            Intent::Cancel(ticket) => loader.cancel_generated_content(ticket),
            Intent::Storage(request) => loader.accept_project_storage(*request).map(|_| ()),
            Intent::Abandon(ticket) => loader.abandon_project_storage(ticket),
        };
        match result {
            Ok(()) => report.committed_count += 1,
            Err(error) => {
                report.rejected_count += 1;
                report.generated_content_errors.push(error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_asset::{RuntimeAssetIndex, RuntimePackageMountTable};
    use crate::runtime_asset_loader::{RuntimeAssetLoader, StreamingBudget};
    use project_runtime_sdk::generated_content::*;

    #[test]
    fn generated_content_376_ingress_admission_refusal_preserves_previous_request_and_can_cancel() {
        let mut world = World::new();
        world.commit_prepared_entity(
            "surface".into(),
            "surface".into(),
            "spatial".into(),
            true,
            crate::components::Hierarchy {
                parent_id: None,
                sibling_order: 0,
            },
            vec![ComponentValue::Transform3D(
                crate::spatial_contract::Transform3D::identity()
                    .canonicalize()
                    .unwrap(),
            )],
        );
        let mut loader = RuntimeInstanceLoader::new(RuntimeAssetLoader::new(
            "unused",
            RuntimeAssetIndex::new(vec![], vec![]),
            RuntimePackageMountTable::new(vec![]),
        ));
        let budget = loader.asset_loader().streaming_budget();
        loader
            .asset_loader_mut()
            .set_streaming_budget(StreamingBudget {
                max_operations: 1,
                ..budget
            })
            .unwrap();
        let request = |id| {
            Intent::Submit(Box::new(GeneratedContentRequest {
                request_id: id,
                target: project_runtime_sdk::ProjectRuntimeCellEntity {
                    entity_id: "surface".into(),
                    generation: world.entity_generation(&"surface".into()).unwrap(),
                },
                expected_applied_revision: 0,
                operation: GeneratedOperation3D::Release,
            }))
        };
        let Intent::Submit(first) = request(1) else {
            unreachable!()
        };
        loader.accept_generated_content(*first, &world).unwrap();
        let mut report = ProjectRuntimeMutationCommitReport::default();
        admit(vec![request(2)], &mut loader, &world, &mut report);
        assert_eq!(
            (
                report.staged_count,
                report.committed_count,
                report.rejected_count
            ),
            (1, 0, 1)
        );
        assert_eq!(
            report.generated_content_errors[0].code,
            if cfg!(feature = "real-wgpu") {
                ContentErrorCode::BudgetExceeded
            } else {
                ContentErrorCode::CapabilityGap
            }
        );
        assert_eq!(
            report.generated_content_errors[0].field_path,
            if cfg!(feature = "real-wgpu") {
                "queue"
            } else {
                "capability.generatedContent.nativePublication"
            }
        );
        assert_eq!(report.generated_content_errors[0].request_id, 2);
        let ContentQueryValue::Receipt { receipt } = loader
            .query_generated_content(ContentQuery::Request { request_id: 1 }, &world)
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(receipt.status, ContentStatus::Accepted);
        assert_eq!(receipt.pending_revision, Some(receipt.candidate_revision));
        admit(
            vec![Intent::Cancel(receipt.ticket)],
            &mut loader,
            &world,
            &mut report,
        );
        let receipt = loader.generated_content_receipt(receipt.ticket).unwrap();
        assert_eq!(receipt.status, ContentStatus::Cancelled);
        assert_eq!(receipt.pending_revision, None);
    }
}
