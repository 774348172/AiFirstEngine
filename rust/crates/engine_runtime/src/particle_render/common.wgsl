struct RenderView {
    world_to_clip: mat4x4<f32>,
    right: vec4<f32>, up: vec4<f32>, forward: vec4<f32>, origin: vec4<f32>,
    control: vec4<u32>, clock: vec4<f32>, atlas: vec4<u32>,
    material_tint:vec4<f32>,
}
@group(0) @binding(0) var<storage,read> records: array<u32>;
@group(0) @binding(1) var<storage,read> scratch: array<u32>;
@group(0) @binding(4) var<uniform> view: RenderView;
fn number(id:u32,field:u32)->f32 { return bitcast<f32>(records[id*STRIDE+field]); }
fn position(id:u32)->vec3<f32> {return vec3<f32>(number(id,0u),number(id,1u),number(id,2u));}
fn world(p:vec3<f32>)->vec3<f32> {if(LOCAL_SPACE){return p+view.origin.xyz;}return p;}
fn history_base(id:u32)->u32 {return id*(4u+TRAIL_SEGMENTS*4u);}
