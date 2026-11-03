//! Immutable native clustered light buffers share the existing completion retirement owner.
use super::*;
pub(super) struct LightBuffer {
    pub buffer: wgpu::Buffer,
    pub binding: wgpu::BindGroup,
}
impl super::super::memory::RetiredGpuResource for LightBuffer {
    fn observe<'a>(
        &'a self,
        gpu: &mut super::super::memory::GpuMemoryCensus<'a>,
        _: &mut crate::resource_memory::ResourceMemorySample,
    ) {
        gpu.buffer(&self.buffer, "renderer", &["retiring"]);
    }
}
pub(super) fn layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("clustered-light-list"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(80),
            },
            count: None,
        }],
    })
}
impl RealWgpuBackend {
    pub(super) fn prepare_scene_lights(
        &mut self,
        frame_index: u64,
        active: BTreeMap<&str, &crate::render_geometry::ClusteredLights>,
    ) -> Result<(), String> {
        if active.len() > 8 {
            return Err("light.view_capacity".into());
        }
        let old: Vec<_> = self
            .geometry
            .light_buffers
            .keys()
            .filter(|k| !active.contains_key(k.as_str()))
            .cloned()
            .collect();
        let retired_bytes = old
            .iter()
            .map(|k| self.geometry.light_buffers[k].buffer.size())
            .sum();
        let new_bytes: u64 = active
            .iter()
            .filter(|(key, _)| !self.geometry.light_buffers.contains_key(**key))
            .map(|(_, v)| (v.words().len() * 4) as u64)
            .sum();
        if new_bytes > 1024 * 1024 {
            return Err("light.upload_capacity".into());
        }
        if active.values().any(|v| {
            (v.words().len() * 4) as u64
                > u64::from(self.device.limits().max_storage_buffer_binding_size)
        }) {
            return Err("light.storage_capacity".into());
        }
        self.admit_geometry_backing(new_bytes)?;
        self.admit_gpu_retirement(retired_bytes, old.len())?;
        self.geometry
            .admit_visual_upload(frame_index, new_bytes as usize)?;
        for key in old {
            let old = self.geometry.light_buffers.remove(&key).unwrap();
            let bytes = old.buffer.size();
            self.retire_gpu(old, bytes);
        }
        for (key, light) in active {
            if !self.geometry.light_buffers.contains_key(key) {
                let bytes = bytemuck::cast_slice(light.words());
                if bytes.len() as u64
                    > u64::from(self.device.limits().max_storage_buffer_binding_size)
                {
                    return Err("light.storage_capacity".into());
                }
                let buffer = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("immutable-view-lights"),
                        contents: bytes,
                        usage: wgpu::BufferUsages::STORAGE,
                    });
                let binding = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("immutable-view-lights"),
                    layout: &self.geometry.light_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer.as_entire_binding(),
                    }],
                });
                self.geometry
                    .light_buffers
                    .insert(key.to_owned(), LightBuffer { buffer, binding });
            }
        }
        Ok(())
    }
}
pub(super) const SHADER: &str = r#"
@group(2) @binding(0) var<storage,read> scene_lights:array<u32>;
fn light_float(i:u32)->f32{return bitcast<f32>(scene_lights[i]);}
fn light_vec(i:u32)->vec3<f32>{return vec3<f32>(light_float(i),light_float(i+1u),light_float(i+2u));}
fn scene_direct(p:vec3<f32>,n:vec3<f32>,v:vec3<f32>,base:vec3<f32>,metallic:f32,roughness:f32,geometric_normal:vec3<f32>)->vec3<f32>{
    let vp=mat4x4<f32>(vec4<f32>(light_vec(0u),light_float(3u)),vec4<f32>(light_vec(4u),light_float(7u)),vec4<f32>(light_vec(8u),light_float(11u)),vec4<f32>(light_vec(12u),light_float(15u)));
    let clip=vp*vec4<f32>(p,1.);let ndc=clip.xyz/clip.w;
    let cell=vec3<u32>(clamp(vec3<f32>((ndc.x+1.)*0.5,(1.-ndc.y)*0.5,ndc.z)*vec3<f32>(16.,9.,24.),vec3<f32>(0.),vec3<f32>(15.,8.,23.)));
    let header=20u+scene_lights[16u]*16u+(cell.z*144u+cell.y*16u+cell.x)*8u;

    var result=vec3<f32>(0.);
    for(var word=0u;word<8u;word++){
      var bits=scene_lights[header+word];
      while(bits!=0u){
        let index=word*32u+firstTrailingBit(bits);bits=bits & (bits-1u);
        let b=20u+index*16u;let kind=light_float(b+3u);let direction=light_vec(b+4u);
        var l= -direction;var attenuation=1.;
        if(kind>0.5){
            let delta=light_vec(b)-p;let d2=dot(delta,delta);let distance=sqrt(d2);l=delta*inverseSqrt(max(d2,1e-12));
            let ratio=distance/light_float(b+7u);let window=max(1.-ratio*ratio*ratio*ratio,0.);attenuation=window*window/max(d2,0.01);
            if(kind>1.5){let cone=dot(direction,-l);attenuation*=smoothstep(light_float(b+12u),light_float(b+11u),cone);}
        }
        // Material samples/derivatives precede this loop; shadow sampling uses
        // explicit LOD. Neither zero attenuation nor N.L<=0 contributes to BRDF.
        if(attenuation<=0. || dot(n,l)<=0.){continue;}
        result+=evaluate_direct(n,v,l,base,metallic,roughness)*light_vec(b+8u)*attenuation;
    }
    }
    return result;
}
"#;
