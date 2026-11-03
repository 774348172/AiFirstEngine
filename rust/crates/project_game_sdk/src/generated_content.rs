//! Typed runtime content submissions. Tickets correlate admission; they do not prove upload/display.
use crate::{
    DeferredMutation, DeferredMutationWriter, EntityHandle, GameError, GameErrorCode, GameResult,
};
pub use project_runtime_sdk::generated_content::*;
pub use project_runtime_sdk::project_storage::*;

fn error(e: ContentError) -> GameError {
    GameError::new(GameErrorCode::InvalidArgument, e.to_string())
}
impl DeferredMutationWriter {
    /// Explicitly release an unread result through the existing deferred result owner.
    pub fn abandon_project_bytes(&mut self, ticket: ContentTicket) -> GameResult<()> {
        StorageQuery::Ticket { ticket }.validate().map_err(error)?;
        self.mutations
            .push(DeferredMutation::ProjectStorageAbandon { ticket });
        Ok(())
    }
    pub fn cancel_generated_content(&mut self, ticket: ContentTicket) -> GameResult<()> {
        ContentQuery::Ticket { ticket }.validate().map_err(error)?;
        self.mutations
            .push(DeferredMutation::GeneratedContentCancel { ticket });
        Ok(())
    }
    fn generated(
        &mut self,
        entity: &EntityHandle,
        request_id: u64,
        expected_applied_revision: u64,
        operation: GeneratedOperation3D,
    ) -> GameResult<u64> {
        let request = GeneratedContentRequest {
            request_id,
            target: project_runtime_sdk::ProjectRuntimeCellEntity {
                entity_id: entity.identity().into(),
                generation: entity.generation(),
            },
            expected_applied_revision,
            operation,
        };
        request.validate().map_err(error)?;
        if request.payload_bytes() > project_runtime_sdk::spatial_bulk::MAX_SESSION_BYTES {
            return Err(error(ContentError::invalid("request.bytes")));
        }
        self.mutations
            .push(DeferredMutation::GeneratedContent { request });
        Ok(request_id)
    }
    pub fn replace_geometry3d(
        &mut self,
        entity: &EntityHandle,
        request_id: u64,
        expected_applied_revision: u64,
        geometry: Geometry3D,
        collision: CollisionUpdate3D,
    ) -> GameResult<u64> {
        self.generated(
            entity,
            request_id,
            expected_applied_revision,
            GeneratedOperation3D::Replace {
                geometry,
                collision,
            },
        )
    }
    pub fn update_geometry_attributes3d(
        &mut self,
        entity: &EntityHandle,
        request_id: u64,
        expected_applied_revision: u64,
        patch: AttributePatch3D,
    ) -> GameResult<u64> {
        self.generated(
            entity,
            request_id,
            expected_applied_revision,
            GeneratedOperation3D::UpdateAttributes { patch },
        )
    }
    pub fn replace_collision3d(
        &mut self,
        entity: &EntityHandle,
        request_id: u64,
        expected_applied_revision: u64,
        collision: CollisionUpdate3D,
    ) -> GameResult<u64> {
        self.generated(
            entity,
            request_id,
            expected_applied_revision,
            GeneratedOperation3D::ReplaceCollision { collision },
        )
    }
    pub fn release_generated_geometry3d(
        &mut self,
        entity: &EntityHandle,
        request_id: u64,
        expected_applied_revision: u64,
    ) -> GameResult<u64> {
        self.generated(
            entity,
            request_id,
            expected_applied_revision,
            GeneratedOperation3D::Release,
        )
    }
    fn storage(&mut self, request_id: u64, operation: StorageOperation) -> GameResult<u64> {
        let request = ProjectStorageRequest {
            request_id,
            operation,
        };
        request.validate().map_err(error)?;
        if matches!(&request.operation,StorageOperation::Write{bytes,..} if bytes.len()>project_runtime_sdk::spatial_bulk::MAX_SESSION_BYTES)
        {
            return Err(error(ContentError::invalid("storage.bytes")));
        }
        self.mutations
            .push(DeferredMutation::ProjectStorage { request });
        Ok(request_id)
    }
    pub fn read_project_bytes(
        &mut self,
        request_id: u64,
        key: impl Into<String>,
    ) -> GameResult<u64> {
        self.storage(request_id, StorageOperation::Read { key: key.into() })
    }
    pub fn write_project_bytes(
        &mut self,
        request_id: u64,
        key: impl Into<String>,
        bytes: Vec<u8>,
    ) -> GameResult<u64> {
        self.storage(
            request_id,
            StorageOperation::Write {
                key: key.into(),
                bytes,
            },
        )
    }
    pub fn delete_project_bytes(
        &mut self,
        request_id: u64,
        key: impl Into<String>,
    ) -> GameResult<u64> {
        self.storage(request_id, StorageOperation::Delete { key: key.into() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_content_376_error_diagnostic_keeps_request_and_revision_repair_fields() {
        let mut error = ContentError::invalid("expectedAppliedRevision");
        error.code = ContentErrorCode::RevisionConflict;
        error.request_id = 7;
        error.expected = Some(4);
        error.actual = Some(3);
        let text = error.to_string();
        for expected in [
            "RevisionConflict",
            "request_id=7",
            "expectedAppliedRevision",
            "expected=Some(4)",
            "actual=Some(3)",
            &error.next_action,
        ] {
            assert!(text.contains(expected), "{text}");
        }
    }
    #[test]
    fn generated_content_376_writer_stages_owned_data_and_preserves_old_tail_on_invalid_input() {
        let target = EntityHandle::new("surface", 7).unwrap();
        let mut writer = DeferredMutationWriter::default();
        writer.release_generated_geometry3d(&target, 1, 0).unwrap();
        assert!(writer.release_generated_geometry3d(&target, 0, 0).is_err());
        assert!(writer.write_project_bytes(2, "../outside", vec![]).is_err());
        assert_eq!(writer.mutations.len(), 1);
        assert!(writer
            .cancel_generated_content(ContentTicket {
                session_generation: 0,
                request_id: 8
            })
            .is_err());
        writer
            .write_project_bytes(2, "world/parameters", vec![0, 1, 255])
            .unwrap();
        writer
            .cancel_generated_content(ContentTicket {
                session_generation: 5,
                request_id: 1,
            })
            .unwrap();
        writer
            .abandon_project_bytes(ContentTicket {
                session_generation: 5,
                request_id: 2,
            })
            .unwrap();
        let mut output = project_runtime_sdk::ProjectRuntimeSessionOutput::no_op();
        output.mutations = writer.mutations;
        let decoded = project_runtime_sdk::spatial_bulk::decode_session(
            &project_runtime_sdk::spatial_bulk::encode_session(output.clone()).unwrap(),
        )
        .unwrap();
        assert_eq!(decoded, output);
        let DeferredMutation::GeneratedContent { request } = &decoded.mutations[0] else {
            panic!()
        };
        assert_eq!(request.target.generation, 7);
    }
}
