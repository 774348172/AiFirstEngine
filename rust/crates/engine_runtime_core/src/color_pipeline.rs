//! Opt-in scene-linear HDR color contract. No renderer or asset registry lives here.
use crate::render_contract::{ContractError, ContractErrorCode};
use crate::render_graph::OrderedF32;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToneMapping3D {
    AcesFitted,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AntiAliasing3D {
    None,
    Fxaa,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColorPipeline3D {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ibl: Option<ImageBasedLighting3D>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sky: Option<GradientSky3D>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fog: Option<DistanceFog3D>,
    pub schema_version: String,
    #[serde(with = "number")]
    pub exposure_ev: OrderedF32,
    pub tone_mapping: ToneMapping3D,
    pub anti_aliasing: AntiAliasing3D,
    #[serde(with = "number3")]
    pub ambient_diffuse: [OrderedF32; 3],
}
impl Default for ColorPipeline3D {
    fn default() -> Self {
        Self {
            ibl: None,
            sky: None,
            fog: None,
            schema_version: "view-color3d.v1".into(),
            exposure_ev: 0.0.into(),
            tone_mapping: ToneMapping3D::AcesFitted,
            anti_aliasing: AntiAliasing3D::Fxaa,
            ambient_diffuse: [0.0.into(); 3],
        }
    }
}
impl ColorPipeline3D {
    #[doc(hidden)]
    pub fn observe_memory(
        &self,
        sample: &mut crate::resource_memory::ResourceMemorySample,
        owner: &str,
        role: &str,
    ) {
        sample.cpu_string(&self.schema_version, owner, &[role]);
        if let Some(ibl) = &self.ibl {
            let reference = &ibl.environment.0;
            for value in [&reference.id, &reference.asset_type].into_iter().chain(
                [reference.guid.as_ref(), reference.sub_asset.as_ref()]
                    .into_iter()
                    .flatten(),
            ) {
                sample.cpu_string(value, owner, &[role]);
            }
        }
    }

    pub fn validate_clear(&self, clear: Option<[f32; 4]>) -> Result<(), ContractError> {
        if clear.is_none_or(|c| {
            c[..3]
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=65504.0).contains(v))
                || !c[3].is_finite()
                || !(0.0..=1.0).contains(&c[3])
        }) {
            return Err(ContractError::new(
                ContractErrorCode::InvalidProjection,
                "camera.colorPipeline.clear",
            ));
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        let valid_color = |c: &[OrderedF32; 3]| {
            c.iter()
                .all(|v| v.to_f32().is_finite() && (0.0..=65504.0).contains(&v.to_f32()))
        };
        if self.sky.as_ref().is_some_and(|s| {
            !valid_color(&s.zenith) || !valid_color(&s.horizon) || !valid_color(&s.nadir)
        }) || self.fog.as_ref().is_some_and(|f| {
            !valid_color(&f.color)
                || !f.density_per_m.to_f32().is_finite()
                || !(0.0..=100.0).contains(&f.density_per_m.to_f32())
                || !f.start_distance_m.to_f32().is_finite()
                || !(0.0..=1e9).contains(&f.start_distance_m.to_f32())
        }) {
            return Err(ContractError::new(
                ContractErrorCode::InvalidProjection,
                "camera.colorPipeline.environment",
            ));
        }
        if let Some(ibl) = &self.ibl {
            let r = &ibl.environment.0;
            if r.asset_type != "environment"
                || r.id.trim().is_empty()
                || r.guid.as_ref().is_none_or(|g| g.trim().is_empty())
                || r.sub_asset.is_some()
                || !ibl.intensity.to_f32().is_finite()
                || !(0.0..=100.0).contains(&ibl.intensity.to_f32())
            {
                return Err(ContractError::new(
                    ContractErrorCode::InvalidBinding,
                    "camera.colorPipeline.ibl",
                ));
            }
        }
        let ev = self.exposure_ev.to_f32();
        if self.schema_version != "view-color3d.v1"
            || !ev.is_finite()
            || !(-16.0..=16.0).contains(&ev)
            || self
                .ambient_diffuse
                .iter()
                .any(|v| !v.to_f32().is_finite() || !(0.0..=65504.0).contains(&v.to_f32()))
        {
            return Err(ContractError::new(
                ContractErrorCode::InvalidProjection,
                "camera.colorPipeline",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GradientSky3D {
    #[serde(with = "number3")]
    pub zenith: [OrderedF32; 3],
    #[serde(with = "number3")]
    pub horizon: [OrderedF32; 3],
    #[serde(with = "number3")]
    pub nadir: [OrderedF32; 3],
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DistanceFog3D {
    #[serde(with = "number3")]
    pub color: [OrderedF32; 3],
    #[serde(with = "number")]
    pub density_per_m: OrderedF32,
    #[serde(with = "number")]
    pub start_distance_m: OrderedF32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ColorResolveStage {
    ToneMap,
    Output,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColorResolve3D {
    pub source: String,
    pub target: String,
    pub extent: [u32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewport: Option<[u32; 4]>,
    pub pipeline: ColorPipeline3D,
    pub stage: ColorResolveStage,
}

impl ColorResolve3D {
    pub fn region(&self) -> [u32; 4] {
        self.viewport
            .unwrap_or([0, 0, self.extent[0], self.extent[1]])
    }
}
fn valid_region([x, y, w, h]: [u32; 4], [width, height]: [u32; 2]) -> bool {
    w > 0
        && h > 0
        && x.checked_add(w).is_some_and(|v| v <= width)
        && y.checked_add(h).is_some_and(|v| v <= height)
}
fn overlaps(a: [u32; 4], b: [u32; 4]) -> bool {
    // Callers validate bounds first, so these additions cannot overflow.
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}
/// Shared outputs currently compose disjoint regions only. Keep this check at
/// both graph production and raw-plan admission, before mutable GPU work.
pub(crate) fn validate_output_regions<'a>(
    views: impl IntoIterator<Item = (&'a str, &'a crate::render_contract::ExecutableRenderView)>,
) -> Result<(), String> {
    let mut seen: Vec<(&str, &crate::render_contract::ExecutableRenderView)> = Vec::new();
    for (target, view) in views {
        if view.is_shadow_map() {
            continue;
        }
        if !valid_region(view.viewport, view.target_extent) {
            return Err("view.invalid_output_region".into());
        }
        for (old_target, old) in &seen {
            if *old_target == target {
                if old.target_extent != view.target_extent {
                    return Err("view.shared_extent_mismatch".into());
                }
                if overlaps(old.viewport, view.viewport) {
                    return Err("view.overlapping_output_unsupported".into());
                }
            }
        }
        seen.push((target, view));
    }
    Ok(())
}

use crate::render_graph::*;
use std::sync::Arc;
pub(crate) fn append_to_graph(graph: &mut RenderGraph) {
    let cameras: Vec<_> = graph
        .passes
        .iter()
        .flat_map(|p| &p.commands)
        .filter_map(|c| match c {
            RenderPassCommand::BeginView { target, view, .. } if view.color_pipeline.is_some() => {
                Some((target.clone(), view.clone()))
            }
            _ => None,
        })
        .collect();
    if !cameras.is_empty() {
        let result =
            validate_output_regions(graph.passes.iter().flat_map(|p| &p.commands).filter_map(
                |c| match c {
                    RenderPassCommand::BeginView { target, view, .. } => {
                        Some((target.as_str(), view.as_ref()))
                    }
                    _ => None,
                },
            ));
        if let Err(e) = result {
            graph.diagnostics.push(RenderGraphDiagnostic::error(
                "color.view_composition_unsupported",
                e,
            ));
            return;
        }
    }
    for (destination, view) in cameras {
        let hdr = format!("color3d-{}-hdr", view.view_id);
        let display = format!("color3d-{}-display", view.view_id);
        for (id, format) in [
            (&hdr, RenderResourceFormat::Rgba16Float),
            (&display, RenderResourceFormat::Rgba8Unorm),
        ] {
            if graph.resources.iter().any(|r| r.resource_id == *id) {
                graph
                    .diagnostics
                    .push(RenderGraphDiagnostic::error("color.resource_conflict", id));
                return;
            }
            graph.resources.push(RenderResource {
                resource_id: id.clone(),
                resource_name: id.clone(),
                resource_kind: RenderResourceKind::Texture,
                format,
                size: RenderResourceSize {
                    width: view.target_extent[0],
                    height: view.target_extent[1],
                },
                usage: vec![
                    RenderResourceUsage::ColorTarget,
                    RenderResourceUsage::Sampled,
                ],
                lifetime: RenderResourceLifetime::FrameLocal,
            });
        }
        let shared_hdr: Arc<str> = hdr.as_str().into();
        let mut last = None;
        for (index, pass) in graph.passes.iter_mut().enumerate() {
            let mut changed = false;
            for command in &mut pass.commands {
                match command {
                    RenderPassCommand::BeginView {
                        target,
                        view: camera,
                        ..
                    } if camera.view_id == view.view_id => {
                        *target = hdr.clone();
                        changed = true;
                    }
                    RenderPassCommand::DrawGeometry { draw }
                        if draw.view.view_id == view.view_id =>
                    {
                        if draw.target.as_ref() != hdr {
                            Arc::make_mut(draw).target = shared_hdr.clone();
                        }
                        changed = true;
                    }
                    RenderPassCommand::DrawGeometryBatch { batch }
                        if batch.draw.view.view_id == view.view_id =>
                    {
                        if batch.draw.target.as_ref() != hdr {
                            Arc::make_mut(&mut Arc::make_mut(batch).draw).target =
                                shared_hdr.clone();
                        }
                        changed = true;
                    }
                    RenderPassCommand::DrawParticles {
                        target,
                        view: camera,
                        ..
                    } if camera.scene_view == Some(view.view_id) => {
                        *target = hdr.clone();
                        changed = true;
                    }
                    _ => {}
                }
            }
            if changed {
                pass.writes
                    .iter_mut()
                    .chain(&mut pass.color_targets)
                    .filter(|id| **id == destination)
                    .for_each(|id| *id = hdr.clone());
                last = Some(index);
            }
        }
        let effects = view.color_pipeline.clone().unwrap();
        let phases = [
            (ColorResolveStage::ToneMap, hdr, display.clone()),
            (ColorResolveStage::Output, display, destination),
        ];
        let passes = phases
            .into_iter()
            .map(|(stage, source, target)| RenderPass {
                pass_id: format!("color3d-{}-{stage:?}", view.view_id),
                pass_name: format!("Color3D {stage:?}"),
                pass_kind: RenderPassKind::ResolveColor3d,
                view_id: format!("geometry-view-{}", view.view_id),
                reads: vec![source.clone()],
                writes: vec![target.clone()],
                color_targets: vec![target.clone()],
                depth_target: None,
                commands: vec![RenderPassCommand::ResolveColor3d {
                    resolve: ColorResolve3D {
                        source,
                        target,
                        extent: view.target_extent,
                        viewport: (view.viewport
                            != [0, 0, view.target_extent[0], view.target_extent[1]])
                        .then_some(view.viewport),
                        pipeline: effects.clone(),
                        stage,
                    },
                }]
                .into(),
                debug_source: Some("RenderProjection.ColorPipeline3D".into()),
            });
        let at = last.unwrap() + 1;
        graph.passes.splice(at..at, passes);
    }
}
fn check(
    resources: &[RenderResource],
    resolves: Vec<&ColorResolve3D>,
    views: Vec<(&str, &crate::render_contract::ExecutableRenderView)>,
) -> Result<(), String> {
    if resolves.len() > 32 {
        return Err("color.resolve_capacity".into());
    }
    let resource = |id: &str| {
        let mut found = resources.iter().filter(|r| r.resource_id == id);
        let r = found.next().ok_or("color.resource_missing")?;
        if found.next().is_some() {
            return Err("color.resource_duplicate");
        }
        Ok(r)
    };
    // Resolve source is unique; destination may be shared by disjoint Output regions.
    let mut sources = std::collections::BTreeSet::new();
    let mut written: std::collections::BTreeMap<&str, Vec<[u32; 4]>> = Default::default();
    if !resolves.is_empty() {
        validate_output_regions(views.iter().map(|(target, view)| {
            let final_target = resolves
                .iter()
                .find(|r| r.source == *target)
                .and_then(|tone| resolves.iter().find(|r| r.source == tone.target))
                .map_or(*target, |r| r.target.as_str());
            (final_target, *view)
        }))?;
    }
    for resolve in &resolves {
        resolve.pipeline.validate().map_err(|e| e.to_string())?;
        let src = resource(&resolve.source)?;
        let dst = resource(&resolve.target)?;
        if resolve.source == resolve.target
            || resolve.extent.contains(&0)
            || !sources.insert(&resolve.source)
            || !valid_region(resolve.region(), resolve.extent)
            || src.resource_kind != RenderResourceKind::Texture
            || !src.usage.contains(&RenderResourceUsage::Sampled)
            || !dst.usage.contains(&RenderResourceUsage::ColorTarget)
            || [src.size.width, src.size.height] != resolve.extent
            || src.size != dst.size
        {
            return Err("color.resolve_attachment".into());
        }
        let regions = written.entry(&resolve.target).or_default();
        if regions.iter().any(|r| overlaps(*r, resolve.region())) {
            return Err("color.resolve_region_overlap".into());
        }
        regions.push(resolve.region());
        match resolve.stage {
            ColorResolveStage::ToneMap => {
                if !resolves.iter().any(|r| {
                    r.stage == ColorResolveStage::Output
                        && r.source == resolve.target
                        && r.pipeline == resolve.pipeline
                        && r.extent == resolve.extent
                        && r.region() == resolve.region()
                }) {
                    return Err("color.output_missing".into());
                }
                if views
                    .iter()
                    .filter(|(target, _)| *target == resolve.source)
                    .count()
                    != 1
                {
                    return Err("color.view_composition_unsupported".into());
                }
                if src.format != RenderResourceFormat::Rgba16Float
                    || dst.format != RenderResourceFormat::Rgba8Unorm
                    || dst.resource_kind != RenderResourceKind::Texture
                    || !dst.usage.contains(&RenderResourceUsage::Sampled)
                    || !views.iter().any(|(target, view)| {
                        *target == resolve.source
                            && view.color_pipeline.as_ref() == Some(&resolve.pipeline)
                            && view.viewport == resolve.region()
                            && view.target_extent == resolve.extent
                    })
                {
                    return Err("color.tone_source".into());
                }
            }
            ColorResolveStage::Output => {
                if src.format != RenderResourceFormat::Rgba8Unorm
                    || !matches!(
                        dst.format,
                        RenderResourceFormat::Rgba8Unorm
                            | RenderResourceFormat::Bgra8Unorm
                            | RenderResourceFormat::Rgba8UnormSrgb
                            | RenderResourceFormat::Bgra8UnormSrgb
                    )
                    || !resolves.iter().any(|r| {
                        r.stage == ColorResolveStage::ToneMap
                            && r.target == resolve.source
                            && r.pipeline == resolve.pipeline
                            && r.extent == resolve.extent
                            && r.region() == resolve.region()
                    })
                {
                    return Err("color.output_source".into());
                }
            }
        }
    }
    for (target, view) in views {
        if view.color_pipeline.is_some()
            && !resolves
                .iter()
                .any(|r| r.stage == ColorResolveStage::ToneMap && r.source == target)
        {
            return Err("color.resolve_missing".into());
        }
    }
    Ok(())
}
pub(crate) fn validate_graph(graph: &RenderGraph) -> Vec<RenderGraphDiagnostic> {
    let mut resolves = Vec::new();
    let mut views = Vec::new();
    for pass in &graph.passes {
        for c in &pass.commands {
            match c {
                RenderPassCommand::ResolveColor3d { resolve } => {
                    if pass.pass_kind != RenderPassKind::ResolveColor3d
                        || pass.reads != [resolve.source.clone()]
                        || pass.color_targets != [resolve.target.clone()]
                        || pass.writes != [resolve.target.clone()]
                        || pass.depth_target.is_some()
                    {
                        return vec![RenderGraphDiagnostic::error(
                            "color.resolve_access",
                            "explicit source and destination required",
                        )];
                    }
                    resolves.push(resolve);
                }
                RenderPassCommand::BeginView { target, view, .. } => {
                    views.push((target.as_str(), view.as_ref()))
                }
                _ => {}
            }
        }
    }
    check(&graph.resources, resolves, views)
        .err()
        .map(|e| vec![RenderGraphDiagnostic::error("color.invalid_graph", e)])
        .unwrap_or_default()
}
#[doc(hidden)]
pub fn validate_plan(plan: &crate::rhi_command_plan::RhiCommandPlan) -> Result<(), String> {
    use crate::rhi_command_plan::RhiCommand;
    let resolves: Vec<_> = plan
        .commands
        .iter()
        .filter_map(|c| match c {
            RhiCommand::ResolveColor3d { resolve } => Some(resolve),
            _ => None,
        })
        .collect();
    let views = plan
        .commands
        .iter()
        .filter_map(|c| match c {
            RhiCommand::BeginView { target, view, .. } => Some((target.as_str(), view.as_ref())),
            _ => None,
        })
        .collect();
    if resolves.is_empty() {
        return check(&[], resolves, views);
    }
    let schedule = plan
        .graph_schedule
        .as_ref()
        .ok_or("color.schedule_missing")?;
    check(&schedule.resources, resolves, views)?;
    // A forged raw plan must not sample an unwritten/stale intermediate.
    let mut initialized = std::collections::BTreeSet::new();
    let mut sealed = std::collections::BTreeSet::new();
    for c in &plan.commands {
        match c {
            RhiCommand::BeginView { target, view, .. } if view.color_pipeline.is_some() => {
                initialized.insert(target.as_str());
            }
            RhiCommand::ResolveColor3d { resolve } => {
                if !initialized.contains(resolve.source.as_str()) {
                    return Err("color.source_not_initialized".into());
                }
                sealed.insert(resolve.source.as_str());
                initialized.insert(resolve.target.as_str());
            }
            c if c.geometry_draw().is_some_and(|draw| {
                draw.view.color_pipeline.is_some() && sealed.contains(draw.target.as_ref())
            }) =>
            {
                return Err("color.writer_after_resolve".into());
            }
            _ => {}
        }
    }
    Ok(())
}

// Authoring values are JSON numbers; OrderedF32's internal bit encoding must not leak into scene assets.
mod number {
    use super::*;
    pub fn serialize<S: serde::Serializer>(v: &OrderedF32, s: S) -> Result<S::Ok, S::Error> {
        v.to_f32().serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<OrderedF32, D::Error> {
        f32::deserialize(d).map(OrderedF32::from_f32)
    }
}
mod number3 {
    use super::*;
    pub fn serialize<S: serde::Serializer>(v: &[OrderedF32; 3], s: S) -> Result<S::Ok, S::Error> {
        v.map(OrderedF32::to_f32).serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> Result<[OrderedF32; 3], D::Error> {
        <[f32; 3]>::deserialize(d).map(|v| v.map(OrderedF32::from_f32))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageBasedLighting3D {
    pub environment: crate::spatial_contract::SpatialAssetRef,
    #[serde(with = "number")]
    pub intensity: OrderedF32,
}

#[cfg(test)]
mod s2d_memory_tests {
    use super::*;
    use crate::resource_memory::{memory_totals, ResourceMemorySample};

    #[test]
    fn color_config_backing_counts_asset_ref_capacity_once_and_unions_roles() {
        let mut config = ColorPipeline3D {
            ibl: Some(
                serde_json::from_value(serde_json::json!({
                    "environment": {"id":"studio", "type":"environment", "guid":"guid-studio"},
                    "intensity": 1.
                }))
                .unwrap(),
            ),
            ..Default::default()
        };
        config.schema_version.reserve(64);
        let reference = &mut config.ibl.as_mut().unwrap().environment.0;
        reference.id.reserve(128);
        reference.guid.as_mut().unwrap().reserve(96);
        config.validate().unwrap();
        let reference = &config.ibl.as_ref().unwrap().environment.0;
        let expected = config.schema_version.capacity()
            + reference.id.capacity()
            + reference.asset_type.capacity()
            + reference.guid.as_ref().unwrap().capacity();
        let mut sample = ResourceMemorySample::new(0, 0, 0);
        config.observe_memory(&mut sample, "renderer", "resident");
        config.observe_memory(&mut sample, "renderer", "retiring");
        let total = memory_totals(&sample.allocations).unwrap();
        assert_eq!(total.cpu_allocated, expected as u64);
        assert_eq!(total.unique_allocations, 4);
        assert_eq!(total.by_role["cpu.resident"], expected as u64);
        assert_eq!(total.by_role["cpu.retiring"], expected as u64);
        let detached = config.clone();
        let r = &detached.ibl.as_ref().unwrap().environment.0;
        let detached_bytes = detached.schema_version.capacity()
            + r.id.capacity()
            + r.asset_type.capacity()
            + r.guid.as_ref().unwrap().capacity();
        detached.observe_memory(&mut sample, "renderer", "resident");
        assert_eq!(
            memory_totals(&sample.allocations).unwrap().cpu_allocated,
            (expected + detached_bytes) as u64,
            "a copied String is a separate backing allocation"
        );
    }
}
