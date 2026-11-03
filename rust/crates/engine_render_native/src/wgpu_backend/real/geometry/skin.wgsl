// Immutable mesh-space morph -> weighted skin. Both pose outputs belong to this snapshot.
@group(0) @binding(0) var<storage,read> base:array<u32>;
@group(0) @binding(1) var<storage,read> influence:array<u32>;
@group(0) @binding(2) var<storage,read> data:array<u32>;
@group(0) @binding(3) var<storage,read_write> current:array<u32>;
@group(0) @binding(4) var<storage,read_write> previous:array<u32>;
fn f(i:u32)->f32 {return bitcast<f32>(data[i]);}
fn v(i:u32)->vec3<f32> {return vec3<f32>(f(i),f(i+1u),f(i+2u));}
fn matrix(i:u32)->mat4x4<f32> {
    return mat4x4<f32>(vec4<f32>(f(i),f(i+1u),f(i+2u),f(i+3u)),vec4<f32>(f(i+4u),f(i+5u),f(i+6u),f(i+7u)),vec4<f32>(f(i+8u),f(i+9u),f(i+10u),f(i+11u)),vec4<f32>(f(i+12u),f(i+13u),f(i+14u),f(i+15u)));
}
fn normal_transform(m:mat4x4<f32>,n:vec3<f32>)->vec3<f32> {
    let a=m[0].xyz;let b=m[1].xyz;let c=m[2].xyz;
    return (cross(b,c)*n.x+cross(c,a)*n.y+cross(a,b)*n.z)/dot(a,cross(b,c));
}
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id:vec3<u32>) {
    let index=id.x;let count=data[0];if index>=count {return;}
    let stride=data[1];let po=data[2];let no=data[3];let to=data[4];let joints=data[5];let morphs=data[6];let start=index*stride;
    for(var i=0u;i<stride;i+=1u) {current[start+i]=base[start+i];previous[start+i]=base[start+i];}
    for(var frame=0u;frame<2u;frame+=1u) {
        var p=vec3<f32>(bitcast<f32>(base[start+po]),bitcast<f32>(base[start+po+1u]),bitcast<f32>(base[start+po+2u]));
        var n=vec3<f32>(0.,0.,1.);if no!=0xffffffffu {n=vec3<f32>(bitcast<f32>(base[start+no]),bitcast<f32>(base[start+no+1u]),bitcast<f32>(base[start+no+2u]));}
        for(var morph_index=0u;morph_index<morphs;morph_index+=1u) {
            let w=f(8u+joints*32u+frame*morphs+morph_index);let delta=8u+joints*32u+morphs*2u+(morph_index*count+index)*6u;
            p+=v(delta)*w;n+=v(delta+3u)*w;
        }
        var op=vec3<f32>(0.);var on=vec3<f32>(0.);var ot=vec3<f32>(0.);
        for(var influence_index=0u;influence_index<4u;influence_index+=1u) {
            let j=influence[index*8u+influence_index];let w=bitcast<f32>(influence[index*8u+4u+influence_index]);if w==0. {continue;}
            let m=matrix(8u+frame*joints*16u+j*16u);op+=(m*vec4<f32>(p,1.)).xyz*w;on+=normal_transform(m,n)*w;
            if to!=0xffffffffu {let tangent=vec3<f32>(bitcast<f32>(base[start+to]),bitcast<f32>(base[start+to+1u]),bitcast<f32>(base[start+to+2u]));ot+=(m*vec4<f32>(tangent,0.)).xyz*w;}
        }
        on=normalize(on);if to!=0xffffffffu {ot=normalize(ot-on*dot(on,ot));}
        for(var axis=0u;axis<3u;axis+=1u) {
            if frame==0u {current[start+po+axis]=bitcast<u32>(op[axis]);if no!=0xffffffffu {current[start+no+axis]=bitcast<u32>(on[axis]);}if to!=0xffffffffu {current[start+to+axis]=bitcast<u32>(ot[axis]);}}
            else {previous[start+po+axis]=bitcast<u32>(op[axis]);if no!=0xffffffffu {previous[start+no+axis]=bitcast<u32>(on[axis]);}if to!=0xffffffffu {previous[start+to+axis]=bitcast<u32>(ot[axis]);}}
        }
    }
}
