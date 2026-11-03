struct EngineRecord {
    particle: Particle,
    base_color: vec4<f32>, base_size: vec2<f32>, alive: u32, generation: u32, serial: u32, birth: u32,
}
struct EngineEvent { position: vec3<f32>, emitter_target: u32, generation: u32, seed: u32, pad0: u32, pad1: u32, }
struct EngineEvents { count: atomic<u32>, dropped: atomic<u32>, pad0: u32, pad1: u32, items: array<EngineEvent>, }
struct EngineReadEvents { count: u32, dropped: u32, pad0: u32, pad1: u32, items: array<EngineEvent>, }
struct EngineScratch {
    live: atomic<u32>, free: atomic<u32>, incoming: atomic<u32>, spawned: atomic<u32>,
    dropped: atomic<u32>, collisions: atomic<u32>, died: atomic<u32>, invalid: atomic<u32>,
    vertex_count: u32, instance_count: u32, first_vertex: u32, first_instance: u32,
    free_slots: array<u32,ENGINE_CAPACITY>, live_slots: array<u32,ENGINE_CAPACITY>,
    incoming_events: array<EngineEvent,ENGINE_CAPACITY>,
}
struct EngineParameters { effect: EffectParams, inputs: ParticleInputs, }
struct EngineStep {
    time: f32, dt: f32, requested: u32, serial_base: u32,
    seed: u32, emitter_index: u32, event_limit: u32, pad: u32,
    origin: vec4<f32>,
}
struct CollisionResult { particle: Particle, hit: bool, }
@group(0) @binding(0) var<storage,read_write> records: array<EngineRecord>;
@group(0) @binding(1) var<storage,read_write> scratch: EngineScratch;
@group(0) @binding(2) var<storage,read> previous_events: EngineReadEvents;
@group(0) @binding(3) var<storage,read_write> next_events: EngineEvents;
@group(0) @binding(4) var<storage,read> parameters: EngineParameters;
@group(0) @binding(5) var<uniform> step: EngineStep;

fn engine_hash(value:u32)->u32 { var x=value; x=(x^(x>>16u))*2146121005u; x=(x^(x>>15u))*2221713035u; return x^(x>>16u); }
fn engine_random(seed:u32,salt:u32)->f32 { return f32(engine_hash(seed^salt)>>8u)/16777216.0; }
fn engine_random3(seed:u32)->vec3<f32> { return vec3<f32>(engine_random(seed,2u),engine_random(seed,3u),engine_random(seed,4u)); }
fn engine_direction(seed:u32)->vec3<f32> { let z=engine_random(seed,5u)*2.0-1.0;let a=engine_random(seed,6u)*6.2831853;let r=sqrt(max(0.0,1.0-z*z));return vec3<f32>(r*cos(a),z,r*sin(a)); }
fn engine_finite(p:Particle)->bool {
    return all(abs(p.position)<=vec3<f32>(3.4e38)) && all(abs(p.velocity)<=vec3<f32>(3.4e38))
        && all(abs(p.color)<=vec4<f32>(3.4e38)) && all(abs(p.size)<=vec2<f32>(3.4e38))
        && abs(p.age)<=3.4e38 && abs(p.lifetime)<=3.4e38 && abs(p.rotation)<=3.4e38 && engine_custom_finite(p);
}
fn engine_sat_add(a:u32,b:u32)->u32 { return a+min(b,0xffffffffu-a); }
fn engine_drop_events(count:u32) {
    loop { let old=atomicLoad(&next_events.dropped);if(atomicCompareExchangeWeak(&next_events.dropped,old,engine_sat_add(old,count)).exchanged){break;} }
}
fn engine_emit(p:Particle,generation:u32,emitter_target:u32,count:u32) {
    var start=0u;var accepted=0u;
    loop {
        let old=atomicLoad(&next_events.count); accepted=min(count,step.event_limit-min(old,step.event_limit));
        if(accepted==0u){engine_drop_events(count);return;}
        if(atomicCompareExchangeWeak(&next_events.count,old,old+accepted).exchanged){start=old;break;}
    }
    engine_drop_events(count-accepted);
    var position=p.position;if(!ENGINE_WORLD){position+=step.origin.xyz;}
    for(var i=0u;i<accepted;i+=1u){next_events.items[start+i]=EngineEvent(position,emitter_target,generation,engine_hash(p.seed^i),0u,0u);}
}
@compute @workgroup_size(64)
fn engine_update(@builtin(global_invocation_id) invocation:vec3<u32>) {
    let id=invocation.x;if(id>=ENGINE_CAPACITY){return;}
    var record=records[id];
    if(record.alive!=0u){
        var p=record.particle;p.age+=step.dt;p.emit_children=0u;
        p=engine_standard(p,record.base_color,record.base_size);
        p=engine_particle_update(p,ParticleContext(step.time,step.dt,record.serial,p.seed),engine_update_params(parameters.effect),parameters.inputs);
        if(!engine_finite(p)){record.alive=0u;atomicAdd(&scratch.invalid,1u);}else{
            let collision=engine_collide(p);p=collision.particle;
            if(!engine_finite(p)){record.alive=0u;atomicAdd(&scratch.invalid,1u);}else{
            if(collision.hit){atomicAdd(&scratch.collisions,1u);engine_events(p,record.generation,3u);}
            engine_events(p,record.generation,0u);
            if(p.age>=p.lifetime){record.alive=0u;atomicAdd(&scratch.died,1u);engine_events(p,record.generation,2u);}
            }
        }
        record.particle=p;records[id]=record;
    }
    if(record.alive!=0u){let index=atomicAdd(&scratch.live,1u);scratch.live_slots[index]=id;}
    else{let index=atomicAdd(&scratch.free,1u);scratch.free_slots[index]=id;}
}
@compute @workgroup_size(64)
fn engine_route(@builtin(global_invocation_id) invocation:vec3<u32>) {
    let id=invocation.x;if(id>=min(previous_events.count,step.event_limit)){return;}
    let event=previous_events.items[id];if(event.emitter_target!=step.emitter_index){return;}
    let index=atomicAdd(&scratch.incoming,1u);if(index<ENGINE_CAPACITY){scratch.incoming_events[index]=event;}
}
@compute @workgroup_size(64)
fn engine_spawn(@builtin(global_invocation_id) invocation:vec3<u32>) {
    let rank=invocation.x;let total=engine_sat_add(step.requested,atomicLoad(&scratch.incoming));
    if(rank>=min(atomicLoad(&scratch.free),total)){return;}
    let id=scratch.free_slots[rank];var origin=vec3<f32>(0.0);if(ENGINE_WORLD){origin=step.origin.xyz;}
    var generation=0u;var serial=step.serial_base+rank;
    if(rank>=step.requested){let event=scratch.incoming_events[rank-step.requested];origin=event.position;if(!ENGINE_WORLD){origin-=step.origin.xyz;}generation=event.generation;serial=event.seed;}
    let p=engine_initial(serial,origin);
    if(!engine_finite(p)){atomicAdd(&scratch.invalid,1u);return;}
    var record:EngineRecord;record.particle=p;record.base_color=p.color;record.base_size=p.size;
    record.birth=records[id].birth+1u;
    record.generation=generation;record.serial=serial;
    if(p.age<p.lifetime){record.alive=1u;let index=atomicAdd(&scratch.live,1u);scratch.live_slots[index]=id;atomicAdd(&scratch.spawned,1u);engine_events(p,generation,1u);engine_events(p,generation,0u);}
    records[id]=record;
}
@compute @workgroup_size(1)
fn engine_finish() {
    let total=engine_sat_add(step.requested,atomicLoad(&scratch.incoming));atomicStore(&scratch.dropped,total-min(total,atomicLoad(&scratch.free)));
    scratch.vertex_count=6u;scratch.instance_count=atomicLoad(&scratch.live);scratch.first_vertex=0u;scratch.first_instance=0u;
}
