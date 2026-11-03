//! Session callbacks submit cell demands separately from ordinary transform writes.
use project_game_sdk::{
    CellTicket, DeferredMutationWriter, GameResult, SpatialAssetRef, WorldRead,
};

fn request(writer: &mut DeferredMutationWriter) -> GameResult<CellTicket> {
    writer.load_cell(
        1,
        "district-west",
        SpatialAssetRef {
            id: "district-prefab".into(),
            asset_type: "prefab".into(),
            guid: Some("district-prefab-guid".into()),
            sub_asset: None,
        },
        None,
        true,
    )
}
fn next_callback(
    world: &dyn WorldRead,
    writer: &mut DeferredMutationWriter,
    ticket: CellTicket,
) -> GameResult<()> {
    if let Some(receipt) = world.cell_status(ticket)? {
        if receipt.status == project_game_sdk::CellStatus::Active {
            let _root = world.resolve_cell_entity("district-west", "root")?;
            // A later separate callback may release it, then reload with a new request ID.
            writer.release_cell(ticket);
        }
    }
    Ok(())
}
fn main() {
    let _ = (request, next_callback);
}
