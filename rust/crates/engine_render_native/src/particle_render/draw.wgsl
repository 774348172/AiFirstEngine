@group(0) @binding(2) var<storage,read> draw_data: array<u32>;
@group(0) @binding(3) var<storage,read> history: array<u32>;
struct MeshVertex {position:vec4<f32>,uv:vec4<f32>,}
@group(0) @binding(5) var<storage,read> mesh:array<MeshVertex>;
@group(0) @binding(6) var particle_texture:texture_2d<f32>;
@group(0) @binding(7) var particle_sampler:sampler;
struct VertexOut {@builtin(position) clip:vec4<f32>,@location(0) color:vec4<f32>,@location(1) uv:vec2<f32>,@location(2) world_position:vec3<f32>,}
fn history_point(base:u32,index:u32)->vec4<f32> {
    let at=base+4u+index*4u;
    return vec4<f32>(bitcast<f32>(history[at]),bitcast<f32>(history[at+1u]),bitcast<f32>(history[at+2u]),bitcast<f32>(history[at+3u]));
}
@vertex
fn vertex_main(@builtin(vertex_index) vertex:u32,@builtin(instance_index) instance:u32)->VertexOut {
    let id=draw_data[5u+instance*2u];
    let corners=array<vec2<f32>,6>(vec2<f32>(-0.5,-0.5),vec2<f32>(0.5,-0.5),vec2<f32>(0.5,0.5),vec2<f32>(-0.5,-0.5),vec2<f32>(0.5,0.5),vec2<f32>(-0.5,0.5));
    var out:VertexOut;
    out.color=vec4<f32>(number(id,8u),number(id,9u),number(id,10u),number(id,11u));
    var p=world(position(id));var uv=vec2<f32>(0.0);
    let angle=number(id,14u);let c=cos(angle);let s=sin(angle);
    if(vertex<view.control.z){
        var offset=vec3<f32>(0.0);
        if(GEOMETRY==2u){offset=mesh[vertex].position.xyz;uv=mesh[vertex].uv.xy;}
        else {offset=vec3<f32>(corners[vertex],0.0);uv=vec2<f32>(offset.x+0.5,0.5-offset.y);}
        offset*=vec3<f32>(number(id,12u),number(id,13u),number(id,12u));
        offset=vec3<f32>(c*offset.x-s*offset.y,s*offset.x+c*offset.y,offset.z);
        let basis=geometry_basis(id);
        if(GEOMETRY==1u){let mirror=select(1.0,-1.0,determinant(basis)<0.0);p+=view.right.xyz*offset.x*length(basis[0])*mirror+view.up.xyz*offset.y*length(basis[1]);}else{p+=basis*offset;}
        if(FLIPBOOK){
            let frame=u32(max(0.0,floor(number(id,3u)*FLIP_FPS)))%(view.atlas.x*view.atlas.y);
            uv=(uv+vec2<f32>(f32(frame%view.atlas.x),f32(frame/view.atlas.x)))/vec2<f32>(view.atlas.xy);
        }
    }else{
        let v=vertex-view.control.z;let segment=v/6u;let corner=corners[v%6u];let base=history_base(id);let count=history[base+1u];
        if(segment+1u>=count){out.clip=vec4<f32>(2.0,2.0,2.0,1.0);out.color=vec4<f32>(0.0);out.world_position=vec3<f32>(0.0);out.uv=vec2<f32>(0.0);return out;}
        let oldest=(history[base+2u]+TRAIL_SEGMENTS-count)%max(TRAIL_SEGMENTS,1u);
        let a=history_point(base,(oldest+segment)%max(TRAIL_SEGMENTS,1u));
        let b=history_point(base,(oldest+segment+1u)%max(TRAIL_SEGMENTS,1u));
        let point=mix(a,b,corner.x+0.5);let elapsed=max(view.clock.x-point.w,0.0);
        var side=cross(world(b.xyz)-world(a.xyz),view.forward.xyz);
        if(dot(side,side)<0.0000001){side=view.right.xyz;}else{side=normalize(side);}
        p=world(point.xyz)+side*corner.y*view.clock.z*length(geometry_basis(id)[0]);
        out.color.a*=clamp(1.0-elapsed/max(view.clock.y,0.000001),0.0,1.0);
        uv=vec2<f32>(corner.x+0.5,corner.y+0.5);
    }
    out.clip=view.world_to_clip*vec4<f32>(p,1.0);out.uv=uv;out.world_position=p;return out;
}
@fragment
fn fragment_main(in:VertexOut)->@location(0) vec4<f32> {
    var color=in.color*view.material_tint;
    if(view.control.w!=0u){color*=textureSample(particle_texture,particle_sampler,in.uv);}
    if(view.fog_color_density.w>0.0){
        let distance=max(length(in.world_position-view.camera_position.xyz)-view.fog_start.x,0.0);
        let transmission=exp(-view.fog_color_density.w*distance);
        color=vec4<f32>(mix(view.fog_color_density.xyz,color.rgb,transmission),color.a);
    }
    return color;
}
