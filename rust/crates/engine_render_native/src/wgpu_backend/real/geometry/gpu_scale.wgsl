struct DrawArgs {index_count:u32, instance_count:u32, first_index:u32, base_vertex:u32, first_instance:u32};
@group(0) @binding(0) var<storage,read> seeds:array<DrawArgs>;
@group(0) @binding(1) var<storage,read> hits:array<vec2<u32>>;
@group(0) @binding(2) var<storage,read_write> arguments:array<DrawArgs>;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id:vec3<u32>){
    let index=id.x;
    if(index>=arrayLength(&seeds)){return;}
    var result=seeds[index];
    if(all(hits[index]==vec2<u32>(0u))){result.instance_count=0u;}
    arguments[index]=result;
}
