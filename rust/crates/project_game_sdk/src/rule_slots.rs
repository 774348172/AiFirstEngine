//! Pure-data, compiler-installed Rule IR slots. No world handles cross this interface.
use crate::{GameError, GameErrorCode, GameResult, Value};
use std::{collections::BTreeMap, sync::OnceLock};

pub type RuleSlotFacts = BTreeMap<String, Value>;
pub type RuleSlotOutput = BTreeMap<String, BTreeMap<String, Value>>;
pub type CompiledRuleSlotDispatcher = fn(&str, &RuleSlotFacts) -> GameResult<RuleSlotOutput>;

static COMPILED_SLOTS: OnceLock<CompiledRuleSlotDispatcher> = OnceLock::new();

/// Called only by the Compiler-owned module glue, before project registration.
#[doc(hidden)]
pub fn install_compiled_slots(dispatch: CompiledRuleSlotDispatcher) -> GameResult<()> {
    let installed = COMPILED_SLOTS.get_or_init(|| dispatch);
    if std::ptr::fn_addr_eq(*installed, dispatch) {
        Ok(())
    } else {
        Err(GameError::new(
            GameErrorCode::Failed,
            "compiled RuleSlot dispatcher already installed",
        ))
    }
}

pub fn evaluate(rule_id: &str, facts: &RuleSlotFacts) -> GameResult<RuleSlotOutput> {
    let dispatch = COMPILED_SLOTS.get().ok_or_else(|| {
        GameError::new(
            GameErrorCode::Unsupported,
            "Compiler-generated RuleSlot dispatcher is missing",
        )
    })?;
    dispatch(rule_id, facts)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_compiled_slot_has_no_fallback() {
        assert!(evaluate("missing.rule", &BTreeMap::new()).is_err());
    }
}
