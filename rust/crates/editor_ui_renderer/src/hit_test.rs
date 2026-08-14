use crate::{HitRegion, UiDrawList, UiPoint};

pub fn hit_test(draw_list: &UiDrawList, point: UiPoint) -> Option<&HitRegion> {
    draw_list
        .hit_regions
        .iter()
        .rev()
        .find(|region| region.enabled && region.rect.contains(point))
}

pub fn hit_test_any(draw_list: &UiDrawList, point: UiPoint) -> Option<&HitRegion> {
    draw_list
        .hit_regions
        .iter()
        .rev()
        .find(|region| region.rect.contains(point))
}
