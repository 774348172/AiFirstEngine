//! Background cell loading through deferred session commands.
//!
//! Submit cell commands in a successful callback separate from synchronous World,
//! audio, particle, and animator mutations. A callback may submit multiple cell commands.
//! Admission is atomic; completion happens later at simulation safe points and must be
//! queried. Request IDs are nonzero and strictly increasing within a session. Receipts
//! retain at most 128 live/recent requests; terminal receipts may be evicted. Placement
//! resolution is a soft reference: it neither starts loading nor keeps resources alive.
use crate::{
    DeferredMutation, DeferredMutationWriter, EntityHandle, GameError, GameErrorCode, GameResult,
};
use project_runtime_sdk::{ProjectRuntimeCellEntity, ProjectRuntimeCellIntent as Intent};
pub use project_runtime_sdk::{
    ProjectRuntimeCellQuery as CellQuery, ProjectRuntimeCellReceipt as CellReceipt,
    ProjectRuntimeCellStatus as CellStatus,
    ProjectRuntimeVegetationFields3D as VegetationPromotionFields3D,
};

/// Session request correlation, never a GPU or entity handle. Do not reuse request IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellTicket {
    request_id: u64,
}
impl CellTicket {
    pub fn request_id(self) -> u64 {
        self.request_id
    }
    pub fn query(self) -> CellQuery {
        CellQuery {
            request_id: Some(self.request_id),
            placement_id: None,
            source_entity_id: None,
        }
    }
}

fn entity(handle: &EntityHandle) -> GameResult<ProjectRuntimeCellEntity> {
    if handle.generation() == 0 {
        return Err(GameError::new(
            GameErrorCode::InvalidArgument,
            "Resolve a live entity before a cell command",
        ));
    }
    Ok(ProjectRuntimeCellEntity {
        entity_id: handle.identity().into(),
        generation: handle.generation(),
    })
}
fn ticket(request_id: u64) -> GameResult<CellTicket> {
    if request_id == 0 {
        return Err(GameError::new(
            GameErrorCode::InvalidArgument,
            "Cell request IDs must be nonzero and strictly increasing",
        ));
    }
    Ok(CellTicket { request_id })
}
impl DeferredMutationWriter {
    /// Transfer a decoration source into a separately loaded gameplay Prefab.
    /// The returned ticket correlates completion; the target's cell owns its lifetime.
    pub fn promote_vegetation3d(
        &mut self,
        request_id: u64,
        owner: &EntityHandle,
        source_instance_id: impl Into<String>,
        target: &EntityHandle,
        fields: VegetationPromotionFields3D,
    ) -> GameResult<CellTicket> {
        let result = ticket(request_id)?;
        let source_instance_id = source_instance_id.into();
        if source_instance_id.is_empty() || source_instance_id.len() > 256 {
            return Err(GameError::new(
                GameErrorCode::InvalidArgument,
                "Invalid vegetation source",
            ));
        }
        self.mutations.push(DeferredMutation::Cell {
            intent: Intent::PromoteVegetation {
                request_id,
                placement_id: format!("vegetation-promotion/{request_id}"),
                owner: entity(owner)?,
                source_instance_id,
                target: entity(target)?,
                fields,
            },
        });
        Ok(result)
    }
    pub fn load_cell(
        &mut self,
        request_id: u64,
        placement_id: impl Into<String>,
        prefab: crate::SpatialAssetRef,
        parent: Option<&EntityHandle>,
        render_required: bool,
    ) -> GameResult<CellTicket> {
        let ticket = ticket(request_id)?;
        let placement_id = placement_id.into();
        if placement_id.trim().is_empty()
            || placement_id.len() > 256
            || prefab.asset_type != "prefab"
        {
            return Err(GameError::new(
                GameErrorCode::InvalidArgument,
                "Use a prefab AssetRef and nonempty cell placement ID",
            ));
        }
        let parent = parent.map(entity).transpose()?;
        self.mutations.push(DeferredMutation::Cell {
            intent: Intent::Load {
                request_id,
                placement_id,
                prefab,
                parent,
                render_required,
            },
        });
        Ok(ticket)
    }
    /// Cancels an invisible load or unloads an active cell at a simulation safe point.
    pub fn release_cell(&mut self, ticket: CellTicket) {
        self.mutations.push(DeferredMutation::Cell {
            intent: Intent::Release {
                request_id: ticket.request_id,
            },
        });
    }
    /// Keep-world migration preserves the entity incarnation and pins its resources.
    pub fn migrate_cell_subtree(
        &mut self,
        source_cell: CellTicket,
        persistent_request_id: u64,
        source: &EntityHandle,
        persistent_placement_id: impl Into<String>,
        parent: Option<&EntityHandle>,
    ) -> GameResult<CellTicket> {
        let result = ticket(persistent_request_id)?;
        let source = entity(source)?;
        let parent = parent.map(entity).transpose()?;
        let persistent_placement_id = persistent_placement_id.into();
        if persistent_placement_id.trim().is_empty() || persistent_placement_id.len() > 256 {
            return Err(GameError::new(
                GameErrorCode::InvalidArgument,
                "Invalid persistent placement ID",
            ));
        }
        self.mutations.push(DeferredMutation::Cell {
            intent: Intent::Migrate {
                request_id: source_cell.request_id,
                persistent_request_id,
                source,
                persistent_placement_id,
                parent,
            },
        });
        Ok(result)
    }
}
