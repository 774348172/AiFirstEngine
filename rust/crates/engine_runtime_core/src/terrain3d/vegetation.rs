//! Bounded decoration clusters. Projection produces ordinary affine matrices;
//! the renderer consumes wind through those matrices, without another GPU owner.
use super::{invalid, TerrainAssetIdentity3D};
use crate::render_contract::{RenderAssetKind, RenderAssetReference, ValidateRenderContract};
use crate::spatial_contract::ContractResult;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MIN_VEGETATION_CLUSTER_INSTANCES: usize = 64;
pub const MAX_VEGETATION_CLUSTER_INSTANCES: usize = 256;
pub const MAX_VEGETATION_INSTANCES: usize = 16_384;
pub const MAX_VEGETATION_CLUSTERS: usize = 64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VegetationWind3D {
    /// X/Z direction. Must be unit length; amplitude is shear per metre of Y.
    pub direction: [f32; 2],
    pub amplitude: f32,
    pub frequency_hz: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VegetationInstance3D {
    /// Stable authored identity, preserved when promoted into gameplay.
    pub source_id: String,
    pub translation: [f32; 3],
    pub yaw_radians: f32,
    pub scale: f32,
    pub wind_phase: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VegetationLodLevel3D {
    pub distance: f64,
    pub mesh: RenderAssetReference,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VegetationCluster3D {
    pub key: String,
    pub mesh: RenderAssetReference,
    pub material: RenderAssetReference,
    /// Optional lower-detail meshes after the original distance-zero mesh.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lods: Vec<VegetationLodLevel3D>,
    /// Mesh-local AABB. Mesh roots use local Y=0; negative Y is rejected.
    pub mesh_bounds: [[f32; 3]; 2],
    pub wind: VegetationWind3D,
    pub instances: Vec<VegetationInstance3D>,
}

impl VegetationCluster3D {
    pub fn mesh_levels(&self) -> impl Iterator<Item = (f64, &RenderAssetReference)> {
        std::iter::once((0., &self.mesh)).chain(self.lods.iter().map(|l| (l.distance, &l.mesh)))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VegetationAsset3D {
    pub schema_version: String,
    pub asset_id: String,
    pub asset_guid: String,
    pub clusters: Vec<VegetationCluster3D>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VegetationBinding3D {
    pub cluster_asset: TerrainAssetIdentity3D,
    #[serde(default)]
    pub promoted_instance_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VegetationDrawInstance3D {
    pub source_id: String,
    pub cluster_key: String,
    /// Column-major, column-vector affine matrix in the binding owner's space.
    pub matrix: [f32; 16],
    /// Conservative AABB over the entire wind cycle, in that same space.
    pub bounds: [[f32; 3]; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct VegetationPromotion3D {
    pub source_id: String,
    pub cluster_key: String,
    pub mesh: RenderAssetReference,
    pub material: RenderAssetReference,
    /// Gameplay starts at the stable root/rest pose, independent of wind time.
    pub matrix: [f32; 16],
}

fn name(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256
}

impl VegetationAsset3D {
    pub fn validate(&self) -> ContractResult<()> {
        TerrainAssetIdentity3D {
            asset_id: self.asset_id.clone(),
            asset_guid: self.asset_guid.clone(),
        }
        .validate()?;
        if self.schema_version != "vegetation3d-asset.v1"
            || self.clusters.is_empty()
            || self.clusters.len() > MAX_VEGETATION_CLUSTERS
        {
            return Err(invalid(
                "/vegetation",
                "Invalid vegetation schema or cluster count.",
            ));
        }
        let mut keys = BTreeSet::new();
        let mut ids = BTreeSet::new();
        let mut count = 0;
        for cluster in &self.clusters {
            if !name(&cluster.key)
                || !keys.insert(&cluster.key)
                || !(MIN_VEGETATION_CLUSTER_INSTANCES..=MAX_VEGETATION_CLUSTER_INSTANCES)
                    .contains(&cluster.instances.len())
            {
                return Err(invalid(
                    "/clusters",
                    "Clusters require unique keys and 64..256 instances.",
                ));
            }
            cluster
                .mesh
                .validate()
                .map_err(|e| invalid("/clusters/mesh", e.to_string()))?;
            cluster
                .material
                .validate()
                .map_err(|e| invalid("/clusters/material", e.to_string()))?;
            if cluster.mesh.expected_kind != RenderAssetKind::Mesh
                || cluster.material.expected_kind != RenderAssetKind::Material
            {
                return Err(invalid(
                    "/clusters",
                    "Expected ordinary mesh and material references.",
                ));
            }
            if cluster.lods.len() > 7 {
                return Err(invalid(
                    "/clusters/lods",
                    "Expected at most eight mesh levels.",
                ));
            }
            let mut previous = 0.;
            for level in &cluster.lods {
                if !level.distance.is_finite()
                    || level.distance <= previous
                    || level.distance > 1_000_000.
                {
                    return Err(invalid(
                        "/clusters/lods/distance",
                        "LOD distances must be finite, positive and strictly increase.",
                    ));
                }
                level
                    .mesh
                    .validate()
                    .map_err(|e| invalid("/clusters/lods/mesh", e.to_string()))?;
                if level.mesh.expected_kind != RenderAssetKind::Mesh {
                    return Err(invalid(
                        "/clusters/lods/mesh",
                        "Expected an ordinary mesh reference.",
                    ));
                }
                previous = level.distance;
            }
            let [min, max] = cluster.mesh_bounds;
            if min
                .into_iter()
                .chain(max)
                .any(|v| !v.is_finite() || v.abs() > 256.)
                || (0..3).any(|i| min[i] > max[i])
                || min[1] < 0.
                || max[1] <= 0.
            {
                return Err(invalid(
                    "/clusters/meshBounds",
                    "Expected bounded nonnegative-height mesh bounds.",
                ));
            }
            let wind = &cluster.wind;
            let direction_length = wind.direction.iter().map(|v| v * v).sum::<f32>();
            if !direction_length.is_finite()
                || (direction_length - 1.).abs() > 0.001
                || !wind.amplitude.is_finite()
                || !(0. ..=0.5).contains(&wind.amplitude)
                || !wind.frequency_hz.is_finite()
                || !(0. ..=10.).contains(&wind.frequency_hz)
            {
                return Err(invalid(
                    "/clusters/wind",
                    "Invalid bounded wind direction, amplitude or frequency.",
                ));
            }
            for instance in &cluster.instances {
                if !name(&instance.source_id)
                    || !ids.insert(&instance.source_id)
                    || instance
                        .translation
                        .iter()
                        .any(|v| !v.is_finite() || v.abs() > 2048.)
                    || !instance.yaw_radians.is_finite()
                    || instance.yaw_radians.abs() > 1000.
                    || !instance.scale.is_finite()
                    || !(0.01..=16.).contains(&instance.scale)
                    || !instance.wind_phase.is_finite()
                    || instance.wind_phase.abs() > 1000.
                {
                    return Err(invalid(
                        "/clusters/instances",
                        "Invalid transform or duplicate stable source identity.",
                    ));
                }
            }
            count += cluster.instances.len();
        }
        if count > MAX_VEGETATION_INSTANCES {
            return Err(invalid("/clusters", "Vegetation instance budget exceeded."));
        }
        Ok(())
    }

    pub fn validate_binding(&self, binding: &VegetationBinding3D) -> ContractResult<()> {
        self.validate()?;
        if binding.cluster_asset.asset_id != self.asset_id
            || binding.cluster_asset.asset_guid != self.asset_guid
        {
            return Err(invalid(
                "/clusterAsset",
                "Vegetation binding identity does not match its asset.",
            ));
        }
        let available: BTreeSet<&str> = self
            .clusters
            .iter()
            .flat_map(|c| c.instances.iter().map(|i| i.source_id.as_str()))
            .collect();
        let mut seen = BTreeSet::new();
        for id in &binding.promoted_instance_ids {
            if !available.contains(id.as_str()) || !seen.insert(id) {
                return Err(invalid(
                    "/promotedInstanceIds",
                    "Unknown or duplicate promoted source identity.",
                ));
            }
        }
        Ok(())
    }

    /// The runtime calls this once per binding/frame and emits these matrices
    /// into its existing mesh instance packet. Promoted sources are excluded.
    pub fn project_instances(
        &self,
        binding: &VegetationBinding3D,
        time_seconds: f64,
    ) -> ContractResult<Vec<VegetationDrawInstance3D>> {
        self.validate_binding(binding)?;
        if !time_seconds.is_finite() || time_seconds < 0. {
            return Err(invalid(
                "/time",
                "Expected finite nonnegative presentation time.",
            ));
        }
        let promoted: BTreeSet<&str> = binding
            .promoted_instance_ids
            .iter()
            .map(String::as_str)
            .collect();
        let mut output = Vec::new();
        for cluster in &self.clusters {
            for instance in &cluster.instances {
                if promoted.contains(instance.source_id.as_str()) {
                    continue;
                }
                // Reduce in f64 before casting, avoiding overflow in long sessions.
                let frequency = f64::from(cluster.wind.frequency_hz);
                let cycles = if frequency == 0. {
                    0.
                } else {
                    time_seconds.rem_euclid(1. / frequency) * frequency
                };
                let phase = (cycles * std::f64::consts::TAU + f64::from(instance.wind_phase))
                    .rem_euclid(std::f64::consts::TAU) as f32;
                let shear = phase.sin() * cluster.wind.amplitude;
                output.push(VegetationDrawInstance3D {
                    source_id: instance.source_id.clone(),
                    cluster_key: cluster.key.clone(),
                    matrix: instance_matrix(instance, cluster.wind.direction, shear),
                    bounds: cycle_bounds(instance, cluster),
                });
            }
        }
        Ok(output)
    }

    pub fn promote(
        &self,
        binding: &VegetationBinding3D,
        source_id: &str,
    ) -> ContractResult<VegetationPromotion3D> {
        self.validate_binding(binding)?;
        if !binding
            .promoted_instance_ids
            .iter()
            .any(|id| id == source_id)
        {
            return Err(invalid(
                "/promotion",
                "Source identity is not marked for gameplay promotion.",
            ));
        }
        let (cluster, instance) = self
            .clusters
            .iter()
            .find_map(|c| {
                c.instances
                    .iter()
                    .find(|i| i.source_id == source_id)
                    .map(|i| (c, i))
            })
            .ok_or_else(|| invalid("/promotion", "Unknown source identity."))?;
        Ok(VegetationPromotion3D {
            source_id: source_id.into(),
            cluster_key: cluster.key.clone(),
            mesh: cluster.mesh.clone(),
            material: cluster.material.clone(),
            matrix: instance_matrix(instance, cluster.wind.direction, 0.),
        })
    }
}

fn instance_matrix(instance: &VegetationInstance3D, direction: [f32; 2], shear: f32) -> [f32; 16] {
    let (sin, cos) = instance.yaw_radians.sin_cos();
    let s = instance.scale;
    // Rotation * uniform scale, then binding-space Y shear. Local Y=0 is fixed.
    [
        s * cos,
        0.,
        -s * sin,
        0.,
        s * direction[0] * shear,
        s,
        s * direction[1] * shear,
        0.,
        s * sin,
        0.,
        s * cos,
        0.,
        instance.translation[0],
        instance.translation[1],
        instance.translation[2],
        1.,
    ]
}

fn cycle_bounds(instance: &VegetationInstance3D, cluster: &VegetationCluster3D) -> [[f32; 3]; 2] {
    let matrix = instance_matrix(instance, cluster.wind.direction, 0.);
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for x in cluster.mesh_bounds.map(|v| v[0]) {
        for y in cluster.mesh_bounds.map(|v| v[1]) {
            for z in cluster.mesh_bounds.map(|v| v[2]) {
                for axis in 0..3 {
                    let value = matrix[axis] * x
                        + matrix[4 + axis] * y
                        + matrix[8 + axis] * z
                        + matrix[12 + axis];
                    min[axis] = min[axis].min(value);
                    max[axis] = max[axis].max(value);
                }
            }
        }
    }
    let sway = instance.scale * cluster.mesh_bounds[1][1] * cluster.wind.amplitude;
    for (axis, component) in [
        (0, cluster.wind.direction[0]),
        (2, cluster.wind.direction[1]),
    ] {
        let margin = sway * component.abs() + 0.0001;
        min[axis] -= margin;
        max[axis] += margin;
    }
    [min, max]
}
