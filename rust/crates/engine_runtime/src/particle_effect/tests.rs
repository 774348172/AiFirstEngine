use super::*;

fn fixture() -> ParticleEffectDescription {
    serde_json::from_str(include_str!("../../tests/fixtures/particle-effect.json")).unwrap()
}

#[test]
fn particle_effect_minimal_defaults_and_roundtrip() {
    let effect = fixture();
    effect.validate().unwrap();
    assert_eq!(effect.budget.quality_scale, 1.0);
    assert_eq!(effect.emitters[0].rate_per_second, 0.0);
    assert_eq!(
        effect,
        serde_json::from_slice(&serde_json::to_vec(&effect).unwrap()).unwrap()
    );
}

#[test]
fn particle_effect_rejects_unknown_fields_types_and_units() {
    let base = serde_json::to_value(fixture()).unwrap();
    for pointer in [
        "/unknown",
        "/emitters/0/unknown",
        "/budget/unknown",
        "/emitters/0/shape/unknown",
        "/emitters/0/updates/0/unknown",
        "/parameters/0/default/unknown",
    ] {
        let mut value = base.clone();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        value
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.into(), true.into());
        assert!(
            serde_json::from_value::<ParticleEffectDescription>(value).is_err(),
            "accepted {pointer}"
        );
    }
    for (pointer, value) in [
        ("/parameters/0/unit", serde_json::json!("milliseconds")),
        ("/emitters/0/durationSeconds", serde_json::json!("2")),
        ("/emitters/0/shape/kind", serde_json::json!("explosion")),
    ] {
        let mut raw = base.clone();
        *raw.pointer_mut(pointer).unwrap() = value;
        assert!(serde_json::from_value::<ParticleEffectDescription>(raw).is_err());
    }
}

#[test]
fn particle_effect_rejects_duplicate_identity_and_unsafe_names() {
    let mut effect = fixture();
    effect.emitters.push(effect.emitters[0].clone());
    assert!(effect.validate().unwrap_err().message.contains("unique"));
    let mut effect = fixture();
    effect.parameters.push(effect.parameters[0].clone());
    assert!(effect.validate().is_err());
    for name in ["", "../escape", "x/y", ".."] {
        let mut effect = fixture();
        effect.asset_id = name.into();
        assert!(effect.validate().is_err());
    }
    for name in ["engine_unsafe", "123name", "x.y", "a:b"] {
        let mut effect = fixture();
        effect.parameters[0].name = name.into();
        assert!(effect.validate().is_err());
    }
}

#[test]
fn particle_effect_rejects_nonfinite_ranges_and_budget_overflow() {
    for bad in [f32::NAN, f32::INFINITY, -1.0] {
        let mut effect = fixture();
        effect.emitters[0].duration_seconds = bad;
        assert!(effect.validate().is_err());
        let mut effect = fixture();
        effect.bounds.half_extents[0] = bad;
        assert!(effect.validate().is_err());
    }
    let mut effect = fixture();
    effect.emitters[0].velocity_min[1] = 3.0;
    assert!(effect.validate().is_err());
    let mut effect = fixture();
    effect.budget.max_particles = 10;
    assert!(effect.validate().is_err());
    let mut effect = fixture();
    effect.parameters[0].range = Some([4.0, 5.0]);
    assert!(effect.validate().is_err());
    let mut effect = fixture();
    effect.parameters[0].default = ParticleValue::Uint(u32::MAX);
    effect.parameters[0].range = Some([0.0, 16_777_216.0]);
    assert!(effect.validate().is_err());
}

#[test]
fn particle_effect_motion_has_exactly_one_owner() {
    let mut effect = fixture();
    effect.emitters[0]
        .updates
        .push(ParticleUpdate::Integrate {});
    assert!(effect.validate().is_err());
    let mut effect = fixture();
    effect.emitters[0].custom_owns_position = true;
    effect.emitters[0].behavior_source = Some("Assets/motion.particle.wgsl".into());
    assert!(effect.validate().is_err());
    effect.emitters[0]
        .updates
        .retain(|v| !matches!(v, ParticleUpdate::Integrate {}));
    effect.validate().unwrap();
    effect.emitters[0].behavior_source = None;
    assert!(effect.validate().is_err());
}

#[test]
fn particle_effect_curves_have_normalized_ordered_finite_keys() {
    let mut effect = fixture();
    effect.emitters[0]
        .updates
        .push(ParticleUpdate::SizeOverLife {
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
        });
    effect.validate().unwrap();
    for times in [[0.0, 0.0], [0.5, 1.0], [0.0, 1.1], [0.0, f32::NAN]] {
        effect.emitters[0].updates[2] = ParticleUpdate::SizeOverLife {
            keys: times
                .into_iter()
                .map(|time| ParticleScalarKey { time, value: 1.0 })
                .collect(),
        };
        assert!(effect.validate().is_err());
    }
}

#[test]
fn particle_effect_child_emission_is_bounded_and_resolves_names() {
    let mut effect = fixture();
    effect.emitters[0]
        .child_emission
        .push(ParticleChildEmission {
            event: ParticleEvent::Collision,
            target_emitter: "main".into(),
            count: 4,
            max_generation: 1,
        });
    effect.validate().unwrap();
    effect.emitters[0].child_emission[0].max_generation = 0;
    assert!(effect.validate().is_err());
    effect.emitters[0].child_emission[0].max_generation = 1;
    effect.emitters[0].child_emission[0].count = 17;
    assert!(effect.validate().is_err());
    effect.emitters[0].child_emission[0].count = 1;
    effect.emitters[0].child_emission[0].target_emitter = "missing".into();
    assert!(effect.validate().is_err());
}

#[test]
fn particle_effect_full_description_preserves_authored_render_and_motion_data() {
    let mut effect = fixture();
    let mut emitter = effect.emitters[0].clone();
    emitter.name = "secondary".into();
    emitter.space = ParticleSpace::Local;
    emitter.shape = ParticleShape::Cone {
        radius: 2.0,
        angle_radians: 0.5,
    };
    emitter.rate_per_second = 50.0;
    emitter.delay_seconds = 0.2;
    emitter.looping = true;
    emitter.draw.geometry = ParticleGeometry::Mesh {
        asset: RuntimeAssetRef {
            id: "shard".into(),
            asset_type: "mesh".into(),
            guid: Some("guid-shard".into()),
            sub_asset: None,
        },
    };
    emitter.draw.texture = Some(RuntimeAssetRef {
        id: "smoke".into(),
        asset_type: "texture".into(),
        guid: Some("guid-smoke".into()),
        sub_asset: None,
    });
    emitter.draw.flipbook = Some(ParticleFlipbook {
        columns: 4,
        rows: 4,
        fps: 12.0,
    });
    emitter.draw.trail = Some(ParticleTrail {
        segments: 8,
        lifetime_seconds: 0.2,
        width_meters: 0.01,
    });
    emitter.draw.blend = ParticleBlend::Alpha;
    emitter.draw.sort = ParticleSort::BackToFront;
    emitter.collisions.push(ParticleCollision {
        shape: ParticleCollider::Plane {
            normal: [0.0, 1.0, 0.0],
            distance: 0.0,
        },
        restitution: 0.4,
        kill: false,
    });
    emitter.collisions.push(ParticleCollision {
        shape: ParticleCollider::Sphere {
            center: [1.0, 1.0, 1.0],
            radius: 1.0,
        },
        restitution: 0.0,
        kill: true,
    });
    emitter.updates.extend([
        ParticleUpdate::Noise {
            amplitude: 0.4,
            frequency: 2.0,
        },
        ParticleUpdate::Drag { coefficient: 0.1 },
        ParticleUpdate::Rotate {
            radians_per_second: 2.0,
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
    ]);
    effect.emitters.push(emitter);
    effect.validate().unwrap();
    let encoded = serde_json::to_string(&effect).unwrap();
    assert!(encoded.contains("angleRadians") && encoded.contains("radiansPerSecond"));
    assert_eq!(effect, serde_json::from_str(&encoded).unwrap());
    assert_eq!(effect.asset_refs().len(), 2);
}

#[test]
fn particle_effect_rejects_invalid_render_and_collision_contracts() {
    let mut effect = fixture();
    effect.emitters[0].draw.trail = Some(ParticleTrail {
        segments: u32::MAX,
        lifetime_seconds: 1.0,
        width_meters: 0.1,
    });
    assert!(effect.validate().is_err());
    let mut effect = fixture();
    effect.emitters[0].draw.flipbook = Some(ParticleFlipbook {
        columns: 1,
        rows: 1,
        fps: 1.0,
    });
    assert!(effect.validate().is_err());
    let mut effect = fixture();
    effect.emitters[0].collisions.push(ParticleCollision {
        shape: ParticleCollider::Plane {
            normal: [0.0; 3],
            distance: 0.0,
        },
        restitution: 0.0,
        kill: false,
    });
    assert!(effect.validate().is_err());
}
