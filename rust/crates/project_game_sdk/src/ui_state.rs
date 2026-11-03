use crate::{
    GameError, GameErrorCode, GameResult, UiBindingSet, UiStateIdentity, UiStateResolveOutput,
    UiStateResolveRequest, Value,
};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_UI_BINDING_PATHS: usize = 4096;
pub const MAX_UI_BINDING_PATH_BYTES: usize = 512;

#[derive(Debug)]
pub struct UiBindingRefreshPlan {
    store_generation: u64,
    binding_set_digest: String,
    replacement_active_paths: Option<Vec<String>>,
    refresh_paths: Vec<String>,
    previous_identity: Option<UiStateIdentity>,
}

impl UiBindingRefreshPlan {
    pub fn refresh_paths(&self) -> &[String] {
        &self.refresh_paths
    }
}

#[derive(Debug)]
pub struct UiBindingStore {
    producer_id: String,
    producer_epoch: u64,
    visible_revision: u64,
    binding_set_digest: Option<String>,
    active_paths: Vec<String>,
    active_path_set: BTreeSet<String>,
    values: BTreeMap<String, Value>,
    dirty_active_paths: BTreeSet<String>,
    force_refresh: bool,
    store_generation: u64,
}

impl UiBindingStore {
    pub fn new(producer_id: impl Into<String>, producer_epoch: u64) -> GameResult<Self> {
        let producer_id = producer_id.into();
        if producer_id.trim() != producer_id || producer_id.is_empty() {
            return Err(invalid(
                "UI binding producer ID must be non-empty and canonical",
            ));
        }
        Ok(Self {
            producer_id,
            producer_epoch,
            visible_revision: 0,
            binding_set_digest: None,
            active_paths: Vec::new(),
            active_path_set: BTreeSet::new(),
            values: BTreeMap::new(),
            dirty_active_paths: BTreeSet::new(),
            force_refresh: false,
            store_generation: 0,
        })
    }

    pub fn visible_revision(&self) -> u64 {
        self.visible_revision
    }

    pub fn active_paths(&self) -> &[String] {
        &self.active_paths
    }

    pub fn notify(&mut self, path: &str) -> GameResult<bool> {
        validate_path(path)?;
        if !self.active_path_set.contains(path) {
            return Ok(false);
        }
        if self.dirty_active_paths.contains(path) {
            return Ok(false);
        }
        self.bump_store_generation()?;
        self.dirty_active_paths.insert(path.to_owned());
        Ok(true)
    }

    pub fn notify_all(&mut self) -> GameResult<bool> {
        if !self.active_paths.is_empty() && !self.force_refresh {
            self.bump_store_generation()?;
            self.force_refresh = true;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn prepare_resolve(
        &self,
        request: &UiStateResolveRequest,
    ) -> GameResult<UiBindingRefreshPlan> {
        let (binding_set_digest, replacement_active_paths) = match &request.binding_set {
            UiBindingSet::Known { digest } => {
                validate_digest(digest)?;
                if self.binding_set_digest.as_deref() != Some(digest) {
                    return Err(failed(
                        "UI binding set digest is not registered in this session",
                    ));
                }
                (digest.clone(), None)
            }
            UiBindingSet::Replace {
                digest,
                active_binding_paths,
            } => {
                validate_digest(digest)?;
                validate_active_paths(active_binding_paths)?;
                if self.binding_set_digest.as_deref() == Some(digest)
                    && self.active_paths != *active_binding_paths
                {
                    return Err(failed(
                        "UI binding set digest was reused for a different active path list",
                    ));
                }
                let changed = self.binding_set_digest.as_deref() != Some(digest);
                (
                    digest.clone(),
                    changed.then(|| active_binding_paths.clone()),
                )
            }
        };

        if let Some(previous) = &request.previous_identity {
            if previous.producer_epoch == self.producer_epoch
                && previous.binding_set_digest == binding_set_digest
                && previous.visible_revision > self.visible_revision
            {
                return Err(failed(
                    "UI binding caller identity revision is newer than the project session",
                ));
            }
        }

        let refresh_paths = if let Some(active_paths) = &replacement_active_paths {
            active_paths.clone()
        } else if self.force_refresh {
            self.active_paths.clone()
        } else {
            self.dirty_active_paths.iter().cloned().collect()
        };
        Ok(UiBindingRefreshPlan {
            store_generation: self.store_generation,
            binding_set_digest,
            replacement_active_paths,
            refresh_paths,
            previous_identity: request.previous_identity.clone(),
        })
    }

    pub fn finish_resolve(
        &mut self,
        plan: UiBindingRefreshPlan,
        refreshed: impl IntoIterator<Item = (String, Option<Value>)>,
    ) -> GameResult<UiStateResolveOutput> {
        if plan.store_generation != self.store_generation {
            return Err(failed(
                "UI binding refresh plan is stale; prepare a new plan after notifications",
            ));
        }

        let expected = plan.refresh_paths.iter().cloned().collect::<BTreeSet<_>>();
        let mut supplied = BTreeMap::new();
        for (path, value) in refreshed {
            validate_path(&path)?;
            if !expected.contains(&path) {
                return Err(invalid(format!(
                    "UI binding refresh returned unexpected path '{path}'"
                )));
            }
            if supplied.insert(path.clone(), value).is_some() {
                return Err(invalid(format!(
                    "UI binding refresh returned duplicate path '{path}'"
                )));
            }
        }
        if supplied.len() != expected.len() {
            let missing = expected
                .iter()
                .find(|path| !supplied.contains_key(*path))
                .cloned()
                .unwrap_or_else(|| "<unknown>".into());
            return Err(invalid(format!(
                "UI binding refresh omitted expected path '{missing}'"
            )));
        }

        let binding_set_changed = plan.replacement_active_paths.is_some();
        if !binding_set_changed && plan.refresh_paths.is_empty() {
            let identity = UiStateIdentity {
                producer_epoch: self.producer_epoch,
                visible_revision: self.visible_revision,
                binding_set_digest: plan.binding_set_digest,
            };
            return if plan.previous_identity.as_ref() == Some(&identity) {
                Ok(UiStateResolveOutput::Reuse { identity })
            } else {
                Ok(UiStateResolveOutput::Replace {
                    identity,
                    producer_id: self.producer_id.clone(),
                    values: self.values.clone(),
                })
            };
        }

        let mut next_values = if binding_set_changed {
            BTreeMap::new()
        } else {
            self.values.clone()
        };
        for path in &plan.refresh_paths {
            match supplied.remove(path).expect("validated refresh result") {
                Some(value) => {
                    next_values.insert(path.clone(), value);
                }
                None => {
                    next_values.remove(path);
                }
            }
        }
        let replacement_active_path_set = plan
            .replacement_active_paths
            .as_ref()
            .map(|paths| paths.iter().cloned().collect::<BTreeSet<_>>());
        if let Some(active_path_set) = &replacement_active_path_set {
            next_values.retain(|path, _| active_path_set.contains(path));
        }
        let values_changed = self.values != next_values;
        let next_visible_revision = if values_changed {
            self.visible_revision
                .checked_add(1)
                .ok_or_else(|| failed("UI binding visible revision overflowed"))?
        } else {
            self.visible_revision
        };
        let next_store_generation = self
            .store_generation
            .checked_add(1)
            .ok_or_else(|| failed("UI binding store generation overflowed"))?;

        self.binding_set_digest = Some(plan.binding_set_digest.clone());
        if let Some(active_paths) = plan.replacement_active_paths {
            self.active_paths = active_paths;
            self.active_path_set = replacement_active_path_set.expect("replacement path set");
        }
        self.values = next_values;
        self.visible_revision = next_visible_revision;
        if binding_set_changed {
            self.dirty_active_paths.clear();
        } else {
            for path in &plan.refresh_paths {
                self.dirty_active_paths.remove(path);
            }
        }
        self.force_refresh = false;
        self.store_generation = next_store_generation;

        let identity = UiStateIdentity {
            producer_epoch: self.producer_epoch,
            visible_revision: self.visible_revision,
            binding_set_digest: plan.binding_set_digest,
        };
        if !binding_set_changed
            && !values_changed
            && plan.previous_identity.as_ref() == Some(&identity)
        {
            Ok(UiStateResolveOutput::Reuse { identity })
        } else {
            Ok(UiStateResolveOutput::Replace {
                identity,
                producer_id: self.producer_id.clone(),
                values: self.values.clone(),
            })
        }
    }

    fn bump_store_generation(&mut self) -> GameResult<()> {
        self.store_generation = self
            .store_generation
            .checked_add(1)
            .ok_or_else(|| failed("UI binding store generation overflowed"))?;
        Ok(())
    }
}

fn validate_active_paths(paths: &[String]) -> GameResult<()> {
    if paths.len() > MAX_UI_BINDING_PATHS {
        return Err(invalid(format!(
            "UI binding set exceeds {MAX_UI_BINDING_PATHS} active paths"
        )));
    }
    let mut previous: Option<&str> = None;
    for path in paths {
        validate_path(path)?;
        if previous.is_some_and(|previous| previous >= path.as_str()) {
            return Err(invalid(
                "UI binding Replace paths must be strictly sorted and unique",
            ));
        }
        previous = Some(path);
    }
    Ok(())
}

fn validate_path(path: &str) -> GameResult<()> {
    if path.is_empty() || path.trim() != path || path.len() > MAX_UI_BINDING_PATH_BYTES {
        return Err(invalid(format!(
            "UI binding path must be non-empty, canonical and at most {MAX_UI_BINDING_PATH_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_digest(digest: &str) -> GameResult<()> {
    if digest.is_empty() || digest.trim() != digest {
        return Err(invalid(
            "UI binding set digest must be non-empty and canonical",
        ));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> GameError {
    GameError::new(GameErrorCode::InvalidArgument, message)
}

fn failed(message: impl Into<String>) -> GameError {
    GameError::new(GameErrorCode::Failed, message)
}

#[cfg(test)]
mod tests {
    use super::{UiBindingStore, MAX_UI_BINDING_PATHS};
    use crate::{
        FrameRequest, GameErrorCode, UiBindingSet, UiStateIdentity, UiStateResolveOutput,
        UiStateResolveRequest, Value,
    };
    use std::collections::BTreeMap;

    fn replace_request(paths: &[&str], digest: &str) -> UiStateResolveRequest {
        UiStateResolveRequest {
            frame: FrameRequest {
                frame_index: 1,
                time: Default::default(),
            },
            previous_identity: None,
            binding_set: UiBindingSet::Replace {
                digest: digest.into(),
                active_binding_paths: paths.iter().map(|path| (*path).into()).collect(),
            },
        }
    }

    fn known_request(identity: UiStateIdentity) -> UiStateResolveRequest {
        UiStateResolveRequest {
            frame: FrameRequest {
                frame_index: 2,
                time: Default::default(),
            },
            binding_set: UiBindingSet::Known {
                digest: identity.binding_set_digest.clone(),
            },
            previous_identity: Some(identity),
        }
    }

    fn identity(output: &UiStateResolveOutput) -> UiStateIdentity {
        match output {
            UiStateResolveOutput::Reuse { identity }
            | UiStateResolveOutput::Replace { identity, .. }
            | UiStateResolveOutput::ReplaceWithVirtualLists { identity, .. } => identity.clone(),
            UiStateResolveOutput::Uncacheable { .. }
            | UiStateResolveOutput::UncacheableWithVirtualLists { .. } => {
                panic!("binding store must always produce cacheable output")
            }
        }
    }

    #[test]
    fn first_resolve_refreshes_only_active_fields_and_clean_resolve_is_constant() {
        let mut store = UiBindingStore::new("fixture.ui", 7).unwrap();
        let first = store
            .prepare_resolve(&replace_request(&["hud.hp", "hud.name"], "sha256:set-a"))
            .unwrap();
        assert_eq!(first.refresh_paths(), ["hud.hp", "hud.name"]);
        let first = store
            .finish_resolve(
                first,
                [
                    ("hud.hp".into(), Some(Value::Integer(3))),
                    ("hud.name".into(), Some(Value::String("城门".into()))),
                ],
            )
            .unwrap();
        let UiStateResolveOutput::Replace { values, .. } = &first else {
            panic!("first binding set must replace")
        };
        assert_eq!(values.len(), 2);

        let clean_generation = store.store_generation;
        let clean = store
            .prepare_resolve(&known_request(identity(&first)))
            .unwrap();
        assert!(clean.replacement_active_paths.is_none());
        assert!(clean.refresh_paths().is_empty());
        assert!(matches!(
            store.finish_resolve(clean, []).unwrap(),
            UiStateResolveOutput::Reuse { .. }
        ));
        assert_eq!(store.store_generation, clean_generation);
    }

    #[test]
    fn notifications_coalesce_refresh_one_active_field_and_ignore_inactive_fields() {
        let mut store = UiBindingStore::new("fixture.ui", 1).unwrap();
        let first = store
            .prepare_resolve(&replace_request(&["hud.hp", "hud.name"], "sha256:set-a"))
            .unwrap();
        let first = store
            .finish_resolve(
                first,
                [
                    ("hud.hp".into(), Some(Value::Integer(3))),
                    ("hud.name".into(), Some(Value::String("城门".into()))),
                ],
            )
            .unwrap();
        let baseline = identity(&first);

        assert!(store.notify("hud.hp").unwrap());
        assert!(!store.notify("hud.hp").unwrap());
        assert!(!store.notify("hud.hidden").unwrap());
        let same = store
            .prepare_resolve(&known_request(baseline.clone()))
            .unwrap();
        assert_eq!(same.refresh_paths(), ["hud.hp"]);
        let same = store
            .finish_resolve(same, [("hud.hp".into(), Some(Value::Integer(3)))])
            .unwrap();
        assert!(matches!(same, UiStateResolveOutput::Reuse { .. }));
        assert_eq!(identity(&same), baseline);

        assert!(store.notify("hud.hp").unwrap());
        let changed = store.prepare_resolve(&known_request(baseline)).unwrap();
        assert_eq!(changed.refresh_paths(), ["hud.hp"]);
        let changed = store
            .finish_resolve(changed, [("hud.hp".into(), Some(Value::Integer(2)))])
            .unwrap();
        let UiStateResolveOutput::Replace {
            identity, values, ..
        } = changed
        else {
            panic!("changed field must replace")
        };
        assert_eq!(identity.visible_revision, 2);
        assert_eq!(values["hud.hp"], Value::Integer(2));
        assert_eq!(values["hud.name"], Value::String("城门".into()));
    }

    #[test]
    fn deletion_and_binding_set_change_publish_complete_active_values() {
        let mut store = UiBindingStore::new("fixture.ui", 2).unwrap();
        let first_plan = store
            .prepare_resolve(&replace_request(&["hud.hp", "hud.name"], "sha256:set-a"))
            .unwrap();
        let first = store
            .finish_resolve(
                first_plan,
                [
                    ("hud.hp".into(), Some(Value::Integer(3))),
                    ("hud.name".into(), Some(Value::String("城门".into()))),
                ],
            )
            .unwrap();
        store.notify("hud.name").unwrap();
        let deletion = store
            .prepare_resolve(&known_request(identity(&first)))
            .unwrap();
        let deletion = store
            .finish_resolve(deletion, [("hud.name".into(), None)])
            .unwrap();
        let UiStateResolveOutput::Replace { values, .. } = &deletion else {
            panic!("deletion must replace")
        };
        assert_eq!(
            *values,
            BTreeMap::from([("hud.hp".into(), Value::Integer(3))])
        );

        let switched = store
            .prepare_resolve(&replace_request(&["hud.hp", "hud.wave"], "sha256:set-b"))
            .unwrap();
        assert_eq!(switched.refresh_paths(), ["hud.hp", "hud.wave"]);
        let switched = store
            .finish_resolve(
                switched,
                [
                    ("hud.hp".into(), Some(Value::Integer(3))),
                    ("hud.wave".into(), Some(Value::Integer(4))),
                ],
            )
            .unwrap();
        let UiStateResolveOutput::Replace {
            identity, values, ..
        } = switched
        else {
            panic!("binding set change must replace")
        };
        assert_eq!(identity.binding_set_digest, "sha256:set-b");
        assert_eq!(values.len(), 2);
        assert!(!values.contains_key("hud.name"));
    }

    #[test]
    fn invalid_or_stale_finish_keeps_dirty_state_and_identity() {
        let mut store = UiBindingStore::new("fixture.ui", 3).unwrap();
        let first = store
            .prepare_resolve(&replace_request(&["hud.hp"], "sha256:set-a"))
            .unwrap();
        let first = store
            .finish_resolve(first, [("hud.hp".into(), Some(Value::Integer(3)))])
            .unwrap();
        let baseline = identity(&first);

        store.notify("hud.hp").unwrap();
        let missing = store
            .prepare_resolve(&known_request(baseline.clone()))
            .unwrap();
        assert_eq!(
            store.finish_resolve(missing, []).unwrap_err().code,
            GameErrorCode::InvalidArgument
        );
        assert_eq!(
            store
                .prepare_resolve(&known_request(baseline.clone()))
                .unwrap()
                .refresh_paths(),
            ["hud.hp"]
        );

        let duplicate = store
            .prepare_resolve(&known_request(baseline.clone()))
            .unwrap();
        assert_eq!(
            store
                .finish_resolve(
                    duplicate,
                    [
                        ("hud.hp".into(), Some(Value::Integer(2))),
                        ("hud.hp".into(), Some(Value::Integer(1))),
                    ],
                )
                .unwrap_err()
                .code,
            GameErrorCode::InvalidArgument
        );

        let stale = store
            .prepare_resolve(&known_request(baseline.clone()))
            .unwrap();
        assert!(store.notify_all().unwrap());
        assert!(!store.notify_all().unwrap());
        assert_eq!(
            store
                .finish_resolve(stale, [("hud.hp".into(), Some(Value::Integer(2)))])
                .unwrap_err()
                .code,
            GameErrorCode::Failed
        );
        assert_eq!(store.visible_revision(), baseline.visible_revision);
    }

    #[test]
    fn binding_registry_and_identity_fail_closed() {
        let mut store = UiBindingStore::new("fixture.ui", 4).unwrap();
        let unknown = UiStateResolveRequest {
            frame: FrameRequest {
                frame_index: 1,
                time: Default::default(),
            },
            previous_identity: None,
            binding_set: UiBindingSet::Known {
                digest: "sha256:unknown".into(),
            },
        };
        assert_eq!(
            store.prepare_resolve(&unknown).unwrap_err().code,
            GameErrorCode::Failed
        );
        assert_eq!(
            store
                .prepare_resolve(&replace_request(&["hud.name", "hud.hp"], "sha256:unsorted",))
                .unwrap_err()
                .code,
            GameErrorCode::InvalidArgument
        );
        let too_many = vec!["x"; MAX_UI_BINDING_PATHS + 1];
        assert_eq!(
            store
                .prepare_resolve(&replace_request(&too_many, "sha256:large"))
                .unwrap_err()
                .code,
            GameErrorCode::InvalidArgument
        );

        let first = store
            .prepare_resolve(&replace_request(&["hud.hp"], "sha256:set-a"))
            .unwrap();
        let first = store
            .finish_resolve(first, [("hud.hp".into(), Some(Value::Integer(3)))])
            .unwrap();
        let mut regressed = identity(&first);
        regressed.visible_revision += 1;
        assert_eq!(
            store
                .prepare_resolve(&known_request(regressed))
                .unwrap_err()
                .code,
            GameErrorCode::Failed
        );

        let mut overflow = UiBindingStore::new("fixture.overflow", 5).unwrap();
        let initial = overflow
            .prepare_resolve(&replace_request(&["hud.hp"], "sha256:overflow"))
            .unwrap();
        overflow
            .finish_resolve(initial, [("hud.hp".into(), Some(Value::Integer(3)))])
            .unwrap();
        overflow.store_generation = u64::MAX;
        assert_eq!(
            overflow.notify("hud.hp").unwrap_err().code,
            GameErrorCode::Failed
        );
        assert!(overflow.dirty_active_paths.is_empty());
        assert_eq!(
            overflow.notify_all().unwrap_err().code,
            GameErrorCode::Failed
        );
        assert!(!overflow.force_refresh);
    }
}
