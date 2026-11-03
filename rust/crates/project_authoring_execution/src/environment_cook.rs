//! Offline IBL derivation in the existing package assembler; never a Runtime source reader.
use super::*;
use engine_runtime::canonical_digest::sha256_prefixed;
use engine_runtime::environment_lighting::{EnvironmentMap, ENVIRONMENT_MAGIC};
use serde::Deserialize;
use std::f64::consts::PI;

const RECIPE: &str = "ibl-cube-sh9-ggx128-lut32x256.v1";
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EnvironmentSource {
    schema_version: String,
    asset_id: String,
    asset_guid: String,
    source_cubemap: String,
    prefilter_face_size: usize,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RadianceCube {
    schema_version: String,
    face_size: usize,
    faces: [Vec<[f64; 3]>; 6],
}
type V = [f64; 3];
fn dot(a: V, b: V) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
fn norm(v: V) -> V {
    let l = dot(v, v).sqrt();
    v.map(|x| x / l)
}
fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn direction(face: usize, u: f64, v: f64) -> V {
    norm(match face {
        0 => [1., -v, -u],
        1 => [-1., -v, u],
        2 => [u, 1., v],
        3 => [u, -1., -v],
        4 => [u, -v, 1.],
        _ => [-u, -v, -1.],
    })
}
fn face_uv(d: V) -> (usize, f64, f64) {
    let [x, y, z] = d;
    let a = d.map(f64::abs);
    if a[0] >= a[1] && a[0] >= a[2] {
        if x >= 0. {
            (0, -z / a[0], -y / a[0])
        } else {
            (1, z / a[0], -y / a[0])
        }
    } else if a[1] >= a[2] {
        if y >= 0. {
            (2, x / a[1], z / a[1])
        } else {
            (3, x / a[1], -z / a[1])
        }
    } else if z >= 0. {
        (4, x / a[2], -y / a[2])
    } else {
        (5, -x / a[2], -y / a[2])
    }
}
impl RadianceCube {
    fn validate(&self) -> Result<(), String> {
        let n = self.face_size;
        if self.schema_version != "cube-radiance.v1"
            || !(2..=1024).contains(&n)
            || !n.is_power_of_two()
            || self.faces.iter().any(|f| {
                f.len() != n * n
                    || f.iter()
                        .flatten()
                        .any(|v| !v.is_finite() || !(0.0..=65504.0).contains(v))
            })
        {
            return Err("environment.source_cube".into());
        }
        Ok(())
    }
    fn texel(&self, face: usize, x: i32, y: i32) -> V {
        let n = self.face_size as i32;
        let (f, x, y) = if (0..n).contains(&x) && (0..n).contains(&y) {
            (face, x, y)
        } else {
            let d = direction(
                face,
                2. * (f64::from(x) + 0.5) / f64::from(n) - 1.,
                2. * (f64::from(y) + 0.5) / f64::from(n) - 1.,
            );
            let (f, u, v) = face_uv(d);
            (
                f,
                (((u + 1.) * 0.5 * f64::from(n)).floor() as i32).clamp(0, n - 1),
                (((v + 1.) * 0.5 * f64::from(n)).floor() as i32).clamp(0, n - 1),
            )
        };
        self.faces[f][(y * n + x) as usize]
    }
    fn sample(&self, d: V) -> V {
        let (f, u, v) = face_uv(d);
        let n = self.face_size as f64;
        let x = (u + 1.) * 0.5 * n - 0.5;
        let y = (v + 1.) * 0.5 * n - 0.5;
        let ix = x.floor() as i32;
        let iy = y.floor() as i32;
        let tx = x - x.floor();
        let ty = y - y.floor();
        let a = self.texel(f, ix, iy);
        let b = self.texel(f, ix + 1, iy);
        let c = self.texel(f, ix, iy + 1);
        let e = self.texel(f, ix + 1, iy + 1);
        std::array::from_fn(|i| {
            (a[i] * (1. - tx) + b[i] * tx) * (1. - ty) + (c[i] * (1. - tx) + e[i] * tx) * ty
        })
    }
}
fn sh(d: V) -> [f64; 9] {
    let [x, y, z] = d;
    [
        0.28209479177387814,
        0.4886025119029199 * y,
        0.4886025119029199 * z,
        0.4886025119029199 * x,
        1.0925484305920792 * x * y,
        1.0925484305920792 * y * z,
        0.31539156525252005 * (3. * z * z - 1.),
        1.0925484305920792 * x * z,
        0.5462742152960396 * (x * x - y * y),
    ]
}
fn radical(i: u32) -> f64 {
    f64::from(i.reverse_bits()) / 4294967296.
}
fn ggx(n: V, rough: f64, x: f64, y: f64) -> V {
    let a = rough * rough;
    let phi = 2. * PI * x;
    let ct = ((1. - y) / (1. + (a * a - 1.) * y)).sqrt();
    let st = (1. - ct * ct).max(0.).sqrt();
    let up = if n[2].abs() < 0.999 {
        [0., 0., 1.]
    } else {
        [1., 0., 0.]
    };
    let t = norm(cross(up, n));
    let b = cross(n, t);
    norm(std::array::from_fn(|i| {
        t[i] * phi.cos() * st + b[i] * phi.sin() * st + n[i] * ct
    }))
}
fn derive(c: &RadianceCube, size: usize) -> Vec<u8> {
    let levels = size.ilog2() + 1;
    let mut words = vec![0u32; 32];
    words[..4].copy_from_slice(&[ENVIRONMENT_MAGIC, size as u32, levels, 32]);
    let mut coeff = [[0.; 3]; 9];
    let area = |u: f64, v: f64| (u * v).atan2((u * u + v * v + 1.).sqrt());
    for f in 0..6 {
        for y in 0..c.face_size {
            for x in 0..c.face_size {
                let n = c.face_size as f64;
                let u = 2. * (x as f64 + 0.5) / n - 1.;
                let v = 2. * (y as f64 + 0.5) / n - 1.;
                let lo = [2. * x as f64 / n - 1., 2. * y as f64 / n - 1.];
                let hi = [lo[0] + 2. / n, lo[1] + 2. / n];
                let weight = area(hi[0], hi[1]) - area(lo[0], hi[1]) - area(hi[0], lo[1])
                    + area(lo[0], lo[1]);
                let basis = sh(direction(f, u, v));
                let color = c.faces[f][y * c.face_size + x];
                for j in 0..9 {
                    for k in 0..3 {
                        coeff[j][k] += color[k] * basis[j] * weight;
                    }
                }
            }
        }
    }
    for j in 0..9 {
        let convolution = if j == 0 {
            1.
        } else if j < 4 {
            2. / 3.
        } else {
            0.25
        };
        for k in 0..3 {
            words[4 + j * 3 + k] = ((coeff[j][k] * convolution) as f32).to_bits();
        }
    }
    for level in 0..levels {
        let n = size >> level;
        let rough = f64::from(level) / f64::from(levels - 1);
        for f in 0..6 {
            for y in 0..n {
                for x in 0..n {
                    let normal = direction(
                        f,
                        2. * (x as f64 + 0.5) / n as f64 - 1.,
                        2. * (y as f64 + 0.5) / n as f64 - 1.,
                    );
                    let color = if level == 0 {
                        c.sample(normal)
                    } else {
                        let mut total = [0.; 3];
                        let mut weight = 0.;
                        for i in 0..128 {
                            let h = ggx(normal, rough, f64::from(i) / 128., radical(i));
                            let l = std::array::from_fn(|j| 2. * dot(normal, h) * h[j] - normal[j]);
                            let nl = dot(normal, l).max(0.);
                            if nl > 0. {
                                let radiance = c.sample(l);
                                for j in 0..3 {
                                    total[j] += radiance[j] * nl;
                                }
                                weight += nl;
                            }
                        }
                        total.map(|v| v / weight.max(1e-12))
                    };
                    words.extend(color.map(|v| (v.clamp(0., 65504.) as f32).to_bits()));
                }
            }
        }
    }
    for y in 0..32 {
        for x in 0..32 {
            let rough = (y as f64 + 0.5) / 32.;
            let nv = (x as f64 + 0.5) / 32.;
            let v = [(1. - nv * nv).sqrt(), 0., nv];
            let mut sum = [0.; 2];
            for i in 0..256 {
                let h = ggx([0., 0., 1.], rough, f64::from(i) / 256., radical(i));
                let vh = dot(v, h).max(0.);
                let l: V = std::array::from_fn(|j| 2. * vh * h[j] - v[j]);
                let nl = l[2].max(0.);
                let nh = h[2].max(0.);
                if nl > 0. {
                    let k = rough * rough / 2.;
                    let g = (nv / (nv * (1. - k) + k)) * (nl / (nl * (1. - k) + k));
                    let visibility = g * vh / (nh * nv).max(1e-12);
                    let fc = (1. - vh).powi(5);
                    sum[0] += (1. - fc) * visibility;
                    sum[1] += fc * visibility;
                }
            }
            words.extend(sum.map(|v| ((v / 256.) as f32).to_bits()));
        }
    }
    words.into_iter().flat_map(u32::to_le_bytes).collect()
}

pub(super) fn cook(
    path: &str,
    document: &serde_json::Value,
    source: &CompilerSourceView,
    asset: &mut RuntimePackageSourceAsset,
    cache: Option<&ProjectAssemblyArtifactCache>,
    reports: &mut Vec<ProjectAssemblyProducerReport>,
) -> Result<(), GameProjectCompilerError> {
    let started = std::time::Instant::now();
    let invalid = |m: String| assembly_error("environment.invalid", format!("{path}: {m}"));
    let descriptor: EnvironmentSource =
        serde_json::from_value(document.clone()).map_err(|e| invalid(e.to_string()))?;
    if descriptor.schema_version != "environment-asset.v1"
        || descriptor.asset_id.trim().is_empty()
        || descriptor.asset_guid.trim().is_empty()
        || !(2..=64).contains(&descriptor.prefilter_face_size)
        || !descriptor.prefilter_face_size.is_power_of_two()
    {
        return Err(invalid(
            "descriptor or derived face size unsupported".into(),
        ));
    }
    crate::ProjectRelativePath::parse(&descriptor.source_cubemap)
        .map_err(|e| invalid(format!("{e:?}")))?;
    let bytes = source
        .bytes(&descriptor.source_cubemap)
        .ok_or_else(|| invalid("source cubemap missing".into()))?;
    if bytes.len() > 192 * 1024 * 1024 {
        return Err(invalid("source byte limit".into()));
    }
    let dependency = sha256_prefixed(&[source.bytes(path).unwrap(), bytes].concat());
    let key = sha256_prefixed(format!("{RECIPE}|{dependency}").as_bytes());
    let mut report = ProjectAssemblyProducerReport::uncached("environment-cook", 0);
    report.producer_recipe_version = RECIPE.into();
    report.recipe_key = Some(key.clone());
    let lookup_started = std::time::Instant::now();
    let cached = cache.and_then(|cache| {
        let lookup = cache.lookup_bytes("environment-cook", &key, RECIPE);
        report.cache_status = lookup.status;
        report.miss_reason = lookup.reason;
        report.artifact_path = lookup.artifact_path.map(|p| p.display().to_string());
        let (Some(value), Some(envelope)) = (lookup.artifact, lookup.envelope) else {
            return None;
        };
        if envelope.dependency_digest == dependency
            && envelope.output_digest == sha256_prefixed(&value)
            && EnvironmentMap::from_bytes(&value).is_ok()
        {
            Some(value)
        } else {
            let _ = cache.quarantine("environment-cook", &key);
            None
        }
    });
    report.lookup_duration_ms = lookup_started.elapsed().as_millis() as u64;
    let produce_started = std::time::Instant::now();
    let output = if let Some(v) = cached {
        v
    } else {
        let cube: RadianceCube =
            serde_json::from_slice(bytes).map_err(|e| invalid(e.to_string()))?;
        cube.validate().map_err(&invalid)?;
        let result = derive(&cube, descriptor.prefilter_face_size);
        EnvironmentMap::from_bytes(&result).map_err(&invalid)?;
        if let Some(cache) = cache {
            let published = cache
                .publish_bytes(
                    "environment-cook",
                    RECIPE,
                    &key,
                    &dependency,
                    &sha256_prefixed(&result),
                    &result,
                )
                .map_err(|e| invalid(format!("{e:?}")))?;
            report.cache_status = match published.status {
                crate::ProjectAssemblyArtifactPublishStatus::Produced => {
                    crate::ProjectAssemblyArtifactCacheStatus::Produced
                }
                crate::ProjectAssemblyArtifactPublishStatus::PublishRaceReused => {
                    crate::ProjectAssemblyArtifactCacheStatus::PublishRaceReused
                }
            };
            report.artifact_path = Some(published.artifact_path.display().to_string());
        }
        result
    };
    report.produce_duration_ms = produce_started.elapsed().as_millis() as u64;
    report.duration_ms = started.elapsed().as_millis() as u64;
    report.output_digest = Some(sha256_prefixed(&output));
    asset.hash = report.output_digest.clone();
    asset.runtime_uri = format!("cooked/environments/{}.ibl", asset.asset_id);
    asset.runtime_payload = Some(output);
    reports.push(report);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ibl_brdf_lut_matches_independent_spherical_quadrature() {
        let c = RadianceCube {
            schema_version: "cube-radiance.v1".into(),
            face_size: 2,
            faces: std::array::from_fn(|_| vec![[1.; 3]; 4]),
        };
        let map = EnvironmentMap::from_bytes(&derive(&c, 2)).unwrap();
        let lut = &map.words()[map.words().len() - 2048..];
        // Integrate in half-vector spherical coordinates, without the producer's
        // importance sampler, radical inverse or tangent frame.
        for (x, y) in [(25usize, 9usize), (20, 20), (12, 27)] {
            let nv = (x as f64 + 0.5) / 32.;
            let r = (y as f64 + 0.5) / 32.;
            let alpha2 = r.powi(4);
            let mut expected = [0.; 2];
            let steps = 512;
            for ti in 0..steps {
                let theta = (ti as f64 + 0.5) * PI / (2. * steps as f64);
                let nh = theta.cos();
                let st = theta.sin();
                let distribution = alpha2 / (PI * (nh * nh * (alpha2 - 1.) + 1.).powi(2));
                for pi in 0..steps {
                    let phi = (pi as f64 + 0.5) * 2. * PI / steps as f64;
                    let vh = ((1. - nv * nv).sqrt() * st * phi.cos() + nv * nh).max(0.);
                    let nl = 2. * vh * nh - nv;
                    if nl <= 0. {
                        continue;
                    }
                    let k = r * r / 2.;
                    let geometry = nv / (nv * (1. - k) + k) * nl / (nl * (1. - k) + k);
                    let weight =
                        distribution * geometry * vh / nv * st * PI * PI / (steps * steps) as f64;
                    let fresnel = (1. - vh).powi(5);
                    expected[0] += (1. - fresnel) * weight;
                    expected[1] += fresnel * weight;
                }
            }
            for k in 0..2 {
                let actual = f64::from(f32::from_bits(lut[(y * 32 + x) * 2 + k]));
                assert!(
                    (actual - expected[k]).abs() < 0.025,
                    "{x},{y},{k}: {actual} vs {}",
                    expected[k]
                );
            }
        }
    }
    #[test]
    fn ibl_cook_cache_hits_source_edits_invalidate_and_unsupported_inputs_fail() {
        let path = "Assets/studio.environment.json";
        let descriptor = serde_json::json!({"schemaVersion":"environment-asset.v1","assetId":"studio","assetGuid":"guid-studio","sourceCubemap":"Models/studio.cube","prefilterFaceSize":2});
        let cube = serde_json::json!({"schemaVersion":"cube-radiance.v1","faceSize":2,"faces":vec![vec![[1.,0.5,0.25];4];6]});
        let mut source = CompilerSourceView {
            files: BTreeMap::from([
                (path.into(), serde_json::to_vec(&descriptor).unwrap()),
                (
                    "Models/studio.cube".into(),
                    serde_json::to_vec(&cube).unwrap(),
                ),
            ]),
        };
        let root = std::env::temp_dir().join(format!("aife-371-ibl-cache-{}", std::process::id()));
        let cache = ProjectAssemblyArtifactCache::open(&root).unwrap();
        let run = |source: &CompilerSourceView, document: &serde_json::Value| {
            let mut asset =
                RuntimePackageSourceAsset::new("studio", "Studio", "environment", path, "");
            let mut reports = Vec::new();
            cook(
                path,
                document,
                source,
                &mut asset,
                Some(&cache),
                &mut reports,
            )
            .map(|_| (asset, reports))
        };
        let (a, first) = run(&source, &descriptor).unwrap();
        assert_eq!(
            first[0].cache_status,
            crate::ProjectAssemblyArtifactCacheStatus::Produced
        );
        let (b, second) = run(&source, &descriptor).unwrap();
        assert_eq!(
            second[0].cache_status,
            crate::ProjectAssemblyArtifactCacheStatus::Hit
        );
        assert_eq!(a.runtime_payload, b.runtime_payload);
        let mut changed = cube;
        changed["faces"][0][0][0] = 2.0.into();
        source.files.insert(
            "Models/studio.cube".into(),
            serde_json::to_vec(&changed).unwrap(),
        );
        let (c, third) = run(&source, &descriptor).unwrap();
        assert_ne!(a.hash, c.hash);
        assert_ne!(first[0].recipe_key, third[0].recipe_key);
        for (field, value) in [
            ("prefilterFaceSize", serde_json::json!(128)),
            ("sourceCubemap", serde_json::json!("../escape.cube")),
            ("rotation", serde_json::json!(1)),
        ] {
            let mut bad = descriptor.clone();
            bad[field] = value;
            assert!(run(&source, &bad).is_err());
        }
        source
            .files
            .insert("Models/studio.cube".into(), b"not a cube".to_vec());
        assert!(run(&source, &descriptor).is_err());
        // Cleanup is performed by the construction owner after preserving evidence.
        eprintln!("construction-owned temporary cache: {}", root.display());
    }
    #[test]
    fn ibl_cube_constant_energy_sh_convolution_and_ggx_all_mips() {
        let c = RadianceCube {
            schema_version: "cube-radiance.v1".into(),
            face_size: 8,
            faces: std::array::from_fn(|_| vec![[4., 0.5, 0.18]; 64]),
        };
        c.validate().unwrap();
        let map = EnvironmentMap::from_bytes(&derive(&c, 8)).unwrap();
        let w = map.words();
        for n in [
            [1., 0., 0.],
            [0., 1., 0.],
            [0., 0., -1.],
            norm([1., 2., 3.]),
        ] {
            let b = sh(n);
            for k in 0..3 {
                let v = (0..9)
                    .map(|j| f64::from(f32::from_bits(w[4 + j * 3 + k])) * b[j])
                    .sum::<f64>();
                assert!((v - [4., 0.5, 0.18][k]).abs() < 1e-5);
            }
        }
        let end = w.len() - 32 * 32 * 2;
        for rgb in w[32..end].chunks_exact(3) {
            for k in 0..3 {
                assert!((f32::from_bits(rgb[k]) - [4., 0.5, 0.18][k]).abs() < 1e-5);
            }
        }
        for (face, d) in [
            [1., 0., 0.],
            [-1., 0., 0.],
            [0., 1., 0.],
            [0., -1., 0.],
            [0., 0., 1.],
            [0., 0., -1.],
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(face_uv(d), (face, 0., 0.));
        }
    }
    #[test]
    fn ibl_cube_face_orientation_seams_and_roughness_are_not_constant_ambient() {
        let c = RadianceCube {
            schema_version: "cube-radiance.v1".into(),
            face_size: 8,
            faces: std::array::from_fn(|f| {
                (0..64)
                    .map(|i| {
                        let d = direction(
                            f,
                            2. * ((i % 8) as f64 + 0.5) / 8. - 1.,
                            2. * ((i / 8) as f64 + 0.5) / 8. - 1.,
                        );
                        [(d[0] + 1.) * 0.5, (d[1] + 1.) * 0.5, (d[2] + 1.) * 0.5]
                    })
                    .collect()
            }),
        };
        let a = c.sample(norm([1., 0., 1. + 1e-7]));
        let b = c.sample(norm([1. + 1e-7, 0., 1.]));
        for k in 0..3 {
            assert!((a[k] - b[k]).abs() < 1e-5);
        }
        let map = EnvironmentMap::from_bytes(&derive(&c, 8)).unwrap();
        let w = map.words();
        let low = f32::from_bits(w[32 + 3 * (4 * 8 + 4)]);
        let last_offset = 32 + 6 * (64 + 16 + 4) * 3;
        let rough = f32::from_bits(w[last_offset]);
        assert!(low > rough + 0.1, "{low} {rough}");
        let mut bad = c;
        bad.faces[0][0][0] = -1.;
        assert!(bad.validate().is_err());
    }
}
