//! Unified World projection for cooked navigation; native handles stay private.
mod contract;
pub use contract::*;

/// Explicit project reads expand the committed path; the per-tick component
/// remains compact and native query handles never cross the project boundary.
pub(crate) fn read_state(
    world: &crate::world::World,
    id: &crate::ids::EntityId,
) -> Option<crate::archetype::ComponentValue> {
    use crate::{
        archetype::ComponentValue, component_value::RuntimeValue, components::ComponentTypeId,
    };
    let state = world.navigation3d_states.get(id)?;
    let kind = ComponentTypeId::new("engine.navigation3d_state");
    let ComponentValue::Dynamic {
        value: RuntimeValue::Object(mut fields),
        ..
    } = world.component_value(id, &kind)?
    else {
        return None;
    };
    fields.insert(
        "points".into(),
        RuntimeValue::Array(
            state
                .points
                .iter()
                .map(|point| {
                    RuntimeValue::Array(point.iter().map(|n| RuntimeValue::F64(*n)).collect())
                })
                .collect(),
        ),
    );
    fields.insert(
        "offMesh".into(),
        RuntimeValue::Array(
            state
                .off_mesh
                .iter()
                .copied()
                .map(RuntimeValue::Bool)
                .collect(),
        ),
    );
    Some(ComponentValue::Dynamic {
        component_type: kind,
        value: RuntimeValue::Object(fields),
    })
}

mod control;
pub(crate) use control::{permits_control_bootstrap, prepare_control};
#[cfg(not(target_arch = "wasm32"))]
mod session;
#[cfg(not(target_arch = "wasm32"))]
pub use session::NavigationModule;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use session::{ActivatedNavigation3D, PreparedNavigation3D};
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Default)]
pub struct NavigationModule;
#[cfg(target_arch = "wasm32")]
impl NavigationModule {
    pub fn advance(
        &mut self,
        world: &mut crate::world::World,
        _loader: &mut crate::runtime_asset_loader::RuntimeAssetLoader,
        _tick: u64,
        _dt: f32,
    ) -> crate::spatial_contract::ContractResult<()> {
        use crate::{components::ComponentTypeId, query::QuerySpec};
        if !world
            .query_entities(&QuerySpec::all([ComponentTypeId::new(
                "engine.navigation3d",
            )]))
            .is_empty()
        {
            return Err(contract::invalid(
                "/backend",
                "Recast/Detour is unavailable on wasm32.",
            ));
        }
        Ok(())
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod steering_tests;
