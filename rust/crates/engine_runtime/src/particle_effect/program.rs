//! Shared particle language layout and compiler-side compute source generation.
//! Runtime consumes the cooked program; it never opens author source files.
use super::*;

pub const DEFAULT_BEHAVIOR: &str = r#"fn particle_init(p: Particle, ctx: ParticleContext, params: EffectParams, inputs: ParticleInputs) -> Particle { return p; }
fn particle_update(p: Particle, ctx: ParticleContext, params: EffectParams, inputs: ParticleInputs) -> Particle { return p; }
"#;
pub const CONTRACT_CALLS: &str = r#"
fn engine_particle_init(p: Particle, ctx: ParticleContext, params: EffectParams, inputs: ParticleInputs) -> Particle { return particle_init(p, ctx, params, inputs); }
fn engine_particle_update(p: Particle, ctx: ParticleContext, params: EffectParams, inputs: ParticleInputs) -> Particle { return particle_update(p, ctx, params, inputs); }
"#;

pub fn prefix(effect: &ParticleEffectDescription, emitter: &ParticleEmitter) -> String {
    let mut source = fields(
        "ParticleState",
        emitter
            .custom_state
            .iter()
            .map(|p| (p.name.as_str(), &p.default)),
    );
    source += &fields(
        "EffectParams",
        effect
            .parameters
            .iter()
            .map(|p| (p.name.as_str(), &p.default)),
    );
    source += &fields(
        "ParticleInputs",
        emitter.inputs.iter().map(|p| (p.name.as_str(), &p.value)),
    );
    source += r#"struct ParticleContext { time: f32, dt: f32, particle_id: u32, seed: u32, }
struct Particle {
    position: vec3<f32>, age: f32,
    velocity: vec3<f32>, lifetime: f32,
    color: vec4<f32>, size: vec2<f32>, rotation: f32, seed: u32,
    emit_children: u32,
    custom: ParticleState,
}
"#;
    source
}
fn fields<'a>(name: &str, entries: impl Iterator<Item = (&'a str, &'a ParticleValue)>) -> String {
    let mut body = String::new();
    for (name, value) in entries {
        body += &format!("    {name}: {},\n", value.wgsl_type());
    }
    if body.is_empty() {
        body += "    engine_unused: u32,\n";
    }
    format!("struct {name} {{\n{body}}}\n")
}
pub fn align(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}
pub fn value_layout(value: &ParticleValue) -> (u32, u32) {
    match value {
        ParticleValue::Float(_) | ParticleValue::Uint(_) => (4, 4),
        ParticleValue::Vec3(_) => (16, 12),
        ParticleValue::Vec4(_) => (16, 16),
    }
}
/// WGSL storage layout: offsets, alignment and padded size of one generated struct.
pub fn layout<'a>(values: impl Iterator<Item = &'a ParticleValue>) -> (Vec<u32>, u32, u32) {
    let mut offset = 0;
    let mut alignment = 4;
    let mut offsets = Vec::new();
    for value in values {
        let (a, size) = value_layout(value);
        alignment = alignment.max(a);
        offset = align(offset, a);
        offsets.push(offset);
        offset += size;
    }
    (offsets, alignment, align(offset.max(4), alignment))
}
pub fn particle_layout(emitter: &ParticleEmitter) -> (u32, u32, u32) {
    let (_, alignment, size) = layout(emitter.custom_state.iter().map(|p| &p.default));
    let custom = align(68, alignment);
    let particle = align(custom + size, 16);
    (custom, particle, align(particle + 40, 16))
}
pub fn parameter_bytes(effect: &ParticleEffectDescription, emitter: &ParticleEmitter) -> Vec<u8> {
    let (offsets, pa, ps) = layout(effect.parameters.iter().map(|p| &p.default));
    let (input_offsets, ia, is) = layout(emitter.inputs.iter().map(|p| &p.value));
    let input_start = align(ps, ia);
    let mut bytes = vec![0; align(input_start + is, pa.max(ia)) as usize];
    for (p, offset) in effect.parameters.iter().zip(offsets) {
        write_value(&mut bytes, offset, &p.default);
    }
    for (p, offset) in emitter.inputs.iter().zip(input_offsets) {
        write_value(&mut bytes, input_start + offset, &p.value);
    }
    bytes
}
fn write_value(bytes: &mut [u8], offset: u32, value: &ParticleValue) {
    let data: Vec<u8> = match value {
        ParticleValue::Float(v) => v.to_le_bytes().to_vec(),
        ParticleValue::Uint(v) => v.to_le_bytes().to_vec(),
        ParticleValue::Vec3(v) => v.iter().flat_map(|v| v.to_le_bytes()).collect(),
        ParticleValue::Vec4(v) => v.iter().flat_map(|v| v.to_le_bytes()).collect(),
    };
    bytes[offset as usize..offset as usize + data.len()].copy_from_slice(&data);
}
fn float(v: f32) -> String {
    format!("{v:?}")
}
fn vector(v: &[f32]) -> String {
    format!(
        "vec{}<f32>({})",
        v.len(),
        v.iter().map(|v| float(*v)).collect::<Vec<_>>().join(",")
    )
}
fn literal(v: &ParticleValue) -> String {
    match v {
        ParticleValue::Float(v) => float(*v),
        ParticleValue::Uint(v) => format!("{v}u"),
        ParticleValue::Vec3(v) => vector(v),
        ParticleValue::Vec4(v) => vector(v),
    }
}

pub fn compute_source(
    effect: &ParticleEffectDescription,
    emitter: &ParticleEmitter,
    library: &str,
) -> String {
    let mut source = library.to_string();
    for (name, excluded) in [
        ("engine_spawn_params", ParticleParameterStage::Update),
        ("engine_update_params", ParticleParameterStage::Spawn),
    ] {
        source += &format!("\nfn {name}(value:EffectParams)->EffectParams {{ var result=value;\n");
        for p in &effect.parameters {
            if p.stage == excluded {
                source += &format!("result.{}={};\n", p.name, literal(&p.default));
            }
        }
        source += "return result; }\n";
    }
    source += &format!(
        "\nconst ENGINE_CAPACITY:u32={}u;\nconst ENGINE_WORLD:bool={};\n",
        emitter.capacity,
        emitter.space == ParticleSpace::World
    );
    source += include_str!("simulation.wgsl");
    source += "\nfn engine_initial(id:u32, origin:vec3<f32>)->Particle {\nvar p:Particle;\n";
    source+=&format!("p.seed=engine_hash(step.seed ^ id); p.position=origin; p.lifetime=mix({}, {}, engine_random(p.seed,1u));\n",float(emitter.lifetime_seconds[0]),float(emitter.lifetime_seconds[1]));
    source += &format!(
        "p.velocity=mix({}, {}, engine_random3(p.seed)); p.color={}; p.size={}; p.rotation={};\n",
        vector(&emitter.velocity_min),
        vector(&emitter.velocity_max),
        vector(&emitter.color),
        vector(&emitter.size_meters),
        float(emitter.rotation_radians)
    );
    match &emitter.shape {
        ParticleShape::Point {}=>(),
        ParticleShape::Box {half_extents}=>source+=&format!("p.position+=(engine_random3(p.seed ^ 137u)*2.0-vec3<f32>(1.0))*{};\n",vector(half_extents)),
        ParticleShape::Sphere {radius}=>source+=&format!("p.position+=engine_direction(p.seed)*pow(engine_random(p.seed,9u),1.0/3.0)*{};\n",float(*radius)),
        ParticleShape::Cone {radius,angle_radians}=>source+=&format!("let a=engine_random(p.seed,7u)*6.2831853; let r=sqrt(engine_random(p.seed,8u))*{}; p.position+=vec3<f32>(cos(a)*r,0.0,sin(a)*r); let c=mix(cos({}),1.0,engine_random(p.seed,9u)); let s=sqrt(max(0.0,1.0-c*c)); p.velocity=vec3<f32>(cos(a)*s,c,sin(a)*s)*length(p.velocity);\n",float(*radius),float(*angle_radians)),
    }
    for field in &emitter.custom_state {
        source += &format!("p.custom.{}={};\n", field.name, literal(&field.default));
    }
    source+="return engine_particle_init(p,ParticleContext(step.time,step.dt,id,p.seed),engine_spawn_params(parameters.effect),parameters.inputs);\n}\n";
    source+="fn engine_standard(particle:Particle,base_color:vec4<f32>,base_size:vec2<f32>)->Particle {\nvar p=particle; let life=clamp(p.age/max(p.lifetime,0.000001),0.0,1.0);\n";
    for update in &emitter.updates {
        match update {
            ParticleUpdate::Integrate {}=>source+="p.position+=p.velocity*step.dt;\n",
            ParticleUpdate::Gravity {acceleration}=>source+=&format!("p.velocity+={}*step.dt;\n",vector(acceleration)),
            ParticleUpdate::Drag {coefficient}=>source+=&format!("p.velocity*=exp(-{}*step.dt);\n",float(*coefficient)),
            ParticleUpdate::Noise {amplitude,frequency}=>source+=&format!("p.velocity+=sin(p.position*{}+vec3<f32>(step.time+engine_random(p.seed,12u)*6.2831853))*{}*step.dt;\n",float(*frequency),float(*amplitude)),
            ParticleUpdate::Rotate {radians_per_second}=>source+=&format!("p.rotation+={}*step.dt;\n",float(*radians_per_second)),
            ParticleUpdate::SizeOverLife {keys}=>source+=&curve("p.size","base_size",keys.iter().map(|k|(k.time,float(k.value))).collect()),
            ParticleUpdate::ColorOverLife {keys}=>source+=&curve("p.color","base_color",keys.iter().map(|k|(k.time,vector(&k.value))).collect()),
        }
    }
    source += "return p;\n}\n";
    source +=
        "fn engine_collide(particle:Particle)->CollisionResult {\nvar p=particle; var hit=false;\n";
    for collision in &emitter.collisions {
        match collision.shape {
            ParticleCollider::Plane {normal,distance}=>source+=&format!("{{let n={};let d=dot(p.position,n)-({});if(d<0.0){{hit=true;p.position-=d*n;p.velocity-=min(dot(p.velocity,n),0.0)*{}*n;{} }} }}\n",vector(&normal),float(distance),float(1.0+collision.restitution),if collision.kill {"p.lifetime=0.0;"} else {""}),
            ParticleCollider::Sphere {center,radius}=>source+=&format!("{{let delta=p.position-{};let d=length(delta);if(d<{}){{hit=true;var n=vec3<f32>(0.0,1.0,0.0);if(d>0.000001){{n=delta/d;}}p.position={}+n*{};p.velocity-=min(dot(p.velocity,n),0.0)*{}*n;{} }} }}\n",vector(&center),float(radius),vector(&center),float(radius),float(1.0+collision.restitution),if collision.kill {"p.lifetime=0.0;"} else {""}),
        }
    }
    source += "return CollisionResult(p,hit);\n}\n";
    source += "fn engine_events(p:Particle,generation:u32,event:u32) {\n";
    for (i, child) in emitter.child_emission.iter().enumerate() {
        let target = effect
            .emitters
            .iter()
            .position(|e| e.name == child.target_emitter)
            .unwrap();
        let event = match child.event {
            ParticleEvent::Spawn => 1,
            ParticleEvent::Death => 2,
            ParticleEvent::Collision => 3,
        };
        source+=&format!("if(generation<{}u && (event=={event}u || (event==0u && (p.emit_children & {}u)!=0u))){{engine_emit(p,generation+1u,{}u,{}u);}}\n",child.max_generation,1u32<<i,target,child.count);
    }
    source += "}\nfn engine_custom_finite(p:Particle)->bool { return true";
    for field in &emitter.custom_state {
        let expression = match &field.default {
            ParticleValue::Uint(_) => None,
            ParticleValue::Float(_) => Some(format!("abs(p.custom.{})<=3.4e38", field.name)),
            _ => Some(format!(
                "all(abs(p.custom.{})<= {}(3.4e38))",
                field.name,
                field.default.wgsl_type()
            )),
        };
        if let Some(v) = expression {
            source += &format!(" && ({v})");
        }
    }
    source += "; }\n";
    source
}
fn curve(field: &str, base: &str, keys: Vec<(f32, String)>) -> String {
    let mut result = String::new();
    for (index, pair) in keys.windows(2).enumerate() {
        result += &format!(
            "{}if(life<={}){{{field}={base}*mix({}, {}, clamp((life-{})/{},0.0,1.0));}}",
            if index == 0 { "" } else { "else " },
            float(pair[1].0),
            pair[0].1,
            pair[1].1,
            float(pair[0].0),
            float(pair[1].0 - pair[0].0)
        );
    }
    result + "\n"
}
