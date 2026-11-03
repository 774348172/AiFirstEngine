//! Bounded immutable per-view Clustered Forward+ lists. CPU bins; GPU consumes the same list.
use super::*;
use crate::render_projection_adapter::LightProjection;
use crate::spatial_contract::LightKind;
pub const LIGHT_GRID: [u32; 3] = [16, 9, 24];
pub const MAX_VIEW_LIGHTS: usize = 256;
const CELLS: usize = 16 * 9 * 24;
const HEADER: usize = 20;
#[cfg(test)]
thread_local! {
    pub(super) static DYNAMIC_LIGHT_PLANES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static DYNAMIC_LIGHT_TESTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static LIGHT_LIST_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClusteredLights {
    #[serde(skip)]
    content_validation: proof::ContentValidation<Self>,
    view: Arc<ExecutableRenderView>,
    words: Vec<u32>,
    identity: String,
}
impl ClusteredLights {
    pub fn build(
        view: &ExecutableRenderView,
        lights: &[LightProjection],
    ) -> Result<Arc<Self>, ContractError> {
        #[cfg(test)]
        LIGHT_LIST_BUILDS.with(|n| n.set(n.get() + 1));
        if lights.len() > MAX_VIEW_LIGHTS {
            return Err(invalid("light.view_capacity"));
        }
        view.validate_extent(view.target_extent[0], view.target_extent[1])?;
        let vp = view.view_projection();
        let row = |i: usize| [vp[i], vp[i + 4], vp[i + 8], vp[i + 12]].map(f64::from);
        let rows = [row(0), row(1), row(2), row(3)];
        let mut words = vec![0; HEADER + lights.len() * 16 + CELLS * 8];
        for (dst, v) in words.iter_mut().zip(vp) {
            *dst = v.to_bits();
        }
        words[16] = lights.len() as u32;
        words[17] = (HEADER + lights.len() * 16) as u32;
        for (i, l) in lights.iter().enumerate() {
            l.component.validate().map_err(|_| invalid("light.value"))?;
            if l.position
                .iter()
                .chain(&l.direction)
                .any(|v| !v.is_finite())
            {
                return Err(invalid("light.pose"));
            }
            let norm = l.direction.iter().map(|v| v * v).sum::<f32>();
            if (norm - 1.).abs() > 1e-4 {
                return Err(invalid("light.direction"));
            }
            let (kind, intensity, range, inner, outer) = match l.component.kind {
                LightKind::Directional { illuminance_lux } => (0., illuminance_lux, 0., 0., 0.),
                LightKind::Point {
                    intensity_candela,
                    range_m,
                } => (1., intensity_candela, range_m, 0., 0.),
                LightKind::Spot {
                    intensity_candela,
                    range_m,
                    inner_half_angle_degrees,
                    outer_half_angle_degrees,
                } => (
                    2.,
                    intensity_candela,
                    range_m,
                    inner_half_angle_degrees.to_radians().cos(),
                    outer_half_angle_degrees.to_radians().cos(),
                ),
            };
            let rgb = l.component.color.map(|v| v * intensity);
            let data = [
                l.position[0],
                l.position[1],
                l.position[2],
                kind,
                l.direction[0],
                l.direction[1],
                l.direction[2],
                range,
                rgb[0],
                rgb[1],
                rgb[2],
                inner,
                outer,
                0.,
                0.,
                0.,
            ];
            for (dst, v) in words[HEADER + i * 16..][..16].iter_mut().zip(data) {
                *dst = v.to_bits();
            }
        }
        // A cell is the conjunction of three independent axis intervals.
        // Keep the original f64 expressions and tolerance, but evaluate each
        // axis plane once, rather than once for every Cartesian-product cell.
        type Plane = ([f64; 4], f64);
        let axes: [Vec<[Plane; 2]>; 3] = std::array::from_fn(|axis| {
            (0..LIGHT_GRID[axis])
                .map(|bin| {
                    let bounds = match axis {
                        0 => [
                            -1. + 2. * f64::from(bin) / 16.,
                            -1. + 2. * f64::from(bin + 1) / 16.,
                        ],
                        1 => [
                            1. - 2. * f64::from(bin + 1) / 9.,
                            1. - 2. * f64::from(bin) / 9.,
                        ],
                        _ => [f64::from(bin) / 24., f64::from(bin + 1) / 24.],
                    };
                    std::array::from_fn(|side| {
                        let b = bounds[side];
                        let sign = if side == 0 { 1. } else { -1. };
                        let plane = std::array::from_fn::<_, 4, _>(|j| {
                            sign * (rows[axis][j] - b * rows[3][j])
                        });
                        #[cfg(test)]
                        DYNAMIC_LIGHT_PLANES.with(|n| n.set(n.get() + 1));
                        let norm = (0..3).map(|j| plane[j] * plane[j]).sum::<f64>().sqrt();
                        (plane, norm)
                    })
                })
                .collect()
        });
        let mut directional = [0u32; 8];
        for (i, light) in lights.iter().enumerate() {
            if matches!(light.component.kind, LightKind::Directional { .. }) {
                directional[i / 32] |= 1 << (i % 32);
            }
        }
        let cells = &mut words[HEADER + lights.len() * 16..];
        for cell in cells.chunks_exact_mut(8) {
            cell.copy_from_slice(&directional);
        }
        for (i, light) in lights.iter().enumerate() {
            let range = match light.component.kind {
                LightKind::Directional { .. } => continue,
                LightKind::Point { range_m, .. } | LightKind::Spot { range_m, .. } => {
                    f64::from(range_m)
                }
            };
            let masks: [u32; 3] = std::array::from_fn(|axis| {
                axes[axis]
                    .iter()
                    .enumerate()
                    .fold(0, |mask, (bin, planes)| {
                        let intersects = planes.iter().all(|(plane, norm)| {
                            #[cfg(test)]
                            DYNAMIC_LIGHT_TESTS.with(|n| n.set(n.get() + 1));
                            let dist = plane[3]
                                + (0..3)
                                    .map(|j| plane[j] * f64::from(light.position[j]))
                                    .sum::<f64>();
                            dist >= -range * norm - 1e-5 * norm
                        });
                        mask | (u32::from(intersects) << bin)
                    })
            });
            // Walk only admitted axis bins; every original cell bit is exact.
            let mut zs = masks[2];
            while zs != 0 {
                let z = zs.trailing_zeros() as usize;
                zs &= zs - 1;
                let mut ys = masks[1];
                while ys != 0 {
                    let y = ys.trailing_zeros() as usize;
                    ys &= ys - 1;
                    let mut xs = masks[0];
                    while xs != 0 {
                        let x = xs.trailing_zeros() as usize;
                        xs &= xs - 1;
                        cells[(z * 9 * 16 + y * 16 + x) * 8 + i / 32] |= 1 << (i % 32);
                    }
                }
            }
        }
        let identity = crate::canonical_digest::sha256_hex(
            &words
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<_>>(),
        );
        Ok(Arc::new(Self {
            content_validation: Default::default(),
            view: Arc::new(view.clone()),
            words,
            identity,
        }))
    }
    pub fn light_count(&self) -> usize {
        self.words.get(16).copied().unwrap_or(0) as usize
    }
    pub fn max_cluster_lights(&self) -> u32 {
        self.words
            .get(HEADER + self.light_count().min(MAX_VIEW_LIGHTS) * 16..)
            .unwrap_or(&[])
            .chunks_exact(8)
            .map(|v| v.iter().map(|v| v.count_ones()).sum())
            .max()
            .unwrap_or(0)
    }
    pub fn cluster_lights(&self, cell: usize) -> Option<Vec<u32>> {
        if cell >= CELLS || self.light_count() > MAX_VIEW_LIGHTS {
            return None;
        }
        let base = HEADER + self.light_count() * 16 + cell * 8;
        let words = self.words.get(base..base + 8)?;
        let mut result = Vec::new();
        for (w, word) in words.iter().enumerate() {
            for b in 0..32 {
                if word & (1 << b) != 0 {
                    result.push((w * 32 + b) as u32);
                }
            }
        }
        Some(result)
    }
    #[doc(hidden)]
    pub fn words(&self) -> &[u32] {
        &self.words
    }
    #[doc(hidden)]
    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub(crate) fn matches_view(&self, view: &ExecutableRenderView) -> bool {
        self.view.as_ref() == view
    }
    pub(crate) fn validate_shared(
        self: &Arc<Self>,
        view: &ExecutableRenderView,
    ) -> Result<(), ContractError> {
        if !self.matches_view(view) {
            return Err(invalid("light.view_identity"));
        }
        self.content_validation
            .validate(self, || self.validate(&self.view))
    }
    pub(crate) fn validate(&self, view: &ExecutableRenderView) -> Result<(), ContractError> {
        if self.view.as_ref() != view
            || self.words.len() < HEADER
            || self.light_count() > MAX_VIEW_LIGHTS
        {
            return Err(invalid("light.view_identity"));
        }
        let list = HEADER + self.light_count() * 16 + CELLS * 8;
        if self.words.len() != list
            || self.words[17] as usize != HEADER + self.light_count() * 16
            || self.words[18..20] != [0; 2]
        {
            return Err(invalid("light.list_capacity"));
        }
        if self.words[..16] != view.view_projection().map(f32::to_bits) {
            return Err(invalid("light.matrix"));
        }
        for record in self.words[HEADER..HEADER + self.light_count() * 16].chunks_exact(16) {
            if record.iter().any(|v| !f32::from_bits(*v).is_finite())
                || ![0., 1., 2.].contains(&f32::from_bits(record[3]))
                || record[8..11].iter().any(|v| f32::from_bits(*v) < 0.)
            {
                return Err(invalid("light.record"));
            }
            let f = |i| f32::from_bits(record[i]);
            let norm = (4..7).map(|i| f(i) * f(i)).sum::<f32>();
            if (norm - 1.).abs() > 1e-4
                || (f(3) > 0.5 && (f(7) <= 0. || f(7) > 32768.))
                || (f(3) > 1.5 && (f(11) <= f(12) || f(11) > 1. || f(12) <= 0.))
                || record[13..16] != [0; 3]
            {
                return Err(invalid("light.record"));
            }
        }
        for cell in self.words[HEADER + self.light_count() * 16..].chunks_exact(8) {
            for (w, mask) in cell.iter().enumerate() {
                let remaining = self.light_count().saturating_sub(w * 32).min(32);
                let allowed = if remaining == 32 {
                    u32::MAX
                } else {
                    (1u32 << remaining) - 1
                };
                if *mask & !allowed != 0 {
                    return Err(invalid("light.cluster_range"));
                }
            }
        }
        #[cfg(test)]
        CONTENT_HASH_WORDS.fetch_add(self.words.len(), std::sync::atomic::Ordering::Relaxed);
        let actual = crate::canonical_digest::sha256_hex(
            &self
                .words
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<_>>(),
        );
        if actual != self.identity {
            return Err(invalid("light.content_identity"));
        }
        Ok(())
    }
    #[doc(hidden)]
    pub fn observe_memory(&self, s: &mut crate::resource_memory::ResourceMemorySample, role: &str) {
        s.cpu_vec(&self.words, "snapshots", &[role]);
        s.cpu_string(&self.identity, "snapshots", &[role]);
    }
}
impl GeometryFrame {
    /// Attach one frozen light list per view; never mutate an older frame's Arc draws.
    pub fn set_view_lights(
        &mut self,
        view_id: u64,
        lights: &[LightProjection],
    ) -> Result<(), ContractError> {
        let view = self
            .views
            .iter_mut()
            .find(|v| v.view.view_id == view_id)
            .ok_or_else(|| invalid("light.view_missing"))?;
        let list = ClusteredLights::build(&view.view, lights)?;
        for draw in &mut view.draws {
            if draw.standard_lit.is_some() {
                Arc::make_mut(draw).lighting = Some(list.clone());
            }
        }
        view.refresh_batch_draws();
        Ok(())
    }
}

#[cfg(test)]
mod dynamic_consumption_tests {
    use super::*;
    fn point_lights(count: usize) -> (ExecutableRenderView, Vec<LightProjection>) {
        let (_, _, view, light) = shadow::receiver_scene(MaterialBlend::Opaque, -100.);
        let lights = (0..count)
            .map(|i| {
                let mut l = light.clone();
                l.component.cast_shadows = false;
                l.component.kind = LightKind::Point {
                    intensity_candela: 10.,
                    range_m: 10. + (i % 11) as f32 * 3.,
                };
                l.position = [
                    (i % 13) as f32 * 7. - 40.,
                    (i % 7) as f32 * 5. - 15.,
                    -3. - (i % 17) as f32 * 20.,
                ];
                l
            })
            .collect();
        (view.evaluate(CameraPose::default()).unwrap(), lights)
    }
    #[test]
    fn dynamic_cluster_axis_work_is_bounded_at_multiple_scales() {
        let mut rows = Vec::new();
        for count in [4, 32, 256] {
            let (view, lights) = point_lights(count);
            DYNAMIC_LIGHT_PLANES.with(|n| n.set(0));
            DYNAMIC_LIGHT_TESTS.with(|n| n.set(0));
            let built = ClusteredLights::build(&view, &lights).unwrap();
            built.validate(&view).unwrap();
            rows.push((
                count,
                DYNAMIC_LIGHT_PLANES.with(|n| n.get()),
                DYNAMIC_LIGHT_TESTS.with(|n| n.get()),
            ));
        }
        eprintln!("dynamic light actual plane constructions/tests {rows:?}");
        assert!(
            rows.iter().all(|&(n, p, t)| p <= 98 && t <= 98 * n),
            "cell product repeated axis work: {rows:?}"
        );
    }
    fn cell_reference(view: &ExecutableRenderView, lights: &[LightProjection]) -> Vec<u32> {
        let vp = view.view_projection();
        let row = |i: usize| [vp[i], vp[i + 4], vp[i + 8], vp[i + 12]].map(f64::from);
        let rows = [row(0), row(1), row(2), row(3)];
        let mut words = vec![0; CELLS * 8];
        for z in 0..24 {
            for y in 0..9 {
                for x in 0..16 {
                    let bounds = [
                        [
                            -1. + 2. * f64::from(x) / 16.,
                            -1. + 2. * f64::from(x + 1) / 16.,
                        ],
                        [1. - 2. * f64::from(y + 1) / 9., 1. - 2. * f64::from(y) / 9.],
                        [f64::from(z) / 24., f64::from(z + 1) / 24.],
                    ];
                    let cell = (z * 9 * 16 + y * 16 + x) as usize;
                    let slot = cell * 8;
                    // Cell planes are independent of the light. Compute once per cell,
                    // rather than rebuilding six planes and square roots per light.
                    let planes = std::array::from_fn::<_, 6, _>(|i| {
                        let axis = i / 2;
                        let side = i % 2;
                        let b = bounds[axis][side];
                        let sign = if side == 0 { 1. } else { -1. };
                        let plane = std::array::from_fn::<_, 4, _>(|j| {
                            sign * (rows[axis][j] - b * rows[3][j])
                        });
                        let norm = (0..3).map(|j| plane[j] * plane[j]).sum::<f64>().sqrt();
                        (plane, norm)
                    });
                    for (i, l) in lights.iter().enumerate() {
                        let range = match l.component.kind {
                            LightKind::Directional { .. } => {
                                words[slot + i / 32] |= 1 << (i % 32);
                                continue;
                            }
                            LightKind::Point { range_m, .. } | LightKind::Spot { range_m, .. } => {
                                f64::from(range_m)
                            }
                        };
                        let intersects = planes.iter().all(|(plane, norm)| {
                            let dist = plane[3]
                                + (0..3)
                                    .map(|j| plane[j] * f64::from(l.position[j]))
                                    .sum::<f64>();
                            dist >= -range * norm - 1e-5 * norm
                        });
                        if intersects {
                            words[slot + i / 32] |= 1 << (i % 32);
                        }
                    }
                }
            }
        }

        words
    }
    #[test]
    fn dynamic_cluster_words_match_independent_cell_reference() {
        for orthographic in [false, true] {
            for case in 0..6 {
                let (_, mut lights) = point_lights(if case == 5 { 256 } else { 32 });
                let angle = case as f32 * 0.17;
                let pose = CameraPose {
                    position: [case as f32 * 2., -3., 1.],
                    rotation: [0., angle.sin(), 0., angle.cos()],
                };
                // Use a complete current camera, including inverse view translation/rotation.
                let (_, _, mut source, _) = shadow::receiver_scene(MaterialBlend::Opaque, -100.);
                if orthographic {
                    source.projection = Projection::Orthographic {
                        size: 300.,
                        near: 0.1,
                        far: 2000.,
                    };
                }
                let view = source.evaluate(pose).unwrap();
                for (i, l) in lights.iter_mut().enumerate() {
                    if i % 7 == 0 {
                        l.component.kind = LightKind::Directional {
                            illuminance_lux: 20.,
                        };
                    } else if i % 5 == 0 {
                        l.component.kind = LightKind::Spot {
                            intensity_candela: 20.,
                            range_m: 0.001 + (i as f32) * 5.,
                            inner_half_angle_degrees: 15.,
                            outer_half_angle_degrees: 30.,
                        };
                    }
                }
                let actual = ClusteredLights::build(&view, &lights).unwrap();
                actual.validate(&view).unwrap();
                assert_eq!(
                    &actual.words()[HEADER + lights.len() * 16..],
                    cell_reference(&view, &lights),
                    "ortho={orthographic} case={case}"
                );
            }
        }
    }
}

#[cfg(test)]
static CONTENT_HASH_WORDS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
mod shared_content_tests {
    use super::*;
    #[test]
    fn shared_light_content_hashes_once_without_borrowing_another_receiver_proof() {
        for count in [4, 32, 256] {
            let view = RenderView::perspective(5, 64, 64, 60., 0.1, 200.)
                .evaluate(CameraPose::default())
                .unwrap();
            let list = ClusteredLights::build(&view, &[]).unwrap();
            CONTENT_HASH_WORDS.store(0, std::sync::atomic::Ordering::Relaxed);
            std::thread::scope(|scope| {
                for _ in 0..4 {
                    let list = &list;
                    let view = &view;
                    scope.spawn(move || {
                        for _ in 0..count {
                            list.validate_shared(view).unwrap();
                        }
                    });
                }
            });
            let actual = CONTENT_HASH_WORDS.load(std::sync::atomic::Ordering::Relaxed);
            eprintln!("shared lights scale={count} actual hashed words={actual}");
            assert_eq!(
                actual,
                list.words.len(),
                "immutable light content hash repeated across tasks"
            );
            let mut other = view.clone();
            other.view_matrix[12] = 0.25f32.into();
            assert_eq!(
                list.validate_shared(&other).unwrap_err(),
                list.validate(&other).unwrap_err()
            );
            let mut changed = list.clone();
            Arc::make_mut(&mut changed).words[HEADER] = u32::MAX;
            assert_eq!(
                changed.validate_shared(&view).unwrap_err(),
                changed.validate(&view).unwrap_err()
            );
            let wire = serde_json::to_value(&list).unwrap();
            let decoded: Arc<ClusteredLights> = Arc::new(serde_json::from_value(wire).unwrap());
            CONTENT_HASH_WORDS.store(0, std::sync::atomic::Ordering::Relaxed);
            decoded.validate_shared(&view).unwrap();
            assert_eq!(
                CONTENT_HASH_WORDS.load(std::sync::atomic::Ordering::Relaxed),
                list.words.len(),
                "wire inherited a private content proof"
            );
        }
    }
}
