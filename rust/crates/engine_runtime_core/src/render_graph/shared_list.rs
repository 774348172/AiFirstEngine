//! Immutable frame products share list backing. Mutable access detaches first;
//! the wire representation remains an ordinary array.
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Weak};

#[derive(PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SharedRenderList<T>(Arc<Vec<T>>);

impl<T> SharedRenderList<T> {
    /// Does not keep elements or their resource leases alive.
    #[doc(hidden)]
    pub fn weak_backing(&self) -> Weak<Vec<T>> {
        Arc::downgrade(&self.0)
    }

    /// Weak ownership prevents allocation-address reuse; mutable access detaches
    /// even when this is the only strong owner of a weak-observed allocation.
    #[doc(hidden)]
    pub fn shares_backing(&self, proof: &Weak<Vec<T>>) -> bool {
        std::ptr::eq(Arc::as_ptr(&self.0), proof.as_ptr())
    }
}

impl<T> Clone for SharedRenderList<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}
impl<T> Default for SharedRenderList<T> {
    fn default() -> Self {
        Self(Arc::new(Vec::new()))
    }
}
impl<T: std::fmt::Debug> std::fmt::Debug for SharedRenderList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl<T> From<Vec<T>> for SharedRenderList<T> {
    fn from(values: Vec<T>) -> Self {
        Self(Arc::new(values))
    }
}
impl<T: Clone> From<SharedRenderList<T>> for Vec<T> {
    fn from(values: SharedRenderList<T>) -> Self {
        Arc::unwrap_or_clone(values.0)
    }
}
impl<T> FromIterator<T> for SharedRenderList<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Vec::from_iter(iter).into()
    }
}
impl<T> Deref for SharedRenderList<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T: Clone> DerefMut for SharedRenderList<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.0)
    }
}
impl<T: Clone> Extend<T> for SharedRenderList<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        Arc::make_mut(&mut self.0).extend(iter);
    }
}
impl<'a, T> IntoIterator for &'a SharedRenderList<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}
impl<'a, T: Clone> IntoIterator for &'a mut SharedRenderList<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        Arc::make_mut(&mut self.0).iter_mut()
    }
}
impl<T: Clone> IntoIterator for SharedRenderList<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        Arc::unwrap_or_clone(self.0).into_iter()
    }
}
