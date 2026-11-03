//! Internal, bounded transfer for typed setters. Only encoded bytes cross the ABI.
//! This module does not commit edits: the host must first accept the containing callback.

use crate::aui::*;
use crate::aui_extension::{UiExtensionPropertyUpdate, UiExtensionValue};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::BTreeMap;

pub const UI_UPDATE_VERSION: u32 = 7;
pub const MAX_UI_UPDATE_PROPERTIES: usize = 4096;
pub const MAX_UI_UPDATE_TEXT_BYTES: usize = 256 * 1024;
// Includes worst-case JSON escaping of text and per-property handle/enum overhead.
pub const MAX_UI_UPDATE_WIRE_BYTES: usize = 4 * 1024 * 1024;
pub const UI_UPDATE_SCHEMA: &str = "aui-update.v7;json;version:u32;updates:[handle(instance:u64,slot:u32,generation:u32),property(text,translation,interactable,visibility,active,value,rotation,scale,pivot,opacity,tint,resource,localStyle),value(text:utf8,translation:[finite-f32;2],bool,visibility:visible|hidden|collapsed,number:finite-f32,vector:[finite-f32;2],color:linear-rgba01,resource:optional(assetId,guid<=128),localStyle:paint-only),notify:bool];customUpdates:[handle,name<=128,value:typed];ownershipUpdates:[handle,property,owner(binding|controller)];items<=4096;text<=262144;wire<=4194304;unique-keys;controller-transfer-requires-value;binding-transfer-forbids-value";

pub trait UiPropertyStage {
    fn stage_properties(&self, batch: UiUpdateBatch) -> Result<(), UiError>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiPropertyUpdate {
    pub handle: UiNodeHandle,
    pub property: UiProperty,
    pub value: UiValue,
    pub notify: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiOwnershipUpdate {
    pub handle: UiNodeHandle,
    pub property: UiProperty,
    pub owner: UiPropertyOwner,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireBatch {
    version: u32,
    #[serde(deserialize_with = "bounded_updates")]
    updates: Vec<UiPropertyUpdate>,
    #[serde(default, deserialize_with = "bounded_custom_updates")]
    custom_updates: Vec<UiExtensionPropertyUpdate>,
    #[serde(default, deserialize_with = "bounded_ownership_updates")]
    ownership_updates: Vec<UiOwnershipUpdate>,
}

fn bounded_updates<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<UiPropertyUpdate>, D::Error> {
    struct Visitor;
    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = Vec<UiPropertyUpdate>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("at most 4096 UI property updates")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut values = Vec::new();
            let mut text_bytes = 0;
            while let Some(value) = seq.next_element::<UiPropertyUpdate>()? {
                text_bytes += text_size(&value.value);
                if values.len() == MAX_UI_UPDATE_PROPERTIES || text_bytes > MAX_UI_UPDATE_TEXT_BYTES
                {
                    return Err(serde::de::Error::custom("UI update capacity exceeded"));
                }
                values.push(value);
            }
            Ok(values)
        }
    }
    deserializer.deserialize_seq(Visitor)
}

fn bounded_custom_updates<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<UiExtensionPropertyUpdate>, D::Error> {
    struct Visitor;
    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = Vec<UiExtensionPropertyUpdate>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("at most 4096 custom UI property updates")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut values = Vec::new();
            while let Some(value) = seq.next_element::<UiExtensionPropertyUpdate>()? {
                if values.len() == MAX_UI_UPDATE_PROPERTIES {
                    return Err(serde::de::Error::custom("UI update capacity exceeded"));
                }
                values.push(value);
            }
            Ok(values)
        }
    }
    deserializer.deserialize_seq(Visitor)
}

fn bounded_ownership_updates<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<UiOwnershipUpdate>, D::Error> {
    struct Visitor;
    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = Vec<UiOwnershipUpdate>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("at most 4096 UI property ownership updates")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut values = Vec::new();
            while let Some(value) = seq.next_element::<UiOwnershipUpdate>()? {
                if values.len() == MAX_UI_UPDATE_PROPERTIES {
                    return Err(serde::de::Error::custom("UI update capacity exceeded"));
                }
                values.push(value);
            }
            Ok(values)
        }
    }
    deserializer.deserialize_seq(Visitor)
}

/// Validated wire batch. Live-handle, node-type and binding ownership checks remain with the host.
#[derive(Debug)]
pub struct UiUpdateBatch(WireBatch);

impl UiUpdateBatch {
    pub fn new(updates: Vec<UiPropertyUpdate>) -> Result<Self, UiError> {
        Self::new_with_custom(updates, Vec::new())
    }

    pub fn new_with_custom(
        updates: Vec<UiPropertyUpdate>,
        custom_updates: Vec<UiExtensionPropertyUpdate>,
    ) -> Result<Self, UiError> {
        Self::new_with_custom_and_ownership(updates, custom_updates, Vec::new())
    }

    pub fn new_with_custom_and_ownership(
        updates: Vec<UiPropertyUpdate>,
        custom_updates: Vec<UiExtensionPropertyUpdate>,
        ownership_updates: Vec<UiOwnershipUpdate>,
    ) -> Result<Self, UiError> {
        let batch = Self(WireBatch {
            version: UI_UPDATE_VERSION,
            updates,
            custom_updates,
            ownership_updates,
        });
        batch.validate()?;
        Ok(batch)
    }

    pub fn updates(&self) -> &[UiPropertyUpdate] {
        &self.0.updates
    }

    pub fn custom_updates(&self) -> &[UiExtensionPropertyUpdate] {
        &self.0.custom_updates
    }

    pub fn ownership_updates(&self) -> &[UiOwnershipUpdate] {
        &self.0.ownership_updates
    }

    pub fn encode(&self) -> Result<Vec<u8>, UiError> {
        let bytes = serde_json::to_vec(&self.0).map_err(|_| failure(UiErrorKind::InvalidValue))?;
        if bytes.len() > MAX_UI_UPDATE_WIRE_BYTES {
            return Err(failure(UiErrorKind::CapacityExceeded));
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, UiError> {
        if bytes.len() > MAX_UI_UPDATE_WIRE_BYTES {
            return Err(failure(UiErrorKind::CapacityExceeded));
        }
        let wire = serde_json::from_slice(bytes).map_err(|_| failure(UiErrorKind::InvalidValue))?;
        let batch = Self(wire);
        batch.validate()?;
        Ok(batch)
    }

    fn validate(&self) -> Result<(), UiError> {
        if self.0.version != UI_UPDATE_VERSION {
            return Err(failure(UiErrorKind::InvalidValue));
        }
        if self
            .0
            .updates
            .len()
            .saturating_add(self.0.custom_updates.len())
            .saturating_add(self.0.ownership_updates.len())
            > MAX_UI_UPDATE_PROPERTIES
        {
            return Err(failure(UiErrorKind::CapacityExceeded));
        }
        let mut keys = std::collections::BTreeSet::new();
        let mut bytes = 0;
        for update in &self.0.updates {
            validate_value(update.handle, update.property, &update.value)?;
            if !keys.insert((update.handle, update.property)) {
                return Err(failure(UiErrorKind::InvalidValue));
            }
            bytes += text_size(&update.value);
            if bytes > MAX_UI_UPDATE_TEXT_BYTES {
                return Err(failure(UiErrorKind::CapacityExceeded));
            }
        }
        let mut custom_keys = std::collections::BTreeSet::new();
        for update in &self.0.custom_updates {
            if update.name.is_empty()
                || update.name.len() > crate::aui_extension::MAX_UI_EXTENSION_IDENTIFIER_BYTES
                || !custom_keys.insert((update.handle, update.name.as_str()))
            {
                return Err(failure(UiErrorKind::InvalidValue));
            }
            validate_extension_value(update.handle, &update.value)?;
            bytes += extension_text_size(&update.value);
            if bytes > MAX_UI_UPDATE_TEXT_BYTES {
                return Err(failure(UiErrorKind::CapacityExceeded));
            }
        }
        let mut ownership_keys = std::collections::BTreeSet::new();
        for update in &self.0.ownership_updates {
            if !ownership_keys.insert((update.handle, update.property)) {
                return Err(failure(UiErrorKind::InvalidValue));
            }
            let has_value = keys.contains(&(update.handle, update.property));
            if (update.owner == UiPropertyOwner::Controller) != has_value {
                return Err(failure(UiErrorKind::OwnershipValueRequired));
            }
        }
        Ok(())
    }
}

/// Callback-local sparse setter buffer, consumed by Glue, never handwritten by gameplay code.
/// The reader must describe stable node identities for the lifetime of this callback.
/// Dropping the buffer has no host-side effect. A failed write poisons the entire segment.
pub struct UiWriteBuffer<'a, R: UiRead + ?Sized> {
    reader: &'a R,
    kinds: RefCell<BTreeMap<UiNodeHandle, UiNodeType>>,
    pending: BTreeMap<(UiNodeHandle, UiProperty), (UiValue, bool)>,
    custom_pending: BTreeMap<(UiNodeHandle, String), UiExtensionValue>,
    ownership_pending: BTreeMap<(UiNodeHandle, UiProperty), UiPropertyOwner>,
    text_bytes: usize,
    failure: Option<UiError>,
}

impl<'a, R: UiRead + ?Sized> UiWriteBuffer<'a, R> {
    pub fn new(reader: &'a R) -> Self {
        Self {
            reader,
            kinds: RefCell::new(BTreeMap::new()),
            pending: BTreeMap::new(),
            custom_pending: BTreeMap::new(),
            ownership_pending: BTreeMap::new(),
            text_bytes: 0,
            failure: None,
        }
    }

    pub fn finish(self) -> Result<UiUpdateBatch, UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        UiUpdateBatch::new_with_custom_and_ownership(
            self.pending
                .into_iter()
                .map(|((handle, property), (value, notify))| UiPropertyUpdate {
                    handle,
                    property,
                    value,
                    notify,
                })
                .collect(),
            self.custom_pending
                .into_iter()
                .map(|((handle, name), value)| UiExtensionPropertyUpdate {
                    handle,
                    name,
                    value,
                })
                .collect(),
            self.ownership_pending
                .into_iter()
                .map(|((handle, property), owner)| UiOwnershipUpdate {
                    handle,
                    property,
                    owner,
                })
                .collect(),
        )
    }

    pub fn set_custom_property(
        &mut self,
        target: UiNodeRef,
        name: impl Into<String>,
        value: UiExtensionValue,
    ) -> Result<(), UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        let handle = target.handle();
        let name = name.into();
        let result = (|| {
            self.node_type(handle)?;
            if name.is_empty()
                || name.len() > crate::aui_extension::MAX_UI_EXTENSION_IDENTIFIER_BYTES
            {
                return Err(UiError {
                    kind: UiErrorKind::InvalidValue,
                    handle: Some(handle),
                    property: None,
                });
            }
            validate_extension_value(handle, &value)?;
            let key = (handle, name);
            let old_bytes = self.custom_pending.get(&key).map_or(0, extension_text_size);
            let bytes = self.text_bytes - old_bytes + extension_text_size(&value);
            if bytes > MAX_UI_UPDATE_TEXT_BYTES
                || (!self.custom_pending.contains_key(&key)
                    && self.pending.len()
                        + self.custom_pending.len()
                        + self.ownership_pending.len()
                        == MAX_UI_UPDATE_PROPERTIES)
            {
                return Err(UiError {
                    kind: UiErrorKind::CapacityExceeded,
                    handle: Some(handle),
                    property: None,
                });
            }
            self.custom_pending.insert(key, value);
            self.text_bytes = bytes;
            Ok(())
        })();
        if let Err(error) = result {
            self.pending.clear();
            self.custom_pending.clear();
            self.ownership_pending.clear();
            self.kinds.get_mut().clear();
            self.text_bytes = 0;
            self.failure = Some(error);
            return Err(error);
        }
        Ok(())
    }

    fn stage(
        &mut self,
        handle: UiNodeHandle,
        property: UiProperty,
        value: UiValue,
        notify: bool,
    ) -> Result<(), UiError> {
        validate_value(handle, property, &value)?;
        let kind = self.node_type(handle)?;
        if (property == UiProperty::Text && kind != UiNodeType::Text)
            || (property == UiProperty::Value && kind != UiNodeType::Slider)
        {
            return Err(UiError {
                kind: UiErrorKind::WrongNodeType,
                handle: Some(handle),
                property: Some(property),
            });
        }
        let key = (handle, property);
        if self.ownership_pending.get(&key) == Some(&UiPropertyOwner::Binding) {
            return Err(UiError {
                kind: UiErrorKind::OwnershipValueRequired,
                handle: Some(handle),
                property: Some(property),
            });
        }
        let bytes = self.text_bytes
            - self
                .pending
                .get(&key)
                .map_or(0, |(value, _)| text_size(value))
            + text_size(&value);
        if bytes > MAX_UI_UPDATE_TEXT_BYTES
            || (!self.pending.contains_key(&key)
                && self.pending.len() + self.custom_pending.len() + self.ownership_pending.len()
                    == MAX_UI_UPDATE_PROPERTIES)
        {
            return Err(UiError {
                kind: UiErrorKind::CapacityExceeded,
                handle: Some(handle),
                property: Some(property),
            });
        }
        self.pending.insert(key, (value, notify));
        self.text_bytes = bytes;
        Ok(())
    }
}

impl<R: UiRead + ?Sized> UiOwnershipWrite for UiWriteBuffer<'_, R> {
    fn transfer_property_ownership(
        &mut self,
        handle: UiNodeHandle,
        property: UiProperty,
        owner: UiPropertyOwner,
    ) -> Result<(), UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        let result = (|| {
            self.node_type(handle)?;
            let key = (handle, property);
            if owner == UiPropertyOwner::Binding && self.pending.contains_key(&key) {
                return Err(UiError {
                    kind: UiErrorKind::OwnershipValueRequired,
                    handle: Some(handle),
                    property: Some(property),
                });
            }
            if !self.ownership_pending.contains_key(&key)
                && self.pending.len() + self.custom_pending.len() + self.ownership_pending.len()
                    == MAX_UI_UPDATE_PROPERTIES
            {
                return Err(UiError {
                    kind: UiErrorKind::CapacityExceeded,
                    handle: Some(handle),
                    property: Some(property),
                });
            }
            self.ownership_pending.insert(key, owner);
            Ok(())
        })();
        if let Err(error) = result {
            self.pending.clear();
            self.custom_pending.clear();
            self.ownership_pending.clear();
            self.kinds.get_mut().clear();
            self.text_bytes = 0;
            self.failure = Some(error);
            return Err(error);
        }
        Ok(())
    }
}

// Geometry queries deliberately read the last committed/published host state.
// Staged writes are visible to property reads, but never form a half-accepted matrix.
impl<R: UiRead + UiGeometryAccess + ?Sized> UiGeometryAccess for UiWriteBuffer<'_, R> {
    fn local_to_screen(&self, target: UiNodeHandle, point: [f32; 2]) -> Result<[f32; 2], UiError> {
        self.reader.local_to_screen(target, point)
    }
    fn screen_to_local(&self, target: UiNodeHandle, point: [f32; 2]) -> Result<[f32; 2], UiError> {
        self.reader.screen_to_local(target, point)
    }
}

impl<R: UiRead + ?Sized> UiRead for UiWriteBuffer<'_, R> {
    fn node_type(&self, handle: UiNodeHandle) -> Result<UiNodeType, UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        if let Some(kind) = self.kinds.borrow().get(&handle) {
            return Ok(*kind);
        }
        if self.kinds.borrow().len() == MAX_UI_UPDATE_PROPERTIES {
            return Err(failure(UiErrorKind::CapacityExceeded));
        }
        let kind = self.reader.node_type(handle)?;
        self.kinds.borrow_mut().insert(handle, kind);
        Ok(kind)
    }

    fn read(&self, handle: UiNodeHandle, property: UiProperty) -> Result<UiValue, UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        match self.pending.get(&(handle, property)) {
            Some((value, _)) => Ok(value.clone()),
            None => self.reader.read(handle, property),
        }
    }
}

impl<R: UiRead + ?Sized> UiWrite for UiWriteBuffer<'_, R> {
    fn write(
        &mut self,
        handle: UiNodeHandle,
        property: UiProperty,
        value: UiValue,
    ) -> Result<(), UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        if let Err(error) = self.stage(handle, property, value, true) {
            self.pending.clear();
            self.custom_pending.clear();
            self.ownership_pending.clear();
            self.kinds.get_mut().clear();
            self.text_bytes = 0;
            self.failure = Some(error);
            return Err(error);
        }
        Ok(())
    }

    fn write_without_notify(
        &mut self,
        handle: UiNodeHandle,
        property: UiProperty,
        value: UiValue,
    ) -> Result<(), UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        if let Err(error) = self.stage(handle, property, value, false) {
            self.pending.clear();
            self.custom_pending.clear();
            self.ownership_pending.clear();
            self.kinds.get_mut().clear();
            self.text_bytes = 0;
            self.failure = Some(error);
            return Err(error);
        }
        Ok(())
    }
}

impl<R: UiAccess + ?Sized> UiWriteBuffer<'_, R> {
    pub fn node(&self, document: &str, node: &str) -> Result<UiNodeRef, UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        self.reader.resolve(document, node).map(UiNodeRef::new)
    }

    pub fn text(&self, document: &str, node: &str) -> Result<UiTextRef, UiError> {
        UiTextRef::try_new(self, self.node(document, node)?.handle())
    }

    pub fn button(&self, document: &str, node: &str) -> Result<UiButtonRef, UiError> {
        UiButtonRef::try_new(self, self.node(document, node)?.handle())
    }

    pub fn slider(&self, document: &str, node: &str) -> Result<UiSliderRef, UiError> {
        UiSliderRef::try_new(self, self.node(document, node)?.handle())
    }
}

impl<R: UiRuntimeAccess + ?Sized> UiWriteBuffer<'_, R> {
    /// Reserve playback in this callback transaction. The host captures base ownership
    /// after accepting all setters, so setters can freely precede or follow this call.
    pub fn play_animation(
        &mut self,
        request: crate::aui_animation::AnimationPlay,
    ) -> Result<crate::aui_animation::AnimationHandle, UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        match self
            .reader
            .animation_request(crate::aui_animation::AnimationRequest::Play { request })?
        {
            crate::aui_animation::AnimationReply::Handle(handle) => Ok(handle),
            _ => Err(failure(UiErrorKind::InvalidValue)),
        }
    }
    pub fn pause_animation(
        &mut self,
        handle: crate::aui_animation::AnimationHandle,
        paused: bool,
    ) -> Result<(), UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        self.reader
            .animation_request(crate::aui_animation::AnimationRequest::SetPaused {
                handle,
                paused,
            })?;
        Ok(())
    }
    pub fn cancel_animation(
        &mut self,
        handle: crate::aui_animation::AnimationHandle,
    ) -> Result<(), UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        self.reader
            .animation_request(crate::aui_animation::AnimationRequest::Cancel { handle })?;
        Ok(())
    }
    pub fn set_game_animation_clock(&mut self, paused: bool, speed: f64) -> Result<(), UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        self.reader
            .animation_request(crate::aui_animation::AnimationRequest::SetGameClock {
                paused,
                speed,
            })?;
        Ok(())
    }
    pub fn flush(&mut self) -> Result<(), UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        if self.pending.is_empty()
            && self.custom_pending.is_empty()
            && self.ownership_pending.is_empty()
        {
            return Ok(());
        }
        let pending = std::mem::take(&mut self.pending);
        let custom_pending = std::mem::take(&mut self.custom_pending);
        let ownership_pending = std::mem::take(&mut self.ownership_pending);
        self.text_bytes = 0;
        let batch = UiUpdateBatch::new_with_custom_and_ownership(
            pending
                .into_iter()
                .map(|((handle, property), (value, notify))| UiPropertyUpdate {
                    handle,
                    property,
                    value,
                    notify,
                })
                .collect(),
            custom_pending
                .into_iter()
                .map(|((handle, name), value)| UiExtensionPropertyUpdate {
                    handle,
                    name,
                    value,
                })
                .collect(),
            ownership_pending
                .into_iter()
                .map(|((handle, property), owner)| UiOwnershipUpdate {
                    handle,
                    property,
                    owner,
                })
                .collect(),
        )?;
        if let Err(error) = self.reader.stage_properties(batch) {
            self.kinds.get_mut().clear();
            self.failure = Some(error);
            return Err(error);
        }
        Ok(())
    }

    pub fn transact_structure(
        &mut self,
        transaction: UiStructureTransaction,
    ) -> Result<UiStructureResult, UiError> {
        self.flush()?;
        match self.reader.transact_structure(transaction) {
            Ok(result) => {
                self.kinds.get_mut().clear();
                Ok(result)
            }
            Err(error) => {
                self.kinds.get_mut().clear();
                self.failure = Some(error);
                Err(error)
            }
        }
    }

    pub fn instantiate(
        &mut self,
        template: impl Into<String>,
        parent: UiNodeRef,
        sibling_index: Option<u32>,
    ) -> Result<UiTemplateInstance, UiError> {
        self.transact_structure(UiStructureTransaction::new(vec![
            UiStructureOperation::Instantiate {
                template: template.into(),
                parent: parent.handle(),
                sibling_index,
            },
        ]))?
        .instantiated
        .into_iter()
        .next()
        .ok_or_else(|| failure(UiErrorKind::InvalidValue))
    }

    pub fn delete(&mut self, target: UiNodeRef) -> Result<(), UiError> {
        self.transact_structure(UiStructureTransaction::new(vec![
            UiStructureOperation::Delete {
                target: target.handle(),
            },
        ]))?;
        Ok(())
    }

    pub fn reparent(
        &mut self,
        target: UiNodeRef,
        new_parent: UiNodeRef,
        sibling_index: Option<u32>,
        mode: UiReparentMode,
    ) -> Result<(), UiError> {
        self.transact_structure(UiStructureTransaction::new(vec![
            UiStructureOperation::Reparent {
                target: target.handle(),
                new_parent: new_parent.handle(),
                sibling_index,
                mode,
            },
        ]))?;
        Ok(())
    }

    pub fn set_sibling_index(
        &mut self,
        target: UiNodeRef,
        sibling_index: u32,
    ) -> Result<(), UiError> {
        self.transact_structure(UiStructureTransaction::new(vec![
            UiStructureOperation::SetSiblingIndex {
                target: target.handle(),
                sibling_index,
            },
        ]))?;
        Ok(())
    }

    pub fn local_to_screen(&self, target: UiNodeRef, point: [f32; 2]) -> Result<[f32; 2], UiError> {
        self.reader.local_to_screen(target.handle(), point)
    }

    pub fn screen_to_local(&self, target: UiNodeRef, point: [f32; 2]) -> Result<[f32; 2], UiError> {
        self.reader.screen_to_local(target.handle(), point)
    }

    pub fn subscribe(
        &self,
        target: UiNodeRef,
        filter: UiEventFilter,
        scope: UiSubscriptionScope,
        callback_id: UiCallbackId,
    ) -> Result<UiSubscriptionToken, UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        self.reader
            .subscribe(target.handle(), filter, scope, callback_id)
    }

    pub fn cancel(&self, token: UiSubscriptionToken) -> Result<bool, UiError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        self.reader.cancel(token)
    }
}

fn extension_text_size(value: &UiExtensionValue) -> usize {
    match value {
        UiExtensionValue::Text(value) | UiExtensionValue::Asset(value) => value.len(),
        _ => 0,
    }
}

fn validate_extension_value(handle: UiNodeHandle, value: &UiExtensionValue) -> Result<(), UiError> {
    let valid = match value {
        UiExtensionValue::Float(value) => value.is_finite(),
        UiExtensionValue::Color(value) => value.iter().all(|channel| channel.is_finite()),
        UiExtensionValue::Asset(value) => !value.is_empty(),
        UiExtensionValue::Bool(_) | UiExtensionValue::Integer(_) | UiExtensionValue::Text(_) => {
            true
        }
    };
    valid.then_some(()).ok_or(UiError {
        kind: UiErrorKind::InvalidValue,
        handle: Some(handle),
        property: None,
    })
}

fn text_size(value: &UiValue) -> usize {
    match value {
        UiValue::Text(value) => value.len(),
        _ => 0,
    }
}

fn failure(kind: UiErrorKind) -> UiError {
    UiError {
        kind,
        handle: None,
        property: None,
    }
}

pub fn validate_value(
    handle: UiNodeHandle,
    property: UiProperty,
    value: &UiValue,
) -> Result<(), UiError> {
    let kind = match (property, value) {
        (UiProperty::Rotation, UiValue::Number(v)) => {
            (!v.is_finite()).then_some(UiErrorKind::InvalidValue)
        }
        (UiProperty::Scale | UiProperty::Pivot, UiValue::Vector(v)) => {
            (!v.iter().all(|v| v.is_finite())).then_some(UiErrorKind::InvalidValue)
        }
        (UiProperty::Opacity, UiValue::Number(v)) => {
            (!v.is_finite() || !(0.0..=1.0).contains(v)).then_some(UiErrorKind::InvalidValue)
        }
        (UiProperty::Tint, UiValue::Color(v)) => crate::aui_presentation::validate_rgba(*v)
            .err()
            .map(|_| UiErrorKind::InvalidValue),
        (UiProperty::LocalStyle, UiValue::LocalStyle(v)) => {
            (!v.validate()).then_some(UiErrorKind::InvalidValue)
        }
        (UiProperty::Resource, UiValue::Resource(v)) => v.as_ref().and_then(|v| {
            (v.asset_id.is_empty()
                || v.asset_id.len() > 128
                || v.guid.is_empty()
                || v.guid.len() > 128)
                .then_some(UiErrorKind::InvalidValue)
        }),
        (UiProperty::Translation, UiValue::Translation(v)) if !v.iter().all(|v| v.is_finite()) => {
            Some(UiErrorKind::InvalidValue)
        }
        (UiProperty::Value, UiValue::Number(value)) if !value.is_finite() => {
            Some(UiErrorKind::InvalidValue)
        }
        (UiProperty::Text, UiValue::Text(_))
        | (UiProperty::Translation, UiValue::Translation(_))
        | (UiProperty::Interactable, UiValue::Bool(_))
        | (UiProperty::Active, UiValue::Bool(_))
        | (UiProperty::Visibility, UiValue::Visibility(_))
        | (UiProperty::Value, UiValue::Number(_)) => None,
        _ => Some(UiErrorKind::WrongValueType),
    };
    match kind {
        Some(kind) => Err(UiError {
            kind,
            handle: Some(handle),
            property: Some(property),
        }),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    const HANDLE: UiNodeHandle = UiNodeHandle {
        instance: 7,
        slot: 2,
        generation: 3,
    };
    struct Reader(Cell<usize>);
    impl UiGeometryAccess for Reader {
        fn local_to_screen(&self, _: UiNodeHandle, point: [f32; 2]) -> Result<[f32; 2], UiError> {
            Ok([point[0] + 100., point[1] + 200.])
        }
        fn screen_to_local(&self, _: UiNodeHandle, point: [f32; 2]) -> Result<[f32; 2], UiError> {
            Ok([point[0] - 100., point[1] - 200.])
        }
    }

    #[test]
    fn callback_buffer_geometry_uses_committed_host_while_properties_read_staged_values() {
        let reader = Reader(Cell::new(0));
        let mut buffer = UiWriteBuffer::new(&reader);
        let node = UiNodeRef::new(HANDLE);
        node.set_translation(&mut buffer, [50., 80.]).unwrap();
        assert_eq!(
            buffer.read(HANDLE, UiProperty::Translation).unwrap(),
            UiValue::Translation([50., 80.])
        );
        let screen = node.local_to_screen(&buffer, [3., 4.]).unwrap();
        assert_eq!(screen, [103., 204.]);
        assert_eq!(node.screen_to_local(&buffer, screen).unwrap(), [3., 4.]);
        assert_eq!(buffer.finish().unwrap().updates().len(), 1);
    }
    impl UiRead for Reader {
        fn node_type(&self, handle: UiNodeHandle) -> Result<UiNodeType, UiError> {
            self.0.set(self.0.get() + 1);
            if handle.instance != 7 {
                return Err(failure(UiErrorKind::InvalidHandle));
            }
            Ok(UiNodeType::Text)
        }
        fn read(&self, _handle: UiNodeHandle, _property: UiProperty) -> Result<UiValue, UiError> {
            Ok(UiValue::Text("人".into()))
        }
    }

    struct SliderReader;
    impl UiRead for SliderReader {
        fn node_type(&self, _handle: UiNodeHandle) -> Result<UiNodeType, UiError> {
            Ok(UiNodeType::Slider)
        }

        fn read(&self, _handle: UiNodeHandle, property: UiProperty) -> Result<UiValue, UiError> {
            match property {
                UiProperty::Value => Ok(UiValue::Number(0.5)),
                _ => Err(UiError {
                    kind: UiErrorKind::WrongValueType,
                    handle: Some(HANDLE),
                    property: Some(property),
                }),
            }
        }
    }
    impl UiAccess for SliderReader {
        fn resolve(&self, document: &str, node: &str) -> Result<UiNodeHandle, UiError> {
            if document == "settings" && node == "speed" {
                Ok(HANDLE)
            } else {
                Err(failure(UiErrorKind::UnknownNode))
            }
        }
    }
    fn update(text: &str) -> UiPropertyUpdate {
        UiPropertyUpdate {
            handle: HANDLE,
            property: UiProperty::Text,
            value: UiValue::Text(text.into()),
            notify: true,
        }
    }

    #[test]
    fn typed_writes_coalesce_without_per_setter_host_calls() {
        let reader = Reader(Cell::new(0));
        let mut buffer = UiWriteBuffer::new(&reader);
        let text = UiTextRef::try_new(&buffer, HANDLE).unwrap();
        for _ in 0..1000 {
            text.set_text(&mut buffer, "骑兵").unwrap();
        }
        assert_eq!(text.text(&buffer).unwrap(), "骑兵");
        assert_eq!(reader.0.get(), 1);
        assert_eq!(
            reader.read(HANDLE, UiProperty::Text).unwrap(),
            UiValue::Text("人".into())
        );
        let batch = buffer.finish().unwrap();
        assert_eq!(batch.updates(), &[update("骑兵")]);
        let bytes = batch.encode().unwrap();
        assert_eq!(
            UiUpdateBatch::decode(&bytes).unwrap().updates(),
            batch.updates()
        );
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["version"], UI_UPDATE_VERSION);
        assert_eq!(json["updates"][0]["property"], "text");
        assert_eq!(json["updates"][0]["value"]["text"], "骑兵");
    }

    #[test]
    fn slider_ref_is_typed_finite_and_preserves_notify_and_ownership() {
        assert_eq!(
            UiSliderRef::try_new(&Reader(Cell::new(0)), HANDLE)
                .unwrap_err()
                .kind,
            UiErrorKind::WrongNodeType
        );

        let reader = SliderReader;
        let mut notified = UiWriteBuffer::new(&reader);
        let slider = notified.slider("settings", "speed").unwrap();
        assert_eq!(slider.value(&notified).unwrap(), 0.5);
        slider
            .node()
            .take_property_ownership(&mut notified, UiProperty::Value)
            .unwrap();
        slider.set_value(&mut notified, 0.75).unwrap();
        let notified = notified.finish().unwrap();
        assert_eq!(notified.updates()[0].value, UiValue::Number(0.75));
        assert!(notified.updates()[0].notify);
        assert_eq!(
            notified.ownership_updates()[0].owner,
            UiPropertyOwner::Controller
        );
        let decoded = UiUpdateBatch::decode(&notified.encode().unwrap()).unwrap();
        assert_eq!(decoded.updates(), notified.updates());

        let mut silent = UiWriteBuffer::new(&reader);
        UiSliderRef::try_new(&silent, HANDLE)
            .unwrap()
            .set_value_without_notify(&mut silent, 0.25)
            .unwrap();
        assert!(!silent.finish().unwrap().updates()[0].notify);

        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut buffer = UiWriteBuffer::new(&reader);
            let slider = UiSliderRef::try_new(&buffer, HANDLE).unwrap();
            assert_eq!(
                slider.set_value(&mut buffer, invalid).unwrap_err().kind,
                UiErrorKind::InvalidValue
            );
            assert_eq!(buffer.finish().unwrap_err().kind, UiErrorKind::InvalidValue);
        }
    }

    #[test]
    fn ownership_transfer_is_bounded_typed_and_requires_an_atomic_controller_value() {
        let reader = Reader(Cell::new(0));
        let mut buffer = UiWriteBuffer::new(&reader);
        let text = UiTextRef::try_new(&buffer, HANDLE).unwrap();
        text.node()
            .take_property_ownership(&mut buffer, UiProperty::Text)
            .unwrap();
        text.set_text_without_notify(&mut buffer, "接管").unwrap();
        let batch = buffer.finish().unwrap();
        assert_eq!(batch.updates().len(), 1);
        assert_eq!(
            batch.ownership_updates(),
            &[UiOwnershipUpdate {
                handle: HANDLE,
                property: UiProperty::Text,
                owner: UiPropertyOwner::Controller,
            }]
        );
        let decoded = UiUpdateBatch::decode(&batch.encode().unwrap()).unwrap();
        assert_eq!(decoded.ownership_updates(), batch.ownership_updates());

        let mut release = UiWriteBuffer::new(&reader);
        text.node()
            .release_property_to_binding(&mut release, UiProperty::Text)
            .unwrap();
        let release = release.finish().unwrap();
        assert!(release.updates().is_empty());
        assert_eq!(
            release.ownership_updates()[0].owner,
            UiPropertyOwner::Binding
        );

        assert_eq!(
            UiUpdateBatch::new_with_custom_and_ownership(
                Vec::new(),
                Vec::new(),
                vec![UiOwnershipUpdate {
                    handle: HANDLE,
                    property: UiProperty::Text,
                    owner: UiPropertyOwner::Controller,
                }],
            )
            .unwrap_err()
            .kind,
            UiErrorKind::OwnershipValueRequired
        );

        let mut invalid_release = UiWriteBuffer::new(&reader);
        text.set_text(&mut invalid_release, "冲突").unwrap();
        assert_eq!(
            text.node()
                .release_property_to_binding(&mut invalid_release, UiProperty::Text)
                .unwrap_err()
                .kind,
            UiErrorKind::OwnershipValueRequired
        );
        assert_eq!(
            invalid_release.finish().unwrap_err().kind,
            UiErrorKind::OwnershipValueRequired
        );
    }

    #[test]
    fn custom_property_writes_coalesce_in_the_same_bounded_batch() {
        let reader = Reader(Cell::new(0));
        let mut buffer = UiWriteBuffer::new(&reader);
        let node = UiNodeRef::new(HANDLE);
        buffer
            .set_custom_property(node, "progress", UiExtensionValue::Float(0.25))
            .unwrap();
        buffer
            .set_custom_property(node, "progress", UiExtensionValue::Float(0.75))
            .unwrap();
        let batch = buffer.finish().unwrap();
        assert!(batch.updates().is_empty());
        assert_eq!(batch.custom_updates().len(), 1);
        assert_eq!(batch.custom_updates()[0].name, "progress");
        assert_eq!(
            batch.custom_updates()[0].value,
            UiExtensionValue::Float(0.75)
        );
        let decoded = UiUpdateBatch::decode(&batch.encode().unwrap()).unwrap();
        assert_eq!(decoded.custom_updates(), batch.custom_updates());
    }

    #[test]
    fn active_round_trips_as_an_independent_bounded_property() {
        let reader = Reader(Cell::new(0));
        let mut buffer = UiWriteBuffer::new(&reader);
        UiNodeRef::new(HANDLE)
            .set_active(&mut buffer, false)
            .unwrap();
        let batch = buffer.finish().unwrap();
        assert_eq!(
            UiUpdateBatch::decode(&batch.encode().unwrap())
                .unwrap()
                .updates(),
            &[UiPropertyUpdate {
                handle: HANDLE,
                property: UiProperty::Active,
                value: UiValue::Bool(false),
                notify: true,
            }]
        );
    }

    #[test]
    fn last_setter_selects_notify_policy_without_extra_host_calls() {
        let reader = Reader(Cell::new(0));
        let mut buffer = UiWriteBuffer::new(&reader);
        let text = UiTextRef::try_new(&buffer, HANDLE).unwrap();
        text.set_text(&mut buffer, "先通知").unwrap();
        text.set_text_without_notify(&mut buffer, "绑定回填")
            .unwrap();
        assert_eq!(text.text(&buffer).unwrap(), "绑定回填");
        let batch = buffer.finish().unwrap();
        assert_eq!(batch.updates().len(), 1);
        assert!(!batch.updates()[0].notify);
        assert_eq!(
            UiUpdateBatch::decode(&batch.encode().unwrap())
                .unwrap()
                .updates(),
            batch.updates()
        );
    }

    #[test]
    fn invalid_write_poison_cannot_be_ignored_by_callback() {
        let reader = Reader(Cell::new(0));
        let mut buffer = UiWriteBuffer::new(&reader);
        buffer
            .write(HANDLE, UiProperty::Text, UiValue::Text("骑".into()))
            .unwrap();
        let error = buffer
            .write(
                HANDLE,
                UiProperty::Translation,
                UiValue::Translation([f32::NAN, 0.]),
            )
            .unwrap_err();
        assert_eq!(error.kind, UiErrorKind::InvalidValue);
        assert_eq!(buffer.read(HANDLE, UiProperty::Text).unwrap_err(), error);
        assert_eq!(
            buffer
                .write(HANDLE, UiProperty::Text, UiValue::Text("修复".into()))
                .unwrap_err(),
            error
        );
        assert_eq!(buffer.finish().unwrap_err(), error);
    }

    #[test]
    fn malformed_versions_fields_duplicates_and_types_are_rejected() {
        let encoded = UiUpdateBatch::new(vec![update("骑")])
            .unwrap()
            .encode()
            .unwrap();
        let original: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        for (field, value) in [
            ("version", serde_json::json!(UI_UPDATE_VERSION + 1)),
            ("extra", serde_json::json!(true)),
        ] {
            let mut bad = original.clone();
            bad[field] = value;
            assert!(UiUpdateBatch::decode(&serde_json::to_vec(&bad).unwrap()).is_err());
        }
        let mut duplicate = original.clone();
        duplicate["updates"]
            .as_array_mut()
            .unwrap()
            .push(original["updates"][0].clone());
        assert!(UiUpdateBatch::decode(&serde_json::to_vec(&duplicate).unwrap()).is_err());
        let mut wrong_type = original;
        wrong_type["updates"][0]["value"] = serde_json::json!({"bool":true});
        assert_eq!(
            UiUpdateBatch::decode(&serde_json::to_vec(&wrong_type).unwrap())
                .unwrap_err()
                .kind,
            UiErrorKind::WrongValueType
        );
        assert!(UiUpdateBatch::decode(b"{\"version\":1,\"updates\":[]} trailing").is_err());
    }

    #[test]
    fn text_budget_counts_utf8_replacements_and_escaped_wire() {
        let reader = Reader(Cell::new(0));
        let mut buffer = UiWriteBuffer::new(&reader);
        buffer
            .write(
                HANDLE,
                UiProperty::Text,
                UiValue::Text("\0".repeat(MAX_UI_UPDATE_TEXT_BYTES)),
            )
            .unwrap();
        let encoded = buffer.finish().unwrap().encode().unwrap();
        assert!(encoded.len() > MAX_UI_UPDATE_TEXT_BYTES * 6);
        assert_eq!(UiUpdateBatch::decode(&encoded).unwrap().updates().len(), 1);
        let mut buffer = UiWriteBuffer::new(&reader);
        for text in [
            "x".repeat(MAX_UI_UPDATE_TEXT_BYTES),
            "骑".into(),
            "x".repeat(MAX_UI_UPDATE_TEXT_BYTES),
        ] {
            buffer
                .write(HANDLE, UiProperty::Text, UiValue::Text(text))
                .unwrap();
        }
        assert!(buffer
            .write(
                HANDLE,
                UiProperty::Text,
                UiValue::Text("骑".repeat(MAX_UI_UPDATE_TEXT_BYTES / 3 + 1))
            )
            .is_err());
        assert!(buffer.finish().is_err());
        assert_eq!(
            UiUpdateBatch::decode(&vec![b' '; MAX_UI_UPDATE_WIRE_BYTES + 1])
                .unwrap_err()
                .kind,
            UiErrorKind::CapacityExceeded
        );
    }

    #[test]
    fn property_limit_and_decoder_limit_are_enforced() {
        let updates: Vec<_> = (0..MAX_UI_UPDATE_PROPERTIES)
            .map(|slot| UiPropertyUpdate {
                handle: UiNodeHandle {
                    slot: slot as u32,
                    ..HANDLE
                },
                ..update("")
            })
            .collect();
        let batch = UiUpdateBatch::new(updates.clone()).unwrap();
        assert_eq!(
            UiUpdateBatch::decode(&batch.encode().unwrap())
                .unwrap()
                .updates()
                .len(),
            MAX_UI_UPDATE_PROPERTIES
        );
        let mut extra = updates;
        extra.push(UiPropertyUpdate {
            handle: UiNodeHandle {
                slot: MAX_UI_UPDATE_PROPERTIES as u32,
                ..HANDLE
            },
            ..update("")
        });
        let raw = serde_json::to_vec(&WireBatch {
            version: UI_UPDATE_VERSION,
            updates: extra.clone(),
            custom_updates: Vec::new(),
            ownership_updates: Vec::new(),
        })
        .unwrap();
        assert!(UiUpdateBatch::decode(&raw).is_err());
        assert_eq!(
            UiUpdateBatch::new(extra).unwrap_err().kind,
            UiErrorKind::CapacityExceeded
        );
    }

    #[test]
    fn buffer_limit_poison_and_read_cache_are_bounded() {
        let reader = Reader(Cell::new(0));
        let mut buffer = UiWriteBuffer::new(&reader);
        for slot in 0..MAX_UI_UPDATE_PROPERTIES / 5 {
            let handle = UiNodeHandle {
                slot: slot as u32,
                ..HANDLE
            };
            for (property, value) in [
                (UiProperty::Text, UiValue::Text("骑".into())),
                (UiProperty::Translation, UiValue::Translation([1., 2.])),
                (UiProperty::Interactable, UiValue::Bool(false)),
                (UiProperty::Active, UiValue::Bool(false)),
                (
                    UiProperty::Visibility,
                    UiValue::Visibility(UiVisibility::Collapsed),
                ),
            ] {
                buffer.write(handle, property, value).unwrap();
            }
        }
        assert_eq!(reader.0.get(), MAX_UI_UPDATE_PROPERTIES / 5);
        let extra = UiNodeHandle {
            slot: MAX_UI_UPDATE_PROPERTIES as u32,
            ..HANDLE
        };
        buffer
            .write(extra, UiProperty::Text, UiValue::Text(String::new()))
            .unwrap();
        assert_eq!(
            buffer
                .write(
                    extra,
                    UiProperty::Translation,
                    UiValue::Translation([0., 0.]),
                )
                .unwrap_err()
                .kind,
            UiErrorKind::CapacityExceeded
        );
        assert!(buffer.finish().is_err());
        let buffer = UiWriteBuffer::new(&reader);
        for slot in 0..MAX_UI_UPDATE_PROPERTIES {
            buffer
                .node_type(UiNodeHandle {
                    slot: slot as u32,
                    ..HANDLE
                })
                .unwrap();
        }
        assert_eq!(
            buffer.node_type(extra).unwrap_err().kind,
            UiErrorKind::CapacityExceeded
        );
        assert!(buffer.finish().unwrap().updates().is_empty());
    }
}
