use super::*;
use crate::{
    archetype::ComponentValue,
    components::{Hierarchy, Transform},
    runtime_instance_loader::RuntimeInstanceLoader,
    runtime_package::*,
    runtime_package_builder::*,
};

fn reference(id: &str, kind: &str) -> RuntimeAssetRef {
    RuntimeAssetRef {
        id: id.into(),
        asset_type: kind.into(),
        guid: Some(format!("guid-{id}")),
        sub_asset: None,
    }
}
fn cooked() -> CookedParticleEffect {
    let mut description: ParticleEffectDescription =
        serde_json::from_str(include_str!("../tests/fixtures/particle-effect.json")).unwrap();
    description.asset_id = "effect".into();
    description.asset_guid = "guid-effect".into();
    let e = &mut description.emitters[0];
    e.capacity = 4;
    e.bursts[0].count = 1;
    e.velocity_min = [0.0; 3];
    e.velocity_max = [0.0; 3];
    e.color = [1.0; 4];
    e.size_meters = [0.5; 2];
    e.lifetime_seconds = [0.25; 2];
    cook(description, program::DEFAULT_BEHAVIOR)
}
fn cook(description: ParticleEffectDescription, author: &str) -> CookedParticleEffect {
    description.validate().unwrap();
    let programs = description
        .emitters
        .iter()
        .map(|e| CookedParticleProgram {
            emitter: e.name.clone(),
            wgsl: program::compute_source(
                &description,
                e,
                &(program::prefix(&description, e) + author + program::CONTRACT_CALLS),
            ),
            author_path: None,
            author_first_line: 0,
            author_line_count: 0,
            particle_stride: program::particle_layout(e).2,
        })
        .collect();
    CookedParticleEffect {
        program_contract: PARTICLE_PROGRAM_CONTRACT.into(),
        description,
        programs,
        source_digests: Default::default(),
    }
}
fn input(effect: &CookedParticleEffect) -> RuntimePackageBuildInput {
    let mut input = RuntimePackageBuildInput::new(RuntimeProjectInfo::explicit_empty(
        "particles-d",
        "Particles D",
        "1",
    ));
    let mapping = engine_input::InputMappingAsset::explicit_empty("input.none");
    input.input_mappings.push(RuntimePackageSourceJson {
        id: mapping.asset_id.clone(),
        document: serde_json::to_value(mapping).unwrap(),
    });
    let mut scene = crate::scene_loader::tests_support::scene_fixture(false);
    scene.id = "particle-scene".into();
    scene.entities.truncate(1);
    scene.entities[0].id = "source".into();
    scene.entities[0].mesh = None;
    scene.entities[0].sprite_renderer2d = None;
    scene.entities[0].components.clear();
    scene.entities[0].transform = Some(RuntimeTransform {
        local_position: Vector3 {
            x: 0.0,
            y: 0.0,
            z: 0.5,
        },
        local_rotation: Vector3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        local_scale: Vector3 {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        },
    });
    scene.entities[0].components.push(RuntimeProjectComponent{component_type:"engine.particle_effect".into(),data:serde_json::json!({"effectRef":reference("effect","particle-effect"),"playOnAwake":true})});
    input.scenes.push(scene);
    input.assets.push(RuntimePackageSourceAsset::new(
        "particle-scene",
        "Scene",
        "scene",
        "Scenes/source.scene.json",
        "scenes/particle-scene.json",
    ));
    let mut asset = RuntimePackageSourceAsset::new(
        "effect",
        "Effect",
        "particle-effect",
        "Assets/effect.particle-effect.json",
        "cooked/effect.json",
    )
    .with_runtime_payload(serde_json::to_vec(effect).unwrap());
    asset.asset_guid = Some("guid-effect".into());
    input.assets.push(asset);
    input
}
struct Fixture {
    root: std::path::PathBuf,
    package: RuntimePackage,
}
impl Fixture {
    fn new(input: RuntimePackageBuildInput) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "particle-gated-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let package_dir = root.join("package");
        let report = RuntimePackageBuilder::build(
            &RuntimePackageBuildRequest::dev_desktop(&package_dir, "particle-scene"),
            &input,
        );
        assert_eq!(
            report.status,
            RuntimePackageBuildStatus::Success,
            "{:?}",
            report.diagnostics
        );
        let loaded = load_runtime_package(&package_dir);
        assert!(loaded.diagnostics.is_ok(), "{:?}", loaded.diagnostics);
        Self {
            root,
            package: loaded.value.unwrap(),
        }
    }
    fn world(
        &self,
    ) -> (
        World,
        RuntimeInstanceLoader,
        crate::runtime_instance::RuntimeSceneInstance,
    ) {
        let mut world = World::new();
        let mut loader = RuntimeInstanceLoader::from_package(&self.package);
        let (scene, report) = loader.load_active_scene_instance(&self.package, &mut world);
        assert!(!report.has_errors(), "{:?}", report.diagnostics);
        (world, loader, scene.unwrap())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let root = self.root.canonicalize().unwrap();
        assert_eq!(
            root.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("particle-gated-"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
fn command(world: &World, action: ParticleAction) -> ParticleCommand {
    ParticleCommand {
        entity_id: "source".into(),
        runtime_id: world.runtime_id_for_source(&"source".into()).unwrap(),
        action,
    }
}
fn update(
    owner: &mut RuntimeParticles,
    world: &World,
    loader: &mut RuntimeInstanceLoader,
    actions: Vec<ParticleAction>,
    frame: u64,
) -> Vec<ParticleSourceFrame> {
    owner.update(
        world,
        Some(loader.asset_loader_mut()),
        actions.into_iter().map(|a| command(world, a)).collect(),
        frame,
        1.0 / 64.0,
    )
}

#[test]
fn particle_lifecycle_control_is_idempotent_and_paused_restart_preserves_pause() {
    let f = Fixture::new(input(&cooked()));
    let (world, mut loader, _) = f.world();
    let mut owner = RuntimeParticles::default();
    let first = update(&mut owner, &world, &mut loader, vec![], 1);
    assert_eq!(first.len(), 1);
    assert!(first[0].step.emitting);
    let play = update(
        &mut owner,
        &world,
        &mut loader,
        vec![ParticleAction::Play, ParticleAction::Play],
        2,
    );
    assert_eq!(first[0].epoch, play[0].epoch);
    assert!(Arc::ptr_eq(&first[0].assets, &play[0].assets));
    let pause = update(
        &mut owner,
        &world,
        &mut loader,
        vec![ParticleAction::SetPaused(true), ParticleAction::Restart],
        3,
    );
    assert!(pause[0].step.paused);
    assert!(pause[0].epoch > play[0].epoch);
    let stop = update(
        &mut owner,
        &world,
        &mut loader,
        vec![
            ParticleAction::SetPaused(false),
            ParticleAction::StopEmitting,
        ],
        4,
    );
    assert!(!stop[0].step.emitting);
    assert!(!stop[0].step.paused);
    assert_eq!(stop[0].epoch, pause[0].epoch);
    let clear = update(
        &mut owner,
        &world,
        &mut loader,
        vec![ParticleAction::Clear],
        5,
    );
    assert!(clear[0].step.paused);
    assert!(clear[0].epoch > stop[0].epoch);
    assert_eq!(owner.source_count(), 1);
}
#[test]
fn particle_lifecycle_retires_removed_disabled_and_recreated_entities_and_releases_package() {
    let f = Fixture::new(input(&cooked()));
    let (mut world, mut loader, scene) = f.world();
    let mut owner = RuntimeParticles::default();
    let first = update(&mut owner, &world, &mut loader, vec![], 1);
    let old = command(&world, ParticleAction::Restart);
    let source = world.particle_effect(&"source".into()).unwrap().clone();
    let mut meta = world.entity(&"source".into()).unwrap().clone();
    meta.enabled = false;
    world.insert_component_value("source".into(), ComponentValue::EntityMeta(meta.clone()));
    assert!(update(&mut owner, &world, &mut loader, vec![], 1).is_empty());
    meta.enabled = true;
    world.insert_component_value("source".into(), ComponentValue::EntityMeta(meta));
    update(&mut owner, &world, &mut loader, vec![], 1);
    world
        .try_remove_component_value(&"source".into(), &ComponentTypeId::particle_effect())
        .unwrap();
    assert!(update(&mut owner, &world, &mut loader, vec![], 2).is_empty());
    world.insert_component_value(
        "source".into(),
        ComponentValue::ParticleEffect(source.clone()),
    );
    let replaced = update(&mut owner, &world, &mut loader, vec![], 3);
    assert_ne!(first[0].instance, replaced[0].instance);
    world.despawn_entity(&"source".into());
    world.spawn_entity(
        "source".into(),
        "new",
        "particles",
        true,
        Hierarchy {
            parent_id: None,
            sibling_order: 0,
        },
    );
    world.insert_component_value("source".into(), ComponentValue::ParticleEffect(source));
    let new = owner.update(
        &world,
        Some(loader.asset_loader_mut()),
        vec![old],
        4,
        1.0 / 64.0,
    );
    assert_ne!(replaced[0].instance, new[0].instance);
    assert!(owner.diagnostics.iter().any(|d| d.contains("stale_target")));
    assert_eq!(new[0].epoch, 0);
    assert!(!loader
        .unload_scene_instance(scene.instance_id, &mut world)
        .has_errors());
    assert!(update(&mut owner, &world, &mut loader, vec![], 5).is_empty());
    assert_eq!(loader.asset_loader().decoded_cache_len(), 0);
}
#[test]
fn particle_lifecycle_failed_and_noop_handlers_never_submit_and_generation_is_captured() {
    use crate::project_runtime_session::*;
    let f = Fixture::new(input(&cooked()));
    let (mut world, _, _) = f.world();
    for status in [
        ProjectRuntimeSessionStatus::NoOp,
        ProjectRuntimeSessionStatus::Unhandled,
        ProjectRuntimeSessionStatus::Rejected,
        ProjectRuntimeSessionStatus::Faulted,
    ] {
        let mut buffer = ProjectRuntimeMutationBuffer::new();
        buffer.particle_command("source".into(), 0, ParticleAction::Restart);
        let mut output = ProjectRuntimeSessionOutput::applied(buffer);
        output.status = status;
        let ProjectRuntimeMutationPreparation::Dropped(report) =
            output.prepare_mutations(&world).unwrap()
        else {
            panic!("non-applied output submitted")
        };
        assert!(report.particle_commands.is_empty());
        assert_eq!(report.committed_count, 0);
    }
    let mut bad = ProjectRuntimeMutationBuffer::new();
    bad.particle_command("source".into(), 0, ParticleAction::Play);
    bad.write_transform("missing".into(), Transform::identity());
    assert!(bad
        .prepare(&world)
        .unwrap_err()
        .report
        .particle_commands
        .is_empty());
    let mut stale = ProjectRuntimeMutationBuffer::new();
    stale.particle_command("source".into(), u64::MAX, ParticleAction::Play);
    assert_eq!(
        stale.prepare(&world).unwrap_err().code,
        "particle_effect.target_unavailable"
    );
    let mut good = ProjectRuntimeMutationBuffer::new();
    good.particle_command("source".into(), 0, ParticleAction::Play);
    let expected = world.runtime_id_for_source(&"source".into()).unwrap();
    let ready = good.prepare(&world).unwrap();
    world.despawn_entity(&"source".into());
    world.spawn_entity(
        "source".into(),
        "new",
        "particles",
        true,
        Hierarchy {
            parent_id: None,
            sibling_order: 0,
        },
    );
    let report = ready.commit(&mut world).unwrap();
    assert_eq!(report.particle_commands[0].runtime_id, expected);
    assert_ne!(
        world.runtime_id_for_source(&"source".into()).unwrap(),
        expected
    );
}
#[test]
fn particle_lifecycle_component_and_package_reference_fail_closed() {
    let mut input = input(&cooked());
    for reference in [
        serde_json::json!({"id":"wrong","type":"particle-effect","guid":"guid-effect"}),
        serde_json::json!({"id":"effect","type":"audio"}),
        serde_json::json!({"id":"effect","type":"particle-effect","guid":"wrong"}),
    ] {
        input.scenes[0].entities[0].components[0].data["effectRef"] = reference;
        let root = std::env::temp_dir().join("particle-invalid-input-no-output");
        let report = RuntimePackageBuilder::build(
            &RuntimePackageBuildRequest::dev_desktop(root, "particle-scene"),
            &input,
        );
        assert_ne!(report.status, RuntimePackageBuildStatus::Success);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "ParticleEffectInvalid"),
            "{:?}",
            report.diagnostics
        );
    }
    assert!(decode_particle_effect(
        &serde_json::json!({"effectRef":reference("effect","particle-effect"),"unknown":true})
    )
    .is_err());
}
#[test]
fn particle_lifecycle_parameters_validate_names_types_ranges_without_changing_last_value() {
    let mut desc = cooked().description;
    desc.parameters = vec![ParticleParameter {
        name: "speed".into(),
        default: ParticleValue::Float(1.0),
        unit: ParticleUnit::Unitless,
        stage: ParticleParameterStage::Both,
        range: Some([0.0, 2.0]),
    }];
    let mut build_input = input(&cook(desc, program::DEFAULT_BEHAVIOR));
    build_input.scenes[0].entities[0].components[0].data["parameters"] =
        serde_json::json!({"speed":{"type":"float","value":0.5}});
    let f = Fixture::new(build_input);
    let (world, mut loader, _) = f.world();
    let mut owner = RuntimeParticles::default();
    let initial = update(&mut owner, &world, &mut loader, vec![], 0);
    assert!(initial[0].parameters.contains("0.5"));
    let good = ParticleAction::SetParameter {
        name: "speed".into(),
        value: r#"{"type":"float","value":2.0}"#.into(),
    };
    let first = update(&mut owner, &world, &mut loader, vec![good], 1);
    assert!(first[0].parameters.contains("2.0"));
    for (name, value) in [
        ("missing", ParticleValue::Float(1.0)),
        ("speed", ParticleValue::Uint(1)),
        ("speed", ParticleValue::Float(3.0)),
    ] {
        let frame = update(
            &mut owner,
            &world,
            &mut loader,
            vec![ParticleAction::SetParameter {
                name: name.into(),
                value: serde_json::to_string(&value).unwrap(),
            }],
            2,
        );
        assert!(!owner.diagnostics.is_empty());
        assert_eq!(first[0].parameters, frame[0].parameters);
    }
}

#[cfg(feature = "real-wgpu")]
fn render(
    backend: &mut crate::wgpu_backend::real::RealWgpuBackend,
    sources: Vec<ParticleSourceFrame>,
    frame: u64,
) -> Vec<u8> {
    use crate::{render_state::*, runtime_renderer::*};
    let mut scene = RenderSceneState::new();
    scene.particle_projection_active = true;
    scene.particle_sources = sources;
    let mut view = RenderViewState::new(
        RenderViewId(1),
        RenderViewKind::Game,
        RenderTargetKind::ViewportTexture,
    );
    view.clear_color = [0.0, 0.0, 0.0, 1.0];
    scene.register_view(view);
    let output = RuntimeRenderer::new().build(RuntimeRendererInput {
        frame_index: frame,
        render_scene_state: &scene,
        render_view_state: None,
        aui_overlay: None,
        aui_composition: None,
        sprite_texture_bindings: None,
        runtime_texture_bindings: None,
        game_view_presentation: None,
        quality_profile: QualityProfile::default(),
        render_target: RenderTarget::headless_texture("out", 128, 128),
    });
    backend
        .render_plan_to_rgba_bytes(&output.rhi_command_plan, 128, 128)
        .unwrap()
}
#[cfg(feature = "real-wgpu")]
#[test]
fn particle_lifecycle_real_gpu_control_pause_clear_and_inflight_retirement() {
    let f = Fixture::new(input(&cooked()));
    let (world, mut loader, _) = f.world();
    let mut owner = RuntimeParticles::default();
    let mut backend = crate::wgpu_backend::real::RealWgpuBackend::new_offscreen(128, 128).unwrap();
    let first = update(&mut owner, &world, &mut loader, vec![], 1);
    let id = first[0].instance;
    let image = render(&mut backend, first, 1);
    assert!(image.chunks_exact(4).any(|p| p[0] > 200));
    render(
        &mut backend,
        update(
            &mut owner,
            &world,
            &mut loader,
            vec![ParticleAction::Play],
            2,
        ),
        2,
    );
    let before = backend.capture_particle_state_for_validation(id).unwrap()[0].particles[0].age;
    assert!(before > 0.0);
    let paused = update(
        &mut owner,
        &world,
        &mut loader,
        vec![ParticleAction::SetPaused(true)],
        3,
    );
    render(&mut backend, paused.clone(), 3);
    render(&mut backend, paused, 3);
    assert_eq!(
        backend.capture_particle_state_for_validation(id).unwrap()[0].particles[0].age,
        before
    );
    render(
        &mut backend,
        update(
            &mut owner,
            &world,
            &mut loader,
            vec![ParticleAction::Restart],
            4,
        ),
        4,
    );
    assert_eq!(
        backend.capture_particle_state_for_validation(id).unwrap()[0].live,
        0
    );
    render(
        &mut backend,
        update(
            &mut owner,
            &world,
            &mut loader,
            vec![ParticleAction::SetPaused(false)],
            5,
        ),
        5,
    );
    assert_eq!(
        backend.capture_particle_state_for_validation(id).unwrap()[0].particles[0].age,
        0.0
    );
    render(
        &mut backend,
        update(
            &mut owner,
            &world,
            &mut loader,
            vec![ParticleAction::Clear],
            6,
        ),
        6,
    );
    assert_eq!(
        backend.capture_particle_state_for_validation(id).unwrap()[0].live,
        0
    );
    let active = update(
        &mut owner,
        &world,
        &mut loader,
        vec![ParticleAction::Restart],
        7,
    );
    backend.reconcile_project_particles(&active).unwrap();
    let submitted = backend
        .simulate_particle_effect(
            id,
            crate::particle_gpu::ParticleGpuStep {
                step_id: 7,
                delta_seconds: 1.0 / 64.0,
                origin: [0.0, 0.0, 0.5],
                emitting: true,
                paused: false,
            },
        )
        .unwrap();
    assert_eq!(submitted.substeps, 1);
    // No readback, poll or wait between submission and retirement.
    backend.reconcile_project_particles(&[]).unwrap();
    assert_eq!(backend.particle_buffer_bytes(id), None);
    render(&mut backend, vec![], 8); // Completes queued work after owners were dropped: no destroyed-buffer validation error.
}

#[cfg(feature = "real-wgpu")]
#[test]
fn particle_lifecycle_real_gpu_parameter_stages_apply_to_spawn_and_update() {
    let mut desc = cooked().description;
    desc.parameters = vec![
        ParticleParameter {
            name: "spawn".into(),
            default: ParticleValue::Float(1.0),
            unit: ParticleUnit::Unitless,
            stage: ParticleParameterStage::Spawn,
            range: None,
        },
        ParticleParameter {
            name: "update".into(),
            default: ParticleValue::Float(2.0),
            unit: ParticleUnit::Unitless,
            stage: ParticleParameterStage::Update,
            range: None,
        },
    ];
    let author = r#"fn particle_init(p:Particle,c:ParticleContext,e:EffectParams,i:ParticleInputs)->Particle{var q=p;q.velocity=vec3<f32>(e.spawn,e.update,0.0);return q;} fn particle_update(p:Particle,c:ParticleContext,e:EffectParams,i:ParticleInputs)->Particle{var q=p;q.velocity=vec3<f32>(e.spawn,e.update,0.0);return q;}"#;
    let f = Fixture::new(input(&cook(desc, author)));
    let (world, mut loader, _) = f.world();
    let mut owner = RuntimeParticles::default();
    let mut backend = crate::wgpu_backend::real::RealWgpuBackend::new_offscreen(128, 128).unwrap();
    let commands = [("spawn", 3.0), ("update", 4.0)]
        .into_iter()
        .map(|(name, value)| ParticleAction::SetParameter {
            name: name.into(),
            value: serde_json::to_string(&ParticleValue::Float(value)).unwrap(),
        })
        .collect();
    let first = update(&mut owner, &world, &mut loader, commands, 1);
    let id = first[0].instance;
    render(&mut backend, first, 1);
    assert_eq!(
        backend.capture_particle_state_for_validation(id).unwrap()[0].particles[0].velocity,
        [3.0, 2.0, 0.0]
    );
    render(
        &mut backend,
        update(&mut owner, &world, &mut loader, vec![], 2),
        2,
    );
    assert_eq!(
        backend.capture_particle_state_for_validation(id).unwrap()[0].particles[0].velocity,
        [1.0, 4.0, 0.0]
    );
}

#[cfg(feature = "real-wgpu")]
#[test]
fn particle_lifecycle_package_mesh_material_texture_reaches_real_pixels() {
    let mut desc = cooked().description;
    desc.emitters[0].draw.geometry = ParticleGeometry::Mesh {
        asset: reference("mesh", "mesh"),
    };
    desc.emitters[0].draw.material = Some(reference("material", "material"));
    let mut input = input(&cook(desc, program::DEFAULT_BEHAVIOR));
    input.assets[1].dependencies = vec!["guid-mesh".into(), "guid-material".into()];
    for (id, kind, value) in [
        (
            "mesh",
            "mesh",
            serde_json::json!({"positions":[[-0.5,-0.5,0.0],[0.5,-0.5,0.0],[0.0,0.5,0.0]],"uvs":[[0.0,1.0],[1.0,1.0],[0.5,0.0]],"indices":[0,1,2]}),
        ),
        (
            "material",
            "material",
            serde_json::json!({"baseColor":[1.0,0.5,0.25,1.0],"texture":reference("texture","texture")}),
        ),
    ] {
        let mut asset = RuntimePackageSourceAsset::new(
            id,
            id,
            kind,
            format!("Assets/{id}.asset"),
            format!("cooked/{id}.json"),
        )
        .with_runtime_payload(serde_json::to_vec(&value).unwrap());
        asset.asset_guid = Some(format!("guid-{id}"));
        if id == "material" {
            asset.dependencies.push("guid-texture".into());
        }
        input.assets.push(asset);
    }
    let mut texture = RuntimePackageSourceAsset::new(
        "texture",
        "Texture",
        "texture",
        "Assets/texture.asset",
        "cooked/texture.json",
    );
    texture.asset_guid = Some("guid-texture".into());
    input.assets.push(texture);
    input.texture_payloads.push(RuntimePackageSourceTexture {
        metadata: CookedTextureAsset {
            schema_version: COOKED_TEXTURE_SCHEMA_VERSION.into(),
            asset_id: "texture".into(),
            cooked_asset_id: "cooked-texture".into(),
            source_hash: "fixture".into(),
            width: 1,
            height: 1,
            format: "rgba8Unorm".into(),
            color_space: "linear".into(),
            mip_count: 1,
            byte_length: 4,
            pixel_data_path: "cooked/texture.rgba8".into(),
            sampler: "nearestClamp".into(),
        },
        rgba8: vec![128, 255, 0, 255],
    });
    let f = Fixture::new(input);
    let (world, mut loader, _) = f.world();
    let mut owner = RuntimeParticles::default();
    let sources = update(&mut owner, &world, &mut loader, vec![], 1);
    assert!(owner.diagnostics.is_empty(), "{:?}", owner.diagnostics);
    let id = sources[0].instance;
    let mut backend = crate::wgpu_backend::real::RealWgpuBackend::new_offscreen(128, 128).unwrap();
    let image = render(&mut backend, sources, 1);
    let pixel = &image[(64 * 128 + 64) * 4..(64 * 128 + 64) * 4 + 4];
    assert!(
        pixel
            .iter()
            .zip([128u8, 128, 0, 255])
            .all(|(a, b)| a.abs_diff(b) <= 2),
        "{pixel:?}"
    );
    if let Some(root) = std::env::var_os("PARTICLE_RENDER_EVIDENCE_DIR") {
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        let mut png = png::Encoder::new(
            std::fs::File::create(root.join("gate-d-package-material-mesh.png")).unwrap(),
            128,
            128,
        );
        png.set_color(png::ColorType::Rgba);
        png.set_depth(png::BitDepth::Eight);
        png.write_header()
            .unwrap()
            .write_image_data(&image)
            .unwrap();
    }
    // A Sprite may start using the same ordinary texture after the particle source.
    let shared_texture = crate::runtime_texture::runtime_texture_render_handle("texture");
    backend
        .register_rgba8_texture(shared_texture, 1, 1, &[128, 255, 0, 255], "nearestClamp")
        .unwrap();
    backend.reconcile_project_particles(&[]).unwrap();
    assert_eq!(backend.particle_buffer_bytes(id), None);
    use crate::render_graph::*;
    let mut graph = RenderGraph::new("shared-texture", 2);
    graph.output_target = Some("out".into());
    graph
        .resources
        .push(RenderResource::surface_backbuffer("out", 128, 128));
    graph.passes.push(RenderPass {
        pass_id: "sprite".into(),
        pass_name: "Sprite".into(),
        pass_kind: RenderPassKind::DrawSpriteTextured,
        view_id: "main".into(),
        reads: vec![],
        writes: vec!["out".into()],
        color_targets: vec!["out".into()],
        depth_target: None,
        debug_source: None,
        commands: vec![RenderPassCommand::DrawSpriteTextured {
            target: "out".into(),
            sprite_ref: "shared".into(),
            material_ref: None,
            sort_key: "0".into(),
            texture: Some(shared_texture),
            binding: None,
            fallback_used: false,
            vertices: vec![],
        }],
    });
    let sprite_plan = crate::rhi_command_plan::compile_render_graph_to_rhi_plan(&graph);
    backend
        .validate_plan_texture_residency(&sprite_plan)
        .expect("retiring particles must retain a subsequently registered Sprite texture");
}

#[test]
fn particle_lifecycle_host_projects_package_and_unload_without_project_commands() {
    use crate::{
        engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode},
        frame_loop::RuntimeFrameContext,
        rhi_command_plan::RhiCommand,
    };
    let f = Fixture::new(input(&cooked()));
    let (mut world, mut loader, scene) = f.world();
    let mut host = EngineHostLoop::new("particle-scene");
    let first = host.tick_with_runtime_context(
        EngineFrameInput::new(EngineHostMode::ExportedGame),
        &mut world,
        RuntimeFrameContext {
            package: &f.package,
            instance_loader: &mut loader,
        },
    );
    let thread = first.render_thread_frame.unwrap();
    assert!(thread
        .renderer_output
        .rhi_command_plan
        .commands
        .iter()
        .any(|c| matches!(c,RhiCommand::ReconcileParticles{sources} if sources.len()==1)));
    assert!(!loader
        .unload_scene_instance(scene.instance_id, &mut world)
        .has_errors());
    let last = host.tick_with_runtime_context(
        EngineFrameInput::new(EngineHostMode::ExportedGame).with_fixed_step_count(0),
        &mut world,
        RuntimeFrameContext {
            package: &f.package,
            instance_loader: &mut loader,
        },
    );
    assert!(last
        .render_thread_frame
        .unwrap()
        .renderer_output
        .rhi_command_plan
        .commands
        .iter()
        .any(|c| matches!(c,RhiCommand::ReconcileParticles{sources} if sources.is_empty())));
    assert_eq!(loader.asset_loader().decoded_cache_len(), 0);
}

#[test]
fn particle_lifecycle_replacement_owner_never_reuses_gpu_instance_identity() {
    let f = Fixture::new(input(&cooked()));
    let (world, mut loader, _) = f.world();
    let first = update(
        &mut RuntimeParticles::default(),
        &world,
        &mut loader,
        vec![],
        10,
    );
    let replacement = update(
        &mut RuntimeParticles::default(),
        &world,
        &mut loader,
        vec![],
        1,
    );
    assert_ne!(
        first[0].instance, replacement[0].instance,
        "a retained renderer must not reuse a previous session's simulation"
    );
}

#[test]
fn particle_lifecycle_host_consumes_fixed_and_immediate_aui_controls_once() {
    use crate::{
        aui::{AuiAction, AuiActionEvent},
        engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode},
        frame_loop::RuntimeFrameContext,
        project_logic::ProjectLogicRunner,
        project_runtime_session::*,
        rhi_command_plan::RhiCommand,
    };
    struct Session(bool);
    impl ProjectRuntimeSession for Session {
        fn session_id(&self) -> &str {
            "particle.control.session"
        }
        fn handle_aui_actions(
            &mut self,
            _: ProjectRuntimeSessionContext<'_>,
            _: ProjectAuiActionBatch<'_>,
        ) -> ProjectRuntimeSessionOutput {
            let mut mutations = ProjectRuntimeMutationBuffer::new();
            mutations.particle_command("source".into(), 0, ParticleAction::Restart);
            ProjectRuntimeSessionOutput::applied(mutations)
        }
        fn fixed_update(
            &mut self,
            _: ProjectRuntimeSessionContext<'_>,
        ) -> ProjectRuntimeSessionOutput {
            if self.0 {
                return ProjectRuntimeSessionOutput::no_op();
            }
            self.0 = true;
            let mut mutations = ProjectRuntimeMutationBuffer::new();
            mutations.particle_command("source".into(), 0, ParticleAction::Restart);
            ProjectRuntimeSessionOutput::applied(mutations)
        }
    }
    let f = Fixture::new(input(&cooked()));
    let (mut world, mut loader, _) = f.world();
    let mut host = EngineHostLoop::with_project_runtime_session(
        "particle-scene",
        ProjectLogicRunner::empty(),
        Box::new(Session(false)),
    );
    let tick =
        |host: &mut EngineHostLoop, world: &mut World, loader: &mut RuntimeInstanceLoader| {
            let output = host.tick_with_runtime_context(
                EngineFrameInput::new(EngineHostMode::ExportedGame),
                world,
                RuntimeFrameContext {
                    package: &f.package,
                    instance_loader: loader,
                },
            );
            output
                .render_thread_frame
                .unwrap()
                .renderer_output
                .rhi_command_plan
                .commands
                .into_iter()
                .find_map(|c| {
                    if let RhiCommand::ReconcileParticles { sources } = c {
                        Some(sources)
                    } else {
                        None
                    }
                })
                .unwrap()[0]
                .clone()
        };
    let fixed = tick(&mut host, &mut world, &mut loader);
    assert_eq!(fixed.epoch, 1);
    assert_eq!(tick(&mut host, &mut world, &mut loader).epoch, fixed.epoch);
    host.dispatch_aui_actions_immediately(
        &[AuiAction {
            action_id: "restart".into(),
            node_id: "button".into(),
            event: AuiActionEvent::Click,
            payload: None,
        }],
        &mut world,
    );
    let clicked = tick(&mut host, &mut world, &mut loader);
    assert_eq!(clicked.epoch, fixed.epoch + 1);
    assert_eq!(
        tick(&mut host, &mut world, &mut loader).epoch,
        clicked.epoch
    );
}
