//! Cubemap IBL backing uses the original geometry binding/completion owners.
use super::*;
pub(super) struct EnvironmentBuffer {
    pub buffer: wgpu::Buffer,
}
impl super::super::memory::RetiredGpuResource for EnvironmentBuffer {
    fn observe<'a>(
        &'a self,
        gpu: &mut super::super::memory::GpuMemoryCensus<'a>,
        _: &mut crate::resource_memory::ResourceMemorySample,
    ) {
        gpu.buffer(&self.buffer, "renderer", &["retiring"]);
    }
}
impl RealWgpuBackend {
    pub(super) fn prepare_environments(
        &mut self,
        frame_index: u64,
        active: BTreeMap<&str, &crate::environment_lighting::EnvironmentMap>,
    ) -> Result<(), String> {
        if active.len() > 8 {
            return Err("environment.active_capacity".into());
        }
        let old: Vec<_> = self
            .geometry
            .environment_buffers
            .keys()
            .filter(|k| !active.contains_key(k.as_str()))
            .cloned()
            .collect();
        let retired: u64 = old
            .iter()
            .map(|k| self.geometry.environment_buffers[k].buffer.size())
            .sum();
        let bytes: usize = active
            .iter()
            .filter(|(k, _)| !self.geometry.environment_buffers.contains_key(**k))
            .map(|(_, m)| m.words().len() * 4)
            .sum();
        let extra = bytes as u64;
        if active.values().any(|m| {
            m.words().len() * 4 > self.device.limits().max_storage_buffer_binding_size as usize
        }) {
            return Err("environment.storage_capacity".into());
        }
        self.admit_geometry_backing(extra)?;
        let existing = self.geometry_residency().physical_mesh_bytes
            + self.textures.values().map(|t| t.byte_len()).sum::<u64>()
            + self
                .geometry
                .environment_buffers
                .values()
                .map(|e| e.buffer.size())
                .sum::<u64>()
            + self.gpu_lifecycle_stats().retiring_bytes;
        self.admit_gpu_retirement(retired, old.len())?;
        self.geometry.streaming.admit_environment_upload(
            frame_index,
            bytes,
            existing.saturating_add(extra),
        )?;
        for key in old {
            let b = self.geometry.environment_buffers.remove(&key).unwrap();
            let n = b.buffer.size();
            self.retire_gpu(b, n);
        }
        for (key, map) in active {
            self.geometry
                .environment_buffers
                .entry(key.to_owned())
                .or_insert_with(|| {
                    let buffer =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("immutable-cubemap-ibl"),
                                contents: bytemuck::cast_slice(map.words()),
                                usage: wgpu::BufferUsages::STORAGE,
                            });
                    EnvironmentBuffer { buffer }
                });
        }
        Ok(())
    }
}
pub(super) const SHADER: &str = r#"
@group(1) @binding(10) var<storage,read> environment:array<u32>;
fn env_f(i:u32)->f32{return bitcast<f32>(environment[i]);}
fn env_rgb(i:u32)->vec3<f32>{return vec3<f32>(env_f(i),env_f(i+1u),env_f(i+2u));}
fn env_direction(face:u32,uv:vec2<f32>)->vec3<f32>{
    switch face {case 0u:{return vec3<f32>(1.,-uv.y,-uv.x);}case 1u:{return vec3<f32>(-1.,-uv.y,uv.x);}case 2u:{return vec3<f32>(uv.x,1.,uv.y);}case 3u:{return vec3<f32>(uv.x,-1.,-uv.y);}case 4u:{return vec3<f32>(uv.x,-uv.y,1.);}default:{return vec3<f32>(-uv.x,-uv.y,-1.);}}
}
fn env_face_uv(d:vec3<f32>)->vec3<f32>{
    let a=abs(d);
    if a.x>=a.y && a.x>=a.z {if d.x>=0. {return vec3<f32>(0.,-d.z/a.x,-d.y/a.x);}return vec3<f32>(1.,d.z/a.x,-d.y/a.x);}
    if a.y>=a.z {if d.y>=0. {return vec3<f32>(2.,d.x/a.y,d.z/a.y);}return vec3<f32>(3.,d.x/a.y,-d.z/a.y);}
    if d.z>=0. {return vec3<f32>(4.,d.x/a.z,-d.y/a.z);}return vec3<f32>(5.,-d.x/a.z,-d.y/a.z);
}
fn env_texel(offset:u32,size:u32,face:u32,p:vec2<i32>)->vec3<f32>{
    var f=face;var q=p;
    if any(q<vec2<i32>(0)) || any(q>=vec2<i32>(i32(size))) {
        let uv=(vec2<f32>(q)+vec2<f32>(0.5))/f32(size)*2.-vec2<f32>(1.);
        let remap=env_face_uv(env_direction(face,uv));f=u32(remap.x);
        q=clamp(vec2<i32>(floor((remap.yz+vec2<f32>(1.))*0.5*f32(size))),vec2<i32>(0),vec2<i32>(i32(size)-1));
    }
    return env_rgb(offset+(f*size*size+u32(q.y)*size+u32(q.x))*3u);
}
fn env_level(d:vec3<f32>,level:u32)->vec3<f32>{
    var size=environment[1u];var offset=32u;
    for(var l=0u;l<level;l++){offset+=6u*size*size*3u;size=size>>1u;}
    let face=env_face_uv(d);let p=(face.yz+vec2<f32>(1.))*0.5*f32(size)-vec2<f32>(0.5);let q=vec2<i32>(floor(p));let t=fract(p);
    return mix(mix(env_texel(offset,size,u32(face.x),q),env_texel(offset,size,u32(face.x),q+vec2<i32>(1,0)),t.x),mix(env_texel(offset,size,u32(face.x),q+vec2<i32>(0,1)),env_texel(offset,size,u32(face.x),q+vec2<i32>(1,1)),t.x),t.y);
}
fn env_lut_texel(offset:u32,q:vec2<i32>)->vec2<f32>{let p=clamp(q,vec2<i32>(0),vec2<i32>(31));let i=offset+u32(p.y*32+p.x)*2u;return vec2<f32>(env_f(i),env_f(i+1u));}
fn env_lut(nv:f32,rough:f32)->vec2<f32>{
    var size=environment[1u];var offset=32u;while(size>0u){offset+=6u*size*size*3u;size=size>>1u;}
    let p=vec2<f32>(nv,rough)*32.-vec2<f32>(0.5);let q=vec2<i32>(floor(p));let t=fract(p);
    return mix(mix(env_lut_texel(offset,q),env_lut_texel(offset,q+vec2<i32>(1,0)),t.x),mix(env_lut_texel(offset,q+vec2<i32>(0,1)),env_lut_texel(offset,q+vec2<i32>(1,1)),t.x),t.y);
}
fn environment_indirect(n:vec3<f32>,v:vec3<f32>,base:vec3<f32>,metallic:f32,rough:f32)->vec3<f32>{
    let basis=array<f32,9>(0.2820947918,0.4886025119*n.y,0.4886025119*n.z,0.4886025119*n.x,1.0925484306*n.x*n.y,1.0925484306*n.y*n.z,0.3153915653*(3.*n.z*n.z-1.),1.0925484306*n.x*n.z,0.5462742153*(n.x*n.x-n.y*n.y));
    var diffuse=vec3<f32>(0.);for(var j=0u;j<9u;j++){diffuse+=env_rgb(4u+j*3u)*basis[j];}
    let f0=mix(vec3<f32>(0.04),base,metallic);let nv=clamp(dot(n,v),0.,1.);
    let f=f0+(max(vec3<f32>(1.-rough),f0)-f0)*pow(1.-nv,5.);
    let r=reflect(-v,n);let lod=rough*f32(environment[2u]-1u);let lo=u32(floor(lod));let hi=min(lo+1u,environment[2u]-1u);
    let specular=mix(env_level(r,lo),env_level(r,hi),fract(lod));let brdf=env_lut(nv,rough);
    return (vec3<f32>(1.)-f)*(1.-metallic)*base*max(diffuse,vec3<f32>(0.))+specular*(f0*brdf.x+vec3<f32>(brdf.y));
}
"#;
