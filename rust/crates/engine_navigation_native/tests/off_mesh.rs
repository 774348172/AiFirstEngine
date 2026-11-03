use engine_navigation_native::*;
fn config() -> BakeConfig {
    BakeConfig {
        cell_size: 0.2,
        cell_height: 0.1,
        radius: 0.3,
        height: 1.8,
        climb: 0.3,
        slope: 45.,
        width: 16.,
        min: [0., -1., 0.],
        max: [16., 4., 16.],
        x: 0,
        z: 0,
    }
}
fn gap(links: &[OffMeshLink]) -> Vec<u8> {
    bake(
        &config(),
        &[
            [0., 0., 0.],
            [0., 0., 16.],
            [6., 0., 16.],
            [6., 0., 0.],
            [10., 0., 0.],
            [10., 0., 16.],
            [16., 0., 16.],
            [16., 0., 0.],
        ],
        &[[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7]],
        links,
    )
    .unwrap()
}
#[test]
fn real_detour_off_mesh_link_direction_flags_and_malformed_translation() {
    let from = [2., 0., 8.];
    let to = [14., 0., 8.];
    let extent = [0.5, 2., 0.5];
    let mut plain = NativeNav::new(16.).unwrap();
    plain.add_tile(&gap(&[])).unwrap();
    assert_eq!(
        plain.find_path(from, to, extent),
        Err(NavError::Unreachable)
    );
    let link = OffMeshLink {
        start: [5., 0., 8.],
        end: [11., 0., 8.],
        radius: 0.6,
        bidirectional: 0,
        id: 1,
    };
    let data = gap(&[link]);
    let mut nav = NativeNav::new(16.).unwrap();
    nav.add_tile(&data).unwrap();
    let path = nav.find_path(from, to, extent).unwrap();
    assert!(path.flags.iter().any(|f| f & 4 != 0), "{path:?}");
    assert_eq!(nav.find_path(to, from, extent), Err(NavError::Unreachable));
    assert_eq!(
        translated_tile(&data, [0.; 3], [i32::MIN, 0]),
        Err(NavError::InvalidInput)
    );
    let mut both = NativeNav::new(16.).unwrap();
    both.add_tile(&gap(&[OffMeshLink {
        bidirectional: 1,
        ..link
    }]))
    .unwrap();
    both.find_path(to, from, extent).unwrap();
}
