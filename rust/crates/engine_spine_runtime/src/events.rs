use crate::Error;

/// Gate A compact index units: integer microseconds, <= 0.5 us quantization.
/// Gate B stores the same quantized source contract alongside full payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventKey {
    pub time_us: u64,
    pub ordinal: u32,
}
#[derive(Debug, Clone)]
pub struct EventIndex {
    pub duration_us: u64,
    pub keys: Vec<EventKey>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentEvent {
    pub absolute_us: u64,
    pub loop_index: u64,
    pub ordinal: u32,
}
#[derive(Debug, Default, Clone)]
pub struct EventCursor {
    last_us: Option<u64>,
}

fn microseconds(value: f32) -> Result<u64, Error> {
    if !value.is_finite() || !(0.0..=3600.0).contains(&value) {
        return Err(Error::InvalidInput);
    }
    Ok((f64::from(value) * 1_000_000.0).round() as u64)
}
impl EventIndex {
    pub fn new(duration: f32, keys: impl IntoIterator<Item = (f32, u32)>) -> Result<Self, Error> {
        let duration_us = microseconds(duration)?;
        if duration_us == 0 {
            return Err(Error::InvalidInput);
        }
        let mut values = Vec::new();
        for (time, ordinal) in keys {
            let time_us = microseconds(time)?;
            if time_us > duration_us {
                return Err(Error::InvalidInput);
            }
            if values.len() >= 4096 {
                return Err(Error::Capacity);
            }
            values.push(EventKey { time_us, ordinal });
        }
        values.sort_by_key(|key| (key.time_us, key.ordinal));
        Ok(Self {
            duration_us,
            keys: values,
        })
    }
}
impl EventCursor {
    fn range(
        &self,
        index: &EventIndex,
        key: &EventKey,
        end_us: u64,
        looping: bool,
    ) -> Option<(u64, u64)> {
        if key.time_us > end_us {
            return None;
        }
        let first = match self.last_us {
            Some(last) if last >= key.time_us => {
                ((last - key.time_us) / index.duration_us).checked_add(1)?
            }
            _ => 0,
        };
        let last = if looping {
            (end_us - key.time_us) / index.duration_us
        } else {
            0
        };
        (first <= last).then_some((first, last))
    }
    /// Counts the complete interval without allocating or enumerating loops.
    /// Owners can sum every layer before allocating any instance event payload.
    pub fn count(
        &self,
        index: &EventIndex,
        end_us: u64,
        looping: bool,
        budget: usize,
    ) -> Result<usize, Error> {
        if self.last_us.is_some_and(|last| end_us < last) {
            return Err(Error::InvalidInput);
        }
        if index.duration_us == 0 {
            return if index.keys.is_empty() {
                Ok(0)
            } else {
                Err(Error::InvalidInput)
            };
        }
        let mut total = 0_u64;
        for key in &index.keys {
            if let Some((first, last)) = self.range(index, key, end_us, looping) {
                total = total
                    .checked_add((last - first).checked_add(1).ok_or(Error::Capacity)?)
                    .ok_or(Error::Capacity)?;
                if total > budget as u64 {
                    return Err(Error::Capacity);
                }
            }
        }
        Ok(total as usize)
    }
    /// Preflight is pure. Call commit only after the corresponding native
    /// evaluation succeeds; over-budget intervals leave the cursor untouched.
    pub fn preview(
        &self,
        index: &EventIndex,
        end_us: u64,
        looping: bool,
        budget: usize,
    ) -> Result<Vec<ContentEvent>, Error> {
        let total = self.count(index, end_us, looping, budget)?;
        let mut events = Vec::with_capacity(total);
        for key in &index.keys {
            let Some((first, last)) = self.range(index, key, end_us, looping) else {
                continue;
            };
            for cycle in first..=last {
                // Product <= end_us - phase by construction, so cannot overflow.
                events.push(ContentEvent {
                    absolute_us: cycle * index.duration_us + key.time_us,
                    loop_index: cycle,
                    ordinal: key.ordinal,
                });
            }
        }
        events.sort_by_key(|event| (event.absolute_us, event.ordinal, event.loop_index));
        Ok(events)
    }
    pub fn commit(&mut self, end_us: u64) -> Result<(), Error> {
        if self.last_us.is_some_and(|last| end_us < last) {
            return Err(Error::InvalidInput);
        }
        self.last_us = Some(end_us);
        Ok(())
    }
}

pub fn seconds_to_ticks(seconds: f64, tick_rate: u32) -> Result<u64, Error> {
    if !seconds.is_finite()
        || !(0.0..=3600.0).contains(&seconds)
        || !(1..=1000).contains(&tick_rate)
    {
        return Err(Error::InvalidInput);
    }
    let ticks = seconds * f64::from(tick_rate);
    let nearest = ticks.round();
    // Decimal author values such as 0.14 * 100 can land a few ULPs above 14.
    // Correct only a positive integer boundary; every positive sub-tick value
    // still maps to at least one tick.
    let ticks = if nearest >= 1.0 && (ticks - nearest).abs() <= 4.0 * f64::EPSILON * ticks {
        nearest
    } else {
        ticks.ceil()
    };
    Ok(ticks as u64)
}

/// A bounded once-action plan: at natural completion, start an empty-animation
/// fade on this track. No public queue and no callback-triggered reentry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OncePlan {
    pub fade_start_tick: u64,
    pub retire_tick: u64,
}
impl OncePlan {
    pub fn new(entry_tick: u64, duration: f64, fade: f64, tick_rate: u32) -> Result<Self, Error> {
        let fade_start_tick = entry_tick
            .checked_add(seconds_to_ticks(duration, tick_rate)?)
            .ok_or(Error::Capacity)?;
        let retire_tick = fade_start_tick
            .checked_add(seconds_to_ticks(fade, tick_rate)?)
            .ok_or(Error::Capacity)?;
        Ok(Self {
            fade_start_tick,
            retire_tick,
        })
    }
}
