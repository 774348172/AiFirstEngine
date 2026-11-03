struct RenderView {
    world_to_clip: mat4x4<f32>,
    right: vec4<f32>, up: vec4<f32>, forward: vec4<f32>, origin: vec4<f32>,
    control: vec4<u32>, clock: vec4<f32>, atlas: vec4<u32>,
    material_tint:vec4<f32>, simulation_to_view:vec4<f32>,
    camera_position:vec4<f32>, fog_color_density:vec4<f32>, fog_start:vec4<f32>,
    emitter_x:vec4<f32>, emitter_y:vec4<f32>, emitter_z:vec4<f32>,
}
@group(0) @binding(0) var<storage,read> records: array<u32>;
@group(0) @binding(1) var<storage,read> scratch: array<u32>;
@group(0) @binding(4) var<uniform> view: RenderView;
fn number(id:u32,field:u32)->f32 { return bitcast<f32>(records[id*STRIDE+field]); }
fn position(id:u32)->vec3<f32> {return vec3<f32>(number(id,0u),number(id,1u),number(id,2u));}
fn world(p:vec3<f32>)->vec3<f32> {if(LOCAL_SPACE){return mat3x3<f32>(view.emitter_x.xyz,view.emitter_y.xyz,view.emitter_z.xyz)*p+view.origin.xyz+view.simulation_to_view.xyz;}return p+view.simulation_to_view.xyz;}
fn birth_column(id:u32,c:u32)->vec3<f32>{let b=id*STRIDE+BIRTH_BASIS+c*4u;return vec3<f32>(bitcast<f32>(records[b]),bitcast<f32>(records[b+1u]),bitcast<f32>(records[b+2u]));}
fn geometry_basis(id:u32)->mat3x3<f32>{
    if(LOCAL_SPACE){return mat3x3<f32>(view.emitter_x.xyz,view.emitter_y.xyz,view.emitter_z.xyz);}
    if(AFFINE_RECORD){return mat3x3<f32>(birth_column(id,0u),birth_column(id,1u),birth_column(id,2u));}
    return mat3x3<f32>(vec3<f32>(1.0,0.0,0.0),vec3<f32>(0.0,1.0,0.0),vec3<f32>(0.0,0.0,1.0));
}
fn history_base(id:u32)->u32 {return id*(4u+TRAIL_SEGMENTS*4u);}
