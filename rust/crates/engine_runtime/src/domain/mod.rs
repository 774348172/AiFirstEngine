//! Stable domain entrypoints for engine runtime modules.
//!
//! These modules are re-export facades only. They do not introduce runtime
//! behavior or replace the existing public module paths.

pub mod asset;
pub mod ecs;
pub mod frame_loop;
pub mod input;
pub mod logic;
pub mod package;
pub mod physics;
pub mod render;
pub mod validation;
