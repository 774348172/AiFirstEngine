//! Ordered visible indices into immutable instance/resource backing.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeometryVisibleInstance {
    pub instance_index: u32,
    pub camera_depth: OrderedF32,
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    #[test]
    fn retained_complete_batch_success_crosses_threads_but_cow_tail_does_not() {
        for count in [4, 32, 256] {
            let frame = batch_test_frame(true, MaterialBlend::Opaque, count, 1);
            let batch = frame.views[0].batches.as_ref().unwrap()[0].clone();
            batch
                .validate_shared_with_view(&mut DrawViewValidation::default())
                .unwrap();
            std::thread::spawn(move || {
                SELECTION_VALIDATIONS.with(|n| n.set(0));
                batch
                    .validate_shared_with_view(&mut DrawViewValidation::default())
                    .unwrap();
                assert_eq!(
                    SELECTION_VALIDATIONS.with(|n| n.get()),
                    0,
                    "complete immutable {count}-row batch repeated validation after task handoff"
                );
                let mut changed = batch.clone();
                Arc::make_mut(&mut changed)
                    .visible
                    .last_mut()
                    .unwrap()
                    .instance_index = u32::MAX;
                assert_eq!(
                    changed
                        .validate_shared_with_view(&mut DrawViewValidation::default())
                        .unwrap_err()
                        .field,
                    "geometry.batch.instance_index"
                );
            })
            .join()
            .unwrap();
        }
    }

    #[test]
    fn consumption_finite_bound_preserves_extreme_fallback_and_strict_oracle() {
        let mut seed = 371u64;
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((seed >> 32) as u32 as f64 / u32::MAX as f64 * 2. - 1.) as f32
        };
        let mut proven = 0;
        let mut fallback = 0;
        for i in 0..4096 {
            let scales = [1., 1e10, 1e20, f32::MAX];
            let world: [f32; 16] = std::array::from_fn(|_| next() * scales[i % 4]);
            let projection: [f32; 16] = std::array::from_fn(|_| next() * scales[(i / 4) % 4]);
            let bound = world.iter().map(|v| f64::from(*v).abs()).fold(0., f64::max);
            if product_is_finite(projection, bound) {
                proven += 1;
                assert!(
                    multiply_matrix(projection, world)
                        .iter()
                        .all(|v| v.is_finite()),
                    "finite proof admitted overflow case {i}"
                );
            } else {
                fallback += 1;
            }
        }
        assert!(proven > 100 && fallback > 100);
        let mut invalid = IDENTITY_MATRIX;
        invalid[0] = f32::NAN;
        assert!(!product_is_finite(invalid, 1.));
        assert!(!product_is_finite(IDENTITY_MATRIX, f64::INFINITY));
        let frame = batch_test_frame(true, MaterialBlend::Opaque, 32, 1);
        let mut batch = frame.views[0].batches.as_ref().unwrap()[0].as_ref().clone();
        let first = batch.visible[0].instance_index as usize;
        Arc::make_mut(&mut Arc::make_mut(&mut batch.instances)[first]).world_transform[0] =
            2f32.into();
        Arc::make_mut(&mut batch.draw).instance = batch.instances[first].clone();
        let manager = RenderResourceManager::new();
        let mut inputs = build_inputs::GeometryBuildInputs::new(&[], &manager, None).unwrap();
        let phase = PhaseCache::default()
            .resolve(
                &batch.draw,
                &batch.instances,
                &vec![MaterialDepth::TestAndWrite; 32],
                &mut inputs,
            )
            .unwrap();
        Arc::make_mut(&mut Arc::make_mut(&mut batch.draw).view).projection_matrix[0] =
            f32::MAX.into();
        SELECTION_MATRIX_PRODUCTS.with(|n| n.set(0));
        let fast = batch
            .validate_prepared_selection(&mut DrawViewValidation::default(), &phase)
            .unwrap_err();
        assert!(
            SELECTION_MATRIX_PRODUCTS.with(|n| n.get()) > 0,
            "extreme projection skipped strict fallback"
        );
        let strict = batch
            .validate_selection_with_view(&mut DrawViewValidation::default())
            .unwrap_err();
        assert_eq!(fast, strict);
    }

    #[test]
    fn consumption_prepared_finite_bound_avoids_matrix_products_at_multiple_scales() {
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            let frame = batch_test_frame(true, MaterialBlend::Opaque, count, 1);
            let batches = frame.views[0].batches.as_ref().unwrap();
            let manager = RenderResourceManager::new();
            let mut inputs = build_inputs::GeometryBuildInputs::new(&[], &manager, None).unwrap();
            let batch = &batches[0];
            let phase = PhaseCache::default()
                .resolve(
                    &batch.draw,
                    &batch.instances,
                    &vec![MaterialDepth::TestAndWrite; count],
                    &mut inputs,
                )
                .unwrap();
            SELECTION_MATRIX_PRODUCTS.with(|n| n.set(0));
            for batch in batches {
                batch
                    .validate_prepared_selection(&mut DrawViewValidation::default(), &phase)
                    .unwrap();
            }
            actual.push((count, SELECTION_MATRIX_PRODUCTS.with(|n| n.get())));
        }
        assert!(
            actual.iter().all(|(_, products)| *products == 0),
            "actual full matrix products: {actual:?}"
        );
    }

    #[test]
    fn prepared_head_reuses_only_static_facts_and_public_values_remain_strict() {
        let frame = batch_test_frame(true, MaterialBlend::Opaque, 2, 1);
        let batch = &frame.views[0].batches.as_ref().unwrap()[0];
        let manager = RenderResourceManager::new();
        let mut inputs = build_inputs::GeometryBuildInputs::new(&[], &manager, None).unwrap();
        let prepared = PhaseCache::default()
            .resolve(
                &batch.draw,
                &batch.instances,
                &vec![MaterialDepth::TestAndWrite; batch.instances.len()],
                &mut inputs,
            )
            .unwrap();
        let draw = Arc::new(batch.draw.as_ref().clone());
        RenderBindingSet::take_validation_count_for_test();
        draw.validate_shared_with_view(&mut DrawViewValidation::default())
            .unwrap();
        assert_eq!(RenderBindingSet::take_validation_count_for_test(), 0);
        draw.validate().unwrap();
        assert!(
            RenderBindingSet::take_validation_count_for_test() > 0,
            "public value validation stopped being strict"
        );
        for change in 0..7 {
            let mut changed = draw.as_ref().clone();
            match change {
                0 => changed.target = "".into(),
                1 => changed.camera_depth = f32::NAN.into(),
                2 => changed.depth_target = None,
                3 => Arc::make_mut(&mut changed.view).domain = RenderDomain::World2d,
                4 => Arc::make_mut(&mut changed.view).view_matrix[0] = 2f32.into(),
                5 => Arc::make_mut(&mut changed.view).projection_matrix[0] = f32::INFINITY.into(),
                _ => changed.base_color[0] = f32::NAN.into(),
            }
            let changed = Arc::new(changed);
            let strict = changed.validate().unwrap_err();
            let shared = changed
                .validate_shared_with_view(&mut DrawViewValidation::default())
                .unwrap_err();
            assert_eq!(
                strict, shared,
                "static proof hid changed execution input {change}"
            );
        }
        assert!(prepared.matches(batch));
    }

    #[test]
    fn prepared_static_table_keeps_dynamic_tail_strict_and_cow_untrusted() {
        let frame = batch_test_frame(true, MaterialBlend::Opaque, 128, 1);
        let original = frame.views[0].batches.as_ref().unwrap()[0].clone();
        let manager = RenderResourceManager::new();
        let mut inputs = build_inputs::GeometryBuildInputs::new(&[], &manager, None).unwrap();
        let depths = vec![MaterialDepth::TestAndWrite; original.instances.len()];
        let mut cache = PhaseCache::default();
        let prepared = cache
            .resolve(&original.draw, &original.instances, &depths, &mut inputs)
            .unwrap();
        assert!(prepared.matches(&original));
        let check = |batch: &Arc<GeometryBatch>| {
            let mut validation = DrawViewValidation::default();
            batch.draw.validate_shared_with_view(&mut validation)?;
            proof::prepared_selection(batch, &mut validation, &prepared)
        };
        for change in 0..7 {
            let mut batch = original.as_ref().clone();
            let last = batch.visible.len() - 1;
            match change {
                0 => batch.visible[last].instance_index = u32::MAX,
                1 => batch.visible[last].instance_index = batch.visible[0].instance_index,
                2 => batch.visible[last].camera_depth = f32::NAN.into(),
                3 => Arc::make_mut(&mut batch.draw).camera_depth = 99f32.into(),
                4 => {
                    let index = batch.visible[last].instance_index as usize;
                    Arc::make_mut(&mut Arc::make_mut(&mut batch.instances)[index])
                        .world_transform[0] = f32::NAN.into();
                }
                5 => {
                    let index = batch.visible[last].instance_index as usize;
                    Arc::make_mut(
                        &mut Arc::make_mut(&mut Arc::make_mut(&mut batch.instances)[index])
                            .material_binding,
                    )
                    .slots
                    .clear();
                }
                _ => {
                    let draw = Arc::make_mut(&mut batch.draw);
                    Arc::make_mut(&mut draw.view).projection_matrix[0] = f32::INFINITY.into();
                }
            }
            let batch = Arc::new(batch);
            assert!(
                batch.validate().is_err(),
                "strict oracle accepted change {change}"
            );
            assert!(
                check(&batch).is_err(),
                "prepared proof hid dynamic/COW change {change}"
            );
            assert!(
                check(&batch).is_err(),
                "failed proof propagated change {change}"
            );
        }
        // A legal new subset still needs full dynamic validation, and old
        // public/wire input remains valid without a preparation proof.
        let mut subset = original.as_ref().clone();
        subset.visible.truncate(2);
        let subset = Arc::new(subset);
        check(&subset).unwrap();
        let wire: GeometryBatch =
            serde_json::from_slice(&serde_json::to_vec(&subset).unwrap()).unwrap();
        assert!(!prepared.matches(&wire));
        wire.validate().unwrap();
        original.validate().unwrap();
    }

    #[test]
    fn hidden_invalid_rows_fall_back_without_proving_partial_table_or_changing_selection_errors() {
        let frame = batch_test_frame(true, MaterialBlend::Opaque, 128, 1);
        let mut batch = frame.views[0].batches.as_ref().unwrap()[0].as_ref().clone();
        batch.visible.truncate(1);
        Arc::make_mut(
            &mut Arc::make_mut(&mut Arc::make_mut(&mut batch.instances)[127]).material_binding,
        )
        .slots
        .clear();
        batch.validate().unwrap();
        let manager = RenderResourceManager::new();
        let mut inputs = build_inputs::GeometryBuildInputs::new(&[], &manager, None).unwrap();
        let mut cache = PhaseCache::default();
        assert!(cache
            .resolve(
                &batch.draw,
                &batch.instances,
                &vec![MaterialDepth::TestAndWrite; 128],
                &mut inputs
            )
            .is_none());
        assert!(
            cache.entries.is_empty(),
            "partial static success was published"
        );
        batch.visible.push(GeometryVisibleInstance {
            instance_index: 127,
            camera_depth: 1f32.into(),
        });
        assert!(
            batch.validate().is_err(),
            "newly selected invalid tail was hidden"
        );
    }

    #[test]
    fn persistent_phase_members_keep_complete_context_and_cow_table_identity() {
        let frame = mixed_phase_frame(true, false, false, false);
        let template = frame.views[0].draws[0].as_ref().clone();
        let table = frame.views[0].batches.as_ref().unwrap()[0]
            .instances
            .clone();
        let depths = vec![MaterialDepth::TestAndWrite; table.len()];
        let manager = RenderResourceManager::new();
        let mut inputs = build_inputs::GeometryBuildInputs::new(&[], &manager, None).unwrap();
        let mut cache = PhaseCache::default();
        let old = cache
            .resolve(&template, &table, &depths, &mut inputs)
            .unwrap();
        COMPACT_STATE_RESOLUTIONS.with(|n| n.set(0));
        assert!(Arc::ptr_eq(
            &old,
            &cache
                .resolve(&template, &table, &depths, &mut inputs)
                .unwrap()
        ));
        assert_eq!(COMPACT_STATE_RESOLUTIONS.with(|n| n.get()), 0);
        let mut shadow = template.clone();
        let view = Arc::make_mut(&mut shadow.view);
        view.view_kind = ViewKind::Shadow;
        view.target_kind = TargetKind::ShadowMap;
        view.depth_policy = DepthPolicy::TestAndWrite;
        view.color_pipeline = None;
        shadow.depth = MaterialDepth::TestAndWrite;
        shadow.depth_target = Some(shadow.target.clone());
        shadow.environment = None;
        shadow.lighting = None;
        shadow.shadows = None;
        let ranks = cache
            .resolve(&shadow, &table, &depths, &mut inputs)
            .unwrap();
        assert!(
            COMPACT_STATE_RESOLUTIONS.with(|n| n.get()) > 0,
            "shadow context borrowed color ranks"
        );
        let mut expected: Vec<_> = table
            .iter()
            .map(|row| {
                let mut draw = shadow.clone();
                draw.instance = row.clone();
                draw.camera_depth = (row.instance_id as f32 * 0.25).into();
                Arc::new(draw)
            })
            .collect();
        batch::sort_opaque_segment(&mut expected, &mut batch::IdentityMemo::default());
        let mut actual: Vec<_> = (0..table.len()).collect();
        actual.sort_by_key(|i| {
            (
                ranks[*i].rank,
                DepthOrder((table[*i].instance_id as f32 * 0.25).into()),
                table[*i].instance_id,
            )
        });
        assert_eq!(
            actual
                .iter()
                .map(|i| table[*i].instance_id)
                .collect::<Vec<_>>(),
            expected.iter().map(|d| d.instance_id).collect::<Vec<_>>()
        );
        let mut changed = table.clone();
        Arc::make_mut(&mut Arc::make_mut(&mut changed)[0]).cull = MaterialCull::Front;
        COMPACT_STATE_RESOLUTIONS.with(|n| n.set(0));
        let new = cache
            .resolve(&shadow, &changed, &depths, &mut inputs)
            .unwrap();
        assert!(
            COMPACT_STATE_RESOLUTIONS.with(|n| n.get()) > 0,
            "COW table borrowed stale ranks"
        );
        assert!(!Arc::ptr_eq(&ranks, &new));
        assert_ne!(changed[0].cull, table[0].cull);
        assert_eq!(
            cache.entries.len(),
            1,
            "new table kept stale phase memberships"
        );
    }

    #[test]
    fn compact_views_consume_prepared_rows_without_repeating_instance_lookup() {
        build_inputs::INSTANCE_DATA_LOOKUPS.with(|n| n.set(0));
        let frame = batch_test_frame(true, MaterialBlend::Opaque, 128, 4);
        assert_eq!(frame.views.len(), 4);
        assert_eq!(
            frame
                .views
                .iter()
                .map(|v| v.selected_instances().count())
                .sum::<usize>(),
            512
        );
        assert_eq!(
            build_inputs::INSTANCE_DATA_LOOKUPS.with(|n| n.get()),
            128,
            "compact views looked up already prepared immutable rows again"
        );
        let flat = batch_test_frame(false, MaterialBlend::Opaque, 128, 4);
        for (compact, flat) in frame.views.iter().zip(&flat.views) {
            assert_eq!(
                compact.selected_draws().collect::<Vec<_>>(),
                flat.selected_draws().collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn compact_row_indices_keep_sparse_visibility_and_reordered_input_identity() {
        let (manager, source, mut view, _) = shadow::receiver_scene(MaterialBlend::Opaque, -2.);
        view.layers = LayerSelection::Named {
            names: vec!["main".into()],
        };
        let mut second = view.clone();
        second.view_id += 1;
        let views = [
            GeometryViewInput {
                view: &view,
                pose: CameraPose::default(),
            },
            GeometryViewInput {
                view: &second,
                pose: CameraPose::default(),
            },
        ];
        let mut instances: Vec<_> = (0..64)
            .map(|i| GeometryInstance {
                instance_id: ((i * 197) % 64) as u64,
                source: &source,
                world_transform: {
                    let mut m = IDENTITY_MATRIX;
                    m[14] = -2. - i as f32 * 0.01;
                    m
                },
                visibility: Visibility {
                    authored: true,
                    inherited: Evaluation::Known(i % 7 != 0),
                },
                layers: LayerSelection::Named {
                    names: vec![if i % 3 == 0 { "other" } else { "main" }.into()],
                },
                provenance: source.geometry_source.as_ref().clone(),
            })
            .collect();
        let lease = manager.lease_resources(source.resource_handles());
        let reuse = std::sync::Mutex::new(build_inputs::InstanceReuse::default());
        let mut retained = Vec::new();
        for reversed in [false, true] {
            if reversed {
                instances.reverse();
            }
            let build = |compact| {
                let mut inputs =
                    build_inputs::GeometryBuildInputs::new(&instances, &manager, Some(&lease))
                        .unwrap()
                        .with_instance_reuse(&reuse, [0; 3]);
                if compact {
                    inputs.enable_compact().unwrap();
                }
                inputs
                    .build("main", [64, 64], &views, DiagnosticLevel::Off)
                    .unwrap()
            };
            let flat = build(false);
            let compact = build(true);
            for (actual, reference) in compact.views.iter().zip(&flat.views) {
                assert!(!actual.draws.is_empty());
                assert!(actual.selected_instances().count() < 64);
                assert_eq!(
                    actual.selected_draws().collect::<Vec<_>>(),
                    reference.selected_draws().collect::<Vec<_>>()
                );
            }
            retained.push(compact);
        }
        assert_eq!(retained.len(), 2);
    }

    #[test]
    fn compact_state_order_compares_full_keys_per_unique_state_not_per_instance() {
        let frame = mixed_phase_frame(true, false, false, false);
        let template = frame.views[0].draws[0].as_ref();
        let table: Arc<Vec<_>> = Arc::new(
            (0..4096)
                .map(|i| {
                    let mut row = template.instance.as_ref().clone();
                    row.instance_id = ((i * 197) % 4096) as u64;
                    row.blend = MaterialBlend::Opaque;
                    row.alpha_cutoff = None;
                    row.cull = if i & 1 == 0 {
                        MaterialCull::None
                    } else {
                        MaterialCull::Back
                    };
                    row.world_transform[0] = if i & 2 == 0 { 1. } else { -1. }.into();
                    row.opaque_alpha = i & 4 != 0;
                    Arc::new(row)
                })
                .collect(),
        );
        let manager = RenderResourceManager::new();
        let mut inputs = build_inputs::GeometryBuildInputs::new(&[], &manager, None).unwrap();
        let mut compact = CompactView::new(table.clone());
        for (i, row) in table.iter().enumerate() {
            compact
                .push(
                    i as u32,
                    MaterialDepth::TestAndWrite,
                    (1. + row.instance_id as f32 * 0.001).into(),
                    || template.clone(),
                )
                .unwrap();
        }
        COMPACT_FULL_STATE_COMPARISONS.with(|n| n.set(0));
        let actual = compact
            .finish(&mut inputs, &FinalViewState::default())
            .unwrap();
        let comparisons = COMPACT_FULL_STATE_COMPARISONS.with(|n| n.get());
        let mut reference: Vec<_> = table
            .iter()
            .map(|row| {
                let mut draw = template.clone();
                draw.instance = row.clone();
                draw.camera_depth = (1. + row.instance_id as f32 * 0.001).into();
                Arc::new(draw)
            })
            .collect();
        batch::sort_opaque_segment(&mut reference, &mut batch::IdentityMemo::default());
        assert_eq!(
            actual
                .iter()
                .flat_map(|b| b
                    .visible
                    .iter()
                    .map(|v| table[v.instance_index as usize].instance_id))
                .collect::<Vec<_>>(),
            reference.iter().map(|d| d.instance_id).collect::<Vec<_>>()
        );
        assert!(
            comparisons <= 128,
            "8 complete states were compared {comparisons} times for 4096 instances"
        );
    }

    #[test]
    fn shadow_selections_share_successful_bindings_without_skipping_tail_validation() {
        let frame = batch_test_frame(true, MaterialBlend::Opaque, 128, 1);
        let original = &frame.views[0].batches.as_ref().unwrap()[0];
        let mut view = original.draw.view.as_ref().clone();
        view.view_kind = ViewKind::Shadow;
        view.target_kind = TargetKind::ShadowMap;
        view.clear_color = None;
        view.depth_clear = Some(1f32.into());
        let view = Arc::new(view);
        let split: Vec<_> = original
            .visible
            .iter()
            .map(|entry| {
                let mut draw = original.selected_draw(
                    &original.instances[entry.instance_index as usize],
                    entry.camera_depth,
                );
                draw.view = view.clone();
                draw.target = "shadow".into();
                draw.depth_target = Some("shadow".into());
                draw.lighting = None;
                draw.shadows = None;
                draw.environment = None;
                Arc::new(GeometryBatch {
                    draw: Arc::new(draw),
                    visible: vec![*entry].into(),
                    ..original.as_ref().clone()
                })
            })
            .collect();
        DRAW_BINDING_VALIDATIONS.with(|n| n.set(0));
        RenderBindingSet::take_validation_count_for_test();
        let mut validation = DrawViewValidation::default();
        for batch in &split {
            batch.validate_shared_with_view(&mut validation).unwrap();
        }
        let checks = DRAW_BINDING_VALIDATIONS.with(|n| n.get());
        assert_eq!(
            checks, 2,
            "immutable geometry/material binding checks restarted per selection"
        );
        assert_eq!(
            RenderBindingSet::take_validation_count_for_test(),
            2,
            "texture slot interpretation repeated strict binding checks after context validation"
        );
        let mut bad = split[127].as_ref().clone();
        bad.visible[0].instance_index = u32::MAX;
        let bad = Arc::new(bad);
        assert!(bad.validate_shared_with_view(&mut validation).is_err());
        let mut bad_tail = original.as_ref().clone();
        bad_tail.draw = split[0].draw.clone();
        let index = bad_tail.visible.last().unwrap().instance_index as usize;
        let tail = Arc::make_mut(&mut Arc::make_mut(&mut bad_tail.instances)[index]);
        Arc::make_mut(&mut tail.material_binding).slots.clear();
        let bad_tail = Arc::new(bad_tail);
        assert!(
            bad_tail.validate_shared_with_view(&mut validation).is_err(),
            "a new invalid tail borrowed the prior binding's successful identity"
        );
    }

    #[test]
    fn compact_sort_resolves_shared_resource_state_once_per_phase() {
        COMPACT_STATE_RESOLUTIONS.with(|n| n.set(0));
        let compact = mixed_phase_frame(true, false, false, false);
        assert_eq!(compact.views[0].draws.len(), 2);
        assert_eq!(
            COMPACT_STATE_RESOLUTIONS.with(|n| n.get()),
            2,
            "shared opaque/mask state resolved again for each selected instance"
        );
    }

    #[test]
    fn compact_shared_state_sort_matches_full_keys_for_same_resource_variants() {
        let frame = mixed_phase_frame(true, false, false, false);
        let template = frame.views[0].draws[0].as_ref();
        let table: Arc<Vec<_>> = Arc::new(
            (0..256)
                .map(|i| {
                    let mut row = template.instance.as_ref().clone();
                    row.instance_id = ((i * 197) % 256) as u64;
                    row.blend = if i & 1 == 0 {
                        MaterialBlend::Opaque
                    } else {
                        MaterialBlend::AlphaMask
                    };
                    row.alpha_cutoff = (row.blend == MaterialBlend::AlphaMask).then(|| 0.5.into());
                    row.opaque_alpha = row.blend == MaterialBlend::Opaque && i & 2 != 0;
                    row.ignore_vertex_color = i & 4 != 0;
                    row.cull = match i % 3 {
                        0 => MaterialCull::None,
                        1 => MaterialCull::Back,
                        _ => MaterialCull::Front,
                    };
                    row.world_transform[0] = if i & 8 == 0 { 1. } else { -1. }.into();
                    if row.blend == MaterialBlend::Opaque && i & 16 != 0 {
                        row.standard_lit = None;
                    }
                    row.base_color[0] = (i as f32 / 256.).into();
                    Arc::new(row)
                })
                .collect(),
        );
        let mut reference: Vec<_> = table
            .iter()
            .map(|row| {
                let mut draw = template.clone();
                draw.instance = row.clone();
                draw.camera_depth = (1. + row.instance_id as f32 * 0.001).into();
                Arc::new(draw)
            })
            .collect();
        batch::sort_opaque_segment(&mut reference, &mut batch::IdentityMemo::default());
        let manager = RenderResourceManager::new();
        let mut inputs = build_inputs::GeometryBuildInputs::new(&[], &manager, None).unwrap();
        let mut compact = CompactView::new(table.clone());
        for (i, row) in table.iter().enumerate() {
            compact
                .push(
                    i as u32,
                    MaterialDepth::TestAndWrite,
                    (1. + row.instance_id as f32 * 0.001).into(),
                    || template.clone(),
                )
                .unwrap();
        }
        let actual: Vec<_> = compact
            .finish(&mut inputs, &FinalViewState::default())
            .unwrap()
            .iter()
            .flat_map(|b| {
                b.visible
                    .iter()
                    .map(|v| b.instances[v.instance_index as usize].instance_id)
            })
            .collect();
        assert_eq!(
            actual,
            reference.iter().map(|d| d.instance_id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn view_decoration_keeps_complete_selection_proof_but_rechecks_changed_heads() {
        let mut frame = mixed_phase_frame(true, false, false, false);
        let (_, _, _, light) = shadow::receiver_scene(MaterialBlend::Opaque, -2.);
        SELECTION_VALIDATIONS.with(|n| n.set(0));
        let id = frame.views[0].view.view_id;
        frame
            .set_view_lights(id, std::slice::from_ref(&light))
            .unwrap();
        let mut validation = DrawViewValidation::default();
        for batch in frame.views[0].batches.as_ref().unwrap() {
            batch.validate_shared_with_view(&mut validation).unwrap();
        }
        assert_eq!(
            SELECTION_VALIDATIONS.with(|n| n.get()),
            0,
            "attaching view lighting invalidated unchanged table/visible/instance/view selection"
        );

        let mut bad = frame.views[0].batches.as_ref().unwrap()[0].clone();
        Arc::make_mut(&mut bad)
            .visible
            .last_mut()
            .unwrap()
            .instance_index = u32::MAX;
        assert_eq!(
            bad.validate_shared_with_view(&mut DrawViewValidation::default())
                .unwrap_err()
                .field,
            "geometry.batch.instance_index"
        );
        let mut invalid_target = frame.views[0].draws[0].as_ref().clone();
        invalid_target.target = "".into();
        frame.views[0].draws[0] = Arc::new(invalid_target);
        frame.views[0].refresh_batch_draws();
        assert!(
            frame.views[0].batches.as_ref().unwrap()[0]
                .validate_shared_with_view(&mut DrawViewValidation::default())
                .is_err(),
            "transferred selection proof bypassed new head validation"
        );
        let mut invalid_head = frame.views[0].draws[1].as_ref().clone();
        invalid_head.depth = MaterialDepth::TestOnly;
        // A new head must still be validated even when its selection is unchanged.
        assert!(invalid_head.validate().is_err());
    }

    #[test]
    fn s2_work_granularity_lit_mask_does_not_fragment_opaque_batches() {
        let frame = mixed_phase_frame(true, false, false, false);
        let batches = frame.views[0].batches.as_ref().unwrap();
        assert_eq!(
            batches.len(),
            2,
            "interleaved mask is still a global opaque batching barrier"
        );
        assert_eq!(batches[0].draw.blend, MaterialBlend::Opaque);
        assert_eq!(batches[1].draw.blend, MaterialBlend::AlphaMask);
        assert_eq!(batches.iter().map(|b| b.visible.len()).sum::<usize>(), 32);
    }

    #[test]
    fn mixed_lit_phase_preserves_legacy_nonwriting_and_transparent_order_in_flat_and_compact() {
        for (unlit, nonwriting, blend) in [
            (false, false, false),
            (true, false, false),
            (false, true, false),
            (false, false, true),
        ] {
            let flat = mixed_phase_frame(false, unlit, nonwriting, blend);
            let compact = mixed_phase_frame(true, unlit, nonwriting, blend);
            let values = |v: &GeometryViewFrame| {
                v.selected_draws()
                    .map(|d| {
                        (
                            d.instance_id,
                            d.camera_depth,
                            d.blend,
                            d.depth,
                            d.base_color,
                        )
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(values(&flat.views[0]), values(&compact.views[0]));
            if unlit || nonwriting {
                assert_eq!(
                    compact.views[0].draws.len(),
                    32,
                    "legacy/nonwriting barrier was removed"
                );
            }
            if blend {
                let ids: Vec<_> = compact.views[0]
                    .selected_draws()
                    .filter(|d| d.blend.is_transparent())
                    .map(|d| d.instance_id)
                    .collect();
                let mut expected: Vec<_> = (0..32).filter(|i| i % 3 == 2).collect();
                expected.reverse();
                assert_eq!(ids, expected, "transparent phase lost back-to-front order");
            }
        }
    }

    fn mixed_phase_frame(
        compact: bool,
        unlit: bool,
        nonwriting: bool,
        blend: bool,
    ) -> GeometryFrame {
        mixed_phase_frame_count(compact, unlit, nonwriting, blend, 32)
    }

    fn mixed_phase_frame_count(
        compact: bool,
        unlit: bool,
        nonwriting: bool,
        blend: bool,
        count: usize,
    ) -> GeometryFrame {
        let (manager, mut source, view, _) = shadow::receiver_scene(MaterialBlend::Opaque, -2.);
        if unlit {
            source.material.preset = MaterialPreset::Unlit;
            source.material.shader_source = UNLIT_SHADER_SOURCE.into();
            source
                .material
                .parameters
                .retain(|name, _| name == "baseColor");
        }
        let masked = {
            let mut s = source.clone();
            s.material.blend = MaterialBlend::AlphaMask;
            s.material
                .parameters
                .insert("alphaCutoff".into(), MaterialParameter::Float(0.5));
            s
        };
        let blended = {
            let mut s = source.clone();
            s.material.blend = MaterialBlend::AlphaBlend;
            s.material.depth = MaterialDepth::TestOnly;
            s
        };
        if nonwriting {
            source.material.depth = MaterialDepth::TestOnly;
        }
        let instances: Vec<_> = (0..count as u64)
            .map(|i| GeometryInstance {
                instance_id: i,
                source: if blend && i % 3 == 2 {
                    &blended
                } else if i % 2 == 0 {
                    &source
                } else {
                    &masked
                },
                world_transform: {
                    let mut m = IDENTITY_MATRIX;
                    m[14] = -2. - i as f32 * 0.01;
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
        let mut inputs =
            build_inputs::GeometryBuildInputs::new(&instances, &manager, None).unwrap();
        if compact {
            inputs.enable_compact().unwrap();
        }
        inputs
            .build(
                "surface-main",
                [64, 64],
                &[GeometryViewInput {
                    view: &view,
                    pose: CameraPose::default(),
                }],
                DiagnosticLevel::Off,
            )
            .unwrap()
    }

    pub(crate) fn batch_test_frame(
        compact: bool,
        blend: MaterialBlend,
        count: usize,
        view_count: usize,
    ) -> GeometryFrame {
        let (manager, source, view, light) = shadow::receiver_scene(blend, -2.);
        let sources: Vec<_> = (0..count)
            .map(|i| {
                let mut source = source.clone();
                source.material.parameters.insert(
                    "baseColor".into(),
                    MaterialParameter::Vector4(if i % 2 == 0 {
                        [1., 0., 0., 0.5]
                    } else {
                        [0., 1., 0., 0.5]
                    }),
                );
                source
            })
            .collect();
        let instances: Vec<_> = sources
            .iter()
            .enumerate()
            .map(|(i, source)| GeometryInstance {
                instance_id: i as u64,
                source,
                world_transform: {
                    let mut m = IDENTITY_MATRIX;
                    m[12] = if i % 2 == 0 { -0.5 } else { 0.5 };
                    m[14] = -2. - (i / 2) as f32 * 0.01;
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
        let mut inputs =
            build_inputs::GeometryBuildInputs::new(&instances, &manager, None).unwrap();
        if compact {
            inputs.enable_compact().unwrap();
        }
        let mut views = vec![view; view_count];
        for (i, view) in views.iter_mut().enumerate() {
            view.view_id = i as u64 + 1;
        }
        let mut frame = inputs
            .build(
                "surface-main",
                [64, 64],
                &views
                    .iter()
                    .map(|view| GeometryViewInput {
                        view,
                        pose: CameraPose::default(),
                    })
                    .collect::<Vec<_>>(),
                DiagnosticLevel::Off,
            )
            .unwrap();
        for view in &views {
            frame
                .set_view_lights(view.view_id, std::slice::from_ref(&light))
                .unwrap();
        }
        frame
    }

    pub(crate) fn batch_test_graph(frame: &GeometryFrame) -> crate::render_graph::RenderGraph {
        use crate::render_graph::*;
        let mut graph = crate::rhi_command_plan::tests::retained_geometry_graph(0);
        graph.views.clear();
        graph
            .passes
            .retain(|p| p.pass_kind == RenderPassKind::Clear);
        for resource in &mut graph.resources {
            resource.size.width = 64;
            resource.size.height = 64;
        }
        frame.append_to_graph(&mut graph);
        graph
    }

    #[test]
    fn immutable_batch_cross_consumer_validation_runs_once_and_cow_tail_rechecks() {
        let frame = batch_test_frame(true, MaterialBlend::Opaque, 128, 1);
        let batch = frame.views[0].batches.as_ref().unwrap()[0].clone();
        SELECTION_VALIDATIONS.with(|n| n.set(0));
        for _ in 0..3 {
            let graph = batch_test_graph(&frame);
            let plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
            assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
            let mut context = DrawViewValidation::default();
            for command in &plan.commands {
                if let crate::rhi_command_plan::RhiCommand::Draw {
                    payload: crate::rhi_command_plan::RhiDrawPayload::GeometryBatch { batch },
                    ..
                } = command
                {
                    batch.validate_shared_with_view(&mut context).unwrap();
                }
            }
        }
        assert!(
            SELECTION_VALIDATIONS.with(|n| n.get()) <= 1,
            "immutable selection was rescanned across graph/RHI consumers"
        );
        let mut changed = batch.clone();
        Arc::make_mut(&mut changed)
            .visible
            .last_mut()
            .unwrap()
            .instance_index = u32::MAX;
        assert!(changed
            .validate_shared_with_view(&mut DrawViewValidation::default())
            .is_err());
        assert!(
            changed
                .validate_shared_with_view(&mut DrawViewValidation::default())
                .is_err(),
            "failed tail validation became a success proof"
        );
        batch
            .validate_shared_with_view(&mut DrawViewValidation::default())
            .unwrap();
    }

    #[test]
    fn mixed_transparent_tail_does_not_presort_opaque_depth_at_multiple_scales() {
        let values = |frame: &GeometryFrame| {
            frame.views[0]
                .selected_draws()
                .map(|d| (d.instance_id, d.camera_depth, d.blend, d.depth))
                .collect::<Vec<_>>()
        };
        let mut actual = Vec::new();
        for count in [4, 32, 256] {
            COMPACT_OPAQUE_DEPTH_COMPARISONS.with(|n| n.set(0));
            let compact = mixed_phase_frame_count(true, false, false, true, count);
            actual.push((count, COMPACT_OPAQUE_DEPTH_COMPARISONS.with(|n| n.get())));
            let reference = mixed_phase_frame_count(false, false, false, true, count);
            assert_eq!(
                values(&compact),
                values(&reference),
                "independent flat order at {count}"
            );
        }
        eprintln!("actual discarded opaque pre-depth comparisons: {actual:?}");
        assert!(
            actual.iter().all(|(_, count)| *count == 0),
            "opaque phase was sorted twice: {actual:?}"
        );
    }

    #[test]
    fn compact_publication_shares_one_visible_backing_per_view_at_multiple_scales() {
        let mut actual = Vec::new();
        let mut attachments = Vec::new();
        let mut head_products = Vec::new();
        for count in [4, 32, 256] {
            let base = batch_test_frame(true, MaterialBlend::Opaque, 1, 1);
            let template = base.views[0].draws[0].as_ref();
            let table = Arc::new(
                (0..count)
                    .map(|i| {
                        let mut row = template.instance.as_ref().clone();
                        row.instance_id = i as u64 + 1;
                        Arc::make_mut(&mut row.material_binding).binding_id =
                            format!("visible-backing-{i}");
                        Arc::new(row)
                    })
                    .collect(),
            );
            let mut compact = CompactView::new(table);
            for i in 0..count {
                compact
                    .push(
                        i as u32,
                        MaterialDepth::TestAndWrite,
                        (1. + i as f32 * 0.01).into(),
                        || template.clone(),
                    )
                    .unwrap();
            }
            let manager = RenderResourceManager::new();
            let mut inputs = build_inputs::GeometryBuildInputs::new(&[], &manager, None).unwrap();
            HEAD_MATRIX_PRODUCTS.with(|n| n.set(0));
            let batches = compact
                .finish(&mut inputs, &FinalViewState::default())
                .unwrap();
            head_products.push((count, HEAD_MATRIX_PRODUCTS.with(|n| n.get())));
            assert_eq!(
                batches.len(),
                count,
                "fixture must contain distinct batches"
            );
            let backings = batches
                .iter()
                .map(|b| b.visible_backing_for_test())
                .collect::<std::collections::HashSet<_>>()
                .len();
            actual.push((count, backings));
            attachments.push((
                count,
                batches
                    .iter()
                    .map(|b| b.draw.target.as_ptr() as usize)
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
                batches
                    .iter()
                    .filter_map(|b| b.draw.depth_target.as_ref())
                    .map(|v| v.as_ptr() as usize)
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
            ));
            assert_eq!(
                batches.iter().map(|b| b.visible.len()).sum::<usize>(),
                count
            );
        }
        eprintln!("actual visible Vec allocation identities per view: {actual:?}");
        eprintln!("actual repeated head matrix products: {head_products:?}");
        // This fixture deliberately has no prepared static facts. Keep its
        // strict initial head check; only the second, post-selection product
        // is redundant. Production prepared selections need neither repeat.
        assert!(
            head_products.iter().all(|(count, n)| count == n),
            "{head_products:?}"
        );
        assert!(
            actual.iter().all(|(_, n)| *n == 1),
            "per-batch selected-index backing: {actual:?}"
        );
        eprintln!("actual per-view target/depth backing identities: {attachments:?}");
        assert!(
            attachments
                .iter()
                .all(|(_, target, depth)| *target == 1 && *depth <= 1),
            "per-batch owned attachment strings: {attachments:?}"
        );
    }

    #[test]
    fn compact_prepared_head_consumes_complete_selection_finiteness_and_cow_stays_strict() {
        let mut observed = Vec::new();
        for count in [4, 32, 256] {
            let (manager, source, view, _) = shadow::receiver_scene(MaterialBlend::Opaque, -2.);
            let instances: Vec<_> = (0..count)
                .map(|i| GeometryInstance {
                    instance_id: i as u64 + 1,
                    source: &source,
                    world_transform: {
                        let mut m = IDENTITY_MATRIX;
                        m[14] = -2.;
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
            HEAD_MATRIX_PRODUCTS.with(|n| n.set(0));
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
            observed.push((count, HEAD_MATRIX_PRODUCTS.with(|n| n.get())));
            assert_eq!(frame.views[0].selected_draws().count(), count);
            let head = &frame.views[0].draws[0];
            let detached = Arc::new(head.as_ref().clone());
            HEAD_MATRIX_PRODUCTS.with(|n| n.set(0));
            detached
                .validate_shared_with_view(&mut DrawViewValidation::default())
                .unwrap();
            assert!(
                HEAD_MATRIX_PRODUCTS.with(|n| n.get()) > 0,
                "detached public draw inherited private selection success"
            );
            let mut changed = head.clone();
            Arc::make_mut(&mut changed).world_transform[0] = f32::NAN.into();
            assert!(changed
                .validate_shared_with_view(&mut DrawViewValidation::default())
                .is_err());
            let mut changed = head.clone();
            Arc::make_mut(&mut Arc::make_mut(&mut changed).view).projection_matrix[0] =
                f32::INFINITY.into();
            assert!(changed
                .validate_shared_with_view(&mut DrawViewValidation::default())
                .is_err());
        }
        eprintln!("prepared production head matrix repeats {observed:?}");
        assert!(observed.iter().all(|(_, n)| *n == 0), "{observed:?}");
    }

    #[test]
    fn fully_sortable_compact_phase_does_not_first_sort_by_depth() {
        COMPACT_DEPTH_COMPARISONS.with(|n| n.set(0));
        let compact = batch_test_frame(true, MaterialBlend::Opaque, 128, 1);
        assert_eq!(
            compact.views[0].batches.as_ref().unwrap()[0].instance_count(),
            128
        );
        assert_eq!(
            COMPACT_DEPTH_COMPARISONS.with(|n| n.get()),
            0,
            "a fully sortable phase was depth-sorted before its complete phase key"
        );
        let flat = batch_test_frame(false, MaterialBlend::Opaque, 128, 1);
        let ids = |frame: &GeometryFrame| {
            frame.views[0]
                .selected_draws()
                .map(|d| (d.instance_id, d.camera_depth))
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&compact), ids(&flat));
    }

    #[test]
    fn compact_sort_allocates_descriptors_for_heads_not_every_selected_sort_key() {
        COMPACT_DESCRIPTOR_BUILDS.with(|n| n.set(0));
        let frame = batch_test_frame(true, MaterialBlend::Opaque, 128, 2);
        let heads = frame.views.iter().map(|v| v.draws.len()).sum::<usize>();
        assert!(
            COMPACT_DESCRIPTOR_BUILDS.with(|n| n.get()) <= heads + frame.views.len(),
            "compact sorting cloned a full descriptor for every selected row"
        );
    }

    #[test]
    fn opaque_shadow_batches_use_geometry_identity_across_color_materials_and_reject_bad_tails() {
        let frame = batch_test_frame(true, MaterialBlend::Opaque, 128, 1);
        let mut batch = frame.views[0].batches.as_ref().unwrap()[0].as_ref().clone();
        let first = batch.visible[0].instance_index as usize;
        for (i, instance) in Arc::make_mut(&mut batch.instances).iter_mut().enumerate() {
            if i != first {
                let instance = Arc::make_mut(instance);
                Arc::make_mut(&mut instance.material_binding).binding_id = format!("color-{i}");
                Arc::make_mut(&mut instance.material_source).asset_id =
                    Some(format!("material-{i}"));
            }
        }
        let head = Arc::make_mut(&mut batch.draw);
        let view = Arc::make_mut(&mut head.view);
        view.view_kind = ViewKind::Shadow;
        view.target_kind = TargetKind::ShadowMap;
        view.clear_color = None;
        view.depth_clear = Some(1f32.into());
        head.target = "shadow".into();
        head.depth_target = Some(head.target.clone());
        head.depth = MaterialDepth::TestAndWrite;
        head.lighting = None;
        head.shadows = None;
        head.environment = None;
        batch
            .validate()
            .expect("opaque depth must accept otherwise valid color material variants");

        let manager = RenderResourceManager::new();
        let mut inputs = build_inputs::GeometryBuildInputs::new(&[], &manager, None).unwrap();
        let mut compact = CompactView::new(batch.instances.clone());
        COMPACT_STATE_RESOLUTIONS.with(|n| n.set(0));
        for entry in &batch.visible {
            compact
                .push(
                    entry.instance_index,
                    MaterialDepth::TestAndWrite,
                    entry.camera_depth,
                    || batch.draw.as_ref().clone(),
                )
                .unwrap();
        }
        let merged = compact
            .finish(&mut inputs, &FinalViewState::default())
            .unwrap();
        assert_eq!(
            COMPACT_STATE_RESOLUTIONS.with(|n| n.get()),
            1,
            "position-only shadow state was resolved per unused color material"
        );
        assert_eq!(
            merged.len(),
            1,
            "middle consumer fragmented a native-compatible shadow batch"
        );
        assert_eq!(merged[0].instance_count(), 128);
        merged[0]
            .validate_shared_with_view(&mut DrawViewValidation::default())
            .unwrap();

        let last = *merged[0].visible.last().unwrap();
        fn changed_tail(batch: &mut GeometryBatch, index: usize) -> &mut DrawInstance {
            Arc::make_mut(&mut Arc::make_mut(&mut batch.instances)[index])
        }
        let mut invalid = merged[0].as_ref().clone();
        let binding = Arc::new(RenderBindingSet {
            binding_kind: RenderBindingKind::Mesh,
            ..invalid.instances[last.instance_index as usize]
                .material_binding
                .as_ref()
                .clone()
        });
        changed_tail(&mut invalid, last.instance_index as usize).material_binding = binding;
        assert!(
            Arc::new(invalid)
                .validate_shared_with_view(&mut DrawViewValidation::default())
                .is_err(),
            "unused color binding still needs structural tail validation"
        );
        let mut invalid = merged[0].as_ref().clone();
        changed_tail(&mut invalid, last.instance_index as usize).world_transform[0] =
            (-1f32).into();
        assert!(
            invalid.validate().is_err(),
            "mirrored caster merged with opposite winding"
        );
        let mut missing_uv = merged[0].as_ref().clone();
        let mut geometry = serde_json::to_value(&missing_uv.draw.geometry).unwrap();
        geometry["attributes"]
            .as_array_mut()
            .unwrap()
            .retain(|a| a["semantic"] != "texCoord0");
        let geometry: Arc<GeometryBuffers> = Arc::new(serde_json::from_value(geometry).unwrap());
        for instance in Arc::make_mut(&mut missing_uv.instances) {
            Arc::make_mut(instance).geometry = geometry.clone();
        }
        let first = missing_uv.visible[0].instance_index as usize;
        Arc::make_mut(&mut missing_uv.draw).instance = missing_uv.instances[first].clone();
        missing_uv.validate().expect("untextured head needs no UV");
        let texture = TextureHandle::try_from(crate::render_resource::RenderResourceHandle {
            kind: crate::render_resource::RenderResourceKind::Texture,
            index: 70,
            generation: 1,
        })
        .unwrap();
        Arc::make_mut(
            &mut changed_tail(&mut missing_uv, last.instance_index as usize).material_binding,
        )
        .slots
        .push(NamedRenderBinding {
            name: "baseColorTexture".into(),
            resource: BindingResource::Texture(texture),
        });
        assert_eq!(
            missing_uv.validate().unwrap_err().field,
            "drawGeometry.uvRequired",
            "textured tail borrowed an untextured head's vertex proof"
        );
        let mut color = batch.draw.as_ref().clone();
        Arc::make_mut(&mut color.view).target_kind = TargetKind::Window;
        assert!(!GeometryBatch::compatible_instance(
            &color,
            &batch.instances[last.instance_index as usize]
        ));
        let mut masked = batch.draw.as_ref().clone();
        masked.blend = MaterialBlend::AlphaMask;
        masked.alpha_cutoff = Some(0.5f32.into());
        let mut tail = batch.instances[last.instance_index as usize]
            .as_ref()
            .clone();
        tail.blend = MaterialBlend::AlphaMask;
        tail.alpha_cutoff = Some(0.5f32.into());
        assert!(
            !GeometryBatch::compatible_instance(&masked, &tail),
            "mask material lost its alpha binding"
        );
    }

    #[test]
    fn compact_batches_preserve_mask_blend_order_and_graph_tail_rejection() {
        use crate::render_graph::RenderPassCommand;
        use crate::rhi_command_plan::*;
        for blend in [
            MaterialBlend::Opaque,
            MaterialBlend::AlphaMask,
            MaterialBlend::AlphaBlend,
        ] {
            let legacy = batch_test_frame(false, blend, 128, 2);
            let compact = batch_test_frame(true, blend, 128, 2);
            for (a, b) in legacy.views.iter().zip(&compact.views) {
                let values = |view: &GeometryViewFrame| {
                    view.selected_draws()
                        .map(|d| {
                            (
                                d.instance_id,
                                d.world_transform,
                                d.base_color,
                                d.depth,
                                d.camera_depth,
                            )
                        })
                        .collect::<Vec<_>>()
                };
                assert_eq!(
                    values(a),
                    values(b),
                    "ordered material semantics changed: {blend:?}"
                );
            }
            let graph = batch_test_graph(&compact);
            let mut cache = crate::render_graph::GraphScheduleCache::default();
            let plan = compile_render_graph_with_cache(&graph, &mut cache);
            assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
            assert_eq!(
                plan.commands
                    .iter()
                    .map(RhiCommand::geometry_instance_count)
                    .sum::<usize>(),
                256
            );
            assert_eq!(
                plan.commands
                    .iter()
                    .filter(|c| c.geometry_draw().is_some())
                    .count(),
                2
            );
            let original = serde_json::to_vec(&plan).unwrap();
            let restored: RhiCommandPlan = serde_json::from_slice(&original).unwrap();
            assert_eq!(restored, plan);
            let mut bad = graph.clone();
            let batch = bad
                .passes
                .iter_mut()
                .flat_map(|p| &mut p.commands)
                .find_map(|c| {
                    if let RenderPassCommand::DrawGeometryBatch { batch } = c {
                        Some(batch)
                    } else {
                        None
                    }
                })
                .unwrap();
            let head = batch.draw.clone();
            Arc::make_mut(batch)
                .visible
                .last_mut()
                .unwrap()
                .instance_index = u32::MAX;
            assert!(
                Arc::ptr_eq(&head, &batch.draw),
                "tail check must not depend on a changed head"
            );
            let invalid = compile_render_graph_with_cache(&bad, &mut cache);
            assert!(invalid.has_errors());
            assert!(invalid
                .diagnostics
                .iter()
                .any(|d| d.code == "geometry.invalid_batch"));
            assert_eq!(
                serde_json::to_vec(&plan).unwrap(),
                original,
                "COW changed old packet"
            );
        }
    }

    #[test]
    fn compact_batch_rejects_changed_source_values_schema_and_detached_head() {
        let frame = batch_test_frame(true, MaterialBlend::Opaque, 128, 1);
        let original = &frame.views[0].batches.as_ref().unwrap()[0];
        let mut unknown = original.as_ref().clone();
        unknown.schema_version = "geometry-batch.v2".into();
        assert!(unknown.validate().is_err());
        let mut bad = original.as_ref().clone();
        let table = Arc::make_mut(&mut bad.instances);
        Arc::make_mut(&mut table[127]).world_transform[12] = f32::NAN.into();
        assert!(
            bad.validate().is_err(),
            "last instance nonfinite values disappeared"
        );
        let mut bad = original.as_ref().clone();
        let table = Arc::make_mut(&mut bad.instances);
        Arc::make_mut(&mut Arc::make_mut(&mut table[127]).resources)
            .data
            .index_count = 6;
        assert_eq!(
            bad.validate().unwrap_err().field,
            "geometry.batch.incompatible_instance"
        );
        let mut changed = frame.clone();
        Arc::make_mut(&mut changed.views[0].draws[0]).world_transform[12] = 1f32.into();
        assert!(batch_test_graph(&changed)
            .diagnostics
            .iter()
            .any(|d| d.code == "geometry.batch.head_desynchronized"));
        original.validate().unwrap();
        let mut wire = serde_json::to_value(original).unwrap();
        wire["unknownField"] = true.into();
        assert!(serde_json::from_value::<GeometryBatch>(wire).is_err());
    }

    #[test]
    fn compact_graph_shared_view_validation_is_not_restarted_for_every_batch() {
        let mut frame = batch_test_frame(true, MaterialBlend::Opaque, 128, 1);
        let view = &mut frame.views[0];
        let original = view.batches.as_ref().unwrap()[0].clone();
        let split: Vec<_> = original
            .visible
            .iter()
            .map(|entry| {
                let mut batch = original.as_ref().clone();
                batch.draw = Arc::new(original.selected_draw(
                    &original.instances[entry.instance_index as usize],
                    entry.camera_depth,
                ));
                batch.visible = vec![*entry].into();
                Arc::new(batch)
            })
            .collect();
        view.draws = split.iter().map(|b| b.draw.clone()).collect();
        view.batches = Some(split);
        let graph = batch_test_graph(&frame);
        DRAW_VIEW_VALIDATIONS.with(|n| n.set(0));
        let mut proofs = Vec::new();
        let (errors, _) = graph::validate_geometry_graph_with_cache(&graph, &mut proofs);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            DRAW_VIEW_VALIDATIONS.with(|n| n.get()),
            1,
            "every batch restarted the complete shared view/light validation"
        );
    }

    #[test]
    fn compact_memory_observes_live_backing_without_temporary_view_copies() {
        let mut frame = batch_test_frame(true, MaterialBlend::Opaque, 128, 1);
        let view = &mut frame.views[0];
        let mut head = view.draws[0].as_ref().clone();
        head.target = "t".repeat(8191).into();
        let capacity = head.target.len() as u64;
        let actual = format!("cpu-storage:{:x}", head.target.as_ptr() as usize);
        let head = Arc::new(head);
        Arc::make_mut(&mut view.batches.as_mut().unwrap()[0]).draw = head.clone();
        view.draws[0] = head;
        let mut sample = crate::resource_memory::ResourceMemorySample::new(1, 1, 0);
        frame.observe_memory(&mut sample);
        let targets: Vec<_> = sample
            .allocations
            .iter()
            .filter(|a| a.owner == "snapshots" && a.bytes == capacity)
            .collect();
        assert!(!targets.is_empty());
        assert!(
            targets.iter().all(|a| a.allocation_id == actual),
            "census reported temporary view strings as persistent backing"
        );
    }

    #[test]
    fn compact_wire_publishes_only_selected_rows_and_preserves_semantic_identity() {
        use std::hash::{Hash, Hasher};
        fn hash(value: &GeometryBatch) -> u64 {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            value.hash(&mut h);
            h.finish()
        }
        let frame = batch_test_frame(true, MaterialBlend::Opaque, 128, 1);
        let mut batch = frame.views[0].batches.as_ref().unwrap()[0].as_ref().clone();
        batch.visible.truncate(2);
        batch.validate().unwrap();
        let wire = serde_json::to_value(&batch).unwrap();
        assert_eq!(wire["instances"].as_array().unwrap().len(), 2);
        let decoded: GeometryBatch = serde_json::from_value(wire).unwrap();
        decoded.validate().unwrap();
        assert_eq!(batch, decoded);
        assert_eq!(hash(&batch), hash(&decoded));
        assert_eq!(batch.instances.len(), 128, "wire mutated shared backing");
        batch.visible[1].instance_index = u32::MAX;
        assert!(
            serde_json::to_value(&batch).is_err(),
            "invalid tail hidden on wire"
        );
    }

    #[test]
    fn compact_production_build_keeps_table_shared_across_views_and_rejects_tail() {
        let (manager, source, view, _) = shadow::receiver_scene(MaterialBlend::Opaque, -100.);
        let instances: Vec<_> = (0..128)
            .map(|id| GeometryInstance {
                instance_id: id,
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
        let mut inputs =
            build_inputs::GeometryBuildInputs::new(&instances, &manager, None).unwrap();
        inputs.enable_compact().unwrap();
        let mut views = vec![view; 4];
        for (i, v) in views.iter_mut().enumerate() {
            v.view_id = i as u64 + 1;
        }
        let view_inputs: Vec<_> = views
            .iter()
            .map(|view| GeometryViewInput {
                view,
                pose: CameraPose::default(),
            })
            .collect();
        let frame = inputs
            .build("main", [64, 64], &view_inputs, DiagnosticLevel::Off)
            .unwrap();
        assert!(
            frame.views.iter().map(|v| v.draws.len()).sum::<usize>() <= 4,
            "production still persists 128 full descriptors for every view"
        );
        let mut first_table = None;
        for view in &frame.views {
            let batches = view
                .batches
                .as_ref()
                .expect("production must select batch commands");
            assert_eq!(batches.len(), 1);
            let batch = &batches[0];
            assert_eq!(batch.instance_count(), 128);
            batch.validate().unwrap();
            if let Some(table) = first_table {
                assert!(Arc::ptr_eq(table, &batch.instances));
            }
            first_table = Some(&batch.instances);
            let mut bad = batch.as_ref().clone();
            bad.visible.last_mut().unwrap().instance_index = 128;
            assert_eq!(
                bad.validate().unwrap_err().field,
                "geometry.batch.instance_index"
            );
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeometryBatch {
    #[serde(skip)]
    pub(super) success: proof::BoundSuccess<GeometryBatch>,
    // Full ID-domain uniqueness only, not selection or cross-batch admission.
    // Weak binds the exact table and forces COW on any row-vector mutation.
    #[serde(skip)]
    pub(super) unique_table: Option<std::sync::Weak<Vec<Arc<DrawInstance>>>>,
    pub schema_version: String,
    /// The complete execution state of this ordered batch, not a residency proof.
    pub draw: Arc<DrawGeometry>,
    pub instances: Arc<Vec<Arc<DrawInstance>>>,
    pub visible: GeometryVisibleInstances,
}

#[cfg(test)]
pub(super) static BATCH_CONSTRUCTIONS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

impl GeometryBatch {
    #[cfg(test)]
    fn visible_backing_for_test(&self) -> usize {
        self.visible.backing_for_test()
    }

    /// Construct a strict, initially unproven batch. Derived success metadata
    /// is private and is never accepted from a wire reader or caller.
    pub fn new(
        draw: Arc<DrawGeometry>,
        instances: Arc<Vec<Arc<DrawInstance>>>,
        visible: Vec<GeometryVisibleInstance>,
    ) -> Self {
        #[cfg(test)]
        BATCH_CONSTRUCTIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self {
            schema_version: "geometry-batch.v1".into(),
            draw,
            instances,
            visible: visible.into(),
            success: Default::default(),
            unique_table: None,
        }
    }
    #[doc(hidden)]
    pub fn validate_shared_with_view<'a>(
        self: &'a Arc<Self>,
        validation: &mut DrawViewValidation<'a>,
    ) -> Result<(), ContractError> {
        self.draw.validate_shared_with_view(validation)?;
        proof::selection_with_view(self, validation)
    }

    #[cfg(feature = "real-wgpu")]
    #[doc(hidden)]
    pub fn unique_instances_proven(&self) -> bool {
        self.unique_table
            .as_ref()
            .is_some_and(|table| table.as_ptr() == Arc::as_ptr(&self.instances))
    }

    /// Borrowed table access; never expands a frame into legacy commands.
    pub fn instance_count(&self) -> usize {
        self.visible.len()
    }

    fn compatible_instance(head: &DrawGeometry, instance: &DrawInstance) -> bool {
        #[cfg(test)]
        STATIC_COMPATIBILITY_CHECKS.with(|n| n.set(n.get() + 1));
        (Arc::ptr_eq(&head.instance.resources, &instance.resources)
            || if head.is_opaque_shadow_depth_writer() {
                // Position-only opaque depth does not consume color bindings.
                // Keep all geometry identity, provenance and range facts exact.
                head.geometry_binding == instance.geometry_binding
                    && head.geometry_source == instance.geometry_source
                    && head.first_index == instance.first_index
                    && head.index_count == instance.index_count
                    && head.geometry == instance.geometry
            } else {
                head.instance.resources.data == instance.resources.data
            })
            && head.blend == instance.blend
            && match (&head.deformation,&instance.deformation) {
                (None,None)=>true,(Some(a),Some(b))=>Arc::ptr_eq(a,b)||a==b,_=>false,
            }
            && head.cull == instance.cull
            && head.decal_bias == instance.decal_bias
            && head.opaque_alpha == instance.opaque_alpha
            && head.ignore_vertex_color == instance.ignore_vertex_color
            && head.standard_lit.is_some() == instance.standard_lit.is_some()
            && head.mirrored() == instance.mirrored()
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        self.validate_with_view(&mut DrawViewValidation::default())
    }

    pub(crate) fn validate_with_view<'a>(
        &'a self,
        validation: &mut DrawViewValidation<'a>,
    ) -> Result<(), ContractError> {
        self.draw.validate_with_view(validation)?;
        self.validate_selection_with_view(validation)
    }

    /// Strict borrowed/value entry. Shared consumers may reuse the successful
    /// fact only for this entire immutable batch, never its head alone.
    pub(super) fn validate_selection_with_view<'a>(
        &'a self,
        validation: &mut DrawViewValidation<'a>,
    ) -> Result<(), ContractError> {
        self.validate_selection(validation, None)
    }

    fn validate_selection<'a>(
        &'a self,
        validation: &mut DrawViewValidation<'a>,
        prepared: Option<&PreparedPhase>,
    ) -> Result<(), ContractError> {
        if self.schema_version != "geometry-batch.v1" {
            return Err(invalid("geometry.batch.schema_or_capacity"));
        }
        Self::validate_selection_parts(
            &self.draw,
            &self.instances,
            &self.visible,
            validation,
            prepared,
        )
    }

    // Both representations use this complete current-selection validator.
    // The scalar producer supplies exactly one member without a batch wrapper.
    fn validate_selection_parts<'a>(
        draw: &'a DrawGeometry,
        instances: &'a Arc<Vec<Arc<DrawInstance>>>,
        visible: &[GeometryVisibleInstance],
        validation: &mut DrawViewValidation<'a>,
        prepared: Option<&PreparedPhase>,
    ) -> Result<(), ContractError> {
        #[cfg(test)]
        SELECTION_VALIDATIONS.with(|n| n.set(n.get() + 1));
        if visible.is_empty() || visible.len() > 65_536 || instances.len() > u32::MAX as usize {
            return Err(invalid("geometry.batch.schema_or_capacity"));
        }
        let first = visible[0];
        if instances
            .get(first.instance_index as usize)
            .is_none_or(|i| !Arc::ptr_eq(i, &draw.instance) && i.as_ref() != draw.instance.as_ref())
            || first.camera_depth != draw.camera_depth
        {
            return Err(invalid("geometry.batch.head_selection_mismatch"));
        }
        let projection = validation.projection(&draw.view);
        let prepared = prepared.filter(|p| p.matches_parts(draw, instances));
        let head_group = prepared.map(|p| p[first.instance_index as usize].compatibility);
        // This is a new current-view proof, not a cached visible result. All
        // rows were finite when the exact immutable table was prepared. A
        // conservative f64 bound proves every original f32 product and prefix
        // sum finite; uncertain/extreme inputs keep the original strict path.
        let matrix_finite = prepared.is_some_and(|p| {
            let key = p.world_magnitude.to_bits();
            if let Some((previous, result)) = validation.product_bound {
                if previous == key {
                    return result;
                }
            }
            let result = product_is_finite(projection, p.world_magnitude);
            validation.product_bound = Some((key, result));
            result
        });
        // Check the entire tail before any upload. The temporary is one stack
        // view at a time; no per-view descriptor array or legacy RHI expansion.
        let direct = validation.selection.begin(instances.len());
        let mut selected = std::collections::HashSet::new();
        for entry in visible {
            let instance = instances
                .get(entry.instance_index as usize)
                .ok_or_else(|| invalid("geometry.batch.instance_index"))?;
            #[cfg(test)]
            let old_capacity = selected.capacity();
            let unique = if direct {
                validation.selection.insert(entry.instance_index as usize)
            } else {
                selected.insert(entry.instance_index)
            };
            #[cfg(test)]
            if selected.capacity() != old_capacity {
                DYNAMIC_SELECTION_GROWS.with(|n| n.set(n.get() + 1));
            }
            if !unique {
                return Err(invalid("geometry.batch.duplicate_instance"));
            }
            let member = prepared.map(|p| &p[entry.instance_index as usize]);
            if if let Some(member) = member {
                Some(member.compatibility) != head_group || member.depth != draw.depth
            } else {
                !Self::compatible_instance(draw, instance)
            } {
                return Err(invalid("geometry.batch.incompatible_instance"));
            }
            if prepared.is_none() && draw.is_opaque_shadow_depth_writer() {
                // Relaxed batch identity is not permission to hide a malformed
                // non-head material or its StandardLit vertex requirements.
                validation.validate_binding(&instance.material_binding)?;
                if instance.material_binding.binding_kind != RenderBindingKind::Material {
                    return Err(invalid("drawGeometry"));
                }
                if instance.standard_lit.is_some()
                    && instance.resources.textures().iter().any(Option::is_some)
                    && !instance
                        .geometry
                        .attributes()
                        .iter()
                        .any(|a| a.semantic == VertexSemantic::TexCoord0)
                {
                    return Err(invalid("drawGeometry.uvRequired"));
                }
            }
            if prepared.is_none() {
                proof::instance(instance)?;
            }
            if !matrix_finite {
                #[cfg(test)]
                SELECTION_MATRIX_PRODUCTS.with(|n| n.set(n.get() + 1));
                finite_draw(&multiply_matrix(
                    projection,
                    instance.world_transform.map(OrderedF32::to_f32),
                ))?;
            }
            finite_draw(&[entry.camera_depth.to_f32()])?;
        }
        Ok(())
    }

    pub(super) fn validate_prepared_selection<'a>(
        &'a self,
        validation: &mut DrawViewValidation<'a>,
        prepared: &PreparedPhase,
    ) -> Result<(), ContractError> {
        self.validate_selection(validation, Some(prepared))
    }

    #[cfg(test)]
    pub(crate) fn selected_draw(
        &self,
        instance: &Arc<DrawInstance>,
        camera_depth: OrderedF32,
    ) -> DrawGeometry {
        #[cfg(test)]
        SELECTED_DESCRIPTOR_BUILDS.with(|n| n.set(n.get() + 1));
        let mut draw = self.draw.as_ref().clone();
        draw.instance = instance.clone();
        draw.camera_depth = camera_depth;
        draw
    }
}

#[cfg(test)]
thread_local! {
    pub(super) static STATIC_COMPATIBILITY_CHECKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static DYNAMIC_SELECTION_GROWS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SELECTION_MATRIX_PRODUCTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SELECTION_VALIDATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static COMPACT_STATE_RESOLUTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static COMPACT_FULL_STATE_COMPARISONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static COMPACT_DESCRIPTOR_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static COMPACT_OPAQUE_DEPTH_COMPARISONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static COMPACT_DEPTH_COMPARISONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static SELECTED_DESCRIPTOR_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

// This builder retains only indices and scalar order data while culling/sorting.
// A stack descriptor interprets existing semantics; only final batch heads are Arc allocated.
#[derive(Clone, Copy)]
struct Selection {
    visible: GeometryVisibleInstance,
    depth: MaterialDepth,
    sort_rank: usize,
}

// One finish call borrows the whole immutable table and a fixed view/template.
// Resource addresses cannot recycle here. All instance inputs consumed by the
// pipeline/batch key are explicit; uniform values remain outside that key.
#[derive(PartialEq, Eq, Hash)]
enum SelectionResources {
    Color(*const data::DrawResources),
    PositionOnly {
        geometry: *const GeometryBuffers,
        handle: GeometryHandle,
        first_index: u32,
        index_count: u32,
    },
}

#[derive(PartialEq, Eq, Hash)]
struct SelectionState {
    resources: SelectionResources,
    depth: MaterialDepth,
    blend: MaterialBlend,
    cull: MaterialCull,
    mirrored: bool,
    decal_bias: Option<i32>,
    opaque_alpha: bool,
    ignore_vertex_color: bool,
    standard_lit: bool,
}

type SortState = (
    bool,
    Arc<str>,
    Option<crate::render_resource::RenderResourceHandle>,
    Option<String>,
    Arc<str>,
);

#[derive(Clone, PartialEq, Eq)]
struct SharedSortState(Arc<SortState>);
impl Ord for SharedSortState {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if Arc::ptr_eq(&self.0, &other.0) {
            std::cmp::Ordering::Equal
        } else {
            #[cfg(test)]
            COMPACT_FULL_STATE_COMPARISONS.with(|n| n.set(n.get() + 1));
            self.0.cmp(&other.0)
        }
    }
}
impl PartialOrd for SharedSortState {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// Call-local duplicate detection only. A fresh epoch begins for each complete
// batch, so this cannot become cross-batch uniqueness or a retained success fact.
#[derive(Default)]
pub(super) struct SelectionScratch {
    marks: Vec<u32>,
    epoch: u32,
}
impl SelectionScratch {
    fn begin(&mut self, rows: usize) -> bool {
        if rows > 65_536 {
            return false;
        }
        if self.epoch == u32::MAX {
            self.marks.fill(0);
            self.epoch = 1;
        } else {
            self.epoch += 1;
        }
        if self.marks.capacity() < rows {
            self.marks.reserve_exact(rows - self.marks.len());
        }
        self.marks.resize(rows, 0);
        true
    }
    fn insert(&mut self, index: usize) -> bool {
        let old = std::mem::replace(&mut self.marks[index], self.epoch);
        old != self.epoch
    }
}

pub(super) struct CompactView {
    table: Arc<Vec<Arc<DrawInstance>>>,
    template: Option<DrawGeometry>,
    selected: Vec<Selection>,
    values_validated: bool,
}
impl CompactView {
    pub(super) fn new(table: Arc<Vec<Arc<DrawInstance>>>) -> Self {
        Self {
            table,
            template: None,
            selected: Vec::new(),
            values_validated: false,
        }
    }
    pub(super) fn with_prepared(mut self, prepared: Option<&build_inputs::PreparedInputs>) -> Self {
        // Reuse only the existing complete immutable-table success. A new/COW
        // table or failed/partial phase retains the selected-only strict path.
        self.values_validated = prepared.is_some_and(|p| {
            Arc::ptr_eq(&p.table, &self.table)
                && p.phases
                    .lock()
                    .expect("geometry phase cache poisoned")
                    .values_validated(&self.table)
        });
        self
    }
    pub(super) fn push(
        &mut self,
        index: u32,
        depth: MaterialDepth,
        camera_depth: OrderedF32,
        make_head: impl FnOnce() -> DrawGeometry,
    ) -> Result<(), ContractError> {
        if !self.values_validated {
            proof::instance(&self.table[index as usize])?;
        }
        let selection = Selection {
            visible: GeometryVisibleInstance {
                instance_index: index,
                camera_depth,
            },
            depth,
            sort_rank: 0,
        };
        if self.template.is_none() {
            self.template = Some(make_head());
        }
        self.selected.push(selection);
        Ok(())
    }
    // Only contiguous chunks of one admitted frozen input may merge, before
    // sorting or publication. The final head is rebuilt from each selected row.
    pub(super) fn append_selection(&mut self, mut other: Self) -> Result<(), ContractError> {
        if !Arc::ptr_eq(&self.table, &other.table)
            || self.values_validated != other.values_validated
        {
            return Err(invalid("geometry.prepare_selection_table"));
        }
        if let (Some(a), Some(b)) = (&self.template, &other.template) {
            if a.view != b.view || a.target != b.target || a.depth_target != b.depth_target {
                return Err(invalid("geometry.prepare_selection_context"));
            }
        }
        if self.template.is_none() {
            self.template = other.template.take();
        }
        self.selected.append(&mut other.selected);
        Ok(())
    }
    pub(super) fn receiver_rows(&self) -> impl Iterator<Item = (u32, &Arc<DrawInstance>)> {
        self.selected.iter().map(|s| {
            (
                s.visible.instance_index,
                &self.table[s.visible.instance_index as usize],
            )
        })
    }
    #[cfg(test)]
    pub(super) fn finish(
        self,
        inputs: &mut build_inputs::GeometryBuildInputs<'_, '_>,
        state: &FinalViewState,
    ) -> Result<Vec<Arc<GeometryBatch>>, ContractError> {
        match self.order(inputs)? {
            Some(ordered) => ordered.finish(inputs, state),
            None => Ok(Vec::new()),
        }
    }
    pub(super) fn order(
        mut self,
        inputs: &mut build_inputs::GeometryBuildInputs<'_, '_>,
    ) -> Result<Option<OrderedCompactView>, ContractError> {
        let Some(template) = self.template else {
            return Ok(None);
        };
        let table = self.table;
        let sortable = |entry: &Selection| {
            let instance = &table[entry.visible.instance_index as usize];
            entry.depth == MaterialDepth::TestAndWrite
                && (instance.blend == MaterialBlend::Opaque
                    || ((template.view.is_shadow_map() || instance.standard_lit.is_some())
                        && instance.blend == MaterialBlend::AlphaMask))
        };
        // A complete sortable phase is ordered by the full phase key below,
        // including depth/id ties. A prior depth sort cannot affect that result.
        // Keep the original barrier discovery order for mixed phases.
        if !self.selected.iter().all(&sortable) {
            let compare_depth = |a: &Selection, b: &Selection| {
                #[cfg(test)]
                COMPACT_DEPTH_COMPARISONS.with(|n| n.set(n.get() + 1));
                let ia = &table[a.visible.instance_index as usize];
                let ib = &table[b.visible.instance_index as usize];
                let alpha_a = ia.blend.is_transparent();
                let alpha_b = ib.blend.is_transparent();
                #[cfg(test)]
                if !alpha_a && !alpha_b {
                    COMPACT_OPAQUE_DEPTH_COMPARISONS.with(|n| n.set(n.get() + 1));
                }
                alpha_a
                    .cmp(&alpha_b)
                    .then_with(|| {
                        let depth = a
                            .visible
                            .camera_depth
                            .to_f32()
                            .total_cmp(&b.visible.camera_depth.to_f32());
                        if alpha_a {
                            depth.reverse()
                        } else {
                            depth
                        }
                    })
                    .then(ia.instance_id.cmp(&ib.instance_id))
            };
            if self.selected.iter().all(|entry| {
                sortable(entry)
                    || table[entry.visible.instance_index as usize]
                        .blend
                        .is_transparent()
            }) {
                // The legacy alpha flag puts every transparent row after the
                // complete sortable phase. That phase's full key already sorts
                // depth/id, so its earlier depth-only sort is overwritten.
                // Stable partition preserves equal-key input order; transparent
                // rows still use exactly the original back-to-front comparator.
                let mut partitioned = Vec::with_capacity(self.selected.len());
                partitioned.extend(self.selected.iter().filter(|e| sortable(e)).copied());
                let color_count = partitioned.len();
                partitioned.extend(self.selected.iter().filter(|e| !sortable(e)).copied());
                partitioned[color_count..].sort_by(compare_depth);
                self.selected = partitioned;
            } else {
                // A nonwriting/legacy non-alpha row is an ordering barrier.
                self.selected.sort_by(compare_depth);
            }
        }
        let prepared = inputs.prepared.filter(|p| Arc::ptr_eq(&p.table, &table));
        let ranks = prepared.and_then(|p| {
            p.phases
                .lock()
                .expect("geometry phase cache poisoned")
                .resolve(&template, &table, &p.depths, inputs)
        });
        let ranks = ranks.filter(|ranks| {
            self.selected
                .iter()
                .all(|s| ranks[s.visible.instance_index as usize].depth == s.depth)
        });
        if let Some(ranks) = &ranks {
            for entry in &mut self.selected {
                entry.sort_rank = ranks[entry.visible.instance_index as usize].rank;
            }
        } else {
            assign_phase_ranks(&mut self.selected, &template, &table, inputs);
        }
        let mut start = 0;
        while start < self.selected.len() {
            if !sortable(&self.selected[start]) {
                start += 1;
                continue;
            }
            let mut end = start + 1;
            while end < self.selected.len() && sortable(&self.selected[end]) {
                end += 1;
            }
            sort_phase_segment(
                &mut self.selected[start..end],
                &table,
                ranks.as_ref().map(|r| r.rank_count),
            );
            start = end;
        }
        let len = self.selected.len();
        Ok(Some(OrderedCompactView {
            template,
            table,
            visible: Arc::new(self.selected.iter().map(|entry| entry.visible).collect()),
            selected: Arc::new(self.selected),
            ranks,
            range: 0..len,
        }))
    }
}

// Current selection is globally ordered once, then shared by disjoint ranges
// that end only at the original batch boundaries. No mutable selection escapes.
pub(super) struct OrderedCompactView {
    template: DrawGeometry,
    table: Arc<Vec<Arc<DrawInstance>>>,
    selected: Arc<Vec<Selection>>,
    visible: Arc<Vec<GeometryVisibleInstance>>,
    ranks: Option<Arc<PreparedPhase>>,
    range: std::ops::Range<usize>,
}
impl OrderedCompactView {
    // Decide once over the entire ordered view, before partitioning. A mixed
    // view must retain its original batch representation in every task.
    pub(super) fn all_singletons(&self) -> bool {
        let Some(ranks) = &self.ranks else {
            return false;
        };
        self.selected[self.range.clone()].windows(2).all(|pair| {
            pair[0].depth != pair[1].depth
                || ranks[pair[0].visible.instance_index as usize].compatibility
                    != ranks[pair[1].visible.instance_index as usize].compatibility
        })
    }

    fn select_draw(&self, entry: &Selection) -> DrawGeometry {
        #[cfg(test)]
        COMPACT_DESCRIPTOR_BUILDS.with(|n| n.set(n.get() + 1));
        let mut draw = self.template.clone();
        draw.instance = self.table[entry.visible.instance_index as usize].clone();
        draw.camera_depth = entry.visible.camera_depth;
        draw.depth = entry.depth;
        draw
    }

    pub(super) fn finish_color_unpublished(
        &self,
        inputs: &mut build_inputs::GeometryBuildInputs<'_, '_>,
        state: &FinalViewState,
        direct: bool,
    ) -> Result<ColorOutput, ContractError> {
        if !direct {
            return self
                .finish_unpublished(inputs, state)
                .map(ColorOutput::Batches);
        }
        let selected = &self.selected[self.range.clone()];
        let visible = &self.visible[self.range.clone()];
        let mut draws: Vec<_> = selected
            .iter()
            .map(|entry| Arc::new(self.select_draw(entry)))
            .collect();
        {
            let mut validation = DrawViewValidation::default();
            for (draw, entry) in draws.iter().zip(visible) {
                GeometryBatch::validate_selection_parts(
                    draw,
                    &self.table,
                    std::slice::from_ref(entry),
                    &mut validation,
                    self.ranks.as_deref(),
                )?;
            }
        }
        inputs.finalize_commands(
            CommandHeads::Draws {
                draws: &mut draws,
                table: &self.table,
                visible,
            },
            state,
            self.ranks.as_deref(),
        );
        let mut validation = DrawViewValidation::default();
        for draw in &draws {
            proof::draw_after_selection(draw, &mut validation)?;
        }
        Ok(ColorOutput::Draws(draws))
    }

    pub(super) fn partitions(self, limit: usize) -> Vec<Self> {
        let Some(ranks) = &self.ranks else {
            return vec![self];
        };
        if self.range.is_empty() || limit <= 1 {
            return vec![self];
        }
        let mut boundaries = vec![self.range.start];
        for i in self.range.start + 1..self.range.end {
            let a = &self.selected[i - 1];
            let b = &self.selected[i];
            if a.depth != b.depth
                || ranks[a.visible.instance_index as usize].compatibility
                    != ranks[b.visible.instance_index as usize].compatibility
            {
                boundaries.push(i);
            }
        }
        boundaries.push(self.range.end);
        let groups = boundaries.len() - 1;
        let count = limit.min(groups);
        (0..count)
            .map(|i| Self {
                template: self.template.clone(),
                table: self.table.clone(),
                selected: self.selected.clone(),
                visible: self.visible.clone(),
                ranks: self.ranks.clone(),
                range: boundaries[groups * i / count]..boundaries[groups * (i + 1) / count],
            })
            .collect()
    }
    #[cfg(test)]
    pub(super) fn finish(
        &self,
        inputs: &mut build_inputs::GeometryBuildInputs<'_, '_>,
        state: &FinalViewState,
    ) -> Result<Vec<Arc<GeometryBatch>>, ContractError> {
        self.finish_unpublished(inputs, state).map(share_batches)
    }

    pub(super) fn finish_unpublished(
        &self,
        inputs: &mut build_inputs::GeometryBuildInputs<'_, '_>,
        state: &FinalViewState,
    ) -> Result<Vec<GeometryBatch>, ContractError> {
        let table = &self.table;
        let ranks = &self.ranks;
        let selected = &self.selected[self.range.clone()];
        let mut batches: Vec<GeometryBatch> = Vec::new();
        let mut start = 0;
        while start < selected.len() {
            let entry = &selected[start];
            let draw = self.select_draw(entry);
            let mut end = start + 1;
            while end < selected.len() {
                let next = &selected[end];
                let compatible = if let Some(ranks) = ranks {
                    ranks[entry.visible.instance_index as usize].compatibility
                        == ranks[next.visible.instance_index as usize].compatibility
                } else {
                    GeometryBatch::compatible_instance(
                        &draw,
                        &table[next.visible.instance_index as usize],
                    )
                };
                if draw.depth != next.depth || !compatible {
                    break;
                }
                end += 1;
            }
            let mut batch = GeometryBatch::new(Arc::new(draw), table.clone(), Vec::new());
            batch.visible = GeometryVisibleInstances::shared(
                self.visible.clone(),
                (self.range.start + start)..(self.range.start + end),
            );
            batches.push(batch);
            start = end;
        }
        // Validate the complete unpublished selection with its original phase
        // context. Only final view attachments change below; instance/view/depth,
        // table and the full visible sequence cannot be changed by FinalViewState.
        {
            let mut validation = DrawViewValidation::default();
            for batch in &batches {
                if let Some(prepared) = ranks {
                    batch.validate_prepared_selection(&mut validation, prepared)?;
                } else {
                    batch.validate_with_view(&mut validation)?;
                }
            }
        }
        inputs.finalize_batch_commands(&mut batches, state, ranks.as_deref());
        let mut validation = DrawViewValidation::default();
        for batch in &batches {
            proof::draw_after_selection(&batch.draw, &mut validation)?;
        }
        Ok(batches)
    }
}

// A single adapter keeps command lookup/attachment identical for both outputs.
pub(super) enum CommandHeads<'a> {
    Batches(&'a mut [GeometryBatch]),
    Draws {
        draws: &'a mut [Arc<DrawGeometry>],
        table: &'a Arc<Vec<Arc<DrawInstance>>>,
        visible: &'a [GeometryVisibleInstance],
    },
}
impl CommandHeads<'_> {
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Batches(batches) => batches.len(),
            Self::Draws { draws, .. } => draws.len(),
        }
    }
    pub(super) fn draw(&self, index: usize) -> &DrawGeometry {
        match self {
            Self::Batches(batches) => &batches[index].draw,
            Self::Draws { draws, .. } => &draws[index],
        }
    }
    pub(super) fn draw_mut(&mut self, index: usize) -> &mut DrawGeometry {
        let draw = match self {
            Self::Batches(batches) => &mut batches[index].draw,
            Self::Draws { draws, .. } => &mut draws[index],
        };
        Arc::get_mut(draw).expect("unpublished output exclusively owns its head")
    }
    pub(super) fn command_slot<'p>(
        &self,
        index: usize,
        phase: &'p PreparedPhase,
    ) -> Option<&'p std::sync::OnceLock<Arc<retained_commands::RetainedDrawCommand>>> {
        match self {
            Self::Batches(batches) => phase.command_slot(&batches[index]),
            Self::Draws {
                draws,
                table,
                visible,
            } => phase.command_slot_parts(&draws[index], table, visible[index].instance_index),
        }
    }
    pub(super) fn mark_unique(&mut self, table: &Arc<Vec<Arc<DrawInstance>>>) {
        if let Self::Batches(batches) = self {
            for batch in batches.iter_mut() {
                if Arc::ptr_eq(&batch.instances, table) {
                    batch.unique_table = Some(Arc::downgrade(table));
                }
            }
        }
    }
}

pub(super) enum ColorOutput {
    Draws(Vec<Arc<DrawGeometry>>),
    Batches(Vec<GeometryBatch>),
}
impl ColorOutput {
    pub(super) fn empty(direct: bool) -> Self {
        if direct {
            Self::Draws(Vec::new())
        } else {
            Self::Batches(Vec::new())
        }
    }
    pub(super) fn append(&mut self, other: Self) -> Result<(), ContractError> {
        match (self, other) {
            (Self::Draws(target), Self::Draws(values)) => target.extend(values),
            (Self::Batches(target), Self::Batches(values)) => target.extend(values),
            _ => return Err(invalid("geometry.color_output_mismatch")),
        }
        Ok(())
    }
}

fn share_batches(batches: Vec<GeometryBatch>) -> Vec<Arc<GeometryBatch>> {
    batches
        .into_iter()
        .map(|batch| {
            let batch = Arc::new(batch);
            proof::publish_complete_selection(&batch);
            batch
        })
        .collect()
}

// Every complete selection and final draw has already passed the same checks.
// Retained views whose groups are all singletons can publish the existing scalar
// draw representation directly. Actual instancing and mixed groups keep batches.
// No batch Arc or second array of head references is needed for independent draws.
pub(super) fn publish_color(frame: &mut GeometryViewFrame, output: ColorOutput, retained: bool) {
    let batches = match output {
        ColorOutput::Draws(draws) => {
            frame.draws = draws;
            frame.batches = None;
            return;
        }
        ColorOutput::Batches(batches) => batches,
    };
    if retained && batches.iter().all(|batch| batch.instance_count() == 1) {
        frame.draws = batches.into_iter().map(|batch| batch.draw).collect();
        frame.batches = None;
    } else {
        let batches = share_batches(batches);
        frame.draws = batches.iter().map(|batch| batch.draw.clone()).collect();
        frame.batches = Some(batches);
    }
}

#[cfg(test)]
thread_local! {static FLOW_RANK_COMPARISONS:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};static FLOW_VISIBLE_GROWS:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}
#[cfg(test)]
#[derive(Eq, PartialEq)]
struct CountedRank(usize);
#[cfg(test)]
impl Ord for CountedRank {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        FLOW_RANK_COMPARISONS.with(|n| n.set(n.get() + 1));
        self.0.cmp(&other.0)
    }
}
#[cfg(test)]
impl PartialOrd for CountedRank {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
#[cfg(test)]
fn rank_key(rank: usize) -> CountedRank {
    CountedRank(rank)
}
#[cfg(not(test))]
fn rank_key(rank: usize) -> usize {
    rank
}
// Existing phase ranks are dense static order facts, not visibility facts.
// Bound the rank scan relative to this barrier segment; sparse/tiny segments
// retain the old comparison path rather than repeatedly scanning the table.
fn sort_phase_segment(
    selected: &mut [Selection],
    table: &[Arc<DrawInstance>],
    rank_count: Option<usize>,
) {
    let Some(rank_count) =
        rank_count.filter(|&n| n > 0 && n <= 65_536 && n <= selected.len().saturating_mul(8))
    else {
        return sort_phase_segment_fallback(selected, table, None);
    };
    let mut offsets = vec![0usize; rank_count];
    for entry in selected.iter() {
        offsets[entry.sort_rank] += 1;
    }
    let mut total = 0;
    for offset in &mut offsets {
        let count = *offset;
        *offset = total;
        total += count;
    }
    let mut ordered = vec![selected[0]; selected.len()];
    for entry in selected.iter().copied() {
        ordered[offsets[entry.sort_rank]] = entry;
        offsets[entry.sort_rank] += 1;
    }
    let mut start = 0;
    for end in offsets {
        // Rank bucketing already made the expensive state key constant. Only
        // two scalar fields remain; a cached-key Vec for every small phase
        // costs more work than reading those scalars during a stable sort.
        ordered[start..end].sort_by(|a, b| {
            DepthOrder(a.visible.camera_depth)
                .cmp(&DepthOrder(b.visible.camera_depth))
                .then_with(|| {
                    table[a.visible.instance_index as usize]
                        .instance_id
                        .cmp(&table[b.visible.instance_index as usize].instance_id)
                })
        });
        start = end;
    }
    selected.copy_from_slice(&ordered);
}
fn sort_phase_segment_fallback(
    selected: &mut [Selection],
    table: &[Arc<DrawInstance>],
    _rank_count: Option<usize>,
) {
    selected.sort_by_cached_key(|entry| {
        #[cfg(test)]
        PUBLICATION_SORT_KEYS.with(|n| n.set(n.get() + 1));
        (
            rank_key(entry.sort_rank),
            DepthOrder(entry.visible.camera_depth),
            table[entry.visible.instance_index as usize].instance_id,
        )
    });
}

fn assign_phase_ranks(
    selected: &mut [Selection],
    template: &DrawGeometry,
    table: &Arc<Vec<Arc<DrawInstance>>>,
    inputs: &mut build_inputs::GeometryBuildInputs<'_, '_>,
) {
    let sortable = |entry: &Selection| {
        let instance = &table[entry.visible.instance_index as usize];
        entry.depth == MaterialDepth::TestAndWrite
            && (instance.blend == MaterialBlend::Opaque
                || ((template.view.is_shadow_map() || instance.standard_lit.is_some())
                    && instance.blend == MaterialBlend::AlphaMask))
    };
    // Sorting needs only a borrowed interpretation of one row at a time.
    // Reuse a single scratch descriptor rather than cloning target strings
    // and all shared state for every sort key in every shadow/color view.
    #[cfg(test)]
    COMPACT_DESCRIPTOR_BUILDS.with(|n| n.set(n.get() + 1));
    let mut scratch = template.clone();
    let mut states = std::collections::HashMap::new();
    let mut values = Vec::new();
    for entry in selected.iter_mut() {
        if !sortable(entry) {
            continue;
        }
        let instance = &table[entry.visible.instance_index as usize];
        let position_only = template.view.is_shadow_map()
            && instance.blend == MaterialBlend::Opaque
            && entry.depth == MaterialDepth::TestAndWrite;
        let resources = if position_only {
            // Match the existing position-only pipeline/batch recipe.
            // This is sorting only: full tail binding/provenance and
            // compatibility checks below remain mandatory.
            scratch.instance = instance.clone();
            SelectionResources::PositionOnly {
                geometry: Arc::as_ptr(&instance.geometry),
                handle: scratch.geometry_handle(),
                first_index: instance.first_index,
                index_count: instance.index_count,
            }
        } else {
            SelectionResources::Color(Arc::as_ptr(&instance.resources))
        };
        let key = SelectionState {
            resources,
            depth: entry.depth,
            blend: instance.blend,
            cull: instance.cull,
            mirrored: instance.mirrored(),
            decal_bias: if position_only {
                None
            } else {
                instance.decal_bias
            },
            opaque_alpha: !position_only && instance.opaque_alpha,
            ignore_vertex_color: !position_only && instance.ignore_vertex_color,
            standard_lit: !position_only && instance.standard_lit.is_some(),
        };
        entry.sort_rank = *states.entry(key).or_insert_with(|| {
            scratch.instance = instance.clone();
            scratch.camera_depth = entry.visible.camera_depth;
            scratch.depth = entry.depth;
            let draw = &scratch;
            #[cfg(test)]
            COMPACT_STATE_RESOLUTIONS.with(|n| n.set(n.get() + 1));
            let (pipeline, batch) = inputs.compact_batch_key(draw);
            let color = !draw.is_opaque_shadow_depth_writer();
            let slot = values.len();
            values.push(SharedSortState(Arc::new((
                draw.blend == MaterialBlend::AlphaMask,
                pipeline,
                color
                    .then(|| draw.material_textures()[0].map(|h| h.raw()))
                    .flatten(),
                color.then(|| draw.material_binding.binding_id.clone()),
                batch,
            ))));
            slot
        });
    }
    // Rank complete keys once, not once per comparison of selected rows.
    // Distinct storage/state candidates can have equal complete keys: they
    // must receive the same rank so depth/id still break those ties.
    let mut order: Vec<_> = (0..values.len()).collect();
    order.sort_by(|a, b| values[*a].cmp(&values[*b]));
    let mut ranks = vec![0; values.len()];
    let mut rank = 0;
    for (i, slot) in order.iter().copied().enumerate() {
        if i > 0 && values[order[i - 1]] != values[slot] {
            rank += 1;
        }
        ranks[slot] = rank;
    }
    for entry in selected.iter_mut() {
        if sortable(entry) {
            entry.sort_rank = ranks[entry.sort_rank];
        }
    }
}

// Only scalar membership/order facts survive a frame. The owning PreparedInputs
// retains the exact immutable table; no view matrix, selected subset or lease
// validity is cached here. These are exactly the template inputs used by the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PhaseContext {
    shadow: bool,
    domain: RenderDomain,
    depth_target: bool,
    color_pipeline: bool,
    environment: bool,
    lighting: bool,
    shadow_mask: u8,
}
impl PhaseContext {
    fn of(draw: &DrawGeometry) -> Self {
        Self {
            shadow: draw.view.is_shadow_map(),
            domain: draw.view.domain,
            depth_target: draw.depth_target.is_some(),
            color_pipeline: draw.view.color_pipeline.is_some(),
            environment: draw.environment.is_some(),
            lighting: draw.lighting.is_some(),
            shadow_mask: draw.shadows.as_ref().map_or(0, |s| s.texture_mask()),
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub(super) struct PhaseMember {
    rank: usize,
    depth: MaterialDepth,
    compatibility: usize,
}

/// Full-table static facts, never a new selection or native admission proof.
#[derive(Debug)]
pub(super) struct PreparedPhase {
    table: std::sync::Weak<Vec<Arc<DrawInstance>>>,
    context: PhaseContext,
    members: Vec<PhaseMember>,
    commands: Vec<std::sync::OnceLock<Arc<retained_commands::RetainedDrawCommand>>>,
    rank_count: usize,
    world_magnitude: f64,
}
impl std::ops::Deref for PreparedPhase {
    type Target = [PhaseMember];
    fn deref(&self) -> &Self::Target {
        &self.members
    }
}
impl PreparedPhase {
    pub(super) fn command_slot(
        &self,
        batch: &GeometryBatch,
    ) -> Option<&std::sync::OnceLock<Arc<retained_commands::RetainedDrawCommand>>> {
        self.command_slot_parts(
            &batch.draw,
            &batch.instances,
            batch.visible.first()?.instance_index,
        )
    }
    fn command_slot_parts(
        &self,
        draw: &DrawGeometry,
        table: &Arc<Vec<Arc<DrawInstance>>>,
        row: u32,
    ) -> Option<&std::sync::OnceLock<Arc<retained_commands::RetainedDrawCommand>>> {
        if self.table.as_ptr() != Arc::as_ptr(table) {
            return None;
        }
        let row = row as usize;
        if !Arc::ptr_eq(table.get(row)?, &draw.instance) {
            return None;
        }
        self.commands.get(row)
    }
    fn matches_parts(&self, draw: &DrawGeometry, table: &Arc<Vec<Arc<DrawInstance>>>) -> bool {
        self.table.as_ptr() == Arc::as_ptr(table)
            && self.context == PhaseContext::of(draw)
            && self.members.len() == table.len()
    }
    #[cfg(test)]
    fn matches(&self, batch: &GeometryBatch) -> bool {
        self.matches_parts(&batch.draw, &batch.instances)
    }
}
#[derive(Debug, Default)]
pub(super) struct PhaseCache {
    table: std::sync::Weak<Vec<Arc<DrawInstance>>>,
    entries: Vec<(PhaseContext, Arc<PreparedPhase>)>,
}
impl PhaseCache {
    fn values_validated(&self, table: &Arc<Vec<Arc<DrawInstance>>>) -> bool {
        self.table.as_ptr() == Arc::as_ptr(table)
            && self.entries.first().is_some_and(|(_, phase)| {
                phase.table.as_ptr() == Arc::as_ptr(table) && phase.members.len() == table.len()
            })
    }
    fn resolve(
        &mut self,
        template: &DrawGeometry,
        table: &Arc<Vec<Arc<DrawInstance>>>,
        depths: &[MaterialDepth],
        inputs: &mut build_inputs::GeometryBuildInputs<'_, '_>,
    ) -> Option<Arc<PreparedPhase>> {
        if table.len() > 65_536 || depths.len() != table.len() {
            return None;
        }
        if self
            .table
            .upgrade()
            .is_none_or(|old| !Arc::ptr_eq(&old, table))
        {
            self.entries.clear();
            self.table = Arc::downgrade(table);
        }
        let context = PhaseContext::of(template);
        if let Some((_, ranks)) = self.entries.iter().find(|(key, _)| *key == context) {
            return Some(ranks.clone());
        }
        // Hidden malformed inputs must not become a new error/panic just because
        // a persistent bin considers all rows. Keep the original selected-only
        // interpretation when a full-table fact cannot be established.
        if table.iter().any(|row| {
            proof::instance(row).is_err()
                || row.geometry_binding.binding_kind != RenderBindingKind::Mesh
                || !row.geometry_binding.slots.iter().any(|slot| {
                    slot.name == "geometry" && matches!(slot.resource, BindingResource::Geometry(_))
                })
        }) {
            return None;
        }
        // All rows must pass before any proof is published. Hidden invalid
        // rows fall back to selected-only validation, preserving that contract.
        let mut validation = DrawViewValidation::default();
        if table
            .iter()
            .any(|row| validate_static_row(row, &mut validation).is_err())
        {
            return None;
        }
        let mut selected: Vec<_> = depths
            .iter()
            .enumerate()
            .map(|(index, depth)| Selection {
                visible: GeometryVisibleInstance {
                    instance_index: index as u32,
                    camera_depth: 0f32.into(),
                },
                depth: if context.shadow {
                    MaterialDepth::TestAndWrite
                } else {
                    *depth
                },
                sort_rank: 0,
            })
            .collect();
        assign_phase_ranks(&mut selected, template, table, inputs);
        let groups = compatibility_groups(template, table, &selected);
        let members: Vec<_> = selected
            .into_iter()
            .zip(groups)
            .map(|(s, compatibility)| PhaseMember {
                rank: s.sort_rank,
                depth: s.depth,
                compatibility,
            })
            .collect();
        let ranks = Arc::new(PreparedPhase {
            table: Arc::downgrade(table),
            context,
            rank_count: members.iter().map(|m| m.rank).max().map_or(0, |n| n + 1),
            commands: (0..members.len())
                .map(|_| std::sync::OnceLock::new())
                .collect(),
            members,
            world_magnitude: table
                .iter()
                .flat_map(|row| row.world_transform.iter())
                .map(|v| f64::from(v.to_f32()).abs())
                .fold(0., f64::max),
        });
        proof::prepared_instances(table);
        if self.entries.len() < 16 {
            self.entries.push((context, ranks.clone()));
        }
        Some(ranks)
    }
    pub(super) fn observe_memory(&self, sample: &mut crate::resource_memory::ResourceMemorySample) {
        sample.partial(
            "cpu.owner.renderer",
            "staticInstanceProofWeakHashBucketsAndAllocationBackingNotInstrumented",
        );
        sample.cpu_vec(&self.entries, "snapshots", &["snapshot"]);
        for (_, ranks) in &self.entries {
            sample.cpu_arc(ranks, "snapshots", &["snapshot"]);
            sample.cpu_vec(&ranks.members, "snapshots", &["snapshot"]);
            sample.cpu_vec(&ranks.commands, "snapshots", &["snapshot"]);
            for command in ranks.commands.iter().filter_map(std::sync::OnceLock::get) {
                sample.cpu_arc(command, "snapshots", &["snapshot"]);
                command.observe_memory(sample, "snapshot");
            }
        }
    }
}

// Each dot has four products and four prefix additions. /16 leaves a
// wide rounding margin for every intermediate, including cancellation.
fn product_is_finite(projection: [f32; 16], world_magnitude: f64) -> bool {
    #[cfg(test)]
    FINITE_BOUND_EVALUATIONS.with(|n| n.set(n.get() + 1));
    projection.iter().all(|v| v.is_finite())
        && world_magnitude.is_finite()
        && projection
            .iter()
            .map(|v| f64::from(*v).abs())
            .fold(0., f64::max)
            * world_magnitude
            < f64::from(f32::MAX) / 16.
}

fn validate_static_row<'a>(
    row: &'a DrawInstance,
    validation: &mut DrawViewValidation<'a>,
) -> Result<(), ContractError> {
    validation.validate_binding(&row.geometry_binding)?;
    validation.validate_binding(&row.material_binding)?;
    if row.geometry_binding.binding_kind != RenderBindingKind::Mesh
        || row.material_binding.binding_kind != RenderBindingKind::Material
        || row.index_count == 0
        || !row.index_count.is_multiple_of(3)
        || !row.first_index.is_multiple_of(3)
        || row
            .first_index
            .checked_add(row.index_count)
            .is_none_or(|n| n > row.geometry.index_count())
    {
        return Err(invalid("drawGeometry"));
    }
    if row.standard_lit.is_some() {
        if row.resources.textures().iter().any(Option::is_some)
            && !row
                .geometry
                .attributes()
                .iter()
                .any(|a| a.semantic == VertexSemantic::TexCoord0)
        {
            return Err(invalid("drawGeometry.uvRequired"));
        }
        if !row
            .geometry
            .attributes()
            .iter()
            .any(|a| a.semantic == VertexSemantic::Normal)
        {
            return Err(invalid("drawGeometry.normal"));
        }
    }
    Ok(())
}

fn compatibility_groups(
    template: &DrawGeometry,
    table: &Arc<Vec<Arc<DrawInstance>>>,
    selected: &[Selection],
) -> Vec<usize> {
    let mut memo = std::collections::HashMap::new();
    let mut candidates = std::collections::HashMap::<_, Vec<usize>>::new();
    let mut scratch = template.clone();
    selected
        .iter()
        .map(|s| {
            let index = s.visible.instance_index as usize;
            let row = &table[index];
            let key = SelectionState {
                resources: SelectionResources::Color(Arc::as_ptr(&row.resources)),
                depth: s.depth,
                blend: row.blend,
                cull: row.cull,
                mirrored: row.mirrored(),
                decal_bias: row.decal_bias,
                opaque_alpha: row.opaque_alpha,
                ignore_vertex_color: row.ignore_vertex_color,
                standard_lit: row.standard_lit.is_some(),
            };
            *memo.entry(key).or_insert_with(|| {
                scratch.instance = row.clone();
                scratch.depth = s.depth;
                // Typed handle/range only narrows candidates. Full equality below
                // proves the class, including detached storage and provenance.
                let bucket = candidates
                    .entry((
                        scratch.geometry_handle(),
                        row.first_index,
                        row.index_count,
                        s.depth,
                    ))
                    .or_default();
                if let Some(&representative) = bucket
                    .iter()
                    .find(|&&r| GeometryBatch::compatible_instance(&scratch, &table[r]))
                {
                    representative
                } else {
                    bucket.push(index);
                    index
                }
            })
        })
        .collect()
}

impl DrawInstance {
    fn mirrored(&self) -> bool {
        let m = self.world_transform.map(|v| f64::from(v.to_f32()));
        let det = m[0] * (m[5] * m[10] - m[9] * m[6]) - m[4] * (m[1] * m[10] - m[9] * m[2])
            + m[8] * (m[1] * m[6] - m[5] * m[2]);
        det < 0.
    }
    pub(super) fn validate_values(&self) -> Result<(), ContractError> {
        if let Some(skin)=&self.deformation {
            skin.validate().map_err(|_|invalid("drawGeometry.skin"))?;
            if skin.source.vertex_count!=self.geometry.vertex_count(){return Err(invalid("drawGeometry.skin.vertexCount"));}
        }
        if self.blend.is_premultiplied()
            && (self.standard_lit.is_some() || self.decal_bias.is_some())
        {
            return Err(invalid("drawGeometry.premultipliedPolicy"));
        }
        validate_world(self.world_transform.map(OrderedF32::to_f32))?;
        finite_draw(&self.base_color.map(OrderedF32::to_f32))?;
        if (self.blend == MaterialBlend::AlphaMask) != self.alpha_cutoff.is_some()
            || self
                .alpha_cutoff
                .is_some_and(|v| !v.to_f32().is_finite() || !(0.0..=1.0).contains(&v.to_f32()))
        {
            return Err(invalid("drawGeometry.alphaCutoff"));
        }
        if self.opaque_alpha && self.blend != MaterialBlend::Opaque {
            return Err(invalid("drawGeometry.opaqueAlpha"));
        }
        if let Some(lit) = &self.standard_lit {
            lit.validate()?;
            if self
                .base_color
                .iter()
                .any(|v| !(0.0..=1.0).contains(&v.to_f32()))
            {
                return Err(invalid("drawGeometry.baseColor"));
            }
        }
        Ok(())
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct DepthOrder(OrderedF32);
impl Ord for DepthOrder {
    fn cmp(&self, b: &Self) -> std::cmp::Ordering {
        self.0.to_f32().total_cmp(&b.0.to_f32())
    }
}
impl PartialOrd for DepthOrder {
    fn partial_cmp(&self, b: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(b))
    }
}

impl GeometryViewFrame {
    pub(super) fn refresh_batch_draws(&mut self) {
        if let Some(batches) = &mut self.batches {
            for (batch, draw) in batches.iter_mut().zip(&self.draws) {
                proof::refresh_selection_draw(batch, draw);
            }
        }
    }
    #[cfg(test)]
    pub(crate) fn selected_instances(
        &self,
    ) -> Box<dyn Iterator<Item = (&DrawGeometry, &Arc<DrawInstance>, OrderedF32)> + '_> {
        if let Some(batches) = &self.batches {
            Box::new(batches.iter().flat_map(|batch| {
                batch.visible.iter().map(|entry| {
                    (
                        batch.draw.as_ref(),
                        &batch.instances[entry.instance_index as usize],
                        entry.camera_depth,
                    )
                })
            }))
        } else {
            Box::new(
                self.draws
                    .iter()
                    .map(|draw| (draw.as_ref(), &draw.instance, draw.camera_depth)),
            )
        }
    }

    #[cfg(test)]
    pub(crate) fn selected_draws(
        &self,
    ) -> Box<dyn Iterator<Item = std::borrow::Cow<'_, DrawGeometry>> + '_> {
        if let Some(batches) = &self.batches {
            Box::new(batches.iter().flat_map(|batch| {
                batch.visible.iter().map(|entry| {
                    std::borrow::Cow::Owned(batch.selected_draw(
                        &batch.instances[entry.instance_index as usize],
                        entry.camera_depth,
                    ))
                })
            }))
        } else {
            Box::new(
                self.draws
                    .iter()
                    .map(|draw| std::borrow::Cow::Borrowed(draw.as_ref())),
            )
        }
    }
}

// A packet shares a full table in memory. A self-contained wire batch only
// publishes referenced rows, avoiding table_size * batch_count JSON growth.
impl Serialize for GeometryBatch {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Wire<'a> {
            schema_version: &'a str,
            draw: &'a Arc<DrawGeometry>,
            instances: Vec<&'a Arc<DrawInstance>>,
            visible: Vec<GeometryVisibleInstance>,
        }
        let mut selected = std::collections::HashMap::new();
        let mut instances = Vec::new();
        let mut visible = Vec::with_capacity(self.visible.len());
        for entry in &self.visible {
            let value = self
                .instances
                .get(entry.instance_index as usize)
                .ok_or_else(|| serde::ser::Error::custom("geometry.batch.instance_index"))?;
            let index = *selected.entry(entry.instance_index).or_insert_with(|| {
                let index = instances.len() as u32;
                instances.push(value);
                index
            });
            visible.push(GeometryVisibleInstance {
                instance_index: index,
                camera_depth: entry.camera_depth,
            });
        }
        Wire {
            schema_version: &self.schema_version,
            draw: &self.draw,
            instances,
            visible,
        }
        .serialize(serializer)
    }
}
impl PartialEq for GeometryBatch {
    fn eq(&self, other: &Self) -> bool {
        self.schema_version == other.schema_version
            && self.draw == other.draw
            && self.visible.len() == other.visible.len()
            && self.visible.iter().zip(&other.visible).all(|(a, b)| {
                a.camera_depth == b.camera_depth
                    && match (
                        self.instances.get(a.instance_index as usize),
                        other.instances.get(b.instance_index as usize),
                    ) {
                        (Some(a), Some(b)) => a == b,
                        (None, None) => a.instance_index == b.instance_index,
                        _ => false,
                    }
            })
    }
}
impl Eq for GeometryBatch {}
impl std::hash::Hash for GeometryBatch {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.schema_version.hash(state);
        self.draw.hash(state);
        self.visible.len().hash(state);
        for entry in &self.visible {
            entry.camera_depth.hash(state);
            match self.instances.get(entry.instance_index as usize) {
                Some(instance) => {
                    true.hash(state);
                    instance.hash(state);
                }
                None => {
                    false.hash(state);
                    entry.instance_index.hash(state);
                }
            }
        }
    }
}

#[cfg(test)]
mod dynamic_consumption_tests {
    use super::*;
    #[test]
    fn dynamic_selection_scratch_avoids_per_batch_hash_growth_at_multiple_scales() {
        let mut rows = Vec::new();
        for count in [4, 32, 256] {
            let frame = tests::batch_test_frame(true, MaterialBlend::Opaque, count, 1);
            let batches = frame.views[0].batches.as_ref().unwrap();
            let mut validation = DrawViewValidation::default();
            DYNAMIC_SELECTION_GROWS.with(|n| n.set(0));
            for _ in 0..32 {
                for batch in batches {
                    batch.validate_with_view(&mut validation).unwrap();
                }
            }
            rows.push((count, DYNAMIC_SELECTION_GROWS.with(|n| n.get())));
        }
        eprintln!("dynamic selection actual HashSet capacity growth {rows:?}");
        assert!(
            rows.iter().all(|&(_, n)| n == 0),
            "per-batch allocation repeated: {rows:?}"
        );
    }
}

#[cfg(test)]
mod dynamic_selection_boundaries {
    use super::*;
    #[test]
    fn dynamic_selection_epoch_wrap_and_large_table_keep_strict_fallback() {
        let mut scratch = SelectionScratch::default();
        assert!(scratch.begin(3));
        assert!(scratch.insert(1));
        assert!(!scratch.insert(1));
        scratch.epoch = u32::MAX;
        assert!(scratch.begin(3));
        assert!(scratch.insert(1));
        assert!(!scratch.insert(1));
        assert!(!scratch.begin(65_537));
        assert_eq!(scratch.marks.len(), 3);
        assert!(scratch.begin(40_000));
        assert!(scratch.begin(65_536));
        assert!(scratch.marks.capacity() <= 65_536);
        let frame = tests::batch_test_frame(true, MaterialBlend::Opaque, 4, 1);
        let source = &frame.views[0].batches.as_ref().unwrap()[0];
        let mut batch = source.as_ref().clone();
        let first = batch.instances[0].clone();
        Arc::make_mut(&mut batch.instances).resize(65_537, first);
        let mut validation = DrawViewValidation::default();
        DYNAMIC_SELECTION_GROWS.with(|n| n.set(0));
        batch.validate_with_view(&mut validation).unwrap();
        assert!(
            DYNAMIC_SELECTION_GROWS.with(|n| n.get()) > 0,
            "large table must use bounded strict fallback"
        );
        drop(validation);
        batch.visible.push(batch.visible[0]);
        let mut validation = DrawViewValidation::default();
        assert!(batch.validate_with_view(&mut validation).is_err());
        source.validate_with_view(&mut validation).unwrap();
    }
}

#[cfg(test)]
thread_local! {static PUBLICATION_SORT_KEYS:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}
#[cfg(test)]
mod publication_cost_tests {
    use super::*;
    #[test]
    fn phase_depth_sort_does_not_materialize_cached_scalar_keys() {
        let frame = tests::batch_test_frame(true, MaterialBlend::Opaque, 256, 1);
        let table = &frame.views[0].batches.as_ref().unwrap()[0].instances;
        let mut observed = Vec::new();
        for count in [4usize, 32, 256] {
            let mut selected: Vec<_> = (0..count)
                .rev()
                .map(|i| Selection {
                    visible: GeometryVisibleInstance {
                        instance_index: i as u32,
                        camera_depth: f32::from_bits(
                            [
                                0x80000000, 0, 0x7fc00001, 0xffc00001, 0x7f800000, 0xff800000, 1,
                                0x3f800000,
                            ][i % 8],
                        )
                        .into(),
                    },
                    depth: MaterialDepth::TestAndWrite,
                    sort_rank: 0,
                })
                .collect();
            let mut expected = selected.clone();
            expected.sort_by_cached_key(|entry| {
                (
                    DepthOrder(entry.visible.camera_depth),
                    table[entry.visible.instance_index as usize].instance_id,
                )
            });
            PUBLICATION_SORT_KEYS.with(|n| n.set(0));
            sort_phase_segment(&mut selected, table, Some(1));
            observed.push((count, PUBLICATION_SORT_KEYS.with(|n| n.get())));
            assert_eq!(
                selected
                    .iter()
                    .map(|s| (
                        s.visible.instance_index,
                        s.visible.camera_depth.to_f32().to_bits()
                    ))
                    .collect::<Vec<_>>(),
                expected
                    .iter()
                    .map(|s| (
                        s.visible.instance_index,
                        s.visible.camera_depth.to_f32().to_bits()
                    ))
                    .collect::<Vec<_>>()
            );
        }
        eprintln!("phase actual cached scalar key materializations: {observed:?}");
        assert!(
            observed.iter().all(|(_, keys)| *keys == 0),
            "per-phase cached key scratch remains: {observed:?}"
        );
    }
    #[test]
    fn publication_phase_keys_are_built_once_per_selected_row_at_multiple_scales() {
        let mut rows = Vec::new();
        for count in [4, 32, 256] {
            PUBLICATION_SORT_KEYS.with(|n| n.set(0));
            let actual = tests::batch_test_frame(true, MaterialBlend::Opaque, count, 1);
            let keys = PUBLICATION_SORT_KEYS.with(|n| n.get());
            let reference = tests::batch_test_frame(false, MaterialBlend::Opaque, count, 1);
            assert_eq!(
                actual.views[0].selected_draws().collect::<Vec<_>>(),
                reference.views[0].selected_draws().collect::<Vec<_>>()
            );
            rows.push((count, keys));
        }
        eprintln!("publication actual sort key builds {rows:?}");
        assert!(
            rows.iter().all(|&(n, keys)| keys <= n),
            "phase key rereads table on every comparison: {rows:?}"
        );
    }
}

#[cfg(test)]
thread_local! {static FINITE_BOUND_EVALUATIONS:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}
#[cfg(test)]
mod production_consumption_tests {
    use super::*;
    #[test]
    fn dynamic_consumption_static_table_success_rejects_cow_and_unprepared_rows() {
        let (manager, source, view, _) = shadow::receiver_scene(MaterialBlend::Opaque, -100.);
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
        let head = &frame.views[0].draws[0];
        assert!(
            CompactView::new(prepared.table.clone())
                .with_prepared(Some(&prepared))
                .values_validated
        );
        assert!(!CompactView::new(prepared.table.clone()).values_validated);
        let mut changed = prepared.table.clone();
        Arc::make_mut(&mut Arc::make_mut(&mut changed)[0]).world_transform[0] = f32::NAN.into();
        let mut builder = CompactView::new(changed).with_prepared(Some(&prepared));
        assert!(
            !builder.values_validated,
            "COW table inherited old static values"
        );
        assert!(builder
            .push(0, head.depth, head.camera_depth, || head.as_ref().clone())
            .is_err());
        assert!(
            prepared.table[0].world_transform[0].to_f32().is_finite(),
            "old in-flight table changed"
        );
    }
    #[test]
    fn dynamic_consumption_current_view_product_bound_is_not_repeated_per_batch() {
        let mut observed = Vec::new();
        for count in [4, 32, 256] {
            let frame = tests::batch_test_frame(true, MaterialBlend::Opaque, count, 1);
            let original = &frame.views[0].batches.as_ref().unwrap()[0];
            let manager = RenderResourceManager::new();
            let mut inputs = build_inputs::GeometryBuildInputs::new(&[], &manager, None).unwrap();
            let prepared = PhaseCache::default()
                .resolve(
                    &original.draw,
                    &original.instances,
                    &vec![MaterialDepth::TestAndWrite; count],
                    &mut inputs,
                )
                .unwrap();
            let split: Vec<_> = original
                .visible
                .iter()
                .map(|entry| {
                    let mut batch = original.as_ref().clone();
                    batch.draw = Arc::new(original.selected_draw(
                        &original.instances[entry.instance_index as usize],
                        entry.camera_depth,
                    ));
                    batch.visible = vec![*entry].into();
                    batch
                })
                .collect();
            let mut validation = DrawViewValidation::default();
            FINITE_BOUND_EVALUATIONS.with(|n| n.set(0));
            for batch in &split {
                batch
                    .validate_prepared_selection(&mut validation, &prepared)
                    .unwrap();
            }
            observed.push((count, FINITE_BOUND_EVALUATIONS.with(|n| n.get())));
            for batch in &split {
                batch
                    .validate_selection_with_view(&mut DrawViewValidation::default())
                    .unwrap();
            }
            let mut bad = split[0].clone();
            Arc::make_mut(&mut Arc::make_mut(&mut bad.draw).view).projection_matrix[0] =
                f32::INFINITY.into();
            assert!(
                bad.validate_prepared_selection(&mut validation, &prepared)
                    .is_err(),
                "changed current view borrowed finite proof"
            );
        }
        eprintln!("actual current-view product bound evaluations {observed:?}");
        assert!(
            observed.iter().all(|(_, calls)| *calls == 1),
            "per-batch matrix bound work {observed:?}"
        );
    }
}

#[cfg(test)]
mod flow_publication_tests {
    use super::*;
    #[test]
    fn flow_prepared_ranks_avoid_cross_rank_comparisons_at_multiple_scales() {
        let frame = tests::batch_test_frame(true, MaterialBlend::Opaque, 256, 1);
        let table = &frame.views[0].batches.as_ref().unwrap()[0].instances;
        let mut counts = Vec::new();
        for n in [4, 32, 256] {
            let mut selected: Vec<_> = (0..n)
                .map(|i| Selection {
                    visible: GeometryVisibleInstance {
                        instance_index: i as u32,
                        camera_depth: f32::from_bits(
                            [
                                0x80000000, 0, 0x7fc00001, 0xffc00001, 0x7f800000, 0xff800000, 1,
                                0x3f800000,
                            ][i % 8],
                        )
                        .into(),
                    },
                    depth: MaterialDepth::TestAndWrite,
                    sort_rank: (i * 17 + 3) % (n / 2),
                })
                .collect();
            let mut expected = selected.clone();
            expected.sort_by(|a, b| {
                a.sort_rank
                    .cmp(&b.sort_rank)
                    .then_with(|| {
                        a.visible
                            .camera_depth
                            .to_f32()
                            .total_cmp(&b.visible.camera_depth.to_f32())
                    })
                    .then_with(|| {
                        table[a.visible.instance_index as usize]
                            .instance_id
                            .cmp(&table[b.visible.instance_index as usize].instance_id)
                    })
            });
            FLOW_RANK_COMPARISONS.with(|v| v.set(0));
            sort_phase_segment(&mut selected, table, Some(n / 2));
            let count = FLOW_RANK_COMPARISONS.with(|v| v.get());
            counts.push((n, count));
            for (a, b) in selected.iter().zip(&expected) {
                assert_eq!(a.sort_rank, b.sort_rank);
                assert_eq!(a.visible.instance_index, b.visible.instance_index);
                assert_eq!(
                    a.visible.camera_depth.to_f32().to_bits(),
                    b.visible.camera_depth.to_f32().to_bits()
                );
            }
        }
        eprintln!("flow actual rank comparisons {counts:?}");
        assert!(counts.iter().all(|(_, c)| *c == 0), "{counts:?}");
    }
    #[test]
    fn flow_contiguous_batches_publish_without_visible_growth() {
        let mut counts = Vec::new();
        for n in [4, 32, 256] {
            FLOW_VISIBLE_GROWS.with(|v| v.set(0));
            let actual = tests::batch_test_frame(true, MaterialBlend::Opaque, n, 1);
            let grows = FLOW_VISIBLE_GROWS.with(|v| v.get());
            let old = tests::batch_test_frame(false, MaterialBlend::Opaque, n, 1);
            assert_eq!(
                actual.views[0].selected_draws().collect::<Vec<_>>(),
                old.views[0].selected_draws().collect::<Vec<_>>()
            );
            counts.push((n, grows));
        }
        eprintln!("flow visible growth {counts:?}");
        assert!(counts.iter().all(|(_, c)| *c == 0), "{counts:?}");
    }
}
