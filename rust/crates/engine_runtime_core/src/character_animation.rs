//! Cooked 3D character data and fixed-tick animation. No source files or backend handles.
#[path = "character_animation/asset.rs"]
mod asset;
#[path = "character_animation/binding.rs"]
mod binding;
#[path = "character_animation/controller.rs"]
mod controller;
#[path = "character_animation/deformation.rs"]
mod deformation;
#[path = "character_animation/modes.rs"]
mod modes;
#[path = "character_animation/pose_ops.rs"]
mod pose_ops;
#[path = "character_animation/presentation.rs"]
mod presentation;
#[path = "character_animation/root_motion.rs"]
mod root_motion;
#[path = "character_animation/sampling.rs"]
mod sampling;
#[path = "character_animation/session.rs"]
pub mod session;
#[path = "character_animation/solvers.rs"]
mod solvers;
pub use asset::*;
pub(crate) use binding::permits_control_bootstrap;
pub(crate) use binding::prepare_control;
pub use binding::*;
pub(crate) use binding::{dynamic_json, runtime_json};
pub use controller::*;
pub use deformation::*;
pub use modes::{CharacterMode3D, Ragdoll3D, RootMotion3D};
pub use pose_ops::*;
pub use sampling::*;
pub use solvers::*;

use crate::spatial_contract::{ContractResult, SpatialContractError};
fn invalid(path: &str, message: impl Into<String>) -> SpatialContractError {
    SpatialContractError::new("character_invalid", path, message)
}
