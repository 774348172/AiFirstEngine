use super::*;
use crate::three_d_contract::{ImportCoordinates, ImportHandedness, ImportUpAxis};
use engine_runtime::spatial_contract::{CanonicalTransform3D, Rotation3D, Transform3D};
use nalgebra::{Matrix3, Matrix4, UnitQuaternion, Vector3};

pub(super) struct Conversion {
    basis: Matrix3<f64>,
    units: f64,
}
impl Conversion {
    pub fn new(settings: &ImportCoordinates) -> Self {
        let mut basis = match settings.up_axis {
            ImportUpAxis::Y => Matrix3::identity(),
            ImportUpAxis::Z => Matrix3::new(1., 0., 0., 0., 0., 1., 0., -1., 0.),
        };
        if matches!(settings.handedness, ImportHandedness::Left) {
            for col in 0..3 {
                basis[(2, col)] = -basis[(2, col)];
            }
        }
        Self {
            basis,
            units: f64::from(settings.meters_per_unit),
        }
    }
    pub fn reflected(&self) -> bool {
        self.basis.determinant() < 0.
    }
    pub fn vector(&self, v: [f32;3]) -> [f32;3] {
        (self.basis * Vector3::from(v.map(f64::from))).map(|v|v as f32).into()
    }
    pub fn rotation_key(&self, q:[f32;4]) -> [f32;4] {
        let v=self.basis*Vector3::new(f64::from(q[0]),f64::from(q[1]),f64::from(q[2]))*self.basis.determinant();
        [v.x as f32,v.y as f32,v.z as f32,q[3]]
    }
    pub fn scale_key(&self,v:[f32;3])->[f32;3] {
        (self.basis.map(f64::abs)*Vector3::from(v.map(f64::from))).map(|v|v as f32).into()
    }
    pub fn affine(&self,columns:[[f32;4];4])->[f32;16] {
        let m=Matrix4::from_column_slice(&columns.into_iter().flatten().map(f64::from).collect::<Vec<_>>());
        let mut c=Matrix4::identity();c.fixed_view_mut::<3,3>(0,0).copy_from(&(self.basis*self.units));
        let result=c*m*c.try_inverse().unwrap();
        std::array::from_fn(|i|result.as_slice()[i] as f32)
    }
    pub fn position(&self, v: [f32; 3]) -> [f32; 3] {
        (self.basis * Vector3::from(v.map(f64::from)) * self.units)
            .map(|v| v as f32)
            .into()
    }
    pub fn normal(&self, v: [f32; 3], source: &str, path: &str) -> Result<[f32; 3]> {
        let n = self.basis * Vector3::from(v.map(f64::from));
        let length = n.norm();
        if !length.is_finite() || length == 0. {
            return Err(fail(
                source,
                path,
                "import_normal_invalid",
                "Normal must be finite and nonzero.",
            ));
        }
        Ok((n / length).map(|v| v as f32).into())
    }
    pub fn node(&self, node: gltf::Node<'_>, source: &str) -> Result<CanonicalTransform3D> {
        let path = format!(
            "/nodes/{}/{}",
            node.index(),
            if matches!(node.transform(), gltf::scene::Transform::Matrix { .. }) {
                "matrix"
            } else {
                "rotation"
            }
        );
        let invalid = || {
            fail(
                source,
                &path,
                "import_transform_unsupported",
                "Node must have finite, invertible, losslessly decomposable TRS (no shear).",
            )
        };
        if let gltf::scene::Transform::Decomposed { rotation, .. } = node.transform() {
            engine_runtime::spatial_contract::canonical_quaternion(rotation)
                .map_err(|_| invalid())?;
        }
        let columns = node.transform().matrix();
        let m = Matrix4::from_column_slice(
            &columns
                .into_iter()
                .flatten()
                .map(f64::from)
                .collect::<Vec<_>>(),
        );
        if m.iter().any(|v| !v.is_finite())
            || (0..4).any(|i| (m[(3, i)] - if i == 3 { 1. } else { 0. }).abs() > 1e-7)
        {
            return Err(invalid());
        }
        // C*M*C^-1: units only affect translation; geometry is converted separately.
        let linear = self.basis * m.fixed_view::<3, 3>(0, 0) * self.basis.transpose();
        let translation = self.basis * m.fixed_view::<3, 1>(0, 3) * self.units;
        let mut scale = Vector3::new(
            linear.column(0).norm(),
            linear.column(1).norm(),
            linear.column(2).norm(),
        );
        if scale.iter().any(|v| !v.is_finite() || *v == 0.) {
            return Err(invalid());
        }
        if linear.determinant() < 0. {
            scale.x = -scale.x;
        }
        let rotation = Matrix3::from_columns(&[
            linear.column(0) / scale.x,
            linear.column(1) / scale.y,
            linear.column(2) / scale.z,
        ]);
        if (rotation.transpose() * rotation - Matrix3::identity()).amax() > 1e-5 {
            return Err(invalid());
        }
        let q = UnitQuaternion::from_matrix(&rotation);
        if (q.to_rotation_matrix().matrix() * Matrix3::from_diagonal(&scale) - linear).amax()
            > 1e-5 * linear.amax().max(1.)
        {
            return Err(invalid());
        }
        let q = q.quaternion();
        Transform3D {
            schema_version: "transform3d.v1".into(),
            translation: translation.map(|v| v as f32).into(),
            rotation: Rotation3D::Quaternion {
                xyzw: [q.i as f32, q.j as f32, q.k as f32, q.w as f32],
            },
            scale: scale.map(|v| v as f32).into(),
        }
        .canonicalize()
        .map_err(|_| invalid())
    }
}
