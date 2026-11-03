//! Native depth comparison bindings; allocated and retired by the existing completion owner.
use super::*;
use crate::render_geometry::ShadowMaps;
#[cfg_attr(test, derive(Clone))]
pub(super) struct ShadowBinding {
    pub buffer: wgpu::Buffer,
    pub binding: wgpu::BindGroup,
    pub depths: Vec<wgpu::TextureView>,
    words: Vec<u32>,
}
impl super::super::memory::RetiredGpuResource for ShadowBinding {
    fn observe<'a>(
        &'a self,
        gpu: &mut super::super::memory::GpuMemoryCensus<'a>,
        sample: &mut crate::resource_memory::ResourceMemorySample,
    ) {
        if !gpu.is_backing_only() {
            sample.cpu_vec(&self.depths, "renderer", &["retiring"]);
            sample.cpu_vec(&self.words, "renderer", &["retiring"]);
        }
        gpu.buffer(&self.buffer, "renderer", &["retiring"]);
        for view in &self.depths {
            gpu.texture(view.texture(), &["retiring"]);
        }
    }
}
pub(super) fn validate_plan(plan: &RhiCommandPlan) -> Result<(), String> {
    let mut checked = BTreeMap::new();
    for (index, command) in plan.commands.iter().enumerate() {
        let Some(draw) = geometry_draw(command) else {
            continue;
        };
        let Some(maps) = &draw.shadows else {
            continue;
        };
        if let Some(previous) = checked.insert(draw.view.view_id, maps) {
            if previous != maps {
                return Err("shadow.inconsistent_view_binding".into());
            }
            continue;
        }
        let schedule = plan
            .graph_schedule
            .as_ref()
            .ok_or("shadow.schedule_missing")?;
        for p in maps.projections() {
            let resource = schedule
                .resources
                .iter()
                .find(|r| r.resource_id == p.atlas)
                .ok_or("shadow.resource_missing")?;
            let shape = schedule
                .resource_shapes
                .get(&crate::render_graph::GraphResourceId(p.atlas.clone()))
                .cloned()
                .unwrap_or_default();
            if resource.format != crate::render_graph::RenderResourceFormat::Depth32Float
                || !resource
                    .usage
                    .contains(&crate::render_graph::RenderResourceUsage::Sampled)
                || [resource.size.width, resource.size.height] != p.extent
                || shape.samples != 1
                || shape.mip_levels != 1
                || shape.layers != 1
            {
                return Err("shadow.sample_descriptor".into());
            }
            let mut found = false;
            for (i, c) in plan.commands.iter().enumerate() {
                match c {
                    RhiCommand::BeginView {
                        target,
                        view,
                        depth_target,
                    } if depth_target.as_deref() == Some(&p.atlas) => {
                        if i >= index {
                            return Err("shadow.writer_after_receiver".into());
                        }
                        if target == &p.atlas && p.matches_writer(view) {
                            found = true;
                        }
                    }
                    _ => {
                        if geometry_draw(c)
                            .is_some_and(|d| d.depth_target.as_deref() == Some(&p.atlas))
                            && i >= index
                        {
                            return Err("shadow.writer_after_receiver".into());
                        }
                    }
                }
            }
            if !found {
                return Err("shadow.writer_mismatch".into());
            }
        }
    }
    Ok(())
}
fn layout(device: &wgpu::Device, mask: u8) -> wgpu::BindGroupLayout {
    let mut entries = vec![wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(80),
        },
        count: None,
    }];
    for slot in 0..2 {
        if mask & (1 << slot) != 0 {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: slot + 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
        }
    }
    entries.push(wgpu::BindGroupLayoutEntry {
        binding: 3,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
        count: None,
    });
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("shadow-comparison-layout"),
        entries: &entries,
    })
}
impl RealWgpuBackend {
    pub(super) fn prepare_shadow_bindings(
        &mut self,
        frame_index: u64,
        active: BTreeMap<u64, &ShadowMaps>,
    ) -> Result<(), String> {
        // Surface readiness and draw both prepare mixed plans. Reuse only the
        // exact uploaded parameters and today's native depth views; retaining a
        // logical atlas name alone would hide replacement/reallocation.
        let unchanged: BTreeSet<_> = active
            .iter()
            .filter_map(|(id, maps)| {
                let old = self.geometry.shadow_bindings.get(id)?;
                let depths = (0..2)
                    .filter_map(|slot| maps.projections().iter().find(|p| p.slot == slot))
                    .map(|p| self.geometry.depths.get(&p.atlas).map(|d| &d.2));
                (old.words == maps.words() && depths.eq(old.depths.iter().map(Some))).then_some(*id)
            })
            .collect();
        let new_bytes: u64 = active
            .iter()
            .filter(|(id, _)| !unchanged.contains(id))
            .map(|(_, maps)| (maps.words().len() * 4) as u64)
            .sum();
        self.admit_geometry_backing(new_bytes)?;
        let removed: Vec<_> = self
            .geometry
            .shadow_bindings
            .keys()
            .filter(|id| !unchanged.contains(id))
            .copied()
            .collect();
        let bytes = removed
            .iter()
            .map(|id| self.geometry.shadow_bindings[id].buffer.size())
            .sum();
        self.admit_gpu_retirement(bytes, removed.len())?;
        self.geometry
            .admit_visual_upload(frame_index, new_bytes as usize)?;
        for id in removed {
            let old = self.geometry.shadow_bindings.remove(&id).unwrap();
            let bytes = old.buffer.size();
            self.retire_gpu(old, bytes);
        }
        for (id, maps) in active {
            if unchanged.contains(&id) {
                continue;
            }
            let mask = maps.texture_mask();
            let layout = self
                .geometry
                .shadow_layouts
                .entry(mask)
                .or_insert_with(|| layout(&self.device, mask));
            let words = maps.words();
            let buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("immutable-shadow-parameters"),
                    contents: bytemuck::cast_slice(&words),
                    usage: wgpu::BufferUsages::STORAGE,
                });
            let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("shadow-3x3-pcf"),
                compare: Some(wgpu::CompareFunction::LessEqual),
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });
            let mut entries = vec![wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }];
            let mut depths = Vec::new();
            for slot in 0..2 {
                if let Some(p) = maps.projections().iter().find(|p| p.slot == slot) {
                    let depth = &self
                        .geometry
                        .depths
                        .get(&p.atlas)
                        .ok_or("shadow.depth_missing")?
                        .2;
                    entries.push(wgpu::BindGroupEntry {
                        binding: slot + 1,
                        resource: wgpu::BindingResource::TextureView(depth),
                    });
                    depths.push(depth.clone());
                }
            }
            entries.push(wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&sampler),
            });
            let binding = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("immutable-shadow-comparison"),
                layout,
                entries: &entries,
            });
            self.geometry.shadow_bindings.insert(
                id,
                ShadowBinding {
                    buffer,
                    binding,
                    depths,
                    words,
                },
            );
        }
        Ok(())
    }
}
pub(super) fn shader(mask: u8) -> String {
    let mut s = String::from(
        r#"
@group(3) @binding(0) var<storage,read> shadow_words:array<u32>;
@group(3) @binding(3) var shadow_sampler:sampler_comparison;
fn shadow_float(i:u32)->f32 {return bitcast<f32>(shadow_words[i]);}
fn shadow_vec(i:u32)->vec4<f32> {return vec4<f32>(shadow_float(i),shadow_float(i+1u),shadow_float(i+2u),shadow_float(i+3u));}
"#,
    );
    for slot in 0..2 {
        if mask & (1 << slot) != 0 {
            s.push_str(&format!(
                "@group(3) @binding({}) var shadow_{slot}:texture_depth_2d;\n",
                slot + 1
            ));
        }
    }
    s.push_str(r#"
fn shadow_visibility(light_index:u32,p:vec3<f32>,normal:vec3<f32>)->f32 {
    let camera=mat4x4<f32>(shadow_vec(4u),shadow_vec(8u),shadow_vec(12u),shadow_vec(16u));
    let distance= -(camera*vec4<f32>(p,1.)).z;
    for(var i=0u;i<shadow_words[0u];i++) {
        let b=20u+i*24u;
        if(shadow_words[b+20u]!=light_index) {continue;}
        let slot=shadow_words[b+21u];
        if(slot==0u && (distance<shadow_float(b+22u) || distance>shadow_float(b+23u))) {continue;}
        let matrix=mat4x4<f32>(shadow_vec(b),shadow_vec(b+4u),shadow_vec(b+8u),shadow_vec(b+12u));
        let clip=matrix*vec4<f32>(p+normal*0.01,1.);
        if(clip.w<=0.) {return 1.;}
        let ndc=clip.xyz/clip.w;
        if(any(ndc.xy<vec2<f32>(-1.)) || any(ndc.xy>vec2<f32>(1.)) || ndc.z<0. || ndc.z>1.) {return 1.;}
        let tile=shadow_vec(b+16u); let uv=tile.xy+vec2<f32>(ndc.x*0.5+0.5,0.5-ndc.y*0.5)*tile.zw;
        var visibility=0.;
"#);
    for slot in 0..2 {
        if mask & (1 << slot) != 0 {
            s.push_str(&format!(r#"
        if(slot=={slot}u) {{
            let texel=1./vec2<f32>(textureDimensions(shadow_{slot}));
            let lo=tile.xy+texel*0.5;let hi=tile.xy+tile.zw-texel*0.5;
            for(var y=-1;y<=1;y++) {{for(var x=-1;x<=1;x++) {{
                visibility+=textureSampleCompareLevel(shadow_{slot},shadow_sampler,clamp(uv+vec2<f32>(f32(x),f32(y))*texel,lo,hi),ndc.z-0.0005);
            }}}}
        }}
"#));
        }
    }
    s.push_str("return visibility/9.; } return 1.; }\n");
    s
}
