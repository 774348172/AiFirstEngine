use super::*;
pub use project_runtime_sdk::animator::{AnimatorEvent, AnimatorSnapshot, AnimatorSocket};
use project_runtime_sdk::ProjectRuntimeAnimatorIntent as I;

/// Intents remain part of the containing callback's atomic deferred batch.
pub struct AnimatorWriter<'a> {
    pub(super) writer: &'a mut DeferredMutationWriter,
    pub(super) entity: &'a EntityHandle,
}
impl AnimatorWriter<'_> {
    fn push(&mut self, intent: I) {
        self.writer.mutations.push(DeferredMutation::Animator {
            entity_id: self.entity.identity().into(),
            generation: self.entity.generation(),
            intent,
        });
    }
    pub fn play(&mut self, name: impl Into<String>) {
        self.push(I::Play { name: name.into() });
    }
    pub fn restart(&mut self, name: impl Into<String>) {
        self.push(I::Restart { name: name.into() });
    }
    pub fn resume(&mut self) {
        self.push(I::Resume {});
    }
    pub fn set_paused(&mut self, paused: bool) {
        self.push(I::SetPaused { paused });
    }
    pub fn set_bool(&mut self, name: impl Into<String>, value: bool) {
        self.push(I::SetBool {
            name: name.into(),
            value,
        });
    }
    pub fn set_trigger(&mut self, name: impl Into<String>) {
        self.push(I::SetTrigger { name: name.into() });
    }
    pub fn fade_out_layer(&mut self, layer: impl Into<String>, duration_ticks: u32) {
        self.push(I::FadeOutLayer {
            layer: layer.into(),
            duration_ticks,
        });
    }
    pub fn clear_layer(&mut self, layer: impl Into<String>) {
        self.push(I::ClearLayer {
            layer: layer.into(),
        });
    }
    pub fn set_skin(&mut self, name: impl Into<String>) {
        self.push(I::SetSkin { name: name.into() });
    }
}
pub struct AnimatorReader<'a, W: WorldRead + ?Sized> {
    world: &'a W,
    entity: &'a EntityHandle,
}
pub trait AnimatorReadExt: WorldRead {
    fn animator<'a>(&'a self, entity: &'a EntityHandle) -> AnimatorReader<'a, Self> {
        AnimatorReader::new(self, entity)
    }
}
impl<W: WorldRead + ?Sized> AnimatorReadExt for W {}
impl<'a, W: WorldRead + ?Sized> AnimatorReader<'a, W> {
    pub(super) fn new(world: &'a W, entity: &'a EntityHandle) -> Self {
        Self { world, entity }
    }
    pub fn snapshot(&self) -> GameResult<AnimatorSnapshot> {
        let snapshot = AnimatorSnapshot::from_value(
            self.world.read_component(self.entity, "engine.animator")?,
        )
        .map_err(|e| GameError::new(GameErrorCode::InvalidArgument, e))?;
        if self.entity.generation() == 0 || snapshot.entity_generation != self.entity.generation() {
            return Err(GameError::new(
                GameErrorCode::InvalidArgument,
                "animator.runtime.stale_handle",
            ));
        }
        Ok(snapshot)
    }
    pub fn socket(&self, name: &str) -> GameResult<AnimatorSocket> {
        self.snapshot()?.sockets.remove(name).ok_or_else(|| {
            GameError::new(
                GameErrorCode::InvalidArgument,
                "animator.binding.unknown_socket",
            )
        })
    }
    pub fn events(&self) -> GameResult<Vec<AnimatorEvent>> {
        Ok(self.snapshot()?.events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn animator374_builder_and_dyn_reader_use_live_generations() {
        let entity = EntityHandle::new("robot", 7).unwrap();
        let mut writer = DeferredMutationWriter::default();
        let mut animator = writer.animator(&entity);
        animator.play("idle");
        animator.restart("attack");
        animator.resume();
        animator.set_paused(true);
        animator.set_bool("moving", true);
        animator.set_trigger("fire");
        animator.fade_out_layer("upper", 6);
        animator.clear_layer("upper");
        animator.set_skin("coat");
        assert_eq!(writer.as_slice().len(), 9);
        assert!(writer.as_slice().iter().all(|c|matches!(c,DeferredMutation::Animator{entity_id,generation:7,..} if entity_id=="robot")));
        assert!(matches!(
            &writer.as_slice()[6],
            DeferredMutation::Animator {
                intent: I::FadeOutLayer {
                    duration_ticks: 6,
                    ..
                },
                ..
            }
        ));
        struct Read;
        impl WorldRead for Read {
            fn query(&self, _: &[String], _: &[String]) -> GameResult<Vec<EntityHandle>> {
                Ok(vec![EntityHandle::new("robot", 7)?])
            }
            fn read_component(&self, _: &EntityHandle, kind: &str) -> GameResult<Value> {
                assert_eq!(kind, "engine.animator");
                Ok(AnimatorSnapshot {
                    entity_generation: 7,
                    tick: 11,
                    pose_revision: 12,
                    ..Default::default()
                }
                .to_value())
            }
        }
        let world: &dyn WorldRead = &Read;
        assert_eq!(world.animator(&entity).snapshot().unwrap().tick, 11);
        assert!(world
            .animator(&EntityHandle::new("robot", 8).unwrap())
            .snapshot()
            .is_err());
        assert!(world
            .animator(&EntityHandle::from_stable_identity("robot").unwrap())
            .snapshot()
            .is_err());
    }
}
