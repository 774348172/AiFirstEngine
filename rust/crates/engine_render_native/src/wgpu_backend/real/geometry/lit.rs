//! StandardLit material evaluation; preview illumination is explicit and defaults to zero.
use super::*;
#[cfg(test)]
thread_local! {
    pub(super) static TEXTURE_KEY_COMPUTATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
pub(super) type TextureKey = ([Option<RenderResourceHandle>; 5], Option<String>);
pub(super) type BorrowedTextureKey<'a> = ([Option<RenderResourceHandle>; 5], Option<&'a str>);
#[cfg(test)]
fn borrowed_texture_key(draw: &DrawGeometry) -> ([Option<RenderResourceHandle>; 5], Option<&str>) {
    #[cfg(test)]
    TEXTURE_KEY_COMPUTATIONS.with(|n| n.set(n.get() + 1));
    (
        draw.material_textures()
            .map(|texture| texture.map(|handle| handle.raw())),
        draw.environment.as_ref().map(|e| e.map.identity()),
    )
}
#[cfg(test)]
pub(super) fn texture_key(draw: &DrawGeometry) -> TextureKey {
    let (textures, environment) = borrowed_texture_key(draw);
    (textures, environment.map(str::to_owned))
}
pub(super) fn layout(device: &wgpu::Device, mask: u8) -> wgpu::BindGroupLayout {
    let mut entries = Vec::new();
    for i in 0..5 {
        if mask & (1 << i) != 0 {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: i * 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: i * 2 + 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            });
        }
    }
    if mask & 32 != 0 {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 10,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(128),
            },
            count: None,
        });
    }
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("standard-lit-five-slot-layout"),
        entries: &entries,
    })
}
impl RealWgpuBackend {
    /// Diagnostic preview only. No scene light is implied. Defaults to all zero.
    /// Direction points from surface toward light; ambient is diffuse irradiance / PI.
    pub fn set_material_preview_illumination(
        &mut self,
        direction: [f32; 3],
        radiance: [f32; 3],
        ambient: [f32; 3],
    ) -> Result<(), String> {
        if direction
            .iter()
            .chain(&radiance)
            .chain(&ambient)
            .any(|v| !v.is_finite())
            || radiance.iter().chain(&ambient).any(|v| *v < 0.)
        {
            return Err("material.preview_illumination_invalid".into());
        }
        let length = direction.iter().map(|v| v * v).sum::<f32>().sqrt();
        if !length.is_finite() || length < 1e-6 {
            return Err("material.preview_direction_invalid".into());
        }
        self.invalidate_geometry_bundles()?;
        self.geometry.validated_plan = None;
        self.geometry.preview = [
            [
                direction[0] / length,
                direction[1] / length,
                direction[2] / length,
                0.,
            ],
            [radiance[0], radiance[1], radiance[2], 0.],
            [ambient[0], ambient[1], ambient[2], 0.],
        ];
        Ok(())
    }
    pub(super) fn prepare_lit_bindings(
        &mut self,
        required: BTreeSet<BorrowedTextureKey<'_>>,
    ) -> Result<(), String> {
        let active: BTreeSet<_> = required
            .into_iter()
            .map(|(textures, environment)| (textures, environment.map(str::to_owned)))
            .collect();
        let old: Vec<_> = self
            .geometry
            .lit_bindings
            .keys()
            .filter(|k| !active.contains(*k))
            .cloned()
            .collect();
        self.admit_gpu_retirement(0, old.len())?;
        for k in old {
            let old = self.geometry.lit_bindings.remove(&k).unwrap();
            self.retire_gpu(old, 0);
        }
        for key in active {
            let mask = key
                .0
                .iter()
                .enumerate()
                .fold(0, |m, (i, k)| m | (u8::from(k.is_some()) << i))
                | (u8::from(key.1.is_some()) << 5);
            let layout = self
                .geometry
                .lit_layouts
                .entry(mask)
                .or_insert_with(|| layout(&self.device, mask));
            if !self.geometry.lit_bindings.contains_key(&key) {
                let mut entries = Vec::new();
                for (i, h) in key.0.iter().enumerate() {
                    if let Some(h) = h {
                        let t = &self.textures[h];
                        entries.push(wgpu::BindGroupEntry {
                            binding: i as u32 * 2,
                            resource: wgpu::BindingResource::TextureView(&t._view),
                        });
                        entries.push(wgpu::BindGroupEntry {
                            binding: i as u32 * 2 + 1,
                            resource: wgpu::BindingResource::Sampler(&t._sampler),
                        });
                    }
                }
                if let Some(id) = &key.1 {
                    entries.push(wgpu::BindGroupEntry {
                        binding: 10,
                        resource: self.geometry.environment_buffers[id]
                            .buffer
                            .as_entire_binding(),
                    });
                }
                let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("standard-lit-material"),
                    layout,
                    entries: &entries,
                });
                self.geometry.lit_bindings.insert(key, group);
                self.geometry.stats.bind_groups_created += 1;
            }
        }
        Ok(())
    }
    pub(super) fn invalidate_lit_texture(
        &mut self,
        handle: &RenderResourceHandle,
    ) -> Result<(), String> {
        let keys: Vec<_> = self
            .geometry
            .lit_bindings
            .keys()
            .filter(|key| key.0.contains(&Some(*handle)))
            .cloned()
            .collect();
        self.admit_gpu_retirement(0, keys.len())?;
        for key in keys {
            let old = self.geometry.lit_bindings.remove(&key).unwrap();
            self.retire_gpu(old, 0);
        }
        if self
            .geometry
            .validated_plan
            .as_ref()
            .is_some_and(|p| p.textures.contains(handle))
        {
            self.geometry.validated_plan = None;
        }
        Ok(())
    }
}

pub(super) const UNIFORMS: &str = parameters::WGSL;
pub(super) const DIRECT_BRDF: &str = r#"
fn evaluate_direct(n:vec3<f32>,v:vec3<f32>,l:vec3<f32>,base:vec3<f32>,metallic:f32,roughness:f32)->vec3<f32>{
    let hv=v+l;let h=hv*inverseSqrt(max(dot(hv,hv),1e-12));
    let nl=max(dot(n,l),0.);let nv=max(dot(n,v),1e-5);let nh=max(dot(n,h),0.);let vh=max(dot(v,h),0.);
    let a=roughness*roughness;let a2=a*a;let d0=nh*nh*(a2-1.)+1.;let distribution=a2/(3.14159265359*d0*d0);
    let g1l=2.*nl/(nl+sqrt(a2+(1.-a2)*nl*nl)+1e-6);let g1v=2.*nv/(nv+sqrt(a2+(1.-a2)*nv*nv)+1e-6);
    let f0=mix(vec3<f32>(0.04),base,metallic);let f=f0+(vec3<f32>(1.)-f0)*pow(1.-vh,5.);
    let specular=distribution*g1l*g1v*f/max(4.*nl*nv,1e-5);
    let diffuse=(vec3<f32>(1.)-f)*(1.-metallic)*base/3.14159265359;
    return (diffuse+specular)*nl;
}
"#;
pub(super) fn shader(draw: &DrawGeometry, has_color: bool, has_uv: bool) -> String {
    let mask = draw.texture_mask();
    let mut s = String::from(UNIFORMS);
    s.push_str(DIRECT_BRDF);
    if draw.environment.is_some() {
        s.push_str(super::ibl::SHADER);
    }
    s.push_str(super::environment::FOG_SHADER);
    if draw.lighting.is_some() {
        let mut direct = super::lights::SHADER.to_owned();
        if let Some(shadows) = &draw.shadows {
            s.push_str(&super::shadows::shader(shadows.texture_mask()));
            direct = direct.replace(
                "*attenuation;",
                "*attenuation*shadow_visibility(index,p,geometric_normal);",
            );
        }
        s.push_str(&direct);
    }
    s.push_str("struct Out { @builtin(position) position: vec4<f32>, @location(0) color: vec4<f32>, @location(1) uv: vec2<f32>, @location(2) world: vec3<f32>, @location(3) normal: vec3<f32>, @location(4) @interpolate(flat) slot: vec3<u32> };\n");
    for i in 0..5 {
        if mask & (1 << i) != 0 {
            s.push_str(&format!("@group(1) @binding({}) var t{i}: texture_2d<f32>; @group(1) @binding({}) var s{i}: sampler;\n", i*2, i*2+1));
        }
    }
    s.push_str("@vertex fn vs_main(@location(0) position: vec3<f32>, @location(3) slot: vec3<u32>, @location(4) normal: vec3<f32>");
    if has_color {
        s.push_str(", @location(1) color: vec4<f32>");
    }
    if has_uv {
        s.push_str(", @location(2) uv: vec2<f32>");
    }
    s.push_str(") -> Out { let u=load_parameters(slot); let world=u.world*vec4<f32>(position,1.); var o: Out; o.position=u.matrix*world; o.world=world.xyz; let a=u.world[0].xyz; let b=u.world[1].xyz; let c=u.world[2].xyz; let det=dot(a,cross(b,c)); o.normal=normalize((cross(b,c)*normal.x+cross(c,a)*normal.y+cross(a,b)*normal.z)/det); o.color=u.color");
    if has_color {
        s.push_str("*color");
    }
    s.push_str("; o.uv=");
    s.push_str(if has_uv { "uv" } else { "vec2<f32>(0.)" });
    s.push_str("; o.slot=slot; return o; }\n");
    s.push_str(
        r#"
fn fresnel(f0: vec3<f32>, vh: f32) -> vec3<f32> { return f0+(vec3<f32>(1.)-f0)*pow(1.-vh,5.); }
@fragment fn fs_main(i: Out, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let u=load_parameters(i.slot);
    var base=i.color;
"#,
    );
    if mask & 1 != 0 {
        s.push_str("base *= textureSample(t0,s0,i.uv);\n");
    }
    // Derivatives and samples precede discard; avoid non-uniform derivative control flow.
    s.push_str("var n=normalize(i.normal); if (!front) { n = -n; } let geometric_normal=n;\n");
    if mask & 2 != 0 {
        s.push_str(r#"
    let map = textureSample(t1,s1,i.uv).xyz*2.-vec3<f32>(1.);
    let px=dpdx(i.world); let py=dpdy(i.world); let ux=dpdx(i.uv); let uy=dpdy(i.uv);
    let det=ux.x*uy.y-ux.y*uy.x;
    let raw_t=(px*uy.y-py*ux.y)/det; let raw_b=(py*ux.x-px*uy.x)/det;
    let t=normalize(raw_t-n*dot(n,raw_t)); let handed=select(-1.,1.,dot(cross(n,t),raw_b)>=0.);
    let b=cross(n,t)*handed; let mapped=t*map.x*u.surface.z+b*map.y*u.surface.z+n*map.z; n=select(n,mapped*inverseSqrt(max(dot(mapped,mapped),1e-12)),dot(mapped,mapped)>1e-12);
"#);
    }
    s.push_str("var metallic=u.surface.x; var roughness=u.surface.y; var ao=1.; var emission=u.emissive.rgb;\n");
    if mask & 4 != 0 {
        s.push_str("let mr=textureSample(t2,s2,i.uv); metallic *= mr.b; roughness *= mr.g;\n");
    }
    if mask & 8 != 0 {
        s.push_str("ao=mix(1.,textureSample(t3,s3,i.uv).r,u.surface.w);\n");
    }
    if mask & 16 != 0 {
        s.push_str("emission *= textureSample(t4,s4,i.uv).rgb;\n");
    }
    if draw.blend == MaterialBlend::AlphaMask {
        s.push_str("if (base.a<u.extra.x) { discard; }\n");
    }
    s.push_str(r#"
    roughness=clamp(roughness,0.045,1.); metallic=clamp(metallic,0.,1.);
    let view_delta=u.camera.xyz-i.world; let v=select(view_delta*inverseSqrt(max(dot(view_delta,view_delta),1e-12)),u.extra.yzw,u.camera.w>0.5);
    let f0=mix(vec3<f32>(0.04),base.rgb,metallic);
"#);
    if draw.lighting.is_some() {
        s.push_str(
            "let direct=scene_direct(i.world,n,v,base.rgb,metallic,roughness,geometric_normal);\n",
        );
    } else {
        s.push_str("let direct=evaluate_direct(n,v,u.light.xyz,base.rgb,metallic,roughness)*u.radiance.rgb;\n");
    }
    s.push_str(
        r#"
    var indirect=(vec3<f32>(1.)-f0)*(1.-metallic)*base.rgb*u.ambient.rgb*ao;
    ENVIRONMENT_INDIRECT
    let lit_result=direct+indirect+emission;
    let result=distance_fog(lit_result,i.world,u.camera.xyz,u.fog,u.fog_settings);
"#,
    );
    s = s.replace(
        "ENVIRONMENT_INDIRECT",
        if draw.environment.is_some() {
            "indirect+=environment_indirect(n,v,base.rgb,metallic,roughness)*u.ambient.w*ao;"
        } else {
            ""
        },
    );
    if draw.view.color_pipeline.is_some() {
        s.push_str("let hdr_result=clamp(result,vec3<f32>(0.),vec3<f32>(65504.));\n");
    }
    let output = if draw.view.color_pipeline.is_some() {
        "hdr_result"
    } else {
        "result"
    };
    s.push_str(&format!("let output_color={output};\n"));
    s.push_str(if draw.opaque_alpha {
        "return vec4<f32>(output_color,1.); }"
    } else {
        "return vec4<f32>(output_color,base.a); }"
    });
    s
}
