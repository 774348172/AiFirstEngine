//! 371 S0-B offline facilities. Production measurements are integrated in S1-D.
mod budget;
mod cells;
mod combined;
mod driver;
mod foundation;
mod lit_cook;
mod measurement;
mod simulation;
mod static_cook;
mod world;
pub use budget::*;
pub use cells::*;
pub use combined::inspect_bm10_input;
pub use driver::*;
pub use foundation::*;
pub use lit_cook::*;
pub use measurement::*;
pub use simulation::{
    inspect_bm05_input, inspect_bm06_input, inspect_bm07_input, inspect_simulation_cook,
};
pub use static_cook::*;
pub use world::inspect_bm13_input;

pub const TARGET_RECIPES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../samples/platform_3d_benchmark_project/Benchmark/target-recipes.json"
));
pub const BASE_SEED: u32 = 0x0371_0A11;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkError {
    pub code: &'static str,
    pub message: String,
    pub owner_details: Option<serde_json::Value>,
}
impl BenchmarkError {
    pub(super) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            owner_details: None,
        }
    }
}
impl std::fmt::Display for BenchmarkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for BenchmarkError {}
pub type BenchmarkResult<T> = Result<T, BenchmarkError>;

pub fn xorshift32(state: &mut u32) -> u32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    *state
}

pub fn target_catalog() -> BenchmarkResult<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_slice(TARGET_RECIPES)
        .map_err(|e| BenchmarkError::new("benchmark.recipe_invalid", e.to_string()))?;
    if value["revision"] != "371-target-v1" || value["baseSeed"] != BASE_SEED {
        return Err(BenchmarkError::new(
            "benchmark.recipe_revision",
            "unexpected target catalog",
        ));
    }
    let rows = value["workloads"]
        .as_array()
        .ok_or_else(|| BenchmarkError::new("benchmark.recipe_missing", "workloads"))?;
    if rows.len() != 15 {
        return Err(BenchmarkError::new(
            "benchmark.recipe_missing",
            "all 15 workloads required",
        ));
    }
    for (index, row) in rows.iter().enumerate() {
        let id = (index + 1) as u32;
        if row["id"] != format!("BM-{id:02}") || row["seed"] != (BASE_SEED ^ id) {
            return Err(BenchmarkError::new(
                "benchmark.recipe_identity",
                format!("BM-{id:02}"),
            ));
        }
    }
    Ok(value)
}
