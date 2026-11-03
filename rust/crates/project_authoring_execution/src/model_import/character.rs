//! glTF character subset. All reads use checked operation-snapshot buffers.
use super::*;
use engine_runtime::character_animation::*;
use std::sync::Arc;

pub(super) fn collect_dependencies(
    document: &gltf::Document,
    selected: &BTreeSet<usize>,
    consumed: &mut BTreeSet<String>,
    buffers: &mut BTreeSet<usize>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let mut accessors = Vec::new();
    let mut key_count = 0usize;
    for node in document.nodes().filter(|n| selected.contains(&n.index())) {
        if let Some(skin) = node.skin() {
            if skin.joints().len() > MAX_JOINTS
                || skin.joints().any(|j| !selected.contains(&j.index()))
            {
                return Err(fail(budget.source,&format!("/skins/{}",skin.index()),"import_joint_invalid","All skin joints must belong to the selected hierarchy and fit the joint budget."));
            }
            consumed.insert(format!("/skins/{}", skin.index()));
            if let Some(a) = skin.inverse_bind_matrices() {
                if a.data_type() != gltf::accessor::DataType::F32
                    || a.dimensions() != gltf::accessor::Dimensions::Mat4
                    || a.normalized()
                    || a.count() != skin.joints().len()
                {
                    return Err(fail(
                        budget.source,
                        &format!("/skins/{}/inverseBindMatrices", skin.index()),
                        "import_bind_invalid",
                        "Inverse bind matrices must be matching-count FLOAT MAT4.",
                    ));
                }
                accessors.push(a);
            }
        }
        if let Some(mesh) = node.mesh() {
            for primitive in mesh.primitives() {
                if primitive.morph_targets().len() > 32 {
                    return Err(fail(
                        budget.source,
                        "/meshes",
                        "import_morph_budget_exceeded",
                        "At most 32 morph targets are supported.",
                    ));
                }
                for target in primitive.morph_targets() {
                    for a in [target.positions(), target.normals()].into_iter().flatten() {
                        if a.data_type() != gltf::accessor::DataType::F32
                            || a.dimensions() != gltf::accessor::Dimensions::Vec3
                            || a.normalized()
                            || primitive
                                .get(&gltf::Semantic::Positions)
                                .is_none_or(|p| p.count() != a.count())
                        {
                            return Err(fail(
                                budget.source,
                                "/meshes",
                                "import_morph_invalid",
                                "Morph deltas require matching-count FLOAT VEC3.",
                            ));
                        }
                        accessors.push(a);
                    }
                    if target.tangents().is_some() {
                        return Err(fail(
                            budget.source,
                            "/meshes",
                            "import_morph_unsupported",
                            "Tangent morphs require a separate supported profile.",
                        ));
                    }
                }
            }
        }
    }
    for animation in document.animations() {
        if !animation
            .channels()
            .any(|c| selected.contains(&c.target().node().index()))
        {
            continue;
        }
        consumed.insert(format!("/animations/{}", animation.index()));
        for channel in animation.channels() {
            if !selected.contains(&channel.target().node().index()) {
                return Err(fail(
                    budget.source,
                    &format!(
                        "/animations/{}/channels/{}",
                        animation.index(),
                        channel.index()
                    ),
                    "import_animation_target_missing",
                    "A consumed clip cannot animate nodes outside the selected scene.",
                ));
            }
            let input = channel.sampler().input();
            let output = channel.sampler().output();
            let dimensions = match channel.target().property() {
                gltf::animation::Property::Rotation => gltf::accessor::Dimensions::Vec4,
                gltf::animation::Property::MorphTargetWeights => gltf::accessor::Dimensions::Scalar,
                _ => gltf::accessor::Dimensions::Vec3,
            };
            if input.data_type() != gltf::accessor::DataType::F32
                || input.dimensions() != gltf::accessor::Dimensions::Scalar
                || input.normalized()
                || output.data_type() != gltf::accessor::DataType::F32
                || output.dimensions() != dimensions
                || output.normalized()
            {
                return Err(fail(
                    budget.source,
                    &format!(
                        "/animations/{}/channels/{}",
                        animation.index(),
                        channel.index()
                    ),
                    "import_animation_stream_invalid",
                    "Animation requires FLOAT SCALAR times and matching FLOAT output dimensions.",
                ));
            }
            key_count = key_count.saturating_add(input.count());
            if key_count > MAX_KEYS {
                return Err(fail(
                    budget.source,
                    "/animations",
                    "import_animation_budget_exceeded",
                    "Animation key budget exceeded before decoding.",
                ));
            }
            accessors.push(input);
            accessors.push(output);
        }
    }
    for a in accessors {
        let view = geometry::validate_accessor(&a, budget)?;
        charge(
            &mut budget.cooked_bytes,
            (a.count() * a.size()) as u64,
            budget.limits.cooked_bytes,
            budget.source,
            "/accessors",
            "import_cooked_budget_exceeded",
        )?;
        consumed.insert(format!("/accessors/{}", a.index()));
        consumed.insert(format!("/bufferViews/{}", view.index()));
        buffers.insert(view.buffer().index());
    }
    Ok(())
}

pub(super) fn decode(
    document: &gltf::Document,
    nodes: &[usize],
    buffers: &BTreeMap<usize, Arc<[u8]>>,
    settings: &ModelImportV1,
    budget: &mut Budget<'_>,
) -> Result<Option<CharacterAsset>> {
    let selected: BTreeSet<_> = nodes.iter().copied().collect();
    for node in document.nodes().filter(|n| selected.contains(&n.index())) {
        if node.skin().is_none()
            && node.mesh().is_some_and(|m| {
                m.primitives().any(|p| {
                    p.get(&gltf::Semantic::Joints(0)).is_some()
                        || p.get(&gltf::Semantic::Weights(0)).is_some()
                })
            })
        {
            return Err(fail(
                &settings.source,
                "/meshes",
                "import_character_invalid",
                "JOINTS_0/WEIGHTS_0 require an explicit skin.",
            ));
        }
    }
    let animated = document.animations().any(|a| {
        a.channels()
            .any(|c| selected.contains(&c.target().node().index()))
    });
    let skinned = document.nodes().any(|n| {
        selected.contains(&n.index())
            && (n.skin().is_some()
                || n.mesh()
                    .is_some_and(|m| m.primitives().any(|p| p.morph_targets().len() > 0)))
    });
    if !animated && !skinned {
        return Ok(None);
    }
    if nodes.len() > MAX_JOINTS {
        return Err(fail(
            &settings.source,
            "/nodes",
            "import_joint_budget_exceeded",
            "Character hierarchy exceeds 1024 joints.",
        ));
    }
    let conversion = coordinates::Conversion::new(&settings.coordinates);
    let invalid = |path: &str, message: &str| {
        fail(&settings.source, path, "import_character_invalid", message)
    };
    let index: BTreeMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (*n, i as u16))
        .collect();
    let mut parents = BTreeMap::new();
    for n in document.nodes().filter(|n| selected.contains(&n.index())) {
        for c in n.children() {
            parents.insert(c.index(), index[&n.index()]);
        }
    }
    let mut joints = Vec::with_capacity(nodes.len());
    for node_index in nodes {
        let node = document.nodes().nth(*node_index).unwrap();
        let locator = format!("/nodes/{node_index}");
        let mapping = settings
            .nodes
            .iter()
            .find(|m| m.source_locator == locator)
            .ok_or_else(|| {
                invalid(
                    &locator,
                    "Every character joint needs a committed stable node key.",
                )
            })?;
        let t = conversion.node(node, &settings.source)?;
        joints.push(CharacterJoint {
            key: mapping.key.clone(),
            parent: parents.get(node_index).copied(),
            bind: JointPose {
                translation: t.translation,
                rotation: t.rotation_xyzw,
                scale: t.scale,
            },
        });
    }
    let mut skins = Vec::new();
    for node in document.nodes().filter(|n| selected.contains(&n.index())) {
        let Some(mesh) = node.mesh() else {
            continue;
        };
        let morph_count = mesh
            .primitives()
            .next()
            .map_or(0, |p| p.morph_targets().len());
        let has_weights = mesh.primitives().any(|p| {
            p.get(&gltf::Semantic::Weights(0)).is_some()
                || p.get(&gltf::Semantic::Joints(0)).is_some()
        });
        if node.skin().is_none() && has_weights {
            return Err(invalid(
                "/meshes",
                "JOINTS_0/WEIGHTS_0 require an explicit skin.",
            ));
        }
        if node.skin().is_none() && morph_count == 0 {
            continue;
        }
        let joint_indices = if let Some(s) = node.skin() {
            s.joints().map(|j| index[&j.index()]).collect::<Vec<_>>()
        } else {
            vec![index[&node.index()]]
        };
        let inverse_bind = if let Some(skin) = node.skin() {
            if let Some(a) = skin.inverse_bind_matrices() {
                super::source::checked_view(&a.view().unwrap(), buffers, &settings.source)?;
            }
            skin.reader(|b| buffers.get(&b.index()).map(|b| b.as_ref()))
                .read_inverse_bind_matrices()
                .map(|m| m.map(|v| conversion.affine(v)).collect())
                .unwrap_or_else(|| vec![JointPose::default().matrix(); joint_indices.len()])
        } else {
            vec![JointPose::default().matrix()]
        };
        let mut influences = Vec::new();
        let mut targets = vec![
            MorphTarget3D {
                position_deltas: Vec::new(),
                normal_deltas: Vec::new()
            };
            morph_count
        ];
        for primitive in mesh.primitives() {
            let path = format!("/meshes/{}/primitives/{}", mesh.index(), primitive.index());
            if primitive.morph_targets().len() != morph_count {
                return Err(invalid(
                    &path,
                    "All primitives must have the same morph target count.",
                ));
            }
            let reader = primitive.reader(|b| buffers.get(&b.index()).map(|b| b.as_ref()));
            let count = primitive.get(&gltf::Semantic::Positions).unwrap().count();
            if node.skin().is_some() {
                let ji = reader
                    .read_joints(0)
                    .ok_or_else(|| invalid(&path, "A skinned primitive requires JOINTS_0."))?
                    .into_u16()
                    .collect::<Vec<_>>();
                let weights = reader
                    .read_weights(0)
                    .ok_or_else(|| invalid(&path, "A skinned primitive requires WEIGHTS_0."))?
                    .into_f32()
                    .collect::<Vec<_>>();
                if ji.len() != count || weights.len() != count {
                    return Err(invalid(&path, "Skin stream count mismatch."));
                }
                influences.extend(
                    ji.into_iter()
                        .zip(weights)
                        .map(|(joints, weights)| VertexInfluence { joints, weights }),
                );
            } else {
                influences.extend((0..count).map(|_| VertexInfluence {
                    joints: [0; 4],
                    weights: [1., 0., 0., 0.],
                }));
            }
            for (i, (positions, normals, _)) in reader.read_morph_targets().enumerate() {
                for a in primitive
                    .morph_targets()
                    .nth(i)
                    .into_iter()
                    .flat_map(|t| [t.positions(), t.normals()])
                    .flatten()
                {
                    super::source::checked_view(&a.view().unwrap(), buffers, &settings.source)?;
                }
                let positions = positions
                    .map(|p| p.map(|v| conversion.position(v)).collect::<Vec<_>>())
                    .unwrap_or_else(|| vec![[0.; 3]; count]);
                let normals = normals
                    .map(|p| p.map(|v| conversion.vector(v)).collect::<Vec<_>>())
                    .unwrap_or_else(|| vec![[0.; 3]; count]);
                if positions.len() != count || normals.len() != count {
                    return Err(invalid(&path, "Morph stream count mismatch."));
                }
                targets[i].position_deltas.extend(positions);
                targets[i].normal_deltas.extend(normals);
            }
        }
        charge(
            &mut budget.cooked_bytes,
            (influences.len() * 24) as u64,
            budget.limits.cooked_bytes,
            &settings.source,
            "/skins",
            "import_cooked_budget_exceeded",
        )?;
        let weights = node
            .weights()
            .or_else(|| mesh.weights())
            .map(|v| v.to_vec())
            .unwrap_or_else(|| vec![0.; morph_count]);
        skins.push(CharacterSkin {
            mesh_joint: index[&node.index()],
            mesh_asset_id: format!("/meshes/{}", mesh.index()),
            joints: joint_indices,
            inverse_bind,
            influences,
            morph_targets: targets,
            morph_weights: weights,
        });
    }
    let mut clips = Vec::new();
    let mut names = BTreeSet::new();
    for animation in document.animations().filter(|a| {
        a.channels()
            .any(|c| selected.contains(&c.target().node().index()))
    }) {
        let path = format!("/animations/{}", animation.index());
        // Named clips are an intentional import identity; ambiguous/missing names are rejected.
        let key = animation
            .name()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| invalid(&path, "Animation clips require unique authored names."))?;
        if !names.insert(key) {
            return Err(invalid(&path,"Duplicate animation names require explicit source repair; no index-based identity guessing."));
        }
        let mut tracks = Vec::new();
        let mut duration = 0f32;
        for channel in animation.channels() {
            for a in [channel.sampler().input(), channel.sampler().output()] {
                super::source::checked_view(&a.view().unwrap(), buffers, &settings.source)?;
            }
            let reader = channel.reader(|b| buffers.get(&b.index()).map(|b| b.as_ref()));
            let times = reader
                .read_inputs()
                .ok_or_else(|| invalid(&path, "Animation input must be FLOAT scalar."))?
                .collect::<Vec<_>>();
            if let Some(t) = times.last() {
                duration = duration.max(*t);
            }
            let (property, width, values) = match reader
                .read_outputs()
                .ok_or_else(|| invalid(&path, "Missing or unsupported animation output."))?
            {
                gltf::animation::util::ReadOutputs::Translations(values) => (
                    TrackProperty::Translation,
                    3,
                    values.flat_map(|v| conversion.position(v)).collect(),
                ),
                gltf::animation::util::ReadOutputs::Rotations(values) => (
                    TrackProperty::Rotation,
                    4,
                    values
                        .into_f32()
                        .flat_map(|v| conversion.rotation_key(v))
                        .collect(),
                ),
                gltf::animation::util::ReadOutputs::Scales(values) => (
                    TrackProperty::Scale,
                    3,
                    values.flat_map(|v| conversion.scale_key(v)).collect(),
                ),
                gltf::animation::util::ReadOutputs::MorphTargetWeights(values) => {
                    let count = channel
                        .target()
                        .node()
                        .mesh()
                        .and_then(|m| m.primitives().next())
                        .map_or(0, |p| p.morph_targets().len());
                    (
                        TrackProperty::MorphWeights,
                        count as u16,
                        values.into_f32().collect(),
                    )
                }
            };
            let interpolation = match channel.sampler().interpolation() {
                gltf::animation::Interpolation::Linear => KeyInterpolation::Linear,
                gltf::animation::Interpolation::Step => KeyInterpolation::Step,
                gltf::animation::Interpolation::CubicSpline => KeyInterpolation::CubicSpline,
            };
            tracks.push(AnimationTrack {
                joint: index[&channel.target().node().index()],
                property,
                interpolation,
                times,
                width,
                values,
            });
        }
        let events = if let Some(extras) = animation.extras().as_ref() {
            let value: Value = serde_json::from_str(extras.get())
                .map_err(|_| invalid(&path, "Invalid animation extras."))?;
            value
                .get("aifeEvents")
                .map(|events| {
                    serde_json::from_value::<Vec<AnimationEvent3D>>(events.clone()).map_err(|_| {
                        invalid(
                            &format!("{path}/extras/aifeEvents"),
                            "Events require time and name.",
                        )
                    })
                })
                .transpose()?
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        clips.push(AnimationClip3D {
            key: key.into(),
            duration,
            tracks,
            events,
        });
    }
    let asset = CharacterAsset {
        schema_version: "character3d.v1".into(),
        asset_id: settings.asset_id.clone(),
        source_digest: sha256_prefixed(&serde_json::to_vec(&settings.nodes).unwrap()),
        recipe: CHARACTER_RECIPE.into(),
        joints,
        skins,
        clips,
    };
    asset.validate().map_err(|error| ImportFailure {
        source_path: settings.source.clone(),
        error,
    })?;
    Ok(Some(asset))
}
