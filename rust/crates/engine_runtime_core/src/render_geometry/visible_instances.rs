//! One immutable visible-row allocation per view; mutable callers detach only
//! their range. This is CPU packet storage, never a GPU/residency proof.
use super::*;
use std::ops::{Deref, DerefMut, Range};

#[derive(Debug, Clone)]
enum Storage {
    Owned(Vec<GeometryVisibleInstance>),
    Shared {
        rows: Arc<Vec<GeometryVisibleInstance>>,
        range: Range<usize>,
    },
}

#[derive(Debug, Clone)]
pub struct GeometryVisibleInstances(Storage);

impl GeometryVisibleInstances {
    pub(super) fn shared(rows: Arc<Vec<GeometryVisibleInstance>>, range: Range<usize>) -> Self {
        assert!(range.start <= range.end && range.end <= rows.len());
        Self(Storage::Shared { rows, range })
    }

    fn owned_mut(&mut self) -> &mut Vec<GeometryVisibleInstance> {
        if matches!(self.0, Storage::Shared { .. }) {
            self.0 = Storage::Owned(self.to_vec());
        }
        match &mut self.0 {
            Storage::Owned(rows) => rows,
            Storage::Shared { .. } => unreachable!("detached above"),
        }
    }

    pub fn push(&mut self, row: GeometryVisibleInstance) {
        self.owned_mut().push(row);
    }

    pub fn truncate(&mut self, len: usize) {
        if len < self.len() {
            self.owned_mut().truncate(len);
        }
    }

    pub fn retain(&mut self, keep: impl FnMut(&GeometryVisibleInstance) -> bool) {
        self.owned_mut().retain(keep);
    }

    #[cfg(test)]
    pub(super) fn backing_for_test(&self) -> usize {
        match &self.0 {
            Storage::Owned(rows) => rows.as_ptr() as usize,
            Storage::Shared { rows, .. } => rows.as_ptr() as usize,
        }
    }

    pub(super) fn observe_memory(
        &self,
        sample: &mut crate::resource_memory::ResourceMemorySample,
        role: &str,
    ) {
        match &self.0 {
            Storage::Owned(rows) => {
                sample.cpu_vec(rows, "snapshots", &[role]);
            }
            Storage::Shared { rows, .. } => {
                sample.cpu_arc(rows, "snapshots", &[role]);
                sample.cpu_vec(rows, "snapshots", &[role]);
            }
        }
    }
}

impl From<Vec<GeometryVisibleInstance>> for GeometryVisibleInstances {
    fn from(rows: Vec<GeometryVisibleInstance>) -> Self {
        Self(Storage::Owned(rows))
    }
}
impl Deref for GeometryVisibleInstances {
    type Target = [GeometryVisibleInstance];
    fn deref(&self) -> &Self::Target {
        match &self.0 {
            Storage::Owned(rows) => rows,
            Storage::Shared { rows, range } => &rows[range.clone()],
        }
    }
}
impl DerefMut for GeometryVisibleInstances {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.owned_mut().as_mut_slice()
    }
}
impl<'a> IntoIterator for &'a GeometryVisibleInstances {
    type Item = &'a GeometryVisibleInstance;
    type IntoIter = std::slice::Iter<'a, GeometryVisibleInstance>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a> IntoIterator for &'a mut GeometryVisibleInstances {
    type Item = &'a mut GeometryVisibleInstance;
    type IntoIter = std::slice::IterMut<'a, GeometryVisibleInstance>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
impl Serialize for GeometryVisibleInstances {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.deref().serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for GeometryVisibleInstances {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<GeometryVisibleInstance>::deserialize(deserializer).map(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranges_detach_without_changing_neighbors_or_old_packet_and_wire_is_self_contained() {
        for count in [4, 32, 256] {
            let rows = Arc::new(
                (0..count)
                    .map(|i| GeometryVisibleInstance {
                        instance_index: i,
                        camera_depth: (i as f32).into(),
                    })
                    .collect::<Vec<_>>(),
            );
            let a = GeometryVisibleInstances::shared(rows.clone(), 1..3);
            let b = GeometryVisibleInstances::shared(rows.clone(), 0..1);
            let mut changed = a.clone();
            assert_eq!(a.backing_for_test(), b.backing_for_test());
            changed[1].instance_index = u32::MAX;
            assert_eq!(a[1].instance_index, 2);
            assert_eq!(b[0].instance_index, 0);
            assert_ne!(a.backing_for_test(), changed.backing_for_test());
            let wire = serde_json::to_value(&a).unwrap();
            assert_eq!(wire.as_array().unwrap().len(), 2);
            let detached: GeometryVisibleInstances = serde_json::from_value(wire).unwrap();
            assert_eq!(&*detached, &*a);
            drop(rows);
            drop(b);
            assert_eq!(a[1].instance_index, 2);
        }
    }
}
