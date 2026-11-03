use super::*;

#[test]
fn generated_content_376_queries_extend_the_original_typed_world_query_contract() {
    let bytes = serde_json::to_vec(&ContentQueryRequest {
        generated_content: ContentQuery::Request { request_id: 7 },
    })
    .unwrap();
    let mut request: crate::ProjectRuntimeWorldQueryRequest =
        serde_json::from_slice(&bytes).unwrap();
    assert!(request.validate_identity_query().is_ok());
    request.all.push("Transform3D".into());
    assert!(request.validate_identity_query().is_err());
    let legacy: crate::ProjectRuntimeWorldQueryRequest =
        serde_json::from_value(serde_json::json!({"all":[],"none":[]})).unwrap();
    assert_eq!(
        serde_json::to_value(legacy).unwrap(),
        serde_json::json!({"all":[],"none":[]})
    );
}

#[test]
fn generated_content_376_cancel_uses_v2_and_query_metadata_is_exclusive_and_compact() {
    let ticket = ContentTicket {
        session_generation: 4,
        request_id: 8,
    };
    let mut output = Output::no_op();
    output
        .mutations
        .push(Mutation::GeneratedContentCancel { ticket });
    let wire = encode_session(output.clone()).unwrap();
    assert!(wire.starts_with(b"AIFESB02"));
    assert_eq!(decode_session(&wire).unwrap(), output);
    assert!(decode_session(&wire[16..table(&wire)]).is_err());
    let query = ContentQueryRequest {
        generated_content: ContentQuery::Ticket { ticket },
    };
    let bytes = serde_json::to_vec(&query).unwrap();
    assert!(bytes.len() < 256);
    assert_eq!(
        serde_json::from_slice::<ContentQueryRequest>(&bytes).unwrap(),
        query
    );
    assert!(serde_json::from_value::<ContentQueryRequest>(
        serde_json::json!({"generatedContent":{"query":"request","requestId":8},"all":[]})
    )
    .is_err());
    assert!(ContentQuery::Request { request_id: 0 }.validate().is_err());
    assert!(ContentQuery::Ticket {
        ticket: ContentTicket {
            session_generation: 0,
            request_id: 8
        }
    }
    .validate()
    .is_err());
}

#[test]
fn generated_content_376_one_material_slot_cannot_refer_to_two_assets() {
    let p = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]];
    let mut sections = sections();
    let mut second = sections[0].clone();
    second.first_index = 3;
    second.material.id = "other".into();
    sections.push(second);
    let e = GeometryBuilder3D::new(&p)
        .triangles_u16(&[0, 1, 2, 0, 2, 1], sections)
        .unwrap_err();
    assert_eq!(e.field_path, "geometry.sections.materialSlot");
}
use crate::{
    spatial_bulk::{decode_session, encode_session, MAX_SESSION_BYTES},
    ProjectRuntimeDeferredMutation as Mutation, ProjectRuntimeSessionOutput as Output,
};

fn sections() -> Vec<GeometrySection3D> {
    vec![GeometrySection3D {
        first_index: 0,
        index_count: 3,
        material_slot: "surface".into(),
        material: ProjectRuntimeSpatialAssetRef {
            id: "mat.surface".into(),
            asset_type: "material".into(),
            guid: Some("material-guid".into()),
            sub_asset: None,
        },
    }]
}
fn geometry() -> Geometry3D {
    GeometryBuilder3D::new(&[[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]])
        .triangles_u16(&[0, 1, 2], sections())
        .unwrap()
}
fn request(geometry: Geometry3D) -> GeneratedContentRequest {
    GeneratedContentRequest {
        request_id: 8,
        target: ProjectRuntimeCellEntity {
            entity_id: "surface".into(),
            generation: 7,
        },
        expected_applied_revision: 0,
        operation: GeneratedOperation3D::Replace {
            geometry,
            collision: CollisionUpdate3D::Keep,
        },
    }
}
fn output(request: GeneratedContentRequest) -> Output {
    let mut o = Output::no_op();
    o.mutations.push(Mutation::GeneratedContent { request });
    o
}
fn put(bytes: &mut [u8], start: usize, value: u32) {
    bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
}
fn table(bytes: &[u8]) -> usize {
    16 + u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize
}

#[test]
fn generated_content_376_builder_computes_layout_and_supports_both_index_formats() {
    let p = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]];
    let colors = [[1., 0., 0., 1.]; 3];
    let uv = [[0., 0.]; 3];
    let n = [[0., 0., 1.]; 3];
    let mesh = GeometryBuilder3D::new(&p)
        .normals(&n)
        .uv(&uv)
        .colors(&colors)
        .triangles_u32(&[0, 1, 2], sections())
        .unwrap();
    assert_eq!(mesh.vertex_stride, 48);
    assert_eq!(
        mesh.attributes.iter().map(|a| a.offset).collect::<Vec<_>>(),
        [0, 12, 24, 32]
    );
    assert_eq!(mesh.vertex_bytes.len(), 144);
    assert_eq!(mesh.index_bytes.len(), 12);
    assert_eq!(geometry().index_bytes, [0, 0, 1, 0, 2, 0]);
    assert!(GeometryBuilder3D::new(&p)
        .colors(&colors[..2])
        .triangles_u16(&[0, 1, 2], sections())
        .is_err());
}
#[test]
fn generated_content_376_server_decode_owns_attachment_bytes_and_preserves_mixed_order() {
    let mut expected = output(request(geometry()));
    expected.mutations.push(Mutation::SpatialLocalTransform {
        entity: crate::spatial_bulk::SpatialEntityKey {
            world_epoch: 2,
            index: 3,
            generation: 4,
        },
        transform: crate::ProjectRuntimeTransform3D::identity(),
    });
    expected.mutations.push(Mutation::ProjectStorage {
        request: crate::project_storage::ProjectStorageRequest {
            request_id: 9,
            operation: crate::project_storage::StorageOperation::Write {
                key: "world/parameters".into(),
                bytes: vec![0, 1, 255],
            },
        },
    });
    let mut wire = encode_session(expected.clone()).unwrap();
    let decoded = decode_session(&wire).unwrap();
    wire.fill(0);
    drop(wire);
    assert_eq!(decoded, expected);
}
#[test]
fn generated_content_376_wire_rejects_all_truncated_ranges_and_overlap_before_hydration() {
    let wire = encode_session(output(request(geometry()))).unwrap();
    for len in 0..wire.len() {
        assert!(decode_session(&wire[..len]).is_err(), "truncation {len}");
    }
    let start = table(&wire);
    for (offset, value) in [
        (8, u32::MAX),
        (12, u32::MAX),
        (start, 20000),
        (start + 4, 1),
        (start + 8, 1),
        (start + 12, u32::MAX),
        (start + 16, 1),
        (start + 16 + 4, 0),
        (start + 16 + 8, 0),
    ] {
        let mut bad = wire.clone();
        put(&mut bad, offset, value);
        assert!(decode_session(&bad).is_err(), "offset {offset}");
    }
    let mut trailing = wire.clone();
    trailing.push(0);
    assert!(decode_session(&trailing).is_err());
    let inner = &wire[16..table(&wire)];
    assert!(
        decode_session(inner).is_err(),
        "v2-only mutation must not enter v1/JSON"
    );
    let mut unknown = wire;
    unknown[7] = b'3';
    assert!(decode_session(&unknown).is_err());
}
#[test]
fn generated_content_376_entry_revalidates_nonfinite_indices_stride_and_attribute_overlap() {
    for kind in 0..4 {
        let mut mesh = geometry();
        match kind {
            0 => mesh.index_bytes[0..2].copy_from_slice(&3u16.to_le_bytes()),
            1 => mesh.vertex_stride = 16,
            2 => mesh.vertex_bytes[0..4].copy_from_slice(&f32::NAN.to_le_bytes()),
            _ => mesh.attributes.push(AttributeLayout3D {
                semantic: Attribute3D::Normal,
                offset: 0,
            }),
        }
        assert!(encode_session(output(request(mesh))).is_err());
    }
    let wire = encode_session(output(request(geometry()))).unwrap();
    let start = table(&wire) + 32;
    let mut bad = wire;
    bad[start..start + 4].copy_from_slice(&f32::INFINITY.to_le_bytes());
    assert!(decode_session(&bad).is_err());
}
#[test]
fn generated_content_376_total_envelope_includes_metadata_and_every_attachment() {
    let mut o = Output::no_op();
    o.mutations.push(Mutation::ProjectStorage {
        request: crate::project_storage::ProjectStorageRequest {
            request_id: 1,
            operation: crate::project_storage::StorageOperation::Write {
                key: "state".into(),
                bytes: vec![],
            },
        },
    });
    let overhead = encode_session(o.clone()).unwrap().len();
    let Mutation::ProjectStorage { request } = &mut o.mutations[0] else {
        panic!()
    };
    let crate::project_storage::StorageOperation::Write { bytes, .. } = &mut request.operation
    else {
        panic!()
    };
    bytes.resize(MAX_SESSION_BYTES - overhead, 0);
    let exact = encode_session(o.clone()).unwrap();
    assert_eq!(exact.len(), MAX_SESSION_BYTES);
    assert_eq!(decode_session(&exact).unwrap(), o);
    let Mutation::ProjectStorage { request } = &mut o.mutations[0] else {
        panic!()
    };
    request.payloads_mut(|bytes| bytes.push(1));
    assert!(encode_session(o).is_err());
    let mut two = Output::no_op();
    for id in 1..=2 {
        two.mutations.push(Mutation::ProjectStorage {
            request: crate::project_storage::ProjectStorageRequest {
                request_id: id,
                operation: crate::project_storage::StorageOperation::Write {
                    key: "state".into(),
                    bytes: vec![0; MAX_SESSION_BYTES / 2],
                },
            },
        });
    }
    assert!(encode_session(two).is_err());
}
#[test]
fn generated_content_376_static_collision_attachments_roundtrip_and_invalid_faces_are_rejected() {
    let shape = StaticShape3D::triangles(&[[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]], &[[0, 1, 2]])
        .unwrap();
    let mut r = request(geometry());
    if let GeneratedOperation3D::Replace { collision, .. } = &mut r.operation {
        *collision = CollisionUpdate3D::Replace {
            body: StaticBody3D::new(shape),
        };
    }
    let expected = output(r);
    let wire = encode_session(expected.clone()).unwrap();
    assert_eq!(decode_session(&wire).unwrap(), expected);
    assert!(StaticShape3D::triangles(&[[0., 0., 0.]; 3], &[[0, 0, 2]]).is_err());
    assert!(StaticShape3D::triangles(&[[0., 0., 0.]; 3], &[[0, 1, 3]]).is_err());
    let mut body =
        StaticBody3D::new(StaticShape3D::triangles(&[[0., 0., 0.]; 3], &[[0, 1, 2]]).unwrap());
    body.sensor = true;
    assert!(body.validate().is_err());
}
#[test]
fn generated_content_376_attribute_builder_rejects_nonfinite_and_range_overflow() {
    assert!(AttributePatch3D::positions(0, &[[f32::NAN, 0., 0.]]).is_err());
    assert!(AttributePatch3D::uv(u32::MAX, &[[0., 0.]]).is_err());
    assert!(AttributePatch3D::colors(0, &[]).is_err());
    let patch = AttributePatch3D::positions(2, &[[5., 6., 7.]]).unwrap();
    assert_eq!(patch.vertex_count, 1);
    assert_eq!(patch.bytes.len(), 12);
}
#[test]
fn generated_content_376_storage_keys_are_relative_and_empty_content_is_legal() {
    use crate::project_storage::*;
    for key in [
        "",
        "/outside",
        "../outside",
        "a/../b",
        "C:/save",
        "\\\\server\\share",
        "x:stream",
        "a//b",
        "a/.",
        "a/b.",
        "a/b ",
    ] {
        assert!(
            ProjectStorageRequest {
                request_id: 1,
                operation: StorageOperation::Read { key: key.into() }
            }
            .validate()
            .is_err(),
            "{key}"
        );
    }
    let mut o = Output::no_op();
    o.mutations.push(Mutation::ProjectStorage {
        request: ProjectStorageRequest {
            request_id: 1,
            operation: StorageOperation::Write {
                key: "world/state".into(),
                bytes: vec![],
            },
        },
    });
    assert_eq!(
        decode_session(&encode_session(o.clone()).unwrap()).unwrap(),
        o
    );
}
