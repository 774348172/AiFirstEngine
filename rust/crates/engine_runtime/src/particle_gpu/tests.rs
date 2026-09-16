use super::*;
use std::sync::{Mutex, OnceLock};

static GPU: OnceLock<(wgpu::Device, wgpu::Queue)> = OnceLock::new();
static LANE: Mutex<()> = Mutex::new(());
fn gpu() -> (
    &'static wgpu::Device,
    &'static wgpu::Queue,
    std::sync::MutexGuard<'static, ()>,
) {
    let guard = LANE.lock().unwrap_or_else(|e| e.into_inner());
    let (device, queue) = GPU.get_or_init(|| {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .expect("real GPU is required; do not skip qualification");
        eprintln!("343 particle GPU adapter: {:?}", adapter.get_info());
        assert!(supports_particle_compute(&adapter.limits()));
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("particle-owner-tests"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .unwrap()
    });
    (device, queue, guard)
}
fn fixture() -> ParticleEffectDescription {
    let mut effect: ParticleEffectDescription =
        serde_json::from_str(include_str!("../../tests/fixtures/particle-effect.json")).unwrap();
    effect.budget.max_particles = 64;
    effect.budget.max_events_per_step = 8;
    let emitter = &mut effect.emitters[0];
    emitter.capacity = 8;
    emitter.bursts[0].count = 1;
    emitter.lifetime_seconds = [1.0; 2];
    emitter.velocity_min = [0.0; 3];
    emitter.velocity_max = [0.0; 3];
    emitter.updates = vec![ParticleUpdate::Integrate {}];
    effect
}
fn cooked(effect: &ParticleEffectDescription, author: &str) -> CookedParticleEffect {
    effect.validate().unwrap();
    let programs = effect
        .emitters
        .iter()
        .map(|emitter| {
            let prefix = program::prefix(effect, emitter);
            let first = prefix.lines().count() as u32 + 1;
            let library = prefix + author + program::CONTRACT_CALLS;
            CookedParticleProgram {
                emitter: emitter.name.clone(),
                wgsl: program::compute_source(effect, emitter, &library),
                author_path: Some("test.particle.wgsl".into()),
                author_first_line: first,
                author_line_count: author.lines().count() as u32,
                particle_stride: program::particle_layout(emitter).2,
            }
        })
        .collect();
    CookedParticleEffect {
        program_contract: PARTICLE_PROGRAM_CONTRACT.into(),
        description: effect.clone(),
        programs,
        source_digests: Default::default(),
    }
}
fn request(id: u64) -> ParticleGpuStep {
    ParticleGpuStep {
        step_id: id,
        delta_seconds: 1.0 / 64.0,
        origin: [0.0; 3],
        emitting: true,
        paused: false,
    }
}
fn tick(
    effect: &mut ParticleGpuEffect,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    request: ParticleGpuStep,
) -> ParticleGpuStepReport {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    let report = effect.encode_step(device, &mut encoder, request).unwrap();
    queue.submit(Some(encoder.finish()));
    report
}
fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() <= 1e-5, "{actual} != {expected}");
}
fn capture(
    effect: &ParticleGpuEffect,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Vec<ParticleGpuSnapshot> {
    let snapshots = effect.readback_for_validation(device, queue).unwrap();
    for state in &snapshots {
        assert_eq!(state.live as usize, state.particles.len());
        assert_eq!(state.live, state.indirect_instances);
        assert_eq!(state.invalid, 0);
    }
    snapshots
}

#[test]
fn particle_gpu_author_code_layout_and_motion_have_red_capable_readback() {
    let (device, queue, _guard) = gpu();
    let mut desc = fixture();
    desc.emitters[0].custom_state = vec![
        ParticleStateField {
            name: "elapsed".into(),
            default: ParticleValue::Float(0.25),
        },
        ParticleStateField {
            name: "direction".into(),
            default: ParticleValue::Vec3([4.0, 5.0, 6.0]),
        },
        ParticleStateField {
            name: "counter".into(),
            default: ParticleValue::Uint(7),
        },
    ];
    desc.emitters[0].inputs = vec![ParticleInput {
        name: "wind".into(),
        value: ParticleValue::Vec3([0.0, 1.0, 0.0]),
    }];
    let author = r#"fn particle_init(p:Particle,c:ParticleContext,e:EffectParams,i:ParticleInputs)->Particle {var q=p;q.position=vec3<f32>(1.0,2.0,3.0);q.velocity=vec3<f32>(3.0,0.0,0.0);return q;}
fn particle_update(p:Particle,c:ParticleContext,e:EffectParams,i:ParticleInputs)->Particle {var q=p;q.position.y+=e.speed*c.dt;q.custom.elapsed+=c.dt;q.custom.direction+=i.wind*c.dt;q.custom.counter+=1u;return q;}
"#;
    let base = cooked(&desc, author);
    let matches = |p: &ParticleGpuSample| {
        (p.position[0] - 1.046875).abs() < 1e-4
            && (p.position[1] - 2.03125).abs() < 1e-4
            && word(&p.custom_bytes, 28) == 8
    };
    for variant in 0..3 {
        let mut program = base.clone();
        if variant == 1 {
            program.programs[0].wgsl = program.programs[0].wgsl.replace(
                "return particle_update(p, ctx, params, inputs);",
                "return p;",
            );
        }
        if variant == 2 {
            program.programs[0].wgsl = program.programs[0].wgsl.replace(
                "p.position+=p.velocity*step.dt;",
                "p.position+=p.velocity*step.dt*2.0;",
            );
        }
        let mut effect = ParticleGpuEffect::new(device, &program).unwrap();
        tick(&mut effect, device, queue, request(1));
        tick(&mut effect, device, queue, request(2));
        let state = capture(&effect, device, queue);
        let particle = &state[0].particles[0];
        eprintln!(
            "343 author variant={variant}: position={:?}, age={}, counter={}",
            particle.position,
            particle.age,
            word(&particle.custom_bytes, 28)
        );
        assert_eq!(
            matches(particle),
            variant == 0,
            "known-input oracle must reject skipped author code and double integration"
        );
        if variant == 0 {
            near(particle.age, 1.0 / 64.0);
            near(real(&particle.custom_bytes, 0), 0.265625);
            near(real(&particle.custom_bytes, 20), 5.015625);
            assert!(tick(&mut effect, device, queue, request(2)).duplicate);
            near(
                capture(&effect, device, queue)[0].particles[0].age,
                1.0 / 64.0,
            );
        }
    }
}

#[test]
fn particle_gpu_capacity_recycling_delay_burst_loop_and_rate() {
    let (device, queue, _guard) = gpu();
    let mut desc = fixture();
    let emitter = &mut desc.emitters[0];
    emitter.capacity = 4;
    emitter.bursts[0].count = 10;
    emitter.delay_seconds = 1.0 / 64.0;
    emitter.duration_seconds = 1.0 / 32.0;
    emitter.looping = true;
    emitter.lifetime_seconds = [1.0 / 64.0; 2];
    let mut effect =
        ParticleGpuEffect::new(device, &cooked(&desc, program::DEFAULT_BEHAVIOR)).unwrap();
    for (step, live, dropped) in [(1, 0, 0), (2, 4, 6), (3, 0, 0), (4, 4, 6)] {
        tick(&mut effect, device, queue, request(step));
        let state = capture(&effect, device, queue);
        assert_eq!(state[0].live, live);
        assert_eq!(state[0].dropped, dropped);
    }
    let mut desc = fixture();
    desc.emitters[0].bursts.clear();
    desc.emitters[0].rate_per_second = 64.0;
    desc.budget.quality_scale = 0.5;
    let mut effect =
        ParticleGpuEffect::new(device, &cooked(&desc, program::DEFAULT_BEHAVIOR)).unwrap();
    for step in 1..=4 {
        tick(&mut effect, device, queue, request(step));
    }
    assert_eq!(
        capture(&effect, device, queue)[0].live,
        2,
        "quality scaling must retain fractional emission instead of flooring every step to zero"
    );
}

#[test]
fn particle_gpu_shapes_curves_gravity_drag_noise_rotation_execute() {
    let (device, queue, _guard) = gpu();
    for shape in [
        ParticleShape::Point {},
        ParticleShape::Sphere { radius: 2.0 },
        ParticleShape::Box {
            half_extents: [1.0, 2.0, 3.0],
        },
        ParticleShape::Cone {
            radius: 2.0,
            angle_radians: 0.5,
        },
    ] {
        let mut desc = fixture();
        let emitter = &mut desc.emitters[0];
        emitter.shape = shape.clone();
        emitter.bursts[0].count = 8;
        emitter.velocity_min = [0.0, 2.0, 0.0];
        emitter.velocity_max = emitter.velocity_min;
        emitter.updates = vec![
            ParticleUpdate::Gravity {
                acceleration: [0.0, -2.0, 0.0],
            },
            ParticleUpdate::Drag { coefficient: 0.5 },
            ParticleUpdate::Noise {
                amplitude: 0.4,
                frequency: 2.0,
            },
            ParticleUpdate::Integrate {},
            ParticleUpdate::Rotate {
                radians_per_second: 1.0,
            },
            ParticleUpdate::SizeOverLife {
                keys: vec![
                    ParticleScalarKey {
                        time: 0.0,
                        value: 1.0,
                    },
                    ParticleScalarKey {
                        time: 1.0,
                        value: 0.0,
                    },
                ],
            },
            ParticleUpdate::ColorOverLife {
                keys: vec![
                    ParticleColorKey {
                        time: 0.0,
                        value: [1.0; 4],
                    },
                    ParticleColorKey {
                        time: 1.0,
                        value: [0.0; 4],
                    },
                ],
            },
        ];
        let mut effect =
            ParticleGpuEffect::new(device, &cooked(&desc, program::DEFAULT_BEHAVIOR)).unwrap();
        tick(&mut effect, device, queue, request(1));
        let initial = capture(&effect, device, queue).remove(0);
        for p in &initial.particles {
            match shape {
                ParticleShape::Point {} => assert_eq!(p.position, [0.0; 3]),
                ParticleShape::Sphere { radius } => {
                    assert!(p.position.iter().map(|x| x * x).sum::<f32>() <= radius * radius + 1e-4)
                }
                ParticleShape::Box { half_extents } => assert!(p
                    .position
                    .iter()
                    .zip(half_extents)
                    .all(|(p, b)| p.abs() <= b)),
                ParticleShape::Cone {
                    radius,
                    angle_radians,
                } => {
                    near(p.position[1], 0.0);
                    assert!(
                        p.position[0] * p.position[0] + p.position[2] * p.position[2]
                            <= radius * radius + 1e-4
                    );
                    assert!(p.velocity[1] >= 2.0 * angle_radians.cos() - 1e-4);
                }
            }
        }
        tick(&mut effect, device, queue, request(2));
        let next = capture(&effect, device, queue).remove(0);
        let dt = 1.0 / 64.0;
        for (before, after) in initial.particles.iter().zip(&next.particles) {
            for axis in 0..3 {
                let gravity = if axis == 1 { -2.0 } else { 0.0 };
                let expected = (before.velocity[axis] + gravity * dt) * (-0.5f32 * dt).exp();
                assert!((after.velocity[axis] - expected).abs() <= 0.4 * dt + 1e-4);
                near(
                    after.position[axis],
                    before.position[axis] + after.velocity[axis] * dt,
                );
            }
            near(after.rotation, dt);
            near(after.size[0], before.size[0] * (1.0 - dt));
            near(after.color[3], 1.0 - dt);
        }
        assert!(next
            .particles
            .iter()
            .any(|p| p.velocity[0].abs() > 1e-6 || p.velocity[2].abs() > 1e-6));
        tick(&mut effect, device, queue, request(3));
        near(
            capture(&effect, device, queue)[0].particles[0].size[0],
            initial.particles[0].size[0] * (1.0 - 2.0 * dt),
        );
    }
}

#[test]
fn particle_gpu_ten_thousand_and_hundred_thousand_capacity_allocate_with_bounded_budget() {
    let (device, _queue, _guard) = gpu();
    for capacity in [1_000_u32, 10_000, 100_000] {
        let mut desc = fixture();
        desc.budget.max_particles = capacity;
        desc.budget.max_events_per_step = capacity.min(65_536);
        let emitter = &mut desc.emitters[0];
        emitter.capacity = capacity;
        emitter.bursts[0].count = capacity.min(65_536);
        emitter.lifetime_seconds = [10.0; 2];
        let program = cooked(&desc, program::DEFAULT_BEHAVIOR);
        let effect = ParticleGpuEffect::new(device, &program)
            .unwrap_or_else(|error| panic!("capacity {capacity} must be allocatable: {error}"));
        assert!(effect.buffer_bytes > u64::from(capacity) * 64);
        assert!(effect.buffer_bytes < 128 * 1024 * 1024);
    }
}

#[test]
fn particle_gpu_emitter_count_variants_share_bounded_batch_contract() {
    let (device, _queue, _guard) = gpu();
    for emitter_count in [1_usize, 16, 64] {
        let per_emitter = 1_000_u32;
        let mut desc = fixture();
        desc.budget.max_particles = per_emitter * emitter_count as u32;
        desc.budget.max_events_per_step = 65_536;
        desc.emitters[0].capacity = per_emitter;
        desc.emitters[0].bursts[0].count = per_emitter.min(65_536);
        for index in 1..emitter_count {
            let mut emitter = desc.emitters[0].clone();
            emitter.name = format!("emitter_{index}");
            desc.emitters.push(emitter);
        }
        let effect = ParticleGpuEffect::new(device, &cooked(&desc, program::DEFAULT_BEHAVIOR))
            .unwrap_or_else(|error| {
                panic!("{emitter_count} emitters must be allocatable: {error}")
            });
        assert!(effect.buffer_bytes < 128 * 1024 * 1024);
    }
}

#[test]
fn particle_gpu_collision_events_are_bounded_and_consumed_next_step() {
    let (device, queue, _guard) = gpu();
    let mut desc = fixture();
    desc.budget.max_events_per_step = 1;
    let mut child = desc.emitters[0].clone();
    child.name = "child".into();
    child.bursts.clear();
    child.capacity = 4;
    let parent = &mut desc.emitters[0];
    parent.capacity = 2;
    parent.bursts[0].count = 2;
    parent.velocity_min = [0.0, -1.0, 0.0];
    parent.velocity_max = parent.velocity_min;
    parent.collisions.push(ParticleCollision {
        shape: ParticleCollider::Plane {
            normal: [0.0, 1.0, 0.0],
            distance: 0.1,
        },
        restitution: 1.0,
        kill: false,
    });
    parent.child_emission.push(ParticleChildEmission {
        event: ParticleEvent::Collision,
        target_emitter: "child".into(),
        count: 1,
        max_generation: 1,
    });
    desc.emitters.push(child);
    let mut effect =
        ParticleGpuEffect::new(device, &cooked(&desc, program::DEFAULT_BEHAVIOR)).unwrap();
    tick(&mut effect, device, queue, request(1));
    tick(&mut effect, device, queue, request(2));
    let state = capture(&effect, device, queue);
    assert_eq!(state[0].collisions, 2);
    assert_eq!(state[1].live, 0);
    near(state[0].particles[0].position[1], 0.1);
    near(state[0].particles[0].velocity[1], 1.0);
    assert_eq!(
        effect
            .read_event_counts_for_validation(device, queue)
            .unwrap(),
        (1, 1)
    );
    tick(&mut effect, device, queue, request(3));
    let state = capture(&effect, device, queue);
    assert_eq!(state[1].live, 1);
    assert_eq!(state[1].particles[0].generation, 1);
    near(state[1].particles[0].position[1], 0.1);
}

#[test]
fn particle_gpu_sphere_kill_death_and_programmable_child_requests() {
    let (device, queue, _guard) = gpu();
    for kill in [false, true] {
        let mut desc = fixture();
        desc.emitters[0].collisions.push(ParticleCollision {
            shape: ParticleCollider::Sphere {
                center: [0.0; 3],
                radius: 1.0,
            },
            restitution: 1.0,
            kill,
        });
        desc.emitters[0].child_emission.push(ParticleChildEmission {
            event: ParticleEvent::Death,
            target_emitter: "main".into(),
            count: 1,
            max_generation: 1,
        });
        let mut effect =
            ParticleGpuEffect::new(device, &cooked(&desc, program::DEFAULT_BEHAVIOR)).unwrap();
        tick(&mut effect, device, queue, request(1));
        tick(&mut effect, device, queue, request(2));
        let state = capture(&effect, device, queue);
        assert_eq!(state[0].collisions, 1);
        if kill {
            assert_eq!(state[0].live, 0);
            assert_eq!(state[0].died, 1);
            assert_eq!(
                effect
                    .read_event_counts_for_validation(device, queue)
                    .unwrap()
                    .0,
                1
            );
        } else {
            near(state[0].particles[0].position[1], 1.0);
        }
    }
    let mut desc = fixture();
    desc.emitters[0].child_emission.push(ParticleChildEmission {
        event: ParticleEvent::Death,
        target_emitter: "main".into(),
        count: 1,
        max_generation: 1,
    });
    let author =
        program::DEFAULT_BEHAVIOR.replacen("return p;", "var q=p;q.emit_children=1u;return q;", 1);
    let mut effect = ParticleGpuEffect::new(device, &cooked(&desc, &author)).unwrap();
    for id in 1..=3 {
        tick(&mut effect, device, queue, request(id));
    }
    let state = capture(&effect, device, queue);
    assert_eq!(state[0].live, 2);
    assert_eq!(
        state[0]
            .particles
            .iter()
            .filter(|p| p.generation == 1)
            .count(),
        1
    );
    assert_eq!(
        effect
            .read_event_counts_for_validation(device, queue)
            .unwrap()
            .0,
        0
    );
}

#[test]
fn particle_gpu_step_uniforms_are_not_overwritten_before_submission() {
    let (device, queue, _guard) = gpu();
    let mut desc = fixture();
    desc.emitters[0].bursts.clear();
    desc.emitters[0].rate_per_second = 64.0;
    let mut effect =
        ParticleGpuEffect::new(device, &cooked(&desc, program::DEFAULT_BEHAVIOR)).unwrap();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    effect
        .encode_step(device, &mut encoder, request(1))
        .unwrap();
    let mut second = request(2);
    second.origin[0] = 10.0;
    effect.encode_step(device, &mut encoder, second).unwrap();
    queue.submit(Some(encoder.finish()));
    let state = capture(&effect, device, queue);
    assert_eq!(state[0].live, 2);
    near(state[0].particles[0].position[0], 0.0);
    near(state[0].particles[1].position[0], 10.0);
}

#[test]
fn particle_gpu_catchup_pause_clear_and_seeded_replay_keep_buffers() {
    let (device, queue, _guard) = gpu();
    let mut desc = fixture();
    desc.emitters[0].shape = ParticleShape::Sphere { radius: 2.0 };
    let mut effect =
        ParticleGpuEffect::new(device, &cooked(&desc, program::DEFAULT_BEHAVIOR)).unwrap();
    let bytes = effect.buffer_bytes();
    let mut first = request(1);
    first.delta_seconds = 1.0;
    let report = tick(&mut effect, device, queue, first);
    assert_eq!(report.substeps, 8);
    near(report.discarded_seconds, 1.0 - 8.0 / 60.0);
    let before = capture(&effect, device, queue)
        .remove(0)
        .particles
        .remove(0);
    near(before.age, 7.0 / 60.0);
    let mut paused = request(2);
    paused.paused = true;
    paused.delta_seconds = 3.0;
    assert_eq!(tick(&mut effect, device, queue, paused).substeps, 0);
    near(
        capture(&effect, device, queue)[0].particles[0].age,
        before.age,
    );
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    effect.encode_clear(&mut encoder);
    queue.submit(Some(encoder.finish()));
    assert_eq!(capture(&effect, device, queue)[0].live, 0);
    assert_eq!(
        effect
            .read_event_counts_for_validation(device, queue)
            .unwrap(),
        (0, 0)
    );
    assert_eq!(effect.buffer_bytes(), bytes);
    tick(&mut effect, device, queue, request(3));
    let after = &capture(&effect, device, queue)[0].particles[0];
    assert_eq!(after.position, before.position);
    assert_eq!(effect.buffer_bytes(), bytes);
}

#[test]
fn particle_gpu_rejects_unsupported_layout_and_invalid_steps() {
    assert!(!supports_particle_compute(
        &wgpu::Limits::downlevel_webgl2_defaults()
    ));
    let (device, queue, _guard) = gpu();
    let desc = fixture();
    let mut code = cooked(&desc, program::DEFAULT_BEHAVIOR);
    code.programs[0].particle_stride += 16;
    assert!(ParticleGpuEffect::new(device, &code)
        .err()
        .unwrap()
        .contains("layout_mismatch"));
    let mut effect =
        ParticleGpuEffect::new(device, &cooked(&desc, program::DEFAULT_BEHAVIOR)).unwrap();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    let mut bad = request(1);
    bad.delta_seconds = f32::NAN;
    assert!(effect.encode_step(device, &mut encoder, bad).is_err());
    assert_eq!(effect.time_seconds(), 0.0);
    tick(&mut effect, device, queue, request(1));
    assert_eq!(capture(&effect, device, queue)[0].live, 1);
}

#[test]
fn particle_gpu_custom_position_ownership_and_stop_emitting() {
    let (device, queue, _guard) = gpu();
    let mut desc = fixture();
    desc.emitters[0].custom_owns_position = true;
    desc.emitters[0].behavior_source = Some("test.particle.wgsl".into());
    desc.emitters[0].updates.clear();
    desc.emitters[0].velocity_min = [100.0; 3];
    desc.emitters[0].velocity_max = [100.0; 3];
    desc.emitters[0].rate_per_second = 64.0;
    desc.emitters[0].bursts.clear();
    let author=program::DEFAULT_BEHAVIOR.replace("fn particle_update(p: Particle, ctx: ParticleContext, params: EffectParams, inputs: ParticleInputs) -> Particle { return p; }", "fn particle_update(p: Particle, ctx: ParticleContext, params: EffectParams, inputs: ParticleInputs) -> Particle { var q=p;q.position.x+=ctx.dt;return q; }");
    let mut effect = ParticleGpuEffect::new(device, &cooked(&desc, &author)).unwrap();
    tick(&mut effect, device, queue, request(1));
    let mut stopped = request(2);
    stopped.emitting = false;
    tick(&mut effect, device, queue, stopped);
    let state = capture(&effect, device, queue);
    assert_eq!(state[0].live, 1);
    assert_eq!(state[0].spawned, 0);
    near(state[0].particles[0].position[0], 1.0 / 64.0);
    near(state[0].particles[0].position[1], 0.0);
    near(state[0].particles[0].age, 1.0 / 64.0);
}

#[test]
fn particle_gpu_loop_rate_retains_fractional_cycle_emissions() {
    let (device, queue, _guard) = gpu();
    let mut desc = fixture();
    desc.emitters[0].bursts.clear();
    desc.emitters[0].looping = true;
    desc.emitters[0].duration_seconds = 1.0 / 128.0;
    desc.emitters[0].rate_per_second = 64.0;
    let mut effect =
        ParticleGpuEffect::new(device, &cooked(&desc, program::DEFAULT_BEHAVIOR)).unwrap();
    tick(&mut effect, device, queue, request(1));
    assert_eq!(capture(&effect, device, queue)[0].live, 1);
}

#[test]
fn particle_gpu_collision_overflow_is_recycled_before_events() {
    let (device, queue, _guard) = gpu();
    let mut desc = fixture();
    desc.emitters[0].velocity_min = [-3.0e38, -3.0e38, 0.0];
    desc.emitters[0].velocity_max = desc.emitters[0].velocity_min;
    desc.emitters[0].collisions.push(ParticleCollision {
        shape: ParticleCollider::Plane {
            normal: [0.6, 0.8, 0.0],
            distance: 1.0,
        },
        restitution: 1.0,
        kill: false,
    });
    let mut effect =
        ParticleGpuEffect::new(device, &cooked(&desc, program::DEFAULT_BEHAVIOR)).unwrap();
    tick(&mut effect, device, queue, request(1));
    tick(&mut effect, device, queue, request(2));
    let state = effect.readback_for_validation(device, queue).unwrap();
    assert_eq!(
        (state[0].live, state[0].invalid),
        (0, 1),
        "GPU state: {:?}",
        state[0]
    );
    assert!(state[0].particles.is_empty());
    assert_eq!(
        effect
            .read_event_counts_for_validation(device, queue)
            .unwrap()
            .0,
        0
    );
}

#[test]
fn particle_gpu_existing_backend_owns_simulation_and_removal() {
    let mut backend = crate::wgpu_backend::real::RealWgpuBackend::new_offscreen(16, 16).unwrap();
    let desc = fixture();
    let code = cooked(&desc, program::DEFAULT_BEHAVIOR);
    backend.install_particle_effect(7, &code).unwrap();
    assert!(backend.install_particle_effect(7, &code).is_err());
    backend.simulate_particle_effect(7, request(1)).unwrap();
    assert_eq!(
        backend.capture_particle_state_for_validation(7).unwrap()[0].live,
        1
    );
    let bytes = backend.particle_buffer_bytes(7);
    backend.clear_particle_effect(7).unwrap();
    assert_eq!(
        backend.capture_particle_state_for_validation(7).unwrap()[0].live,
        0
    );
    assert_eq!(backend.particle_buffer_bytes(7), bytes);
    assert!(backend.remove_particle_effect(7));
    assert_eq!(backend.particle_buffer_bytes(7), None);
    assert!(backend.simulate_particle_effect(7, request(2)).is_err());
}

#[test]
fn particle_gpu_sixty_four_emitters_execute_with_renderer_limits() {
    let mut desc = fixture();
    let template = desc.emitters[0].clone();
    desc.budget.max_particles = 100_000;
    desc.emitters = (0..64)
        .map(|i| {
            let mut e = template.clone();
            e.name = format!("emitter_{i}");
            e.capacity = 100_000 / 64 + u32::from(i < 100_000 % 64);
            e.bursts[0].count = e.capacity;
            e
        })
        .collect();
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: renderer_device_limits(adapter.limits()),
        ..Default::default()
    }))
    .unwrap();
    let mut effect =
        ParticleGpuEffect::new(&device, &cooked(&desc, program::DEFAULT_BEHAVIOR)).unwrap();
    device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut encoder = device.create_command_encoder(&Default::default());
    effect
        .encode_step(&device, &mut encoder, request(1))
        .unwrap();
    queue.submit(Some(encoder.finish()));
    let error = pollster::block_on(device.pop_error_scope());
    assert!(error.is_none(), "actual renderer limits: {error:?}");
    let live: u32 = effect
        .emitters
        .iter()
        .zip(&desc.emitters)
        .map(|(e, desc)| {
            let header = read_buffer(&device, &queue, &e.scratch, 48).unwrap();
            assert_eq!(word(&header, 0), desc.capacity);
            assert_eq!(word(&header, 12), desc.capacity);
            word(&header, 0)
        })
        .sum();
    assert_eq!(live, 100_000);
}
