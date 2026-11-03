//! Private native adapter: fixed Recast/Detour 1.6.0, 64-bit polygon references.
//! Projects consume engine contracts, never these solver handles.
use std::{ffi::c_void, ptr::NonNull};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavError {
    InvalidInput,
    BuildFailed,
    EmptyTile,
    InvalidTile,
    Unreachable,
    NotResident,
    Capacity,
    Backend,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BakeConfig {
    pub cell_size: f32,
    pub cell_height: f32,
    pub radius: f32,
    pub height: f32,
    pub climb: f32,
    pub slope: f32,
    pub width: f32,
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub x: i32,
    pub z: i32,
}
impl BakeConfig {
    pub fn validate(&self) -> Result<(), NavError> {
        if [
            self.cell_size,
            self.cell_height,
            self.radius,
            self.height,
            self.climb,
            self.slope,
            self.width,
        ]
        .iter()
        .any(|v| !v.is_finite())
            || self
                .min
                .iter()
                .chain(&self.max)
                .any(|v| !v.is_finite() || v.abs() > 32768.)
            || !(0.025..=1.).contains(&self.cell_size)
            || !(0.01..=0.5).contains(&self.cell_height)
            || !(0.05..=4.).contains(&self.radius)
            || !(0.2..=8.).contains(&self.height)
            || !(0.0..=2.).contains(&self.climb)
            || !(1.0..=85.).contains(&self.slope)
            || !(4.0..=64.).contains(&self.width)
            || (0..3).any(|i| self.min[i] >= self.max[i])
            || (self.max[0] - self.min[0] - self.width).abs() > 0.001
            || (self.max[2] - self.min[2] - self.width).abs() > 0.001
            || (self.width / self.cell_size).round() > 960.
            || (self.height / self.cell_height).ceil() < 3.
            || ((self.width / self.cell_size).round() * self.cell_size - self.width).abs() > 0.001
        {
            return Err(NavError::InvalidInput);
        }
        Ok(())
    }
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OffMeshLink {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub radius: f32,
    pub bidirectional: u32,
    pub id: u32,
}
#[derive(Debug, Clone, PartialEq)]
pub struct NativePath {
    pub points: Vec<[f32; 3]>,
    pub flags: Vec<u8>,
    pub references: Vec<u64>,
    pub end_reference: u64,
}
extern "C" {
    fn aife_nav_ref_bits() -> i32;
    fn aife_nav_free_bytes(p: *mut u8);
    fn aife_nav_validate(p: *const u8, n: i32) -> i32;
    fn aife_nav_translate(bytes: *mut u8, size: i32, delta: *const f32, dx: i32, dz: i32) -> i32;
    fn aife_nav_bake(
        cfg: *const BakeConfig,
        verts: *const f32,
        nv: i32,
        tris: *const i32,
        nt: i32,
        links: *const OffMeshLink,
        nl: i32,
        out: *mut *mut u8,
        n: *mut i32,
    ) -> i32;
    fn aife_nav_create(width: f32) -> *mut c_void;
    fn aife_nav_destroy(nav: *mut c_void);
    fn aife_nav_add(nav: *mut c_void, bytes: *const u8, size: i32, result: *mut u64) -> i32;
    fn aife_nav_remove(nav: *mut c_void, tile: u64) -> i32;
    fn aife_nav_valid(nav: *mut c_void, poly: u64) -> i32;
    fn aife_nav_path(
        nav: *mut c_void,
        from: *const f32,
        to: *const f32,
        extent: *const f32,
        points: *mut f32,
        flags: *mut u8,
        refs: *mut u64,
        count: *mut i32,
        end_ref: *mut u64,
    ) -> i32;
    fn aife_nav_obstacles(nav: *mut c_void, boxes: *const f32, count: i32) -> i32;
    fn aife_nav_move(
        nav: *mut c_void,
        from: *const f32,
        to: *const f32,
        extent: *const f32,
        result: *mut f32,
    ) -> i32;
}
pub fn poly_ref_bits() -> u32 {
    unsafe { aife_nav_ref_bits() as u32 }
}
pub fn validate_tile(bytes: &[u8]) -> Result<(), NavError> {
    if bytes.is_empty()
        || bytes.len() > 16 * 1024 * 1024
        || unsafe { aife_nav_validate(bytes.as_ptr(), bytes.len() as i32) } != 0
    {
        Err(NavError::InvalidTile)
    } else {
        Ok(())
    }
}
pub fn translated_tile(
    bytes: &[u8],
    delta: [f32; 3],
    tile_delta: [i32; 2],
) -> Result<Vec<u8>, NavError> {
    validate_tile(bytes)?;
    if delta.iter().any(|v| !v.is_finite() || v.abs() > 32768.)
        || tile_delta.iter().any(|v| v.unsigned_abs() > 8192)
    {
        return Err(NavError::InvalidInput);
    }
    let mut result = bytes.to_vec();
    if unsafe {
        aife_nav_translate(
            result.as_mut_ptr(),
            result.len() as i32,
            delta.as_ptr(),
            tile_delta[0],
            tile_delta[1],
        )
    } == 0
    {
        Ok(result)
    } else {
        Err(NavError::InvalidTile)
    }
}
pub fn bake(
    config: &BakeConfig,
    vertices: &[[f32; 3]],
    triangles: &[[u32; 3]],
    links: &[OffMeshLink],
) -> Result<Vec<u8>, NavError> {
    config.validate()?;
    if vertices.len() < 3
        || vertices.len() > 1_000_000
        || triangles.is_empty()
        || triangles.len() > 1_000_000
        || links.len() > 1024
        || vertices
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || v.abs() > 32768.)
        || triangles
            .iter()
            .flatten()
            .any(|i| *i as usize >= vertices.len())
        || links.iter().any(|l| {
            l.start
                .iter()
                .chain(&l.end)
                .any(|v| !v.is_finite() || v.abs() > 32768.)
                || !l.radius.is_finite()
                || !(0.05..=4.).contains(&l.radius)
                || l.bidirectional > 1
        })
    {
        return Err(NavError::InvalidInput);
    }
    let triangles = triangles
        .iter()
        .flatten()
        .map(|i| *i as i32)
        .collect::<Vec<_>>();
    let mut ptr = std::ptr::null_mut();
    let mut n = 0;
    // All pointers cover validated, stable buffers for this synchronous native call.
    let status = unsafe {
        aife_nav_bake(
            config,
            vertices.as_ptr().cast(),
            vertices.len() as i32,
            triangles.as_ptr(),
            (triangles.len() / 3) as i32,
            links.as_ptr(),
            links.len() as i32,
            &mut ptr,
            &mut n,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => NavError::InvalidInput,
            3 => NavError::EmptyTile,
            4 => NavError::InvalidTile,
            _ => NavError::BuildFailed,
        });
    }
    if ptr.is_null() || !(1..=16 * 1024 * 1024).contains(&n) {
        if !ptr.is_null() {
            unsafe { aife_nav_free_bytes(ptr) }
        }
        return Err(NavError::BuildFailed);
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, n as usize).to_vec() };
    unsafe { aife_nav_free_bytes(ptr) };
    Ok(bytes)
}
pub struct NativeNav {
    ptr: NonNull<c_void>,
}
impl std::fmt::Debug for NativeNav {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeNav").finish_non_exhaustive()
    }
}
// The opaque object has no thread-affine state. Every operation requires &mut self.
unsafe impl Send for NativeNav {}
impl NativeNav {
    pub fn new(width: f32) -> Result<Self, NavError> {
        if !width.is_finite() || !(4.0..=64.).contains(&width) {
            return Err(NavError::InvalidInput);
        }
        NonNull::new(unsafe { aife_nav_create(width) })
            .map(|ptr| Self { ptr })
            .ok_or(NavError::Backend)
    }
    pub fn add_tile(&mut self, bytes: &[u8]) -> Result<u64, NavError> {
        if bytes.is_empty() || bytes.len() > 16 * 1024 * 1024 {
            return Err(NavError::InvalidTile);
        }
        let mut tile = 0;
        let status = unsafe {
            aife_nav_add(
                self.ptr.as_ptr(),
                bytes.as_ptr(),
                bytes.len() as i32,
                &mut tile,
            )
        };
        if status == 0 {
            Ok(tile)
        } else {
            Err(if status == 1 {
                NavError::InvalidTile
            } else {
                NavError::Capacity
            })
        }
    }
    pub fn remove_tile(&mut self, tile: u64) -> Result<(), NavError> {
        if unsafe { aife_nav_remove(self.ptr.as_ptr(), tile) } == 0 {
            Ok(())
        } else {
            Err(NavError::InvalidTile)
        }
    }
    pub fn valid_reference(&mut self, poly: u64) -> bool {
        unsafe { aife_nav_valid(self.ptr.as_ptr(), poly) != 0 }
    }
    pub fn find_path(
        &mut self,
        from: [f32; 3],
        to: [f32; 3],
        extent: [f32; 3],
    ) -> Result<NativePath, NavError> {
        if from
            .iter()
            .chain(&to)
            .any(|v| !v.is_finite() || v.abs() > 32768.)
            || extent
                .iter()
                .any(|v| !v.is_finite() || *v <= 0. || *v > 16.)
        {
            return Err(NavError::InvalidInput);
        }
        let mut points = [[0.; 3]; 256];
        let mut flags = [0; 256];
        let mut refs = [0; 256];
        let mut count = 0;
        let mut end_reference = 0;
        let status = unsafe {
            aife_nav_path(
                self.ptr.as_ptr(),
                from.as_ptr(),
                to.as_ptr(),
                extent.as_ptr(),
                points.as_mut_ptr().cast(),
                flags.as_mut_ptr(),
                refs.as_mut_ptr(),
                &mut count,
                &mut end_reference,
            )
        };
        if status != 0 {
            return Err(match status {
                1 => NavError::Unreachable,
                2 => NavError::NotResident,
                3 => NavError::Capacity,
                _ => NavError::Backend,
            });
        }
        if !(1..=256).contains(&count) {
            return Err(NavError::Backend);
        }
        let n = count as usize;
        Ok(NativePath {
            points: points[..n].to_vec(),
            flags: flags[..n].to_vec(),
            references: refs[..n].to_vec(),
            end_reference,
        })
    }
    /// Keep an automatic motor step on the current filtered navigation surface.
    pub fn move_on_surface(
        &mut self,
        from: [f32; 3],
        to: [f32; 3],
        extent: [f32; 3],
    ) -> Result<[f32; 3], NavError> {
        if from
            .iter()
            .chain(&to)
            .any(|v| !v.is_finite() || v.abs() > 32768.)
            || extent
                .iter()
                .any(|v| !v.is_finite() || *v <= 0. || *v > 16.)
        {
            return Err(NavError::InvalidInput);
        }
        let mut result = [0.; 3];
        let status = unsafe {
            aife_nav_move(
                self.ptr.as_ptr(),
                from.as_ptr(),
                to.as_ptr(),
                extent.as_ptr(),
                result.as_mut_ptr(),
            )
        };
        match status {
            0 if result.iter().all(|v| v.is_finite() && v.abs() <= 32768.) => Ok(result),
            2 => Err(NavError::NotResident),
            3 => Err(NavError::Capacity),
            _ => Err(NavError::Backend),
        }
    }
    pub fn set_obstacles(&mut self, boxes: &[[f32; 6]]) -> Result<(), NavError> {
        if boxes.len() > 128
            || boxes.iter().any(|b| {
                b.iter().any(|v| !v.is_finite() || v.abs() > 32768.)
                    || (0..3).any(|i| b[i] > b[i + 3])
            })
        {
            return Err(NavError::InvalidInput);
        }
        if unsafe {
            aife_nav_obstacles(self.ptr.as_ptr(), boxes.as_ptr().cast(), boxes.len() as i32)
        } == 0
        {
            Ok(())
        } else {
            Err(NavError::Capacity)
        }
    }
}
impl Drop for NativeNav {
    fn drop(&mut self) {
        unsafe { aife_nav_destroy(self.ptr.as_ptr()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dense_full_terrain_halo_bakes_and_keeps_a_real_path() {
        let mut config = cfg(0);
        config.width = 32.;
        config.max[0] = 32.;
        config.max[2] = 32.;
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        for z in 0..256 {
            for x in 0..256 {
                let x = x as f32 * 0.125;
                let z = z as f32 * 0.125;
                let corners = [
                    [x, 0., z],
                    [x + 0.125, 0., z],
                    [x + 0.125, 0., z + 0.125],
                    [x, 0., z + 0.125],
                ];
                let center = [x + 0.0625, 0., z + 0.0625];
                for i in 0..4 {
                    let base = vertices.len() as u32;
                    vertices.extend([corners[(i + 1) % 4], corners[i], center]);
                    triangles.push([base, base + 1, base + 2]);
                }
            }
        }
        assert_eq!(vertices.len(), 786432);
        assert_eq!(triangles.len(), 262144);
        assert_eq!(
            bake(&config, &vec![[0., 0., 0.]; 1_000_001], &[[0, 1, 2]], &[]),
            Err(NavError::InvalidInput)
        );
        assert_eq!(
            bake(
                &config,
                &[[0., 0., 0.], [1., 0., 0.], [0., 0., 1.]],
                &vec![[0, 1, 2]; 1_000_001],
                &[]
            ),
            Err(NavError::InvalidInput)
        );
        let tile = bake(&config, &vertices, &triangles, &[])
            .expect("the original full-density terrain navigation surface must be admitted");
        validate_tile(&tile).unwrap();
        let mut nav = NativeNav::new(32.).unwrap();
        nav.add_tile(&tile).unwrap();
        let path = nav
            .find_path([2., 0., 16.], [30., 0., 16.], [0.5, 2., 0.5])
            .unwrap();
        assert!((path.points.last().unwrap()[0] - 30.).abs() < 0.01);
    }
    fn cfg(x: i32) -> BakeConfig {
        BakeConfig {
            cell_size: 0.2,
            cell_height: 0.1,
            radius: 0.3,
            height: 1.8,
            climb: 0.3,
            slope: 45.,
            width: 16.,
            min: [x as f32 * 16., -1., 0.],
            max: [(x + 1) as f32 * 16., 4., 16.],
            x,
            z: 0,
        }
    }
    fn tile(x: i32) -> Vec<u8> {
        bake(
            &cfg(x),
            &[
                [-2., 0., -2.],
                [-2., 0., 18.],
                [34., 0., 18.],
                [34., 0., -2.],
            ],
            &[[0, 1, 2], [0, 2, 3]],
            &[],
        )
        .unwrap()
    }
    #[test]
    fn real_recast_bake_cross_tile_detour64_and_salt_lifecycle() {
        assert_eq!(poly_ref_bits(), 64);
        let a = tile(0);
        let b = tile(1);
        validate_tile(&a).unwrap();
        let mut nav = NativeNav::new(16.).unwrap();
        nav.add_tile(&a).unwrap();
        let tb = nav.add_tile(&b).unwrap();
        let path = nav
            .find_path([2., 0., 8.], [30., 0., 8.], [0.5, 2., 0.5])
            .unwrap();
        assert!(path.points.len() >= 2);
        assert!((path.points.last().unwrap()[0] - 30.).abs() < 0.01);
        let end = path.end_reference;
        assert!(nav.valid_reference(end));
        let prepared = tile(0);
        assert_eq!(
            path,
            nav.find_path([2., 0., 8.], [30., 0., 8.], [0.5, 2., 0.5])
                .unwrap()
        );
        drop(prepared);
        nav.remove_tile(tb).unwrap();
        assert!(!nav.valid_reference(end));
        assert_eq!(
            nav.find_path([2., 0., 8.], [30., 0., 8.], [0.5, 2., 0.5]),
            Err(NavError::NotResident)
        );
        let reloaded = nav.add_tile(&b).unwrap();
        assert_ne!(reloaded, tb);
        assert!(!nav.valid_reference(end));
    }
    #[test]
    fn invalid_tile_and_loaded_obstacle_have_real_failure_signals() {
        for bytes in [vec![], vec![0; 128], tile(0)[..64].to_vec()] {
            assert_eq!(validate_tile(&bytes), Err(NavError::InvalidTile));
        }
        let mut nav = NativeNav::new(16.).unwrap();
        nav.add_tile(&tile(0)).unwrap();
        nav.find_path([2., 0., 8.], [14., 0., 8.], [0.5, 2., 0.5])
            .unwrap();
        nav.set_obstacles(&[[0., -1., 0., 16., 1., 16.]]).unwrap();
        assert_eq!(
            nav.find_path([2., 0., 8.], [14., 0., 8.], [0.5, 2., 0.5]),
            Err(NavError::NotResident)
        );
        nav.set_obstacles(&[]).unwrap();
        nav.find_path([2., 0., 8.], [14., 0., 8.], [0.5, 2., 0.5])
            .unwrap();
    }
    #[test]
    fn automatic_surface_step_stays_before_blocked_or_unloaded_polygons() {
        let mut nav = NativeNav::new(16.).unwrap();
        nav.add_tile(&tile(0)).unwrap();
        nav.add_tile(&tile(1)).unwrap();
        let extent = [0.35, 1.8, 0.35];
        let from = [15., 0.85, 8.];
        let to = [17., 0.85, 8.];
        assert!((nav.move_on_surface(from, to, extent).unwrap()[0] - 17.).abs() < 0.001);
        nav.set_obstacles(&[[17., -1., 1., 31., 1., 15.]]).unwrap();
        let constrained = nav.move_on_surface(from, to, extent).unwrap();
        assert!(constrained[0] >= 15. && constrained[0] <= 16.);
        assert!(nav.find_path(constrained, from, extent).is_ok());
        assert_eq!(
            nav.move_on_surface([100., 0., 100.], to, extent),
            Err(NavError::NotResident)
        );
        assert_eq!(
            nav.move_on_surface([f32::NAN, 0., 0.], to, extent),
            Err(NavError::InvalidInput)
        );
        assert_eq!(
            nav.move_on_surface(from, to, [0.; 3]),
            Err(NavError::InvalidInput)
        );
    }
}
