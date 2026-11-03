//! Directional cascade and spot views use the same retained instance/culling owner as the camera.
use super::super::*;
use crate::render_projection_adapter::LightProjection;
use crate::spatial_contract::LightKind;
const EXTENT: [u32; 2] = [4096, 4096];
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn caster_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    #[cfg(test)]
    CASTER_PROJECTION_DOTS.with(|n| n.set(n.get() + 1));
    dot(a, b)
}
#[cfg(test)]
thread_local! {static CASTER_PROJECTION_DOTS:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normalize(v: [f32; 3]) -> [f32; 3] {
    let n = dot(v, v).sqrt();
    v.map(|v| v / n)
}
fn basis(direction: [f32; 3]) -> [[f32; 3]; 3] {
    let back = direction.map(|v| -v);
    let up = if back[1].abs() < 0.99 {
        [0., 1., 0.]
    } else {
        [1., 0., 0.]
    };
    let right = normalize(cross(up, back));
    [right, cross(back, right), back]
}
fn rotation(b: [[f32; 3]; 3]) -> [f32; 4] {
    let m = |r: usize, c: usize| b[c][r];
    let trace = m(0, 0) + m(1, 1) + m(2, 2);
    let mut q = [0.; 4];
    if trace > 0. {
        let s = (trace + 1.).sqrt() * 2.;
        q = [
            (m(2, 1) - m(1, 2)) / s,
            (m(0, 2) - m(2, 0)) / s,
            (m(1, 0) - m(0, 1)) / s,
            s * 0.25,
        ];
    } else {
        let i = if m(0, 0) > m(1, 1) && m(0, 0) > m(2, 2) {
            0
        } else if m(1, 1) > m(2, 2) {
            1
        } else {
            2
        };
        let j = (i + 1) % 3;
        let k = (i + 2) % 3;
        let s = (1. + m(i, i) - m(j, j) - m(k, k)).sqrt() * 2.;
        q[i] = s * 0.25;
        q[j] = (m(j, i) + m(i, j)) / s;
        q[k] = (m(k, i) + m(i, k)) / s;
        q[3] = (m(k, j) - m(j, k)) / s;
    }
    let length = q.iter().map(|v| v * v).sum::<f32>().sqrt();
    q.map(|v| v / length)
}
fn frustum_corners(
    view: &RenderView,
    pose: CameraPose,
    near: f32,
    far: f32,
) -> Result<[[f32; 3]; 8], ContractError> {
    let executable = view.evaluate(pose)?;
    let v = executable.view_matrix.map(OrderedF32::to_f32);
    let aspect = view.viewport[2] as f32 / view.viewport[3] as f32;
    Ok(std::array::from_fn(|i| {
        let z = if i & 4 == 0 { near } else { far };
        let h = match view.projection {
            Projection::Perspective { fov_y_degrees, .. } => {
                z * (fov_y_degrees.to_radians() * 0.5).tan()
            }
            Projection::Orthographic { size, .. } => size * 0.5,
            _ => unreachable!(),
        };
        let p = [
            if i & 1 == 0 { -h * aspect } else { h * aspect },
            if i & 2 == 0 { -h } else { h },
            -z,
        ];
        std::array::from_fn(|r| {
            pose.position[r] + v[r * 4] * p[0] + v[r * 4 + 1] * p[1] + v[r * 4 + 2] * p[2]
        })
    }))
}
fn world_corners(instance: &build_inputs::GeometryInstanceRef<'_>) -> [[f32; 3]; 8] {
    let bounds = &instance.source.bounds;
    std::array::from_fn(|i| {
        let p = std::array::from_fn::<_, 3, _>(|axis| {
            if i & (1 << axis) == 0 {
                bounds.min[axis]
            } else {
                bounds.max[axis]
            }
        });
        let p = workset::world_corner(instance.world_transform, [p[0], p[1], p[2], 1.]);
        [p[0], p[1], p[2]]
    })
}
// Same-frame per-light workspace, shared by this light's four cascades.
// No scene/visibility result or native resource is retained across frames.
struct LightSpaceCaster {
    xy: [f32; 4],
    z: [f32; 8],
}
fn project_casters(casters: &[[[f32; 3]; 8]], direction: [f32; 3]) -> Vec<LightSpaceCaster> {
    let b = basis(direction);
    casters
        .iter()
        .map(|caster| {
            let mut xy = [
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
            ];
            let z = std::array::from_fn(|i| {
                let x = caster_dot(b[0], caster[i]);
                let y = caster_dot(b[1], caster[i]);
                xy[0] = xy[0].min(x);
                xy[1] = xy[1].max(x);
                xy[2] = xy[2].min(y);
                xy[3] = xy[3].max(y);
                caster_dot(b[2], caster[i])
            });
            LightSpaceCaster { xy, z }
        })
        .collect()
}

fn directional_pose(
    corners: [[f32; 3]; 8],
    direction: [f32; 3],
    casters: &[LightSpaceCaster],
    origin: [f64; 3],
) -> (CameraPose, f32, f32) {
    let b = basis(direction);
    let center: [f32; 3] =
        std::array::from_fn(|axis| corners.iter().map(|p| p[axis]).sum::<f32>() / 8.);
    let radius = corners
        .iter()
        .map(|p| {
            dot(
                std::array::from_fn(|i| p[i] - center[i]),
                std::array::from_fn(|i| p[i] - center[i]),
            )
            .sqrt()
        })
        .fold(0f32, f32::max);
    // Quantized radius plus a texel guard keeps snapped receiver corners inside the tile.
    let radius = ((radius * 16.).ceil() / 16.).max(0.0625) * 1.002;
    let step = 2. * radius / 2048.;
    // Snap to the absolute light-space grid, then return to this view's origin.
    // Snapping only relative coordinates shifts a stationary shadow at a rebase.
    let snap = |axis: [f32; 3]| {
        let offset: f64 = axis
            .into_iter()
            .zip(origin)
            .map(|(a, o)| f64::from(a) * o)
            .sum();
        let local: f64 = axis
            .into_iter()
            .zip(center)
            .map(|(a, c)| f64::from(a) * f64::from(c))
            .sum();
        (((offset + local) / f64::from(step)).round() * f64::from(step) - offset) as f32
    };
    let x = snap(b[0]);
    let y = snap(b[1]);
    let mut zmin = corners
        .iter()
        .map(|p| dot(b[2], *p))
        .fold(f32::INFINITY, f32::min);
    let mut zmax = corners
        .iter()
        .map(|p| dot(b[2], *p))
        .fold(f32::NEG_INFINITY, f32::max);
    for caster in casters {
        let [xmin, xmax, ymin, ymax] = caster.xy;
        if xmax < x - radius || xmin > x + radius || ymax < y - radius || ymin > y + radius {
            continue;
        }
        // Preserve the original corner order, including signed zero/NaN min
        // behavior. Pre-reducing Z would introduce another arithmetic tree.
        for &z in &caster.z {
            zmin = zmin.min(z);
            zmax = zmax.max(z);
        }
    }
    let position = std::array::from_fn(|i| b[0][i] * x + b[1][i] * y + b[2][i] * (zmax + 0.1));
    (
        CameraPose {
            position,
            rotation: rotation(b),
        },
        radius * 2.,
        (zmax - zmin + 0.2).max(0.2),
    )
}

#[cfg(test)]
thread_local! {
    static RECEIVER_MAGNITUDE_ROWS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static RECEIVER_VIEW_COMPARISONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static RECEIVER_SOURCE_ROWS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn receiver_view_matches(a: &ExecutableRenderView, b: &ExecutableRenderView) -> bool {
    #[cfg(test)]
    RECEIVER_VIEW_COMPARISONS.with(|n| n.set(n.get() + 1));
    a == b
}

// Complete for the current production sampler set: only StandardLit geometry
// calls shadow_visibility; mesh decal/particles/sky/fog do not sample these maps.
// New sampling consumers must extend this proof or return unknown (None).
#[cfg(test)]
fn receiver_depth_ranges(
    frame: &GeometryFrame,
    view_id: u64,
    instances: &[GeometryInstance<'_>],
) -> Option<Vec<[f32; 2]>> {
    receiver_depth_ranges_inputs(
        frame,
        view_id,
        build_inputs::InstanceInputs::Owned(instances),
        None,
    )
}

#[cfg(test)]
fn receiver_depth_ranges_inputs(
    frame: &GeometryFrame,
    view_id: u64,
    instances: build_inputs::InstanceInputs<'_, '_>,
    prepared_indices: Option<&std::collections::HashMap<u64, u32>>,
) -> Option<Vec<[f32; 2]>> {
    receiver_depth_ranges_prepared(frame, view_id, instances, prepared_indices, None)
}

#[cfg(test)]
fn receiver_depth_ranges_prepared(
    frame: &GeometryFrame,
    view_id: u64,
    instances: build_inputs::InstanceInputs<'_, '_>,
    prepared_indices: Option<&std::collections::HashMap<u64, u32>>,
    prepared: Option<&build_inputs::PreparedInputs>,
) -> Option<Vec<[f32; 2]>> {
    let main = frame.views.iter().find(|v| v.view.view_id == view_id)?;
    if instances.len() > u32::MAX as usize {
        return None;
    }
    // The build group already owns this exact immutable instance order and
    // ID index. Borrow it in production instead of materializing another full
    // set of source descriptors for each moving view. Legacy callers retain
    // their call-local complete index and duplicate-ID check.
    let fallback;
    let sources = if let Some(indices) = prepared_indices {
        indices
    } else {
        fallback = instances
            .iter()
            .enumerate()
            .map(|(index, instance)| {
                #[cfg(test)]
                RECEIVER_SOURCE_ROWS.with(|n| n.set(n.get() + 1));
                (instance.instance_id, index as u32)
            })
            .collect::<std::collections::HashMap<_, _>>();
        &fallback
    };
    if sources.len() != instances.len() {
        return None;
    }
    let mut ranges = Vec::new();
    let camera_matrix = main.view.view_matrix.map(OrderedF32::to_f32);
    let receiver_view = workset::ReceiverView::new(camera_matrix);
    // Borrowed views cannot change during this proof. Reuse only a successful
    // complete comparison for the same allocation; a detached/new view still
    // compares all fields, even when it has the same public view_id.
    let mut checked_view = None;
    let mut consume = |draw: &DrawGeometry, selected: &Arc<DrawInstance>, direct: Option<u32>| {
        if selected.standard_lit.is_none() {
            return Some(());
        }
        let index = match direct {
            Some(index) => index,
            None => *sources.get(&selected.instance_id)?,
        };
        let instance = instances.get(index as usize)?;
        if instance.instance_id != selected.instance_id {
            return None;
        }
        if !checked_view.is_some_and(|old| std::ptr::eq(old, draw.view.as_ref())) {
            if !receiver_view_matches(&draw.view, &main.view) {
                return None;
            }
            checked_view = Some(draw.view.as_ref());
        }
        if !Arc::ptr_eq(&selected.geometry, &instance.source.buffers)
            || !Arc::ptr_eq(
                &selected.geometry_binding,
                &instance.source.geometry_binding,
            )
        {
            return None;
        }
        ranges.push(
            match prepared.and_then(|p| p.receiver_bounds(index as usize, selected)) {
                Some(bounds) => bounds.depth_range_in(&receiver_view)?,
                None => receiver_depth_range(
                    camera_matrix,
                    selected.world_transform.map(OrderedF32::to_f32),
                    &instance.source.bounds,
                )?,
            },
        );
        Some(())
    };
    if let Some(batches) = &main.batches {
        for batch in batches {
            let direct =
                prepared.is_some_and(|p| p.owns_receiver_indices(&batch.instances, sources));
            for entry in &batch.visible {
                let selected = batch.instances.get(entry.instance_index as usize)?;
                consume(
                    &batch.draw,
                    selected,
                    direct.then_some(entry.instance_index),
                )?;
            }
        }
    } else {
        for draw in &main.draws {
            consume(draw, &draw.instance, None)?;
        }
    }
    Some(ranges)
}

#[cfg(test)]
fn receiver_depth_range(
    camera_matrix: [f32; 16],
    world_matrix: [f32; 16],
    bounds: &GeometryBounds,
) -> Option<[f32; 2]> {
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    // Positive row bounds dominate every corner's absolute products.
    // Use the same f32 sum order as the old per-corner guard. Overflow is
    // unknown, so it can only retain more casters, never prove an empty range.
    fn absolute_rows(matrix: [f32; 16], input: [f32; 4]) -> Option<[f32; 4]> {
        let rows = std::array::from_fn(|row| {
            #[cfg(test)]
            RECEIVER_MAGNITUDE_ROWS.with(|n| n.set(n.get() + 1));
            (0..4)
                .map(|col| (matrix[col * 4 + row] * input[col]).abs())
                .sum::<f32>()
        });
        rows.iter().all(|v| v.is_finite()).then_some(rows)
    }
    let local_max = [
        bounds.min[0].abs().max(bounds.max[0].abs()),
        bounds.min[1].abs().max(bounds.max[1].abs()),
        bounds.min[2].abs().max(bounds.max[2].abs()),
        1.,
    ];
    let world_max = absolute_rows(world_matrix, local_max)?;
    let camera_max = absolute_rows(camera_matrix, world_max)?;
    let mut magnitude = world_max.into_iter().chain(camera_max).fold(1f32, f32::max);
    for index in 0..8 {
        let local = std::array::from_fn::<_, 3, _>(|axis| {
            if index & (1 << axis) == 0 {
                bounds.min[axis]
            } else {
                bounds.max[axis]
            }
        });
        let local = [local[0], local[1], local[2], 1.];
        let world = workset::world_corner(world_matrix, local);
        let camera = workset::camera_corner(camera_matrix, world);
        if world.iter().chain(&camera).any(|v| !v.is_finite()) {
            return None;
        }
        lo = lo.min(-camera[2]);
        hi = hi.max(-camera[2]);
        magnitude = world
            .iter()
            .chain(&camera)
            .fold(magnitude, |m, v| m.max(v.abs()));
    }
    // Bound f32 matrix evaluation/FMA differences outwards. Cascade selection
    // uses unoffset world p; normal bias and clamped PCF cannot cross its range.
    let guard = 0.001 + 64. * f32::EPSILON * magnitude;
    if !(lo - guard).is_finite() || !(hi + guard).is_finite() {
        return None;
    }
    Some([lo - guard, hi + guard])
}

#[cfg(test)]
fn receiver_depth_range_corner_reference(
    camera_matrix: [f32; 16],
    world_matrix: [f32; 16],
    bounds: &GeometryBounds,
) -> Option<[f32; 2]> {
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    let mut magnitude = 1f32;
    for index in 0..8 {
        let local = std::array::from_fn::<_, 3, _>(|axis| {
            if index & (1 << axis) == 0 {
                bounds.min[axis]
            } else {
                bounds.max[axis]
            }
        });
        let local = [local[0], local[1], local[2], 1.];
        let world = transform_point(world_matrix, local);
        let camera = transform_point(camera_matrix, world);
        if world.iter().chain(&camera).any(|v| !v.is_finite()) {
            return None;
        }
        lo = lo.min(-camera[2]);
        hi = hi.max(-camera[2]);
        magnitude = world
            .iter()
            .chain(&camera)
            .fold(magnitude, |m, v| m.max(v.abs()));
        // Include absolute products, not just their possibly cancelling sum.
        // Very large/sheared transforms conservatively widen or abandon proof.
        for row in 0..4 {
            for (matrix, point) in [(world_matrix, local), (camera_matrix, world)] {
                let terms = (0..4)
                    .map(|col| (matrix[col * 4 + row] * point[col]).abs())
                    .sum::<f32>();
                if !terms.is_finite() {
                    return None;
                }
                magnitude = magnitude.max(terms);
            }
        }
    }
    // Bound f32 matrix evaluation/FMA differences outwards. Cascade selection
    // uses unoffset world p; normal bias and clamped PCF cannot cross its range.
    let guard = 0.001 + 64. * f32::EPSILON * magnitude;
    if !(lo - guard).is_finite() || !(hi + guard).is_finite() {
        return None;
    }
    Some([lo - guard, hi + guard])
}

fn cascade_has_receiver(ranges: Option<&[[f32; 2]]>, lo: f32, hi: f32) -> bool {
    ranges.is_none_or(|ranges| ranges.iter().any(|range| range[1] >= lo && range[0] <= hi))
}

type CasterCorners = Vec<[[f32; 3]; 8]>;
#[cfg(test)]
struct ShadowWorkset {
    receivers: Option<Vec<[f32; 2]>>,
    casters: CasterCorners,
}

fn collect_casters(
    instances: build_inputs::InstanceInputs<'_, '_>,
    prepared: Option<&build_inputs::PreparedInputs>,
    view: &RenderView,
) -> Result<CasterCorners, ContractError> {
    if let Some(prepared) = prepared {
        return prepared.shadow_caster_corners(instances, view);
    }
    let mut bounds = Vec::new();
    for (index, instance) in instances.iter().enumerate() {
        if instance.visibility.authored
            && instance.visibility.inherited == Evaluation::Known(true)
            && !instance.source.material.blend.is_transparent()
            && instance.layers.matches(&view.layers)?
        {
            bounds.push(
                prepared
                    .and_then(|p| p.shadow_corners(index))
                    .unwrap_or_else(|| world_corners(&instance)),
            );
        }
    }
    Ok(bounds)
}

// Independent read-only prerequisites. No selection, registry, GPU state or
// mutable BuildInputs crosses the scope. The same bounded executor owns both
// these jobs and the later shadow views; the groups never overlap/nest.
#[cfg(test)]
fn shadow_workset(
    frame: &GeometryFrame,
    view: &RenderView,
    instances: build_inputs::InstanceInputs<'_, '_>,
    indices: Option<&std::collections::HashMap<u64, u32>>,
    prepared: Option<&build_inputs::PreparedInputs>,
    workers: usize,
) -> Result<ShadowWorkset, ContractError> {
    enum Part {
        Receivers(Option<Vec<[f32; 2]>>),
        Casters(CasterCorners),
    }
    let parts = crate::render_geometry::prepare_tasks::run_bounded(2, workers, &|job| match job {
        0 => Ok(Part::Receivers(receiver_depth_ranges_prepared(
            frame,
            view.view_id,
            instances,
            indices,
            prepared,
        ))),
        _ => collect_casters(instances, prepared, view).map(Part::Casters),
    })?;
    #[cfg(test)]
    SHADOW_WORKSET_THREADS.with(|threads| *threads.borrow_mut() = parts.threads);
    let mut values = parts.values.into_iter();
    let Some(Part::Receivers(receivers)) = values.next() else {
        unreachable!()
    };
    let Some(Part::Casters(casters)) = values.next() else {
        unreachable!()
    };
    Ok(ShadowWorkset { receivers, casters })
}

#[cfg(test)]
thread_local! { static SHADOW_WORKSET_THREADS: std::cell::RefCell<Vec<std::thread::ThreadId>> = const { std::cell::RefCell::new(Vec::new()) }; }

// Pre-migration scalar production composition remains only as a test oracle.
#[cfg(test)]
pub(crate) fn append_scene_lighting(
    frame: &mut GeometryFrame,
    input: GeometryViewInput<'_>,
    origin: [f64; 3],
    inputs: &mut crate::render_geometry::build_inputs::GeometryBuildInputs<'_, '_>,
    lights: &[LightProjection],
) -> Result<(), ContractError> {
    let GeometryViewInput { view, pose } = input;
    // One producer owns the frozen list consumed by both color and shadow
    // validation. Rebuilding its cluster grid for shadows duplicated the exact
    // same (view, light) work after production had already installed the list.
    let main = frame
        .views
        .iter_mut()
        .find(|v| v.view.view_id == view.view_id)
        .ok_or_else(|| invalid("light.view_missing"))?;
    let list = ClusteredLights::build(&main.view, lights)?;
    for draw in &mut main.draws {
        if draw.standard_lit.is_some() {
            Arc::make_mut(draw).lighting = Some(list.clone());
        }
    }
    // Receiver selection below consumes geometry and view dependencies only.
    // Publish the final head once, after all attachments, including no shadows.
    let instances = inputs.instances();
    let selected: Vec<_> = lights
        .iter()
        .enumerate()
        .filter(|(_, l)| l.component.cast_shadows)
        .collect();
    if selected.is_empty() {
        let main = frame
            .views
            .iter_mut()
            .find(|v| v.view.view_id == view.view_id)
            .ok_or_else(|| invalid("light.view_missing"))?;
        inputs.refresh_commands(main);
        return Ok(());
    }
    let directional = selected
        .iter()
        .filter(|(_, l)| matches!(l.component.kind, LightKind::Directional { .. }))
        .count();
    let spots = selected
        .iter()
        .filter(|(_, l)| matches!(l.component.kind, LightKind::Spot { .. }))
        .count();
    if directional > 1 || spots > 8 {
        return Err(invalid("shadow.projection_capacity"));
    }
    let (near, far) = match view.projection {
        Projection::Perspective { near, far, .. } | Projection::Orthographic { near, far, .. } => {
            (near, far)
        }
        _ => return Err(invalid("shadow.camera_projection")),
    };
    let workers = if instances.len() >= 512 && inputs.prepared.is_some() {
        std::thread::available_parallelism()
            .map_or(1, usize::from)
            .min(2)
    } else {
        1
    };
    let ShadowWorkset {
        receivers,
        casters: caster_bounds,
    } = shadow_workset(
        frame,
        view,
        instances,
        inputs
            .compact_table
            .as_ref()
            .map(|_| inputs.compact_indices.as_ref()),
        inputs.prepared,
        workers,
    )?;
    let mut shadows = Vec::new();
    let mut projections = Vec::new();
    let mut spot_index = 0;
    let mut jobs = Vec::new();
    let mut metadata = Vec::new();
    let planning = (|| -> Result<(), ContractError> {
        for (light_index, light) in selected {
            light
                .component
                .validate()
                .map_err(|_| invalid("shadow.light"))?;
            let count = if matches!(light.component.kind, LightKind::Directional { .. }) {
                4
            } else {
                1
            };
            let mut projected_casters = None;
            for cascade in 0..count {
                let mut receiver_empty = false;
                let mut shadow = view.clone();
                // Shadow writers inherit the lens/culling context, never display color processing.
                shadow.color_pipeline = None;
                shadow.view_kind = ViewKind::Shadow;
                shadow.target = TargetKind::ShadowMap;
                shadow.depth_policy = DepthPolicy::TestAndWrite;
                let key =
                    serde_json::to_vec(&("shadow-view.v1", view.view_id, light_index, cascade))
                        .expect("shadow view identity");
                shadow.view_id =
                    u64::from_str_radix(&crate::canonical_digest::sha256_hex(&key)[..16], 16)
                        .expect("hex identity");
                let (slot, atlas, shadow_pose, range, first) = match light.component.kind {
                    LightKind::Directional { .. } => {
                        let split = |i: u32| {
                            let t = i as f32 / 4.;
                            if i == 0 {
                                near
                            } else if i == 4 {
                                far
                            } else if near > 0. {
                                0.5 * (near * (far / near).powf(t) + near + (far - near) * t)
                            } else {
                                near + (far - near) * t
                            }
                        };
                        let lo = split(cascade);
                        let hi = split(cascade + 1);
                        receiver_empty = !cascade_has_receiver(receivers.as_deref(), lo, hi);
                        let corners = frustum_corners(view, pose, lo, hi)?;
                        let (p, size, far) = directional_pose(
                            corners,
                            light.direction,
                            if receiver_empty {
                                &[]
                            } else {
                                projected_casters
                                    .get_or_insert_with(|| {
                                        project_casters(&caster_bounds, light.direction)
                                    })
                                    .as_slice()
                            },
                            origin,
                        );
                        shadow.projection = Projection::Orthographic {
                            size,
                            near: 0.05,
                            far,
                        };
                        shadow.viewport = [(cascade % 2) * 2048, (cascade / 2) * 2048, 2048, 2048];
                        (
                            0,
                            format!("{}:shadow:{}:directional", frame.target, view.view_id),
                            p,
                            [lo, hi],
                            cascade == 0,
                        )
                    }
                    LightKind::Spot {
                        range_m,
                        outer_half_angle_degrees,
                        ..
                    } => {
                        if range_m <= 0.05 {
                            return Err(invalid("shadow.spot_near"));
                        }
                        shadow.projection = Projection::Perspective {
                            fov_y_degrees: outer_half_angle_degrees * 2.,
                            near: 0.05,
                            far: range_m,
                        };
                        shadow.viewport =
                            [(spot_index % 4) * 1024, (spot_index / 4) * 1024, 1024, 1024];
                        let first = spot_index == 0;
                        spot_index += 1;
                        (
                            1,
                            format!("{}:shadow:{}:spot", frame.target, view.view_id),
                            CameraPose {
                                position: light.position,
                                rotation: rotation(basis(light.direction)),
                            },
                            [near, far],
                            first,
                        )
                    }
                    LightKind::Point { .. } => return Err(invalid("shadow.point_unsupported")),
                };
                shadow.clear_policy = if first {
                    ClearPolicy::Depth { depth: 1. }
                } else {
                    ClearPolicy::Load
                };
                jobs.push(crate::render_geometry::prepare_tasks::ViewJob {
                    target: atlas,
                    extent: EXTENT,
                    view: shadow,
                    pose: shadow_pose,
                    empty: receiver_empty,
                });
                metadata.push((slot, light_index as u32, range));
            }
        }
        Ok(())
    })();
    if let Err(error) = planning {
        // Preserve the original ordered first error: an earlier preparation
        // error outranks a later projection/planning error. Nothing publishes.
        crate::render_geometry::prepare_tasks::build_jobs(inputs, &jobs)?;
        return Err(error);
    }
    let built = crate::render_geometry::prepare_tasks::build_jobs(inputs, &jobs)?;
    for ((job, (slot, light_index, range)), built) in jobs.iter().zip(metadata).zip(built) {
        let v = &built.views[0].view;
        projections.push(ShadowProjection {
            atlas: job.target.clone(),
            slot,
            light_index,
            matrix: v.view_projection().map(Into::into),
            viewport: v.viewport,
            extent: job.extent,
            depth_range: range.map(Into::into),
        });
        shadows.extend(built.views);
    }
    let main = frame
        .views
        .iter_mut()
        .find(|v| v.view.view_id == view.view_id)
        .ok_or_else(|| invalid("shadow.receiver_missing"))?;
    let maps = ShadowMaps::build(&main.view, &list, projections)?;
    for draw in &mut main.draws {
        if draw.standard_lit.is_some() {
            Arc::make_mut(draw).shadows = Some(maps.clone());
        }
    }
    inputs.refresh_commands(main);
    frame.views.splice(0..0, shadows);
    Ok(())
}

pub(in crate::render_geometry) fn build_scene_lighting(
    pending: PendingGeometryView,
    target: &str,
    input: GeometryViewInput<'_>,
    origin: [f64; 3],
    inputs: &mut crate::render_geometry::build_inputs::GeometryBuildInputs<'_, '_>,
    lights: &[LightProjection],
    environment: Option<Arc<crate::environment_lighting::EnvironmentLighting>>,
) -> Result<GeometryFrame, ContractError> {
    let extent = pending.frame.view.target_extent;
    let GeometryViewInput { view, pose } = input;
    let list = ClusteredLights::build(&pending.frame.view, lights);
    let list = match list {
        Ok(list) => list,
        Err(error) => {
            pending
                .finish(inputs, &FinalViewState::default())
                .map_err(|e| e.error)?;
            return Err(error);
        }
    };
    let instances = inputs.instances();
    let selected: Vec<_> = lights
        .iter()
        .enumerate()
        .filter(|(_, l)| l.component.cast_shadows)
        .collect();
    if selected.is_empty() {
        let main = pending
            .finish(
                inputs,
                &FinalViewState {
                    environment,
                    lighting: Some(list),
                    shadows: None,
                },
            )
            .map_err(|e| e.error)?;
        return Ok(GeometryFrame {
            target: target.into(),
            extent,
            views: vec![main],
        });
    }
    let directional = selected
        .iter()
        .filter(|(_, l)| matches!(l.component.kind, LightKind::Directional { .. }))
        .count();
    let spots = selected
        .iter()
        .filter(|(_, l)| matches!(l.component.kind, LightKind::Spot { .. }))
        .count();
    if directional > 1 || spots > 8 {
        return Err(invalid("shadow.projection_capacity"));
    }
    let (near, far) = match view.projection {
        Projection::Perspective { near, far, .. } | Projection::Orthographic { near, far, .. } => {
            (near, far)
        }
        _ => return Err(invalid("shadow.camera_projection")),
    };
    // Receiver membership is the unpublished exact current selection, not
    // provisional per-batch heads. Unknown geometry retains every cascade.
    enum Part {
        Receivers(Option<Vec<[f32; 2]>>),
        Casters(CasterCorners),
    }
    let workers = if instances.len() >= 512 && inputs.prepared.is_some() {
        std::thread::available_parallelism()
            .map_or(1, usize::from)
            .min(2)
    } else {
        1
    };
    let prepared = inputs.prepared;
    let parts = crate::render_geometry::prepare_tasks::run_bounded(2, workers, &|job| match job {
        0 => Ok(Part::Receivers(pending.receiver_ranges(prepared))),
        _ => collect_casters(instances, prepared, view).map(Part::Casters),
    })?;
    let mut parts = parts.values.into_iter();
    let Some(Part::Receivers(receivers)) = parts.next() else {
        unreachable!()
    };
    let Some(Part::Casters(caster_bounds)) = parts.next() else {
        unreachable!()
    };
    let mut projections = Vec::new();
    let mut spot_index = 0;
    let mut jobs = Vec::new();
    let mut metadata = Vec::new();
    let planning = (|| -> Result<(), ContractError> {
        for (light_index, light) in selected {
            light
                .component
                .validate()
                .map_err(|_| invalid("shadow.light"))?;
            let count = if matches!(light.component.kind, LightKind::Directional { .. }) {
                4
            } else {
                1
            };
            let mut projected_casters = None;
            for cascade in 0..count {
                let mut receiver_empty = false;
                let mut shadow = view.clone();
                // Shadow writers inherit the lens/culling context, never display color processing.
                shadow.color_pipeline = None;
                shadow.view_kind = ViewKind::Shadow;
                shadow.target = TargetKind::ShadowMap;
                shadow.depth_policy = DepthPolicy::TestAndWrite;
                let key =
                    serde_json::to_vec(&("shadow-view.v1", view.view_id, light_index, cascade))
                        .expect("shadow view identity");
                shadow.view_id =
                    u64::from_str_radix(&crate::canonical_digest::sha256_hex(&key)[..16], 16)
                        .expect("hex identity");
                let (slot, atlas, shadow_pose, range, first) = match light.component.kind {
                    LightKind::Directional { .. } => {
                        let split = |i: u32| {
                            let t = i as f32 / 4.;
                            if i == 0 {
                                near
                            } else if i == 4 {
                                far
                            } else if near > 0. {
                                0.5 * (near * (far / near).powf(t) + near + (far - near) * t)
                            } else {
                                near + (far - near) * t
                            }
                        };
                        let lo = split(cascade);
                        let hi = split(cascade + 1);
                        receiver_empty = !cascade_has_receiver(receivers.as_deref(), lo, hi);
                        let corners = frustum_corners(view, pose, lo, hi)?;
                        let (p, size, far) = directional_pose(
                            corners,
                            light.direction,
                            if receiver_empty {
                                &[]
                            } else {
                                projected_casters
                                    .get_or_insert_with(|| {
                                        project_casters(&caster_bounds, light.direction)
                                    })
                                    .as_slice()
                            },
                            origin,
                        );
                        shadow.projection = Projection::Orthographic {
                            size,
                            near: 0.05,
                            far,
                        };
                        shadow.viewport = [(cascade % 2) * 2048, (cascade / 2) * 2048, 2048, 2048];
                        (
                            0,
                            format!("{}:shadow:{}:directional", target, view.view_id),
                            p,
                            [lo, hi],
                            cascade == 0,
                        )
                    }
                    LightKind::Spot {
                        range_m,
                        outer_half_angle_degrees,
                        ..
                    } => {
                        if range_m <= 0.05 {
                            return Err(invalid("shadow.spot_near"));
                        }
                        shadow.projection = Projection::Perspective {
                            fov_y_degrees: outer_half_angle_degrees * 2.,
                            near: 0.05,
                            far: range_m,
                        };
                        shadow.viewport =
                            [(spot_index % 4) * 1024, (spot_index / 4) * 1024, 1024, 1024];
                        let first = spot_index == 0;
                        spot_index += 1;
                        (
                            1,
                            format!("{}:shadow:{}:spot", target, view.view_id),
                            CameraPose {
                                position: light.position,
                                rotation: rotation(basis(light.direction)),
                            },
                            [near, far],
                            first,
                        )
                    }
                    LightKind::Point { .. } => return Err(invalid("shadow.point_unsupported")),
                };
                shadow.clear_policy = if first {
                    ClearPolicy::Depth { depth: 1. }
                } else {
                    ClearPolicy::Load
                };
                jobs.push(crate::render_geometry::prepare_tasks::ViewJob {
                    target: atlas,
                    extent: EXTENT,
                    view: shadow,
                    pose: shadow_pose,
                    empty: receiver_empty,
                });
                metadata.push((slot, light_index as u32, range));
            }
        }
        Ok(())
    })();
    if let Err(error) = planning {
        pending
            .finish(inputs, &FinalViewState::default())
            .map_err(|e| e.error)?;
        crate::render_geometry::prepare_tasks::build_jobs(inputs, &jobs)?;
        return Err(error);
    }
    let projections_result = (|| -> Result<_, ContractError> {
        for (job, (slot, light_index, range)) in jobs.iter().zip(metadata) {
            let v = job.view.evaluate(job.pose)?;
            projections.push(ShadowProjection {
                atlas: job.target.clone(),
                slot,
                light_index,
                matrix: v.view_projection().map(Into::into),
                viewport: v.viewport,
                extent: job.extent,
                depth_range: range.map(Into::into),
            });
        }
        ShadowMaps::build(&pending.frame.view, &list, projections)
    })();
    let maps = match projections_result {
        Ok(maps) => maps,
        Err(error) => {
            pending
                .finish(inputs, &FinalViewState::default())
                .map_err(|e| e.error)?;
            crate::render_geometry::prepare_tasks::build_jobs(inputs, &jobs)?;
            return Err(error);
        }
    };
    let views = crate::render_geometry::prepare_tasks::build_lit_jobs(
        inputs,
        &jobs,
        pending,
        FinalViewState {
            environment,
            lighting: Some(list),
            shadows: Some(maps),
        },
    )?;
    Ok(GeometryFrame {
        target: target.into(),
        extent,
        views,
    })
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    #[test]
    fn dynamic_shadow_workset_parallel_matches_scalar_complete_ranges_casters_and_errors() {
        for count in [4, 32, 256] {
            let (manager, source, view, _) = receiver_scene(MaterialBlend::Opaque, -100.);
            let mut instances: Vec<_> = (0..count)
                .map(|i| GeometryInstance {
                    instance_id: i as u64 + 1,
                    source: &source,
                    world_transform: {
                        let mut m = IDENTITY_MATRIX;
                        m[12] = i as f32 * 0.001;
                        m[14] = -100.;
                        m
                    },
                    visibility: Visibility {
                        authored: true,
                        inherited: Evaluation::Known(i % 7 != 0),
                    },
                    layers: LayerSelection::Named { names: vec![] },
                    provenance: source.geometry_source.as_ref().clone(),
                })
                .collect();
            let mut inputs =
                build_inputs::GeometryBuildInputs::new(&instances, &manager, None).unwrap();
            inputs.enable_compact().unwrap();
            let frame = inputs
                .build(
                    "main",
                    [64, 64],
                    &[GeometryViewInput {
                        view: &view,
                        pose: CameraPose::default(),
                    }],
                    DiagnosticLevel::Off,
                )
                .unwrap();
            let a = shadow_workset(&frame, &view, inputs.instances(), None, None, 1).unwrap();
            let b = shadow_workset(&frame, &view, inputs.instances(), None, None, 2).unwrap();
            assert_eq!(a.receivers, b.receivers);
            assert_eq!(a.casters, b.casters);
            assert!(!a.receivers.as_ref().unwrap().is_empty() && !a.casters.is_empty());
            assert_eq!(
                SHADOW_WORKSET_THREADS.with(|t| t
                    .borrow()
                    .iter()
                    .copied()
                    .collect::<std::collections::HashSet<_>>()
                    .len()),
                2
            );
            let reference: Vec<_> = instances
                .iter()
                .filter(|i| i.visibility.inherited == Evaluation::Known(true))
                .map(|i| {
                    world_corners(
                        &build_inputs::InstanceInputs::Owned(std::slice::from_ref(i))
                            .get(0)
                            .unwrap(),
                    )
                })
                .collect();
            assert_eq!(
                b.casters, reference,
                "caster order / tail changed at {count}"
            );
            drop(inputs);
            instances.last_mut().unwrap().layers = LayerSelection::Legacy { mask: None };
            let scalar = shadow_workset(
                &frame,
                &view,
                build_inputs::InstanceInputs::Owned(&instances),
                None,
                None,
                1,
            )
            .err();
            let parallel = shadow_workset(
                &frame,
                &view,
                build_inputs::InstanceInputs::Owned(&instances),
                None,
                None,
                2,
            )
            .err();
            assert!(scalar.is_some());
            assert_eq!(scalar, parallel);
        }
    }

    // Independent pre-change scalar oracle, not the projected implementation.
    fn directional_pose_reference(
        corners: [[f32; 3]; 8],
        direction: [f32; 3],
        casters: &[[[f32; 3]; 8]],
        origin: [f64; 3],
    ) -> (CameraPose, f32, f32) {
        let b = basis(direction);
        let center: [f32; 3] =
            std::array::from_fn(|axis| corners.iter().map(|p| p[axis]).sum::<f32>() / 8.);
        let radius = corners
            .iter()
            .map(|p| {
                dot(
                    std::array::from_fn(|i| p[i] - center[i]),
                    std::array::from_fn(|i| p[i] - center[i]),
                )
                .sqrt()
            })
            .fold(0f32, f32::max);
        // Quantized radius plus a texel guard keeps snapped receiver corners inside the tile.
        let radius = ((radius * 16.).ceil() / 16.).max(0.0625) * 1.002;
        let step = 2. * radius / 2048.;
        // Snap to the absolute light-space grid, then return to this view's origin.
        // Snapping only relative coordinates shifts a stationary shadow at a rebase.
        let snap = |axis: [f32; 3]| {
            let offset: f64 = axis
                .into_iter()
                .zip(origin)
                .map(|(a, o)| f64::from(a) * o)
                .sum();
            let local: f64 = axis
                .into_iter()
                .zip(center)
                .map(|(a, c)| f64::from(a) * f64::from(c))
                .sum();
            (((offset + local) / f64::from(step)).round() * f64::from(step) - offset) as f32
        };
        let x = snap(b[0]);
        let y = snap(b[1]);
        let mut zmin = corners
            .iter()
            .map(|p| dot(b[2], *p))
            .fold(f32::INFINITY, f32::min);
        let mut zmax = corners
            .iter()
            .map(|p| dot(b[2], *p))
            .fold(f32::NEG_INFINITY, f32::max);
        for caster in casters {
            let xmin = caster
                .iter()
                .map(|p| dot(b[0], *p))
                .fold(f32::INFINITY, f32::min);
            let xmax = caster
                .iter()
                .map(|p| dot(b[0], *p))
                .fold(f32::NEG_INFINITY, f32::max);
            let ymin = caster
                .iter()
                .map(|p| dot(b[1], *p))
                .fold(f32::INFINITY, f32::min);
            let ymax = caster
                .iter()
                .map(|p| dot(b[1], *p))
                .fold(f32::NEG_INFINITY, f32::max);
            if xmax < x - radius || xmin > x + radius || ymax < y - radius || ymin > y + radius {
                continue;
            }
            for p in caster {
                let z = dot(b[2], *p);
                zmin = zmin.min(z);
                zmax = zmax.max(z);
            }
        }
        let position = std::array::from_fn(|i| b[0][i] * x + b[1][i] * y + b[2][i] * (zmax + 0.1));
        (
            CameraPose {
                position,
                rotation: rotation(b),
            },
            radius * 2.,
            (zmax - zmin + 0.2).max(0.2),
        )
    }

    #[test]
    fn directional_cascades_project_each_caster_only_once_at_multiple_scales() {
        for count in [4, 32, 256] {
            let casters: Vec<_> = (0..count)
                .map(|i| {
                    std::array::from_fn(|c| {
                        [
                            i as f32 * 0.25 + (c & 1) as f32,
                            ((c >> 1) & 1) as f32 - 0.5,
                            -3. - i as f32 - ((c >> 2) & 1) as f32,
                        ]
                    })
                })
                .collect();
            let direction = normalize([0.3, -1., 0.2]);
            CASTER_PROJECTION_DOTS.with(|n| n.set(0));
            let projected = project_casters(&casters, direction);
            for cascade in 0..4 {
                let corners = std::array::from_fn(|c| {
                    let extent = (cascade + 1) as f32 * 10.;
                    [
                        if c & 1 == 0 { -extent } else { extent },
                        if c & 2 == 0 { -extent } else { extent },
                        if c & 4 == 0 { -extent } else { -0.1 },
                    ]
                });
                let actual = directional_pose(corners, direction, &projected, [0.; 3]);
                let expected = directional_pose_reference(corners, direction, &casters, [0.; 3]);
                assert_eq!(
                    actual.0.position.map(f32::to_bits),
                    expected.0.position.map(f32::to_bits)
                );
                assert_eq!(
                    actual.0.rotation.map(f32::to_bits),
                    expected.0.rotation.map(f32::to_bits)
                );
                assert_eq!(
                    [actual.1.to_bits(), actual.2.to_bits()],
                    [expected.1.to_bits(), expected.2.to_bits()]
                );
            }
            let actual = CASTER_PROJECTION_DOTS.with(|n| n.get());
            assert!(actual <= count * 24,
                "four cascades repeated physical light-space dot products: {count} casters -> {actual}");
        }
    }

    #[test]
    fn projected_casters_preserve_old_pose_for_empty_grazing_and_rebased_inputs() {
        for count in [0, 1, 4, 64] {
            let casters: Vec<_> = (0..count)
                .map(|i| {
                    std::array::from_fn(|c| {
                        [
                            i as f32 * 1.25 + (c & 1) as f32 - 8.,
                            ((c >> 1) & 1) as f32 * 2. - 1.,
                            -2. - i as f32 - ((c >> 2) & 1) as f32,
                        ]
                    })
                })
                .collect();
            for direction in [
                [0., -1., 0.],
                normalize([1., -0.001, 0.1]),
                normalize([-0.3, 1., 0.2]),
            ] {
                let projected = project_casters(&casters, direction);
                for origin in [[0.; 3], [1e9, -2e8, 5e7], [-1e14, 2e13, 4e12]] {
                    let corners = std::array::from_fn(|c| {
                        [
                            if c & 1 == 0 { -10. } else { 10. },
                            if c & 2 == 0 { -5. } else { 5. },
                            if c & 4 == 0 { -100. } else { -0.1 },
                        ]
                    });
                    let a = directional_pose(corners, direction, &projected, origin);
                    let b = directional_pose_reference(corners, direction, &casters, origin);
                    assert_eq!(
                        a.0.position.map(f32::to_bits),
                        b.0.position.map(f32::to_bits)
                    );
                    assert_eq!(
                        a.0.rotation.map(f32::to_bits),
                        b.0.rotation.map(f32::to_bits)
                    );
                    assert_eq!(
                        [a.1.to_bits(), a.2.to_bits()],
                        [b.1.to_bits(), b.2.to_bits()]
                    );
                }
            }
        }
    }

    #[test]
    fn consumption_prepared_shadow_geometry_avoids_repeated_world_transforms_at_multiple_scales() {
        let mut observed = Vec::new();
        for count in [4, 32, 256] {
            let (manager, source, view, light) = receiver_scene(MaterialBlend::Opaque, -100.);
            let instances: Vec<_> = (0..count)
                .map(|id| GeometryInstance {
                    instance_id: id as u64,
                    source: &source,
                    world_transform: {
                        let mut m = IDENTITY_MATRIX;
                        m[14] = -100.;
                        m
                    },
                    visibility: Visibility {
                        authored: true,
                        inherited: Evaluation::Known(true),
                    },
                    layers: LayerSelection::Named { names: vec![] },
                    provenance: source.geometry_source.as_ref().clone(),
                })
                .collect();
            let prepared;
            let mut inputs =
                build_inputs::GeometryBuildInputs::new(&instances, &manager, None).unwrap();
            inputs.enable_compact().unwrap();
            prepared = inputs.freeze();
            inputs.prepared = Some(&prepared);
            let mut frame = inputs
                .build(
                    "main",
                    [64, 64],
                    &[GeometryViewInput {
                        view: &view,
                        pose: CameraPose::default(),
                    }],
                    DiagnosticLevel::Off,
                )
                .unwrap();
            assert_eq!(frame.views[0].selected_instances().count(), count);
            workset::SHADOW_WORLD_TRANSFORMS.with(|n| n.set(0));
            workset::RECEIVER_CAMERA_COMPONENTS.with(|n| n.set(0));
            append_scene_lighting(
                &mut frame,
                GeometryViewInput {
                    view: &view,
                    pose: CameraPose::default(),
                },
                [0.; 3],
                &mut inputs,
                std::slice::from_ref(&light),
            )
            .unwrap();
            let world = workset::SHADOW_WORLD_TRANSFORMS.with(|n| n.get());
            let camera = workset::RECEIVER_CAMERA_COMPONENTS.with(|n| n.get());
            observed.push((count, world, camera));
        }
        assert!(
            observed
                .iter()
                .all(|(n, world, camera)| *world == 0 && *camera == n * 8),
            "actual prepared shadow work (rows, world corners, camera components): {observed:?}"
        );
    }

    #[test]
    fn consumption_receiver_bounds_recheck_camera_and_reject_changed_instance_identity() {
        let (manager, source, view, _) = receiver_scene(MaterialBlend::Opaque, -100.);
        let instances = [GeometryInstance {
            instance_id: 7,
            source: &source,
            world_transform: {
                let mut m = IDENTITY_MATRIX;
                m[14] = -100.;
                m
            },
            visibility: Visibility {
                authored: true,
                inherited: Evaluation::Known(true),
            },
            layers: LayerSelection::Named { names: vec![] },
            provenance: source.geometry_source.as_ref().clone(),
        }];
        let prepared;
        let mut inputs =
            build_inputs::GeometryBuildInputs::new(&instances, &manager, None).unwrap();
        inputs.enable_compact().unwrap();
        prepared = inputs.freeze();
        inputs.prepared = Some(&prepared);
        for z in [0., 10., -25.] {
            let frame = inputs
                .build(
                    "main",
                    [64, 64],
                    &[GeometryViewInput {
                        view: &view,
                        pose: CameraPose {
                            position: [0., 0., z],
                            ..Default::default()
                        },
                    }],
                    DiagnosticLevel::Off,
                )
                .unwrap();
            let expected = receiver_depth_ranges_inputs(
                &frame,
                view.view_id,
                inputs.instances(),
                Some(&inputs.compact_indices),
            )
            .unwrap();
            assert_eq!(
                receiver_depth_ranges_prepared(
                    &frame,
                    view.view_id,
                    inputs.instances(),
                    Some(&inputs.compact_indices),
                    Some(&prepared)
                ),
                Some(expected)
            );
            let mut changed = frame.clone();
            assert!(changed.views[0].batches.is_none());
            let selected = &mut Arc::make_mut(&mut changed.views[0].draws[0]).instance;
            Arc::make_mut(selected).world_transform[14] = (-600f32).into();
            assert!(
                prepared.receiver_bounds(0, selected).is_none(),
                "COW row inherited static world corners"
            );
            let expected = receiver_depth_ranges_inputs(
                &changed,
                view.view_id,
                inputs.instances(),
                Some(&inputs.compact_indices),
            )
            .unwrap();
            let actual = receiver_depth_ranges_prepared(
                &changed,
                view.view_id,
                inputs.instances(),
                Some(&inputs.compact_indices),
                Some(&prepared),
            )
            .unwrap();
            assert_eq!(
                actual, expected,
                "changed selected transform failed strict fallback"
            );
            assert!(
                actual[0][0] > 500.,
                "test failed to exercise changed world depth"
            );
        }
        let bounds = prepared.receiver_bounds(0, &prepared.table[0]).unwrap();
        for v in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut camera = IDENTITY_MATRIX;
            camera[0] = v;
            assert!(bounds.depth_range(camera).is_none());
        }
        let mut camera = IDENTITY_MATRIX;
        camera[14] = f32::MAX;
        assert!(
            bounds.depth_range(camera).is_none(),
            "overflowing outward guard proved finite depth"
        );
    }

    pub(crate) fn receiver_scene(
        blend: MaterialBlend,
        z: f32,
    ) -> (
        RenderResourceManager,
        PreparedGeometry,
        RenderView,
        crate::render_projection_adapter::LightProjection,
    ) {
        use crate::render_resource::*;
        let mut manager = RenderResourceManager::new();
        let graph = crate::rhi_command_plan::tests::retained_geometry_graph(1);
        let template = graph
            .passes
            .iter()
            .flat_map(|p| &p.commands)
            .find_map(|c| {
                if let crate::render_graph::RenderPassCommand::DrawGeometry { draw } = c {
                    Some(draw)
                } else {
                    None
                }
            })
            .unwrap();
        let mut binding = |name: &str, kind, binding_kind, source| {
            let handle = manager.request_resource(
                0,
                RenderResourceRequest {
                    key: RenderAssetKey::new(name, "1", kind),
                    source,
                    byte_len: 64,
                    reason: "receiver-proof-test".into(),
                },
            );
            Arc::new(
                RenderBindingSet::single(
                    name.into(),
                    binding_kind,
                    handle,
                    "nearestClamp".into(),
                    false,
                    "receiver-proof-test".into(),
                )
                .unwrap(),
            )
        };
        let geometry_binding = binding(
            "mesh",
            RenderResourceKind::MeshBuffer,
            RenderBindingKind::Mesh,
            RenderResourceSource::MeshBufferDescriptor {
                vertex_count: 3,
                index_count: 3,
            },
        );
        let material_binding = binding(
            "material",
            RenderResourceKind::MaterialParams,
            RenderBindingKind::Material,
            RenderResourceSource::MaterialParamsDescriptor { param_count: 6 },
        );
        let mut wire = serde_json::to_value(template.geometry.as_ref()).unwrap();
        let bytes: Vec<_> = [[-0.8f32, -0.8, 0.], [0.8, -0.8, 0.], [0., 0.8, 0.]]
            .into_iter()
            .flat_map(|vertex| {
                vertex
                    .into_iter()
                    .chain([0f32, 0., 1.])
                    .flat_map(f32::to_le_bytes)
            })
            .collect();
        wire["vertexBytes"] = serde_json::json!(bytes);
        wire["vertexStride"] = 24.into();
        wire["attributes"] = serde_json::json!([
            {"semantic":"position","format":"float32x3","offset":0},
            {"semantic":"normal","format":"float32x3","offset":12}
        ]);
        let mut parameters = std::collections::BTreeMap::from([
            ("baseColor".into(), MaterialParameter::Vector4([1.; 4])),
            ("emissive".into(), MaterialParameter::Vector4([0.; 4])),
        ]);
        for (key, value) in [
            ("metallic", 0.),
            ("roughness", 0.5),
            ("normalScale", 1.),
            ("occlusionStrength", 1.),
        ] {
            parameters.insert(key.into(), MaterialParameter::Float(value));
        }
        if blend == MaterialBlend::AlphaMask {
            parameters.insert("alphaCutoff".into(), MaterialParameter::Float(0.5));
        }
        let source = PreparedGeometry {
            buffers: Arc::new(serde_json::from_value(wire).unwrap()),
            geometry_binding,
            material_binding,
            geometry_source: template.geometry_source.clone(),
            material_source: template.material_source.clone(),
            material: CookedMaterial {
                decal_bias: None,
                preset: MaterialPreset::StandardLit,
                texture_sampling: Default::default(),
                parameters,
                textures: Default::default(),
                base_color_texture_sampling: None,
                opaque_alpha: false,
                ignore_vertex_color: false,
                blend,
                cull: MaterialCull::None,
                depth: if blend == MaterialBlend::AlphaBlend {
                    MaterialDepth::TestOnly
                } else {
                    MaterialDepth::TestAndWrite
                },
                shader_source: STANDARD_LIT_SHADER_SOURCE.into(),
            }
            .into(),
            bounds: GeometryBounds {
                min: [-1.; 3],
                max: [1.; 3],
                sphere_center: [0.; 3],
                sphere_radius: 1.75,
            },
            submesh: SubmeshRange {
                first_index: 0,
                index_count: 3,
                material_slot: "surface".into(),
            },
            convention: CoordinateConvention::World3dRightHandedYUpMetersCameraNegativeZ,
        };
        source.material.validate().unwrap();
        let view = RenderView::perspective(1, 64, 64, 60., 0.1, 2000.);
        let light = crate::render_projection_adapter::LightProjection {
            component: crate::spatial_contract::Light3D {
                cast_shadows: true,
                schema_version: "light3d.v1".into(),
                enabled: true,
                color: [1.; 3],
                kind: LightKind::Directional {
                    illuminance_lux: 1.,
                },
                layers: vec![],
            },
            position: [0.; 3],
            direction: [0., 0., -1.],
            origin: [0.; 3],
        };
        assert!(z.is_finite());
        (manager, source, view, light)
    }

    #[test]
    fn receiver_row_bound_never_shrinks_the_original_corner_error_guard() {
        let (_, source, _, _) = receiver_scene(MaterialBlend::Opaque, -100.);
        for translation in [0., 10_000., 100_000_000.] {
            for scale in [0.125, 1., 64.] {
                for sign in [-1., 1.] {
                    for shear in [0., 0.75] {
                        for size in [0.01, 1., 100.] {
                            let mut bounds = source.bounds.clone();
                            bounds.min = [-size, -2. * size, -3. * size];
                            bounds.max = [2. * size, size, size];
                            let mut world = IDENTITY_MATRIX;
                            world[0] = sign * scale;
                            world[4] = shear;
                            world[5] = 3.;
                            world[10] = 0.5;
                            world[12] = translation;
                            world[14] = -100. - translation;
                            let mut camera = IDENTITY_MATRIX;
                            camera[0] = 0.6;
                            camera[2] = -0.8;
                            camera[8] = 0.8;
                            camera[10] = 0.6;
                            camera[12] = -translation;
                            let reference =
                                receiver_depth_range_corner_reference(camera, world, &bounds)
                                    .expect("finite original corner reference");
                            let actual = receiver_depth_range(camera, world, &bounds)
                                .expect("bounded finite scene");
                            let retained = workset::ReceiverBounds::new(world, &bounds)
                                .expect("finite retained world bounds");
                            assert_eq!(
                                retained.depth_range(camera),
                                Some(actual),
                                "retained depth changed the original absolute-row guard"
                            );
                            assert!(
                                actual[0] <= reference[0] && actual[1] >= reference[1],
                                "receiver proof shrank: {actual:?} versus {reference:?}"
                            );
                        }
                    }
                }
            }
        }
        for nonfinite in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut world = IDENTITY_MATRIX;
            world[0] = nonfinite;
            assert!(receiver_depth_range(IDENTITY_MATRIX, world, &source.bounds).is_none());
            assert!(workset::ReceiverBounds::new(world, &source.bounds).is_none());
        }
        let mut world = IDENTITY_MATRIX;
        world[12] = f32::MAX;
        let mut camera = IDENTITY_MATRIX;
        camera[12] = f32::MAX;
        assert!(receiver_depth_range(camera, world, &source.bounds).is_none());
    }

    #[test]
    fn compact_receivers_borrow_instances_without_expanding_draw_descriptors() {
        for blend in [
            MaterialBlend::Opaque,
            MaterialBlend::AlphaMask,
            MaterialBlend::AlphaBlend,
        ] {
            let (manager, source, view, _) = receiver_scene(blend, -100.);
            let instances: Vec<_> = (0..32)
                .map(|instance_id| GeometryInstance {
                    instance_id,
                    source: &source,
                    world_transform: {
                        let mut m = IDENTITY_MATRIX;
                        m[14] = -100. - instance_id as f32;
                        m
                    },
                    visibility: Visibility {
                        authored: true,
                        inherited: Evaluation::Known(true),
                    },
                    layers: LayerSelection::Named { names: vec![] },
                    provenance: source.geometry_source.as_ref().clone(),
                })
                .collect();
            let build = |compact| {
                let mut inputs =
                    build_inputs::GeometryBuildInputs::new(&instances, &manager, None).unwrap();
                if compact {
                    inputs.enable_compact().unwrap();
                }
                inputs
                    .build(
                        "main",
                        [64, 64],
                        &[GeometryViewInput {
                            view: &view,
                            pose: CameraPose::default(),
                        }],
                        DiagnosticLevel::Off,
                    )
                    .unwrap()
            };
            let flat = build(false);
            let compact = build(true);
            let expected = receiver_depth_ranges(&flat, view.view_id, &instances).unwrap();
            commands::SELECTED_DESCRIPTOR_BUILDS.with(|n| n.set(0));
            RECEIVER_MAGNITUDE_ROWS.with(|n| n.set(0));
            RECEIVER_VIEW_COMPARISONS.with(|n| n.set(0));
            let actual = receiver_depth_ranges(&compact, view.view_id, &instances).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(actual.len(), 32);
            assert_eq!(
                RECEIVER_VIEW_COMPARISONS.with(|n| n.get()),
                1,
                "receiver compared the same immutable view for every selected instance"
            );
            assert!(
                RECEIVER_MAGNITUDE_ROWS.with(|n| n.get()) <= 32 * 8,
                "receiver magnitude evaluated {} rows for 32 immutable bounds",
                RECEIVER_MAGNITUDE_ROWS.with(|n| n.get())
            );
            let mut prepared =
                build_inputs::GeometryBuildInputs::new(&instances, &manager, None).unwrap();
            prepared.enable_compact().unwrap();
            RECEIVER_SOURCE_ROWS.with(|n| n.set(0));
            build_inputs::INSTANCE_INPUT_ROWS.with(|n| n.set(0));
            let indexed = receiver_depth_ranges_inputs(
                &compact,
                view.view_id,
                prepared.instances(),
                Some(&prepared.compact_indices),
            )
            .unwrap();
            assert_eq!(indexed, expected);
            assert!(
                build_inputs::INSTANCE_INPUT_ROWS.with(|n| n.get()) <= 32,
                "indexed receiver projected {} source rows for 32 selected instances",
                build_inputs::INSTANCE_INPUT_ROWS.with(|n| n.get())
            );
            assert_eq!(
                RECEIVER_SOURCE_ROWS.with(|n| n.get()),
                0,
                "receiver rebuilt the build group's already validated ID index"
            );
            let mut stale = prepared.compact_indices.as_ref().clone();
            stale.insert(0, 31);
            assert!(
                receiver_depth_ranges_inputs(
                    &compact,
                    view.view_id,
                    prepared.instances(),
                    Some(&stale)
                )
                .is_none(),
                "same-sized stale index proved a receiver range"
            );
            stale.insert(0, u32::MAX);
            assert!(
                receiver_depth_ranges_inputs(
                    &compact,
                    view.view_id,
                    prepared.instances(),
                    Some(&stale)
                )
                .is_none(),
                "out-of-range source index proved a receiver range"
            );
            stale.remove(&0);
            assert!(
                receiver_depth_ranges_inputs(
                    &compact,
                    view.view_id,
                    prepared.instances(),
                    Some(&stale)
                )
                .is_none(),
                "incomplete source index proved a receiver range"
            );
            assert_eq!(
                commands::SELECTED_DESCRIPTOR_BUILDS.with(|n| n.get()),
                0,
                "receiver proof expanded compact instances to owned draw descriptors"
            );
            let mut changed = flat.clone();
            Arc::make_mut(&mut Arc::make_mut(changed.views[0].draws.last_mut().unwrap()).view)
                .view_matrix[12] = 1f32.into();
            assert!(
                receiver_depth_ranges(&changed, view.view_id, &instances).is_none(),
                "late same-ID changed view borrowed a previous receiver proof"
            );
            assert_eq!(
                receiver_depth_ranges(&compact, view.view_id, &instances).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn actual_receivers_leave_far_cascades_empty_without_removing_projection_or_clear() {
        for blend in [
            MaterialBlend::Opaque,
            MaterialBlend::AlphaMask,
            MaterialBlend::AlphaBlend,
        ] {
            let (manager, source, view, light) = receiver_scene(blend, -100.);
            let instances = [GeometryInstance {
                instance_id: 1,
                source: &source,
                world_transform: {
                    let mut matrix = IDENTITY_MATRIX;
                    matrix[14] = -100.;
                    matrix
                },
                visibility: Visibility {
                    authored: true,
                    inherited: Evaluation::Known(true),
                },
                layers: LayerSelection::Named {
                    names: vec!["main".into()],
                },
                provenance: source.geometry_source.as_ref().clone(),
            }];
            let mut inputs =
                build_inputs::GeometryBuildInputs::new(&instances, &manager, None).unwrap();
            inputs.enable_compact().unwrap();
            let mut frame = inputs
                .build(
                    "main",
                    [64, 64],
                    &[GeometryViewInput {
                        view: &view,
                        pose: CameraPose::default(),
                    }],
                    DiagnosticLevel::Off,
                )
                .unwrap();
            assert_eq!(frame.views[0].draws.len(), 1);
            let ranges = receiver_depth_ranges(&frame, view.view_id, &instances).unwrap();
            assert_eq!(ranges.len(), 1, "mask/blend receivers must not disappear");
            assert!(cascade_has_receiver(Some(&ranges), 99., 101.));
            lighting::LIGHT_LIST_BUILDS.with(|n| n.set(0));
            RECEIVER_SOURCE_ROWS.with(|n| n.set(0));
            append_scene_lighting(
                &mut frame,
                GeometryViewInput {
                    view: &view,
                    pose: CameraPose::default(),
                },
                [0.; 3],
                &mut inputs,
                &[light],
            )
            .unwrap();
            assert_eq!(
                RECEIVER_SOURCE_ROWS.with(|n| n.get()),
                0,
                "production lighting ignored its current build-group source index"
            );
            assert_eq!(
                lighting::LIGHT_LIST_BUILDS.with(|n| n.get()),
                1,
                "color and shadow composition rebuilt the same clustered list"
            );
            assert_eq!(frame.views.len(), 5);
            let maps = frame.views.last().unwrap().draws[0]
                .shadows
                .as_ref()
                .unwrap();
            assert_eq!(maps.projections().len(), 4);
            assert_eq!(frame.views[0].view.depth_clear, Some(1f32.into()));
            for index in 1..4 {
                assert!(
                    frame.views[index].draws.is_empty(),
                    "{blend:?}: cascade {index} has no actual receiver, still drew {} casters",
                    frame.views[index].draws.len()
                );
            }
            if blend != MaterialBlend::AlphaBlend {
                assert!(!frame.views[0].draws.is_empty());
            }
        }
    }

    fn shared_frame(
        source: &PreparedGeometry,
        manager: &RenderResourceManager,
        lease: &Arc<()>,
        reuse: &std::sync::Mutex<build_inputs::InstanceReuse>,
        world: [f32; 16],
        origin: [u64; 3],
        camera_x: f32,
    ) -> Result<GeometryFrame, GeometryBuildError> {
        let instances = [GeometryInstance {
            instance_id: 1,
            source,
            world_transform: world,
            visibility: Visibility {
                authored: true,
                inherited: Evaluation::Known(true),
            },
            layers: LayerSelection::Named {
                names: vec!["main".into()],
            },
            provenance: source.geometry_source.as_ref().clone(),
        }];
        let view = RenderView::perspective(1, 64, 64, 60., 0.1, 2000.);
        let mut inputs = build_inputs::GeometryBuildInputs::new(&instances, manager, Some(lease))?
            .with_instance_reuse(reuse, origin);
        inputs.build(
            "main",
            [64, 64],
            &[GeometryViewInput {
                view: &view,
                pose: CameraPose {
                    position: [camera_x, 0., 0.],
                    ..Default::default()
                },
            }],
            DiagnosticLevel::Off,
        )
    }

    #[test]
    fn s2_work_granularity_camera_change_does_not_reinterpret_stable_material() {
        let (manager, source, _, _) = receiver_scene(MaterialBlend::Opaque, -100.);
        let lease = manager.lease_resources(source.resource_handles());
        let reuse = std::sync::Mutex::new(build_inputs::InstanceReuse::default());
        let mut world = IDENTITY_MATRIX;
        world[14] = -100.;
        let first = shared_frame(&source, &manager, &lease, &reuse, world, [0; 3], 0.).unwrap();
        build_inputs::MATERIAL_INTERPRETATIONS.with(|n| n.set(0));
        let second = shared_frame(&source, &manager, &lease, &reuse, world, [0; 3], 1.).unwrap();
        assert!(Arc::ptr_eq(
            &first.views[0].draws[0].instance,
            &second.views[0].draws[0].instance
        ));
        assert_eq!(
            build_inputs::MATERIAL_INTERPRETATIONS.with(|n| n.get()),
            0,
            "stable prepared material was parsed before a successful instance reuse"
        );
        let table = |order: [u64; 2], x: f32| {
            let instances = order.map(|id| GeometryInstance {
                instance_id: id,
                source: &source,
                world_transform: {
                    let mut m = world;
                    m[12] = x;
                    m
                },
                visibility: Visibility {
                    authored: true,
                    inherited: Evaluation::Known(true),
                },
                layers: LayerSelection::Named { names: vec![] },
                provenance: source.geometry_source.as_ref().clone(),
            });
            let mut inputs =
                build_inputs::GeometryBuildInputs::new(&instances, &manager, Some(&lease))
                    .unwrap()
                    .with_instance_reuse(&reuse, [0; 3]);
            inputs.enable_compact().unwrap();
            (
                inputs.compact_table.clone().unwrap(),
                inputs.compact_indices.clone(),
            )
        };
        let a = table([1, 2], 0.);
        let b = table([1, 2], 0.);
        assert!(
            Arc::ptr_eq(&a.0, &b.0),
            "unchanged ordered table was rebuilt"
        );
        assert!(Arc::ptr_eq(&a.1, &b.1));
        let changed = table([1, 2], 0.5);
        assert!(!Arc::ptr_eq(&b.0, &changed.0));
        assert!(
            Arc::ptr_eq(&b.1, &changed.1),
            "world change rebuilt stable ID layout"
        );
        assert_eq!(a.0[0].world_transform[12].to_f32(), 0.);
        let reordered = table([2, 1], 0.5);
        assert_eq!(reordered.1[&2], 0);
        assert_eq!(reordered.1[&1], 1);
        assert!(!Arc::ptr_eq(&changed.1, &reordered.1));
        let weak = Arc::downgrade(&a.0);
        drop(a);
        drop(b);
        assert!(
            weak.upgrade().is_none(),
            "table reuse retained an old packet"
        );
    }

    #[test]
    fn cross_snapshot_instance_reuse_requires_full_values_current_lease_and_does_not_pin_old_data()
    {
        let (mut manager, source, _, _) = receiver_scene(MaterialBlend::Opaque, -100.);
        let lease = manager.lease_resources(source.resource_handles());
        let reuse = std::sync::Mutex::new(build_inputs::InstanceReuse::default());
        let mut world = IDENTITY_MATRIX;
        world[14] = -100.;
        let first = shared_frame(&source, &manager, &lease, &reuse, world, [0; 3], 0.).unwrap();
        let second = shared_frame(&source, &manager, &lease, &reuse, world, [0; 3], 1.).unwrap();
        assert!(
            Arc::ptr_eq(
                &first.views[0].draws[0].instance,
                &second.views[0].draws[0].instance
            ),
            "camera-only snapshot rebuilt unchanged instance backing"
        );
        assert_ne!(first.views[0].view, second.views[0].view);
        let old = Arc::downgrade(&first.views[0].draws[0].instance);
        world[12] = 0.25;
        let changed = shared_frame(&source, &manager, &lease, &reuse, world, [0; 3], 1.).unwrap();
        assert!(!Arc::ptr_eq(
            &first.views[0].draws[0].instance,
            &changed.views[0].draws[0].instance
        ));
        assert_eq!(first.views[0].draws[0].world_transform[12].to_f32(), 0.);
        let other_origin =
            shared_frame(&source, &manager, &lease, &reuse, world, [1; 3], 1.).unwrap();
        assert!(!Arc::ptr_eq(
            &changed.views[0].draws[0].instance,
            &other_origin.views[0].draws[0].instance
        ));
        let other_lease = manager.lease_resources(source.resource_handles());
        let other =
            shared_frame(&source, &manager, &other_lease, &reuse, world, [1; 3], 1.).unwrap();
        assert!(!Arc::ptr_eq(
            &other_origin.views[0].draws[0].instance,
            &other.views[0].draws[0].instance
        ));
        let mut material = source.clone();
        material
            .material
            .parameters
            .insert("baseColor".into(), MaterialParameter::Vector4([0.5; 4]));
        let modified =
            shared_frame(&material, &manager, &other_lease, &reuse, world, [1; 3], 1.).unwrap();
        assert_eq!(modified.views[0].draws[0].base_color[0].to_f32(), 0.5);
        material.submesh.index_count = 6;
        assert!(
            shared_frame(&material, &manager, &other_lease, &reuse, world, [1; 3], 1.).is_err()
        );
        drop(first);
        drop(second);
        assert!(
            old.upgrade().is_none(),
            "reuse table pinned an old instance/lease"
        );
        manager.mark_device_lost(1);
        assert!(
            shared_frame(&source, &manager, &lease, &reuse, world, [0; 3], 1.).is_err(),
            "CPU reuse bypassed current manager admission"
        );
    }

    #[test]
    fn receiver_ranges_are_inclusive_and_unknown_never_means_empty() {
        assert!(cascade_has_receiver(None, 500., 1000.));
        assert!(!cascade_has_receiver(Some(&[]), 500., 1000.));
        assert!(cascade_has_receiver(Some(&[[100., 250.]]), 250., 500.));
        assert!(cascade_has_receiver(Some(&[[250., 600.]]), 100., 250.));
        assert!(!cascade_has_receiver(Some(&[[100., 200.]]), 250., 500.));
    }

    #[test]
    fn stable_sort_identity_survives_released_packets_without_pinning_native_lease() {
        fn sorted_frame(
            source: &PreparedGeometry,
            manager: &RenderResourceManager,
            lease: &Arc<()>,
            reuse: &std::sync::Mutex<build_inputs::InstanceReuse>,
            camera_x: f32,
        ) -> GeometryFrame {
            let instances = [1, 2].map(|instance_id| GeometryInstance {
                instance_id,
                source,
                world_transform: {
                    let mut m = IDENTITY_MATRIX;
                    m[12] = instance_id as f32 * 0.1;
                    m[14] = -100.;
                    m
                },
                visibility: Visibility {
                    authored: true,
                    inherited: Evaluation::Known(true),
                },
                layers: LayerSelection::Named {
                    names: vec!["main".into()],
                },
                provenance: source.geometry_source.as_ref().clone(),
            });
            let view = RenderView::perspective(1, 64, 64, 60., 0.1, 2000.);
            let mut inputs =
                build_inputs::GeometryBuildInputs::new(&instances, manager, Some(lease))
                    .unwrap()
                    .with_instance_reuse(reuse, [0; 3]);
            let frame = inputs
                .build(
                    "main",
                    [64, 64],
                    &[GeometryViewInput {
                        view: &view,
                        pose: CameraPose {
                            position: [camera_x, 0., 0.],
                            ..Default::default()
                        },
                    }],
                    DiagnosticLevel::Off,
                )
                .unwrap();
            assert_eq!(
                frame.views[0].draws.len(),
                2,
                "fixture must execute sorting"
            );
            frame
        }
        let (mut manager, source, _, _) = receiver_scene(MaterialBlend::Opaque, -100.);
        let reuse = std::sync::Mutex::new(build_inputs::InstanceReuse::default());
        batch::IDENTITY_COMPUTATIONS.with(|n| n.set(0));
        let lease = manager.lease_resources(source.resource_handles());
        let old_lease = Arc::downgrade(&lease);
        let first = sorted_frame(&source, &manager, &lease, &reuse, 0.);
        let old_identity = first.views[0].draws[0].batch_identity();
        drop(first);
        drop(lease);
        assert!(
            old_lease.upgrade().is_none(),
            "sort identity retained a native lease"
        );
        let lease = manager.lease_resources(source.resource_handles());
        let second = sorted_frame(&source, &manager, &lease, &reuse, 1.);
        assert_eq!(second.views[0].draws[0].batch_identity(), old_identity);
        assert_eq!(
            batch::IDENTITY_COMPUTATIONS.with(|n| n.get()),
            1,
            "camera-only snapshots recomputed the stable batch identity"
        );
        let mut changed = source.clone();
        Arc::make_mut(&mut changed.material_binding).binding_id = "new-binding".into();
        let changed_frame = sorted_frame(&changed, &manager, &lease, &reuse, 2.);
        assert_ne!(
            changed_frame.views[0].draws[0].batch_identity(),
            old_identity
        );
        assert_eq!(batch::IDENTITY_COMPUTATIONS.with(|n| n.get()), 2);
        manager.mark_device_lost(1);
        let mut world = IDENTITY_MATRIX;
        world[14] = -100.;
        assert!(shared_frame(&source, &manager, &lease, &reuse, world, [0; 3], 3.).is_err());
        assert_eq!(batch::IDENTITY_COMPUTATIONS.with(|n| n.get()), 2);
    }

    #[test]
    #[cfg(feature = "real-wgpu")]
    #[ignore = "requires a real GPU"]
    fn gpu_empty_receiver_cascades_match_conservative_rendering() {
        use crate::render_graph::{RenderGraph, RenderResource};
        use crate::wgpu_backend::real::RealWgpuBackend;
        let mut backend = RealWgpuBackend::new_offscreen(64, 64).unwrap();
        for blend in [
            MaterialBlend::Opaque,
            MaterialBlend::AlphaMask,
            MaterialBlend::AlphaBlend,
        ] {
            let (manager, source, view, light) = receiver_scene(blend, -100.);
            let instances = [GeometryInstance {
                instance_id: 1,
                source: &source,
                world_transform: {
                    let mut m = IDENTITY_MATRIX;
                    m[0] = 30.;
                    m[5] = 30.;
                    m[14] = -100.;
                    m
                },
                visibility: Visibility {
                    authored: true,
                    inherited: Evaluation::Known(true),
                },
                layers: LayerSelection::Named {
                    names: vec!["main".into()],
                },
                provenance: source.geometry_source.as_ref().clone(),
            }];
            let mut outputs = Vec::new();
            let mut counts = Vec::new();
            for unknown in [false, true] {
                let mut inputs =
                    build_inputs::GeometryBuildInputs::new(&instances, &manager, None).unwrap();
                let mut frame = inputs
                    .build(
                        "main",
                        [64, 64],
                        &[GeometryViewInput {
                            view: &view,
                            pose: CameraPose::default(),
                        }],
                        DiagnosticLevel::Off,
                    )
                    .unwrap();
                frame
                    .set_view_lights(view.view_id, std::slice::from_ref(&light))
                    .unwrap();
                if unknown {
                    // Only break the proof's source association. All geometry,
                    // shading, transforms and original caster inputs stay equal.
                    Arc::make_mut(&mut Arc::make_mut(&mut frame.views[0].draws[0]).instance)
                        .instance_id = 99;
                }
                append_scene_lighting(
                    &mut frame,
                    GeometryViewInput {
                        view: &view,
                        pose: CameraPose::default(),
                    },
                    [0.; 3],
                    &mut inputs,
                    std::slice::from_ref(&light),
                )
                .unwrap();
                counts.push(
                    frame.views[..4]
                        .iter()
                        .map(|v| v.draws.len())
                        .sum::<usize>(),
                );
                let mut graph = RenderGraph::new("receiver-equivalence", 1);
                graph.output_target = Some(frame.target.clone());
                graph
                    .resources
                    .push(RenderResource::surface_backbuffer(&frame.target, 64, 64));
                frame.append_to_graph(&mut graph);
                let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
                assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
                outputs.push(backend.render_plan_to_rgba_bytes(&plan, 64, 64).unwrap());
            }
            assert_eq!(
                outputs[0], outputs[1],
                "{blend:?}: empty cascade pruning changed pixels"
            );
            assert!(
                outputs[0].chunks_exact(4).filter(|p| p[1] > 10).count() > 50,
                "{blend:?}: fixture must contain a substantial directly lit receiver"
            );
            if blend != MaterialBlend::AlphaBlend {
                assert!(
                    counts[0] < counts[1],
                    "fixture did not exercise conservative fallback"
                );
            }
        }
        backend
            .shutdown_gpu(std::time::Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    fn receiver_coverage_rechecks_new_depths_and_falls_back_on_missing_or_nonfinite_sources() {
        let (manager, mut source, view, light) = receiver_scene(MaterialBlend::Opaque, -100.);
        for z in [-100., -600., -100.] {
            let instances = [GeometryInstance {
                instance_id: 1,
                source: &source,
                world_transform: {
                    let mut m = IDENTITY_MATRIX;
                    m[14] = z;
                    m
                },
                visibility: Visibility {
                    authored: true,
                    inherited: Evaluation::Known(true),
                },
                layers: LayerSelection::Named {
                    names: vec!["main".into()],
                },
                provenance: source.geometry_source.as_ref().clone(),
            }];
            let mut inputs =
                build_inputs::GeometryBuildInputs::new(&instances, &manager, None).unwrap();
            let mut frame = inputs
                .build(
                    "main",
                    [64, 64],
                    &[GeometryViewInput {
                        view: &view,
                        pose: CameraPose::default(),
                    }],
                    DiagnosticLevel::Off,
                )
                .unwrap();
            assert!(receiver_depth_ranges(&frame, view.view_id, &[]).is_none());
            append_scene_lighting(
                &mut frame,
                GeometryViewInput {
                    view: &view,
                    pose: CameraPose::default(),
                },
                [0.; 3],
                &mut inputs,
                std::slice::from_ref(&light),
            )
            .unwrap();
            let active = if z == -100. { 0 } else { 2 };
            assert!(
                !frame.views[active].draws.is_empty(),
                "new receiver depth was not reconsidered"
            );
            for i in 0..4 {
                if i != active {
                    assert!(frame.views[i].draws.is_empty());
                }
            }
        }
        let frame = {
            let instances = [GeometryInstance {
                instance_id: 1,
                source: &source,
                world_transform: {
                    let mut m = IDENTITY_MATRIX;
                    m[14] = -100.;
                    m
                },
                visibility: Visibility {
                    authored: true,
                    inherited: Evaluation::Known(true),
                },
                layers: LayerSelection::Named { names: vec![] },
                provenance: source.geometry_source.as_ref().clone(),
            }];
            let mut inputs =
                build_inputs::GeometryBuildInputs::new(&instances, &manager, None).unwrap();
            inputs
                .build(
                    "main",
                    [64, 64],
                    &[GeometryViewInput {
                        view: &view,
                        pose: CameraPose::default(),
                    }],
                    DiagnosticLevel::Off,
                )
                .unwrap()
        };
        assert_eq!(frame.views[0].draws.len(), 1);
        source.bounds.min[0] = f32::NAN;
        let instances = [GeometryInstance {
            instance_id: 1,
            source: &source,
            world_transform: IDENTITY_MATRIX,
            visibility: Visibility {
                authored: true,
                inherited: Evaluation::Known(true),
            },
            layers: LayerSelection::Named { names: vec![] },
            provenance: source.geometry_source.as_ref().clone(),
        }];
        assert!(receiver_depth_ranges(&frame, view.view_id, &instances).is_none());
    }
    #[test]
    fn cascade_texel_grid_does_not_shift_when_the_world_origin_is_rebased() {
        let camera = RenderView::perspective(1, 160, 90, 60., 0.1, 50.);
        let corners = frustum_corners(&camera, CameraPose::default(), 0.1, 50.).unwrap();
        let direction = normalize([1., -2., -3.]);
        let (first, _, _) = directional_pose(corners, direction, &[], [0.; 3]);
        let offset = [100., 0., 0.];
        let relative = corners.map(|p| std::array::from_fn(|i| p[i] - offset[i]));
        let (rebased, _, _) = directional_pose(relative, direction, &[], offset.map(f64::from));
        for axis in 0..3 {
            assert!(
                (first.position[axis] - rebased.position[axis] - offset[axis]).abs() < 0.0001,
                "origin rebasing moved a stationary light grid: {:?} {:?}",
                first.position,
                rebased.position
            );
        }
    }
    #[test]
    fn light_rotation_points_negative_z_along_all_axis_and_oblique_directions() {
        for direction in [
            [0., 0., -1.],
            [0., 1., 0.],
            [0., -1., 0.],
            normalize([1., 2., 3.]),
        ] {
            let pose = CameraPose {
                rotation: rotation(basis(direction)),
                ..Default::default()
            };
            let view = RenderView::perspective(1, 64, 64, 60., 0.1, 10.)
                .evaluate(pose)
                .unwrap();
            let v = transform_point(
                view.view_matrix.map(OrderedF32::to_f32),
                [direction[0], direction[1], direction[2], 0.],
            );
            for (a, b) in v.into_iter().zip([0., 0., -1., 0.]) {
                assert!((a - b).abs() < 2e-6, "{direction:?}: {v:?}");
            }
        }
    }
    #[test]
    fn cascade_fit_contains_receivers_and_camera_external_upstream_caster_and_snaps_xy() {
        for orthographic in [false, true] {
            let mut camera = RenderView::perspective(1, 160, 90, 60., 0.1, 50.);
            if orthographic {
                camera.projection = Projection::Orthographic {
                    size: 10.,
                    near: 0.1,
                    far: 50.,
                };
            }
            let corners = frustum_corners(&camera, CameraPose::default(), 0.1, 50.).unwrap();
            // Behind the game camera, but upstream of the receiving frustum in this light.
            let caster = std::array::from_fn(|i| {
                [
                    if i & 1 == 0 { -0.5 } else { 0.5 },
                    if i & 2 == 0 { -0.5 } else { 0.5 },
                    if i & 4 == 0 { 99.5 } else { 100.5 },
                ]
            });
            let projected = project_casters(&[caster], [0., 0., -1.]);
            let (pose, size, far) = directional_pose(corners, [0., 0., -1.], &projected, [0.; 3]);
            let mut light = RenderView::perspective(2, 2048, 2048, 60., 0.05, far);
            light.projection = Projection::Orthographic {
                size,
                near: 0.05,
                far,
            };
            let matrix = light.evaluate(pose).unwrap().view_projection();
            for p in corners.into_iter().chain(caster) {
                let clip = transform_point(matrix, [p[0], p[1], p[2], 1.]);
                assert!(
                    clip[0].abs() <= 1.00001
                        && clip[1].abs() <= 1.00001
                        && (-1e-5..=1.00001).contains(&clip[2]),
                    "{orthographic}: {p:?} -> {clip:?}"
                );
            }
            let dx = size / 2048. * 0.1;
            let shifted = corners.map(|mut p| {
                p[0] += dx;
                p
            });
            let (next, _, _) = directional_pose(shifted, [0., 0., -1.], &projected, [0.; 3]);
            assert!(
                (next.position[0] - pose.position[0]).abs() < 1e-6,
                "subtexel camera motion moved snapped shadow XY"
            );
        }
    }
}

#[cfg(test)]
mod dynamic_consumption_tests {
    use super::*;
    #[test]
    fn dynamic_consumption_receiver_checks_current_camera_once_and_reuses_static_value_proof() {
        let mut rows = Vec::new();
        for count in [4, 32, 256] {
            let (manager, source, view, _) = tests::receiver_scene(MaterialBlend::Opaque, -100.);
            let instances: Vec<_> = (0..count)
                .map(|id| GeometryInstance {
                    instance_id: id as u64,
                    source: &source,
                    world_transform: {
                        let mut m = IDENTITY_MATRIX;
                        m[14] = -100.;
                        m
                    },
                    visibility: Visibility {
                        authored: true,
                        inherited: Evaluation::Known(true),
                    },
                    layers: LayerSelection::Named { names: vec![] },
                    provenance: source.geometry_source.as_ref().clone(),
                })
                .collect();
            let prepared;
            let mut inputs =
                build_inputs::GeometryBuildInputs::new(&instances, &manager, None).unwrap();
            inputs.enable_compact().unwrap();
            prepared = inputs.freeze();
            inputs.prepared = Some(&prepared);
            inputs
                .build(
                    "main",
                    [64, 64],
                    &[GeometryViewInput {
                        view: &view,
                        pose: CameraPose::default(),
                    }],
                    DiagnosticLevel::Off,
                )
                .unwrap();
            proof::INSTANCE_PROOF_LOOKUPS.with(|n| n.set(0));
            let frame = inputs
                .build(
                    "main",
                    [64, 64],
                    &[GeometryViewInput {
                        view: &view,
                        pose: CameraPose {
                            position: [0., 0., 2.],
                            ..Default::default()
                        },
                    }],
                    DiagnosticLevel::Off,
                )
                .unwrap();
            let lookups = proof::INSTANCE_PROOF_LOOKUPS.with(|n| n.get());
            workset::RECEIVER_FINITE_COMPONENTS.with(|n| n.set(0));
            let actual = receiver_depth_ranges_prepared(
                &frame,
                view.view_id,
                inputs.instances(),
                Some(&inputs.compact_indices),
                Some(&prepared),
            )
            .unwrap();
            let components = workset::RECEIVER_FINITE_COMPONENTS.with(|n| n.get());
            let expected = receiver_depth_ranges_inputs(
                &frame,
                view.view_id,
                inputs.instances(),
                Some(&inputs.compact_indices),
            )
            .unwrap();
            assert_eq!(actual, expected);
            assert_eq!(actual.len(), count);
            rows.push((count, components, lookups));
        }
        eprintln!("current receiver finite components and instance proof lookups {rows:?}");
        assert!(
            rows.iter().all(|(_, components, _)| *components == 16),
            "per-row current-camera checks {rows:?}"
        );
        assert!(
            rows.iter().all(|(_, _, lookups)| *lookups == 0),
            "prepared table repeated per-row proof lookups {rows:?}"
        );
    }
}
