//! PBR fixture for the affected material and retained-GPU tests.
use super::geometry_fixture::{triangle, unlit};
use engine_runtime::render_contract::*;

pub(crate) fn standard_lit(color: [f32; 4]) -> CookedMaterial {
    let mut m = unlit(color);
    m.preset = MaterialPreset::StandardLit;
    m.shader_source = STANDARD_LIT_SHADER_SOURCE.into();
    for (name, value) in [
        ("metallic", 0.),
        ("roughness", 0.5),
        ("normalScale", 1.),
        ("occlusionStrength", 1.),
    ] {
        m.parameters
            .insert(name.into(), MaterialParameter::Float(value));
    }
    m.parameters.insert(
        "emissive".into(),
        MaterialParameter::Vector4([1., 0., 0., 0.]),
    );
    m
}
pub(crate) fn lit_triangle() -> CookedGeometry {
    let mut g = triangle();
    g.vertex_bytes = g
        .vertex_bytes
        .chunks_exact(12)
        .flat_map(|v| {
            v.iter()
                .copied()
                .chain([0.0f32, 0., 1.].into_iter().flat_map(f32::to_le_bytes))
        })
        .collect();
    g.vertex_stride = 24;
    g.attributes.push(VertexAttribute {
        semantic: VertexSemantic::Normal,
        format: VertexFormat::Float32x3,
        offset: 12,
    });
    g
}
