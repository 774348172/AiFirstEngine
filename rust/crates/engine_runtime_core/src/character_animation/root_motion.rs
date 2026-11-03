use super::*;
/// Integrate translation in clip space, preserving complete loop displacement.
/// A seek/play changes the starting clock before this interval is measured.
pub(super) fn translation_interval(
    asset: &CharacterAsset,
    clip: usize,
    joint: usize,
    old: f64,
    new: f64,
    looping: bool,
) -> ContractResult<[f32; 3]> {
    let duration = f64::from(asset.clips[clip].duration);
    let sample = |time: f64| {
        asset
            .sample(clip, time as f32)
            .map(|p| p.joints[joint].translation)
    };
    let (from, to, cycles) = if looping {
        (
            old.rem_euclid(duration),
            new.rem_euclid(duration),
            (new / duration).floor() - (old / duration).floor(),
        )
    } else {
        (old.min(duration), new.min(duration), 0.)
    };
    let a = sample(from)?;
    let b = sample(to)?;
    let start = sample(0.)?;
    let end = sample(duration)?;
    let delta = std::array::from_fn(|i| {
        (f64::from(b[i]) - f64::from(a[i]) + cycles * (f64::from(end[i]) - f64::from(start[i])))
            as f32
    });
    if delta.iter().any(|v| !v.is_finite() || v.abs() > 1000.) {
        return Err(invalid(
            "/rootMotion",
            "Root displacement exceeds the bounded motor request.",
        ));
    }
    Ok(delta)
}
