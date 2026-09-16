@group(0) @binding(2) var<storage,read_write> draw_data: array<u32>;
@group(0) @binding(3) var<storage,read_write> history: array<u32>;

@compute @workgroup_size(64)
fn capture_history(@builtin(global_invocation_id) invocation:vec3<u32>) {
    let id=invocation.x;
    if(id>=CAPACITY || TRAIL_SEGMENTS==0u){return;}
    if(records[id*STRIDE+ALIVE]==0u){return;}
    let base=history_base(id);
    let birth=records[id*STRIDE+SERIAL+1u];
    if(history[base+3u]==0u || history[base]!=birth){
        history[base]=birth;history[base+1u]=0u;history[base+2u]=0u;history[base+3u]=1u;
    }
    let head=history[base+2u];
    let point=base+4u+head*4u;
    history[point]=records[id*STRIDE];history[point+1u]=records[id*STRIDE+1u];history[point+2u]=records[id*STRIDE+2u];
    history[point+3u]=bitcast<u32>(view.clock.x);
    history[base+1u]=min(history[base+1u]+1u,TRAIL_SEGMENTS);
    history[base+2u]=(head+1u)%max(TRAIL_SEGMENTS,1u);
}

@compute @workgroup_size(64)
fn prepare_draw(@builtin(global_invocation_id) invocation:vec3<u32>) {
    let rank=invocation.x;
    if(rank==0u){draw_data[0]=view.control.z+max(TRAIL_SEGMENTS,1u)*6u-6u;draw_data[1]=scratch[0];draw_data[2]=0u;draw_data[3]=0u;}
    if(rank>=SORT_SIZE){return;}
    var id=0xffffffffu;var depth=-3.4e38;
    if(rank<scratch[0]){id=scratch[12u+CAPACITY+rank];depth=dot(world(position(id)),view.forward.xyz);}
    draw_data[4u+rank*2u]=bitcast<u32>(depth);draw_data[5u+rank*2u]=id;
}

@compute @workgroup_size(64)
fn sort_draw(@builtin(global_invocation_id) invocation:vec3<u32>) {
    let a=invocation.x;let b=a^view.control.x;
    if(a>=SORT_SIZE || b<=a){return;}
    let ad=bitcast<f32>(draw_data[4u+a*2u]);let bd=bitcast<f32>(draw_data[4u+b*2u]);
    let ai=draw_data[5u+a*2u];let bi=draw_data[5u+b*2u];
    let a_before=(ad>bd || (ad==bd && ai<bi));
    let descending=(a & view.control.y)==0u;
    if(a_before!=descending){
        draw_data[4u+a*2u]=bitcast<u32>(bd);draw_data[5u+a*2u]=bi;
        draw_data[4u+b*2u]=bitcast<u32>(ad);draw_data[5u+b*2u]=ai;
    }
}

// A bounded emitter that fits one workgroup needs no inter-dispatch barrier.
// Keep the same depth/id ordering as the large-emitter bitonic path.
var<workgroup> small_depth: array<f32,64>;
var<workgroup> small_id: array<u32,64>;
@compute @workgroup_size(64)
fn sort_small_draw(@builtin(local_invocation_index) a:u32) {
    small_depth[a]=-3.4e38;small_id[a]=0xffffffffu;
    if(a<SORT_SIZE){small_depth[a]=bitcast<f32>(draw_data[4u+a*2u]);small_id[a]=draw_data[5u+a*2u];}
    workgroupBarrier();
    for(var k=2u;k<=SORT_SIZE;k*=2u){
        for(var j=k/2u;j>0u;j/=2u){
            let b=a^j;
            if(a<SORT_SIZE && b>a){
                let ad=small_depth[a];let bd=small_depth[b];let ai=small_id[a];let bi=small_id[b];
                let a_before=(ad>bd || (ad==bd && ai<bi));
                if(a_before!=((a & k)==0u)){
                    small_depth[a]=bd;small_id[a]=bi;small_depth[b]=ad;small_id[b]=ai;
                }
            }
            workgroupBarrier();
        }
    }
    if(a<SORT_SIZE){draw_data[4u+a*2u]=bitcast<u32>(small_depth[a]);draw_data[5u+a*2u]=small_id[a];}
}
