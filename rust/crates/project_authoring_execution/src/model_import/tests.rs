use super::*;
use base64::Engine;
use engine_runtime::render_contract::{GeometryIndexFormat, VertexSemantic};
use serde_json::json;

struct Fixture {
    settings: ModelImportV1,
    json: Value,
    files: BTreeMap<String, Vec<u8>>,
}
impl Fixture {
    fn new() -> Self {
        let settings: ModelImportV1 = serde_json::from_slice(include_bytes!(
            "../../tests/fixtures/three_d_a0/import.json"
        ))
        .unwrap();
        let mut bytes = Vec::new();
        for v in [-1.0f32, 0., 0., 1., 0., 0., 0., 1., 0.] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        bytes.extend_from_slice(&[0, 1, 2, 0]);
        Self {
            settings,
            json: json!({"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0}],"meshes":[{"primitives":[{"attributes":{"POSITION":0},"indices":1}]}],"buffers":[{"uri":"data.bin","byteLength":40}],"bufferViews":[{"buffer":0,"byteLength":36},{"buffer":0,"byteOffset":36,"byteLength":3}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[-1,0,0],"max":[1,1,0]},{"bufferView":1,"componentType":5121,"count":3,"type":"SCALAR"}]}),
            files: BTreeMap::from([("Models/data.bin".into(), bytes)]),
        }
    }
    fn decode(&self) -> Result<DecodedModel> {
        let mut files = self.files.clone();
        files
            .entry(self.settings.source.clone())
            .or_insert_with(|| serde_json::to_vec(&self.json).unwrap());
        decode_model(
            &CompilerSourceView { files },
            "Assets/import.json",
            &self.settings,
        )
    }
    fn error(&self, code: &str, field: &str) {
        let error = self.decode().unwrap_err();
        assert!(error.error.code.ends_with(code), "{error:?}");
        assert!(error.error.field_path.contains(field), "{error:?}");
        assert_eq!(error.source_path, self.settings.source);
    }
    fn texture(&mut self, bytes: Vec<u8>, mime: &str) {
        self.json["materials"] = json!([{"extensions":{"KHR_materials_unlit":{}},"pbrMetallicRoughness":{"baseColorTexture":{"index":0}}}]);
        self.json["textures"] = json!([{"source":0}]);
        self.json["images"] = json!([{"uri":format!("data:{mime};base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes))}]);
        self.json["meshes"][0]["primitives"][0]["material"] = json!(0);
        // Independent float UV values, tightly packed in a separate buffer.
        let uv: Vec<u8> = [0.0f32, 0., 1., 0., 0., 1.]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        self.files.insert("Models/uv.bin".into(), uv);
        self.json["buffers"]
            .as_array_mut()
            .unwrap()
            .push(json!({"uri":"uv.bin","byteLength":24}));
        self.json["bufferViews"]
            .as_array_mut()
            .unwrap()
            .push(json!({"buffer":1,"byteLength":24}));
        self.json["accessors"]
            .as_array_mut()
            .unwrap()
            .push(json!({"bufferView":2,"componentType":5126,"count":3,"type":"VEC2"}));
        self.json["meshes"][0]["primitives"][0]["attributes"]["TEXCOORD_0"] = json!(2);
    }
    fn make_glb(&mut self) {
        self.settings.source = "Models/scene.glb".into();
        self.json["buffers"][0]
            .as_object_mut()
            .unwrap()
            .remove("uri");
        let bin = self.files["Models/data.bin"].clone();
        self.files
            .insert(self.settings.source.clone(), glb(&self.json, &bin));
    }
}
fn glb(json: &Value, bin: &[u8]) -> Vec<u8> {
    let mut json = serde_json::to_vec(json).unwrap();
    while json.len() % 4 != 0 {
        json.push(b' ');
    }
    let mut bin = bin.to_vec();
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let mut bytes = b"glTF".to_vec();
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(&((28 + json.len() + bin.len()) as u32).to_le_bytes());
    bytes.extend_from_slice(&(json.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"JSON");
    bytes.extend_from_slice(&json);
    bytes.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"BIN\0");
    bytes.extend_from_slice(&bin);
    bytes
}
fn png() -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&[255, 0, 0, 255, 0, 255, 0, 128])
            .unwrap();
    }
    bytes
}
fn floats(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect()
}

#[test]
fn import_decode_index_widths_nonindexed_and_glb_have_identical_geometry() {
    let expected = [-1., 0., 0., 1., 0., 0., 0., 1., 0.];
    for kind in [None, Some(5121), Some(5123), Some(5125)] {
        let mut f = Fixture::new();
        if let Some(kind) = kind {
            let width = match kind {
                5121 => 1,
                5123 => 2,
                _ => 4,
            };
            f.json["accessors"][1]["componentType"] = json!(kind);
            f.json["bufferViews"][1]["byteLength"] = json!(width * 3);
            f.json["buffers"][0]["byteLength"] = json!(36 + width * 3);
            let bytes = f.files.get_mut("Models/data.bin").unwrap();
            bytes.truncate(36);
            for v in [0u32, 1, 2] {
                bytes.extend_from_slice(&v.to_le_bytes()[..width]);
            }
        } else {
            f.json["meshes"][0]["primitives"][0]
                .as_object_mut()
                .unwrap()
                .remove("indices");
        }
        for binary in [false, true] {
            if binary {
                f.make_glb();
            }
            let decoded = f.decode().unwrap();
            let g = &decoded.primitives[0].geometry;
            assert_eq!(floats(&g.vertex_bytes), expected);
            assert_eq!(g.index_bytes, [0, 0, 1, 0, 2, 0]);
            assert_eq!(g.index_format, GeometryIndexFormat::Uint16);
            assert_eq!(g.bounds.min, [-1., 0., 0.]);
            assert_eq!(g.bounds.max, [1., 1., 0.]);
            assert!((g.bounds.sphere_radius - 1.118034).abs() < 1e-5);
        }
    }
}

#[test]
fn import_decode_offsets_stride_and_normalized_vertex_streams() {
    for component in [5121, 5123, 5126] {
        for color_size in [3, 4] {
            let mut f = Fixture::new();
            let mut packed = vec![0; 8];
            for p in [[-1.0f32, 0., 0.], [1., 0., 0.], [0., 1., 0.]] {
                packed.extend_from_slice(&[0; 4]);
                for v in p {
                    packed.extend_from_slice(&v.to_le_bytes());
                }
                packed.extend_from_slice(&[0; 4]);
            }
            f.files.insert("Models/data.bin".into(), packed);
            f.json["buffers"][0]["byteLength"] = json!(68);
            f.json["bufferViews"] =
                json!([{"buffer":0,"byteOffset":8,"byteLength":60,"byteStride":20}]);
            f.json["accessors"].as_array_mut().unwrap().truncate(1);
            f.json["accessors"][0]["byteOffset"] = json!(4);
            f.json["meshes"][0]["primitives"][0]
                .as_object_mut()
                .unwrap()
                .remove("indices");
            let width = match component {
                5121 => 1,
                5123 => 2,
                _ => 4,
            };
            for (semantic, count) in [("TEXCOORD_0", 2), ("COLOR_0", color_size)] {
                let mut data = Vec::new();
                let stride = (count * width + 3) / 4 * 4;
                for _ in 0..3 {
                    for i in 0..count {
                        if component == 5126 {
                            data.extend_from_slice(
                                &(if i == 0 { 1.0f32 } else { 0. }).to_le_bytes(),
                            );
                        } else {
                            data.extend(std::iter::repeat_n(if i == 0 { 255 } else { 0 }, width));
                        }
                    }
                    while data.len() % stride != 0 {
                        data.push(0);
                    }
                }
                let bi = f.json["buffers"].as_array().unwrap().len();
                let ai = f.json["accessors"].as_array().unwrap().len();
                let name = format!("{semantic}.bin");
                f.files.insert(format!("Models/{name}"), data.clone());
                f.json["buffers"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"uri":name,"byteLength":data.len()}));
                f.json["bufferViews"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"buffer":bi,"byteLength":data.len(),"byteStride":stride}));
                f.json["accessors"].as_array_mut().unwrap().push(json!({"bufferView":bi,"componentType":component,"count":3,"type":format!("VEC{count}"),"normalized":component!=5126}));
                f.json["meshes"][0]["primitives"][0]["attributes"][semantic] = json!(ai);
            }
            let decoded = f.decode().unwrap();
            let g = &decoded.primitives[0].geometry;
            assert_eq!(g.vertex_stride, 36);
            assert_eq!(
                floats(&g.vertex_bytes[..36]),
                [
                    -1.,
                    0.,
                    0.,
                    1.,
                    0.,
                    1.,
                    0.,
                    0.,
                    if color_size == 3 { 1. } else { 0. }
                ]
            );
            assert_eq!(g.attributes[2].semantic, VertexSemantic::Color0);
        }
    }
}

#[test]
fn import_decode_u32_output_and_multi_primitive_mesh_are_preserved() {
    let mut f = Fixture::new();
    let count = 65537;
    let mut bytes = vec![0u8; count * 12];
    bytes.extend_from_slice(
        &[0u32, 1, 65536]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>(),
    );
    f.json["buffers"][0]["byteLength"] = json!(bytes.len());
    f.files.insert("Models/data.bin".into(), bytes);
    f.json["bufferViews"][0]["byteLength"] = json!(count * 12);
    f.json["bufferViews"][1] = json!({"buffer":0,"byteOffset":count*12,"byteLength":12});
    f.json["accessors"][0]["count"] = json!(count);
    f.json["accessors"][1]["componentType"] = json!(5125);
    let decoded = f.decode().unwrap();
    assert_eq!(
        decoded.primitives[0].geometry.index_format,
        GeometryIndexFormat::Uint32
    );
    assert_eq!(
        decoded.primitives[0].geometry.index_bytes[8..],
        [0, 0, 1, 0]
    );
    let mut f = Fixture::new();
    let p = f.json["meshes"][0]["primitives"][0].clone();
    f.json["meshes"][0]["primitives"]
        .as_array_mut()
        .unwrap()
        .push(p);
    let decoded = f.decode().unwrap();
    assert_eq!(decoded.primitives.len(), 2);
    assert_eq!(decoded.primitives[1].primitive_index, 1);
    assert_eq!(
        decoded.report.source_bytes,
        (serde_json::to_vec(&f.json).unwrap().len() + 40) as u64
    );
}

#[test]
fn import_decode_png_jpeg_data_uri_external_and_glb_buffer_view_images() {
    let mut f = Fixture::new();
    f.texture(png(), "image/png");
    let decoded = f.decode().unwrap();
    assert_eq!(decoded.images[&0].rgba8, [255, 0, 0, 255, 0, 255, 0, 128]);
    f.files.insert("Models/image file.png".into(), png());
    f.json["images"][0] = json!({"uri":"image%20file.png"});
    let decoded = f.decode().unwrap();
    assert_eq!(decoded.images[&0].width, 2);
    assert!(decoded
        .report
        .dependencies
        .iter()
        .any(|d| d.source_path == "Models/image file.png"));
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 100)
        .encode(
            &[240, 10, 20, 240, 10, 20],
            2,
            1,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
    f.texture(jpeg, "image/jpeg");
    let image = f.decode().unwrap().images.remove(&0).unwrap();
    assert_eq!((image.width, image.height), (2, 1));
    assert!(image.rgba8[0] > 220 && image.rgba8[1] < 30);
    let mut f = Fixture::new();
    f.texture(png(), "image/png");
    let offset = f.files["Models/data.bin"].len();
    f.files.get_mut("Models/data.bin").unwrap().extend(png());
    f.json["buffers"][0]["byteLength"] = json!(f.files["Models/data.bin"].len());
    f.json["bufferViews"]
        .as_array_mut()
        .unwrap()
        .push(json!({"buffer":0,"byteOffset":offset,"byteLength":png().len()}));
    f.json["images"][0] = json!({"bufferView":3,"mimeType":"image/png"});
    f.make_glb();
    assert_eq!(
        f.decode().unwrap().images[&0].rgba8,
        [255, 0, 0, 255, 0, 255, 0, 128]
    );
}

#[test]
fn import_decode_selected_closure_ignores_unused_missing_and_unsupported_content() {
    let mut f = Fixture::new();
    f.json["scenes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"nodes":[1]}));
    f.json["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"mesh":1}));
    f.json["meshes"].as_array_mut().unwrap().push(json!({"primitives":[{"attributes":{"POSITION":0},"mode":0,"extensions":{"KHR_draco_mesh_compression":{}}}]}));
    f.json["buffers"]
        .as_array_mut()
        .unwrap()
        .push(json!({"uri":"https://must-not-fetch.invalid/a.bin","byteLength":4}));
    let d = f.decode().unwrap();
    assert_eq!(d.primitives.len(), 1);
    assert!(d.report.unconsumed.contains("/meshes/1"));
    assert!(d.report.unconsumed.contains("/buffers/1"));
    assert!(d.report.notices.iter().any(|n| n.outcome == "unconsumed"));
    f.settings.scene = 1;
    f.error("import_topology_unsupported", "/meshes/1/primitives/0/mode");
}

#[test]
fn import_decode_unsafe_uris_missing_and_truncated_dependencies_fail_with_paths() {
    for uri in [
        "https://example.invalid/a",
        "file:///etc/a",
        "C:/a",
        "/a",
        "../a",
        "%2e%2e/a",
        "..%5ca",
        "data.bin?x",
        "data.bin#x",
        "%00",
        "%zz",
        "a\\b",
    ] {
        let mut f = Fixture::new();
        f.json["buffers"][0]["uri"] = json!(uri);
        let e = f.decode().unwrap_err();
        assert!(e.error.code.contains("import_uri_"), "{uri}: {e:?}");
        assert_eq!(e.error.field_path, "/buffers/0/uri");
    }
    let mut f = Fixture::new();
    f.files.clear();
    f.error("import_dependency_missing", "/buffers/0/uri");
    let mut f = Fixture::new();
    f.files.get_mut("Models/data.bin").unwrap().truncate(38);
    f.error("import_buffer_truncated", "/buffers/0");
    let mut f = Fixture::new();
    f.json["buffers"][0]["uri"] = json!(format!(
        "data:application/gltf-buffer;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&f.files["Models/data.bin"])
    ));
    f.files.clear();
    assert_eq!(f.decode().unwrap().primitives.len(), 1);
}

#[test]
fn import_decode_container_version_and_required_extensions_are_strict() {
    for (key, value) in [
        ("version", "1.0"),
        ("version", "2.1"),
        ("minVersion", "2.1"),
    ] {
        let mut f = Fixture::new();
        f.json["asset"][key] = json!(value);
        f.error("import_version_unsupported", "/asset");
    }
    let mut f = Fixture::new();
    f.json["extensionsRequired"] = json!(["VENDOR_unknown"]);
    f.error(
        "import_required_extension_unsupported",
        "/extensionsRequired/0",
    );
    let mut f = Fixture::new();
    f.make_glb();
    let original = f.files[&f.settings.source].clone();
    for case in 0..5 {
        let mut bytes = original.clone();
        match case {
            0 => bytes[4] = 1,
            1 => bytes[8] ^= 1,
            2 => bytes[16] = b'B',
            3 => {
                bytes.push(0);
                let n = bytes.len() as u32;
                bytes[8..12].copy_from_slice(&n.to_le_bytes());
            }
            _ => bytes[12] = 255,
        }
        f.files.insert(f.settings.source.clone(), bytes);
        f.error("import_container_invalid", "");
    }
}

#[test]
fn import_decode_accessor_ranges_sparse_indices_and_nonfinite_never_fallback() {
    for case in 0..9 {
        let mut f = Fixture::new();
        match case {
            0 => f.json["accessors"][0]["byteOffset"] = json!(4),
            1 => f.json["bufferViews"][0]["byteStride"] = json!(8),
            2 => f.json["bufferViews"][0]["byteOffset"] = json!(u64::MAX - 16),
            3 => f.json["accessors"][0]["byteOffset"] = json!(2),
            4 => {
                f.json["accessors"][0]["sparse"] = json!({"count":1,"indices":{"bufferView":1,"componentType":5121},"values":{"bufferView":0}})
            }
            5 => f.files.get_mut("Models/data.bin").unwrap()[38] = 3,
            6 => f.files.get_mut("Models/data.bin").unwrap()[..4]
                .copy_from_slice(&f32::NAN.to_le_bytes()),
            7 => f.json["accessors"][1]["count"] = json!(2),
            _ => f.json["accessors"][0]["componentType"] = json!(5123),
        }
        let e = f.decode().unwrap_err();
        assert!(e.error.code.starts_with("three_d.import_"), "{case}: {e:?}");
        assert!(!e.error.field_path.is_empty(), "{case}: {e:?}");
    }
}

#[test]
fn import_decode_unsupported_selected_structure_and_optional_extensions_are_located() {
    for extension in [
        "KHR_draco_mesh_compression",
        "EXT_meshopt_compression",
        "KHR_texture_transform",
    ] {
        let mut f = Fixture::new();
        let path = if extension == "EXT_meshopt_compression" {
            "/bufferViews/0"
        } else {
            "/meshes/0/primitives/0"
        };
        f.json.pointer_mut(path).unwrap()["extensions"] = json!({extension:{}});
        f.error("import_extension_unsupported", "/extensions/");
    }
    let mut f = Fixture::new();
    f.json["meshes"][0]["primitives"][0]["targets"] = json!([{"POSITION":0}]);
    assert_eq!(f.decode().unwrap().character.unwrap().skins[0].morph_targets.len(),1);
    let mut f = Fixture::new();
    f.json["skins"] = json!([{"joints":[0]}]);
    f.json["nodes"][0]["skin"] = json!(0);
    f.error("import_character_invalid", "/meshes/0/primitives/0");
    let mut f = Fixture::new();
    f.json["animations"] = json!([{"channels":[{"sampler":0,"target":{"node":0,"path":"translation"}}],"samplers":[{"input":0,"output":0}]}]);
    f.error("import_animation_stream_invalid", "/animations/0/channels/0");
    let mut f = Fixture::new();
    f.json["nodes"][0]["extensions"] = json!({"VENDOR_note":{"value":1}});
    let d = f.decode().unwrap();
    assert_eq!(d.report.status, "degraded");
    assert_eq!(
        d.report.notices[0].field_path,
        "/nodes/0/extensions/VENDOR_note"
    );
    let mut f = Fixture::new();
    f.json["cameras"] = json!([{"type":"perspective","perspective":{"yfov":1,"znear":0.1}}]);
    f.json["nodes"][0]["camera"] = json!(0);
    let d = f.decode().unwrap();
    assert!(d.report.notices.iter().any(|n| n.outcome == "notImported"));
}

#[test]
fn import_decode_hierarchy_cycles_and_multiple_parents_fail() {
    let mut f = Fixture::new();
    f.json["nodes"][0]["children"] = json!([0]);
    f.error("import_hierarchy_invalid", "/nodes/0");
    let mut f = Fixture::new();
    f.json["scenes"][0]["nodes"] = json!([0, 0]);
    f.error("import_hierarchy_invalid", "/nodes/0");
}

#[test]
fn budget_source_vertices_indices_and_cooked_bytes_fail_before_allocation() {
    for case in 0..5 {
        let mut f = Fixture::new();
        let code = match case {
            0 => {
                f.settings.budgets.source_bytes =
                    serde_json::to_vec(&f.json).unwrap().len() as u64 + 39;
                "source_budget_exceeded"
            }
            1 => {
                f.json["accessors"][0]["count"] = json!(u64::MAX);
                "import_vertex_budget_exceeded"
            }
            2 => {
                f.json["accessors"][1]["count"] = json!(u64::MAX);
                "import_index_budget_exceeded"
            }
            3 => {
                f.settings.budgets.cooked_bytes = 47;
                "import_cooked_budget_exceeded"
            }
            _ => {
                f.json["buffers"][0]["byteLength"] = json!(u64::MAX);
                "source_budget_exceeded"
            }
        };
        f.error(code, "");
    }
}

#[test]
fn budget_node_depth_and_image_expansion_are_bounded() {
    let mut f = Fixture::new();
    f.json["nodes"] = json!([{"children":[1]},{"children":[2]},{"mesh":0}]);
    f.settings.budgets.depth = 2;
    f.error("import_node_budget_exceeded", "/nodes/2");
    f.settings.budgets.depth = 128;
    f.settings.budgets.nodes = 2;
    f.error("import_node_budget_exceeded", "/nodes/2");
    let mut f = Fixture::new();
    f.texture(png(), "image/png");
    f.settings.budgets.image_pixels = 1;
    f.error("import_image_budget_exceeded", "/images/0");
    let mut f = Fixture::new();
    f.texture(png(), "image/png");
    f.settings.budgets.decoded_image_bytes = 7;
    f.error("import_image_budget_exceeded", "/images/0");
    let mut f = Fixture::new();
    f.texture(png(), "image/png");
    f.settings.budgets.cooked_bytes = 3 * 20 + 3 * 4 + 7;
    f.error("import_cooked_budget_exceeded", "/images/0");
}

#[test]
fn import_decode_missing_uv_and_corrupt_images_fail_closed() {
    let mut f = Fixture::new();
    f.texture(png(), "image/png");
    f.json["meshes"][0]["primitives"][0]["attributes"]
        .as_object_mut()
        .unwrap()
        .remove("TEXCOORD_0");
    f.error("import_uv_missing", "TEXCOORD_0");
    let mut f = Fixture::new();
    f.texture(vec![137, 80, 78, 71, 13, 10, 26, 10], "image/png");
    f.error("import_image_invalid", "/images/0");
}

#[test]
fn import_decode_data_uri_mime_must_match_the_image_payload() {
    let mut f = Fixture::new();
    f.texture(png(), "image/jpeg");
    f.error("import_image_invalid", "/images/0");
}

#[test]
fn import_decode_light_definitions_and_unused_declarations_are_not_consumed() {
    let mut f = Fixture::new();
    f.json["extensionsUsed"] = json!(["KHR_lights_punctual", "VENDOR_note"]);
    f.json["extensions"] = json!({"KHR_lights_punctual":{"lights":[{"type":"point"}]}});
    let d = f.decode().unwrap();
    assert_eq!(d.report.status, "accepted");
    assert!(d
        .report
        .notices
        .iter()
        .any(|n| n.field_path == "/extensionsUsed/1" && n.outcome == "unconsumed"));
    assert!(d
        .report
        .notices
        .iter()
        .any(|n| n.field_path == "/extensions/KHR_lights_punctual" && n.outcome == "unconsumed"));
    f.json["nodes"][0]["extensions"] = json!({"KHR_lights_punctual":{"light":0}});
    f.error(
        "import_extension_unsupported",
        "/nodes/0/extensions/KHR_lights_punctual",
    );
}

#[test]
fn import_decode_independent_collada2gltf_box_matches_binary_container() {
    let mut f = Fixture::new();
    f.files.clear();
    f.settings.source = "Models/Box.gltf".into();
    f.files.insert(
        f.settings.source.clone(),
        include_bytes!("../../tests/fixtures/three_d_import/khronos_box/glTF/Box.gltf").to_vec(),
    );
    f.files.insert(
        "Models/Box0.bin".into(),
        include_bytes!("../../tests/fixtures/three_d_import/khronos_box/glTF/Box0.bin").to_vec(),
    );
    let text = f.decode().unwrap();
    f.settings.source = "Models/Box.glb".into();
    f.files.insert(
        f.settings.source.clone(),
        include_bytes!("../../tests/fixtures/three_d_import/khronos_box/glTF-Binary/Box.glb")
            .to_vec(),
    );
    let binary = f.decode().unwrap();
    let g = &text.primitives[0].geometry;
    assert_eq!(
        (g.vertex_count, g.index_count, g.vertex_stride),
        (24, 36, 24)
    );
    assert_eq!(g.bounds.min, [-0.5; 3]);
    assert_eq!(g.bounds.max, [0.5; 3]);
    assert_eq!(g.attributes[1].semantic, VertexSemantic::Normal);
    for vertex in g.vertex_bytes.chunks_exact(24) {
        let values = floats(vertex);
        assert!(values[..3].iter().all(|v| v.abs() == 0.5));
        assert_eq!(values[3..].iter().map(|v| v * v).sum::<f32>(), 1.0);
    }
    assert_eq!(g.vertex_bytes, binary.primitives[0].geometry.vertex_bytes);
    assert_eq!(g.index_bytes, binary.primitives[0].geometry.index_bytes);
    assert!(text.report.consumed.contains("/nodes/1"));
    assert!(
        !text.report.assembly_available,
        "PBR conversion and node matrices belong to A2.2"
    );
}

#[test]
fn budget_all_document_nodes_are_bounded_before_typed_parser_allocation() {
    let mut f = Fixture::new();
    f.json["nodes"] = json!([{"mesh":0}, {}, {}]);
    f.settings.budgets.nodes = 2;
    f.error("import_node_budget_exceeded", "/nodes/2");
}

#[test]
fn import_decode_invalid_reference_reports_the_source_field() {
    let mut f = Fixture::new();
    f.json["nodes"][0]["mesh"] = json!(1);
    f.error("import_schema_invalid", "/nodes/0/mesh");
}


#[cfg(test)]
mod s3a {
    use super::*;
    use engine_runtime::character_animation::*;
    fn stream(f:&mut Fixture, values:&[f32],count:usize,kind:&str)->usize {
        let bytes=f.files.get_mut("Models/data.bin").unwrap();
        while bytes.len()%4!=0 {bytes.push(0);}
        let offset=bytes.len();bytes.extend(values.iter().flat_map(|v|v.to_le_bytes()));
        f.json["buffers"][0]["byteLength"]=json!(bytes.len());
        let view=f.json["bufferViews"].as_array().unwrap().len();
        f.json["bufferViews"].as_array_mut().unwrap().push(json!({"buffer":0,"byteOffset":offset,"byteLength":values.len()*4}));
        let accessor=f.json["accessors"].as_array().unwrap().len();
        f.json["accessors"].as_array_mut().unwrap().push(json!({"bufferView":view,"componentType":5126,"count":count,"type":kind}));
        accessor
    }
    fn character()->Fixture {
        let mut f=Fixture::new();
        f.settings.material_strategy=crate::three_d_contract::ImportMaterialStrategy::BaseColorOnly;
        f.json["nodes"]=json!([{"mesh":0,"skin":0,"children":[1]}, {"translation":[0,1,0]}]);
        f.json["skins"]=json!([{"joints":[0,1]}]);
        let bytes=f.files.get_mut("Models/data.bin").unwrap();let offset=bytes.len();
        bytes.extend([0u8,1,0,0,0,1,0,0,0,1,0,0]);
        f.json["bufferViews"].as_array_mut().unwrap().push(json!({"buffer":0,"byteOffset":offset,"byteLength":12}));
        f.json["accessors"].as_array_mut().unwrap().push(json!({"bufferView":2,"componentType":5121,"count":3,"type":"VEC4"}));
        f.json["meshes"][0]["primitives"][0]["attributes"]["JOINTS_0"]=json!(2);
        let weights=stream(&mut f,&[0.5,0.5,0.,0.,1.,0.,0.,0.,0.,1.,0.,0.],3,"VEC4");
        f.json["meshes"][0]["primitives"][0]["attributes"]["WEIGHTS_0"]=json!(weights);
        let times=stream(&mut f,&[0.,0.25,2.],3,"SCALAR");
        f.json["accessors"][times]["min"]=json!([0.]);f.json["accessors"][times]["max"]=json!([2.]);
        let values=stream(&mut f,&[0.,0.,0.,1.,0.,0.,8.,0.,0.],3,"VEC3");
        f.json["animations"]=json!([{"name":"walk","channels":[{"sampler":0,"target":{"node":1,"path":"translation"}}],"samplers":[{"input":times,"output":values}],"extras":{"aifeEvents":[{"time":0.25,"name":"step"}]}}]);
        f.settings.nodes.push(serde_json::from_value(json!({"key":"joint-hand","entityId":"imported.hand","sourceLocator":"/nodes/1","sourceFingerprint":format!("sha256:{}","0".repeat(64))})).unwrap());
        f
    }
    fn mappings(f:&mut Fixture){
        f.settings.outputs.push(serde_json::from_value(json!({"key":"material","assetId":"material","assetGuid":"guid-material","kind":"material","sourceLocator":"/materials/default","sourceFingerprint":assembly::fingerprint(&f.json,"/materials/default").unwrap(),"materialSlots":[]})).unwrap());
        f.settings.outputs.push(serde_json::from_value(json!({"key":"character","assetId":"character","assetGuid":"guid-character","kind":"character","sourceLocator":"/scenes/0/character","sourceFingerprint":assembly::fingerprint(&f.json,"/scenes/0/character").unwrap(),"materialSlots":[]})).unwrap());
        for output in &mut f.settings.outputs {output.source_fingerprint=assembly::fingerprint(&f.json,&output.source_locator).unwrap();}
        for node in &mut f.settings.nodes {node.source_fingerprint=assembly::fingerprint(&f.json,&node.source_locator).unwrap();}
    }
    #[test]
    fn gltf_character_skin_named_clip_event_and_cooked_prefab_have_stable_identity(){
        let mut f=character();mappings(&mut f);let decoded=f.decode().unwrap();
        assert!(decoded.report.consumed.contains("/skins/0"));assert!(decoded.report.consumed.contains("/animations/0"));
        let asset=decoded.character.as_ref().unwrap();assert_eq!(asset.joints[1].key,"joint-hand");assert_eq!(asset.clips[0].events[0].name,"step");
        assert_eq!(asset.sample(0,1.125).unwrap().joints[1].translation,[4.5,0.,0.]);
        let compiled=assembly::compile(f.settings.clone(),decoded).unwrap();let (_,asset)=compiled.character.unwrap();
        assert_eq!(asset.skins[0].mesh_asset_id,"imported.mesh");
        let binding=&compiled.prefab.1["entities"][0]["components"][0]["data"];
        assert_eq!(binding["joints"][1]["entityRef"],"imported.hand");
        assert_eq!(binding["character"]["id"],"character");
        let bytes=serde_json::to_vec(&asset).unwrap();let restored:CharacterAsset=serde_json::from_slice(&bytes).unwrap();restored.validate().unwrap();assert_eq!(asset,restored);
    }
    #[test]
    fn malformed_character_streams_targets_weights_and_ambiguous_clip_names_are_located(){
        let mut f=character();f.json["animations"][0]["name"]=json!("");f.error("import_character_invalid","/animations/0");
        let mut f=character();let a=f.json["animations"][0].clone();f.json["animations"].as_array_mut().unwrap().push(a);f.error("import_character_invalid","/animations/1");
        let mut f=character();f.json["skins"][0]["joints"]=json!([0,0]);f.error("character_invalid","/skins/0");
        let mut f=character();let offset=f.json["bufferViews"][3]["byteOffset"].as_u64().unwrap() as usize;f.files.get_mut("Models/data.bin").unwrap()[offset..offset+4].copy_from_slice(&(-1f32).to_le_bytes());f.error("character_invalid","/skins/0");
        let mut f=character();f.json["accessors"][4]["count"]=json!(10000);f.error("import_accessor_range_invalid","/accessors/4");
    }
    #[test]
    fn clip_change_preserves_geometry_and_mesh_output_but_invalidates_character_payload(){
        let mut f=character();mappings(&mut f);let first=assembly::compile(f.settings.clone(),f.decode().unwrap()).unwrap();
        let offset=f.json["bufferViews"][5]["byteOffset"].as_u64().unwrap() as usize;
        f.files.get_mut("Models/data.bin").unwrap()[offset+12..offset+16].copy_from_slice(&3f32.to_le_bytes());
        let second=assembly::compile(f.settings.clone(),f.decode().unwrap()).unwrap();
        assert_eq!(first.report.geometry_digests,second.report.geometry_digests);assert_eq!(serde_json::to_value(&first.assets).unwrap(),serde_json::to_value(&second.assets).unwrap());
        assert_ne!(first.character.unwrap().1,second.character.unwrap().1);
    }
}
