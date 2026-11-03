//! Opt-in accounting of Player-owned report/history storage, never heap totals.
use super::NativeWindowHostReport;
use engine_runtime::resource_memory::ResourceMemorySample;

impl NativeWindowHostReport {
    pub(crate) fn observe_memory(&self, sample: &mut ResourceMemorySample) {
        for value in [
            &self.schema_version,
            &self.run_id,
            &self.runtime_package_path,
            &self.package_status,
            &self.scene_status,
            &self.world_status,
            &self.logic_status,
            &self.render_status,
            &self.rhi_status,
            &self.input_status,
            &self.window_status,
            &self.surface_status,
            &self.surface.format,
            &self.surface.present_mode,
            &self.screenshot.format,
            &self.input.backend,
            &self.input.platform,
            &self.input.mapping_source,
            &self.input.mapping_status,
        ]
        .into_iter()
        .chain(
            [
                self.render_thread_report_schema.as_ref(),
                self.surface.last_error.as_ref(),
                self.screenshot.path.as_ref(),
                self.input.mapping_id.as_ref(),
                self.input.mapping_path.as_ref(),
            ]
            .into_iter()
            .flatten(),
        ) {
            sample.cpu_string(value, "instrumentation", &["instrumentation"]);
        }
        sample.cpu_vec(
            &self.input.last_action_ids,
            "instrumentation",
            &["instrumentation"],
        );
        for action in &self.input.last_action_ids {
            sample.cpu_string(action, "instrumentation", &["instrumentation"]);
        }
        self.aui.observe_memory(sample);
        sample.cpu_vec(&self.diagnostics, "instrumentation", &["instrumentation"]);
        for diagnostic in &self.diagnostics {
            for value in [&diagnostic.code, &diagnostic.layer, &diagnostic.message]
                .into_iter()
                .chain(diagnostic.path.iter())
            {
                sample.cpu_string(value, "instrumentation", &["instrumentation"]);
            }
        }
        sample.cpu_vec(
            &self.gameplay_trace_records,
            "instrumentation",
            &["instrumentation"],
        );
        for record in &self.gameplay_trace_records {
            for value in [
                &record.phase,
                &record.rule_id,
                &record.operation,
                &record.result,
            ]
            .into_iter()
            .chain(
                [
                    record.entity_id.as_ref(),
                    record.component_type.as_ref(),
                    record.field_path.as_ref(),
                    record.before.as_ref(),
                    record.after.as_ref(),
                    record.source.as_ref(),
                    record.error_code.as_ref(),
                ]
                .into_iter()
                .flatten(),
            ) {
                sample.cpu_string(value, "instrumentation", &["instrumentation"]);
            }
        }
        if let Some(retained) = &self.resource_memory_summary {
            for text in retained
                .owner_peaks
                .keys()
                .chain(retained.role_peaks.keys())
                .chain(retained.capacity_peaks.keys())
                .chain(retained.coverage.keys())
                .chain(retained.missing_inputs.iter())
                .chain(retained.observed_scopes.iter())
                .chain(retained.errors.iter())
            {
                sample.cpu_string(text, "instrumentation", &["instrumentation"]);
            }
            // Peak values are historical numeric facts, not current allocations.
            // Shared first_identity and std map/set nodes remain partial.
            sample.partial(
                "cpu.owner.instrumentation",
                "player.priorMemoryRunSummary.ownedStringBacking;sharedFirstIdentity+stdNodesNotIntegrated",
            );
        }
        sample.partial("cpu.owner.instrumentation", "player.report.status+input+aui+diagnostics+gameplayTraceRecords.actualCapacity;otherOptionalReportNested+sharedScreenshotIdentity+stdNodesNotIntegrated");
    }
}

impl super::NativeAuiPresentSummary {
    fn observe_memory(&self, sample: &mut ResourceMemorySample) {
        for value in [
            &self.aui_interaction_status,
            &self.aui_modal_blocking_status,
            &self.aui_focus_trap_status,
            &self.aui_scroll_status,
            &self.aui_ime_platform_coverage,
            &self.snapshot_source,
            &self.cache_status,
            &self.status,
        ]
        .into_iter()
        .chain(
            [
                self.aui_active_screen_id.as_ref(),
                self.font_atlas_id.as_ref(),
                self.font_source_kind.as_ref(),
                self.font_asset_id.as_ref(),
                self.font_asset_status.as_ref(),
                self.glyph_plan_hash.as_ref(),
                self.producer_id.as_ref(),
            ]
            .into_iter()
            .flatten(),
        ) {
            sample.cpu_string(value, "instrumentation", &["instrumentation"]);
        }
        for values in [
            &self.active_binding_paths,
            &self.produced_paths,
            &self.declared_binding_paths,
            &self.missing_paths,
            &self.type_mismatch_paths,
            &self.dirty_domains,
            &self.cache_hit_paths,
            &self.cache_miss_paths,
            &self.source_paths,
            &self.next_actions,
        ] {
            sample.cpu_vec(values, "instrumentation", &["instrumentation"]);
            for value in values {
                sample.cpu_string(value, "instrumentation", &["instrumentation"]);
            }
        }
        // The std tree's node storage remains opaque; its String keys do not.
        for kind in self.aui_consumed_event_count_by_kind.keys() {
            sample.cpu_string(kind, "instrumentation", &["instrumentation"]);
        }
    }
}

pub(super) fn observe_measurement_history(
    sample: &mut ResourceMemorySample,
    timings: [&Vec<u64>; 5],
    work_backing: (usize, usize),
) {
    for timing in timings {
        sample.cpu_vec(timing, "instrumentation", &["instrumentation"]);
    }
    sample.cpu_storage(
        work_backing.0,
        work_backing.1,
        "instrumentation",
        &["instrumentation"],
    );
    sample.partial(
        "cpu.owner.instrumentation",
        "player.measurementHistory.outerCapacity;workNestedAndPriorCensusNotTraversed",
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AuiFrameWorkCounters;
    use crate::{NativePlayerWindowRunRequest, NativeWindowHostDiagnostic};
    use engine_runtime::resource_memory::{memory_totals, MemoryCoverage};
    use engine_runtime::windowed_player::WindowedPlayerGameplayTraceRecord;

    fn reserved(value: &str, capacity: usize) -> String {
        let mut result = String::with_capacity(capacity);
        result.push_str(value);
        result
    }
    fn identity(value: &String) -> (String, u64) {
        (
            format!("cpu-storage:{:x}", value.as_ptr() as usize),
            value.capacity() as u64,
        )
    }
    fn assert_rows(sample: &ResourceMemorySample, expected: &[(String, u64)]) {
        for (id, bytes) in expected {
            let row = sample
                .allocations
                .iter()
                .find(|row| &row.allocation_id == id)
                .unwrap_or_else(|| panic!("missing actual backing {id} {bytes}"));
            assert_eq!(row.bytes, *bytes);
            assert_eq!(row.owner, "instrumentation");
            assert_eq!(row.roles, ["instrumentation".to_string()].into());
        }
    }
    #[test]
    fn retained_memory_summary_keys_are_observed_without_retention_or_peak_summing() {
        use engine_runtime::resource_memory::{MemoryPeak, ResourceMemoryRunSummary};
        for capacity in [256, 4096] {
            for populated in [false, true] {
                let mut report = NativeWindowHostReport::base(
                    &NativePlayerWindowRunRequest::windowed("unused-package"),
                );
                let mut before = ResourceMemorySample::new(1, 1, 0);
                report.observe_memory(&mut before);
                let baseline = memory_totals(&before.allocations).unwrap().cpu_allocated;
                let mut expected = Vec::new();
                let mut text = |label| {
                    let result = reserved(if populated { label } else { "" }, capacity);
                    expected.push(identity(&result));
                    result
                };
                let mut retained = ResourceMemoryRunSummary::default();
                retained.owner_peaks.insert(
                    text("owner"),
                    MemoryPeak {
                        bytes: u64::MAX,
                        frame_index: 1,
                    },
                );
                retained.role_peaks.insert(
                    text("role"),
                    MemoryPeak {
                        bytes: u64::MAX,
                        frame_index: 2,
                    },
                );
                retained.capacity_peaks.insert(text("capacity"), u64::MAX);
                retained
                    .coverage
                    .insert(text("coverage"), MemoryCoverage::Partial);
                retained.missing_inputs.insert(text("missing"));
                retained.observed_scopes.insert(text("scope"));
                retained.errors.insert(text("error"));
                report.resource_memory_summary = Some(retained);
                let mut sample = ResourceMemorySample::new(2, 1, 0);
                report.observe_memory(&mut sample);
                assert_rows(&sample, &expected);
                assert_eq!(
                    memory_totals(&sample.allocations).unwrap().cpu_allocated,
                    baseline + expected.iter().map(|(_, bytes)| bytes).sum::<u64>(),
                    "count retained String backing, never historical peak byte values"
                );
                report.observe_memory(&mut sample);
                assert_eq!(
                    memory_totals(&sample.allocations).unwrap().cpu_allocated,
                    baseline + expected.iter().map(|(_, bytes)| bytes).sum::<u64>()
                );
                assert_eq!(
                    report
                        .resource_memory_summary
                        .as_ref()
                        .unwrap()
                        .owner_peaks
                        .values()
                        .next()
                        .unwrap()
                        .bytes,
                    u64::MAX
                );
                report.resource_memory_summary = None;
                let mut after = ResourceMemorySample::new(3, 1, 0);
                report.observe_memory(&mut after);
                assert_eq!(
                    memory_totals(&after.allocations).unwrap().cpu_allocated,
                    baseline
                );
                assert!(expected
                    .iter()
                    .all(|(id, _)| !after.allocations.iter().any(|a| &a.allocation_id == id)));
            }
        }
    }
    #[test]
    fn player_report_status_input_and_aui_capacity_is_observed_and_released() {
        fn text_fields(report: &mut NativeWindowHostReport) -> Vec<&mut String> {
            let mut fields = vec![
                &mut report.schema_version,
                &mut report.run_id,
                &mut report.runtime_package_path,
                &mut report.package_status,
                &mut report.scene_status,
                &mut report.world_status,
                &mut report.logic_status,
                &mut report.render_status,
                &mut report.rhi_status,
                &mut report.input_status,
                &mut report.window_status,
                &mut report.surface_status,
                &mut report.surface.format,
                &mut report.surface.present_mode,
                &mut report.screenshot.format,
                &mut report.input.backend,
                &mut report.input.platform,
                &mut report.input.mapping_source,
                &mut report.input.mapping_status,
                &mut report.aui.aui_interaction_status,
                &mut report.aui.aui_modal_blocking_status,
                &mut report.aui.aui_focus_trap_status,
                &mut report.aui.aui_scroll_status,
                &mut report.aui.aui_ime_platform_coverage,
                &mut report.aui.snapshot_source,
                &mut report.aui.cache_status,
                &mut report.aui.status,
            ];
            for field in [
                &mut report.render_thread_report_schema,
                &mut report.surface.last_error,
                &mut report.screenshot.path,
                &mut report.input.mapping_id,
                &mut report.input.mapping_path,
                &mut report.aui.aui_active_screen_id,
                &mut report.aui.font_atlas_id,
                &mut report.aui.font_source_kind,
                &mut report.aui.font_asset_id,
                &mut report.aui.font_asset_status,
                &mut report.aui.glyph_plan_hash,
                &mut report.aui.producer_id,
            ] {
                fields.push(field.get_or_insert_with(String::new));
            }
            fields
        }
        fn vector_fields(report: &mut NativeWindowHostReport) -> Vec<&mut Vec<String>> {
            vec![
                &mut report.input.last_action_ids,
                &mut report.aui.active_binding_paths,
                &mut report.aui.produced_paths,
                &mut report.aui.declared_binding_paths,
                &mut report.aui.missing_paths,
                &mut report.aui.type_mismatch_paths,
                &mut report.aui.dirty_domains,
                &mut report.aui.cache_hit_paths,
                &mut report.aui.cache_miss_paths,
                &mut report.aui.source_paths,
                &mut report.aui.next_actions,
            ]
        }
        for capacity in [256, 4096] {
            for populated in [false, true] {
                let mut report = NativeWindowHostReport::base(
                    &NativePlayerWindowRunRequest::windowed("unused-package"),
                );
                let mut expected = Vec::new();
                let value = if populated { "retained-value" } else { "" };
                for field in text_fields(&mut report) {
                    *field = reserved(value, capacity);
                    expected.push(identity(field));
                }
                for field in vector_fields(&mut report) {
                    *field = Vec::with_capacity(8);
                    for _ in 0..2 {
                        let text = reserved(value, capacity);
                        expected.push(identity(&text));
                        field.push(text);
                    }
                    expected.push((
                        format!("cpu-storage:{:x}", field.as_ptr() as usize),
                        (field.capacity() * std::mem::size_of::<String>()) as u64,
                    ));
                }
                let event = reserved(value, capacity);
                expected.push(identity(&event));
                report.aui.aui_consumed_event_count_by_kind.insert(event, 1);
                let mut sample = ResourceMemorySample::new(1, 1, 0);
                report.observe_memory(&mut sample);
                assert_rows(&sample, &expected);
                assert_eq!(
                    memory_totals(&sample.allocations).unwrap().cpu_allocated,
                    expected.iter().map(|(_, bytes)| bytes).sum::<u64>()
                );
                report.observe_memory(&mut sample);
                assert_eq!(
                    memory_totals(&sample.allocations).unwrap().cpu_allocated,
                    expected.iter().map(|(_, bytes)| bytes).sum::<u64>(),
                    "repeated report observations count physical backing once"
                );
                for field in text_fields(&mut report) {
                    *field = String::new();
                }
                for field in vector_fields(&mut report) {
                    *field = Vec::new();
                }
                report.aui.aui_consumed_event_count_by_kind.clear();
                let mut after = ResourceMemorySample::new(2, 1, 0);
                report.observe_memory(&mut after);
                assert!(after.allocations.is_empty());
            }
        }
    }
    #[test]
    fn player_report_nested_capacity_is_observed_deduplicated_and_released() {
        for capacity in [256, 4096] {
            let mut report = NativeWindowHostReport::base(&NativePlayerWindowRunRequest::windowed(
                "unused-package",
            ));
            let diagnostic = NativeWindowHostDiagnostic {
                severity: crate::NativeWindowHostDiagnosticSeverity::Error,
                code: reserved("code", capacity),
                layer: reserved("layer", capacity),
                message: reserved("message", capacity),
                path: Some(reserved("", capacity)),
            };
            let trace = WindowedPlayerGameplayTraceRecord {
                frame_index: 1,
                phase: reserved("phase", capacity),
                rule_id: reserved("rule", capacity),
                operation: reserved("op", capacity),
                entity_id: Some(reserved("entity", capacity)),
                component_type: Some(reserved("component", capacity)),
                field_path: Some(reserved("field", capacity)),
                before: Some(reserved("", capacity)),
                after: Some(reserved("after", capacity)),
                source: Some(reserved("source", capacity)),
                result: reserved("ok", capacity),
                error_code: Some(reserved("error", capacity)),
            };
            let mut expected: Vec<_> = [
                &diagnostic.code,
                &diagnostic.layer,
                &diagnostic.message,
                diagnostic.path.as_ref().unwrap(),
                &trace.phase,
                &trace.rule_id,
                &trace.operation,
                trace.entity_id.as_ref().unwrap(),
                trace.component_type.as_ref().unwrap(),
                trace.field_path.as_ref().unwrap(),
                trace.before.as_ref().unwrap(),
                trace.after.as_ref().unwrap(),
                trace.source.as_ref().unwrap(),
                &trace.result,
                trace.error_code.as_ref().unwrap(),
            ]
            .into_iter()
            .map(identity)
            .collect();
            report.diagnostics = Vec::with_capacity(8);
            report.diagnostics.push(diagnostic);
            report.gameplay_trace_records = Vec::with_capacity(16);
            report.gameplay_trace_records.push(trace);
            expected.push((
                format!("cpu-storage:{:x}", report.diagnostics.as_ptr() as usize),
                (report.diagnostics.capacity() * std::mem::size_of::<NativeWindowHostDiagnostic>())
                    as u64,
            ));
            expected.push((
                format!(
                    "cpu-storage:{:x}",
                    report.gameplay_trace_records.as_ptr() as usize
                ),
                (report.gameplay_trace_records.capacity()
                    * std::mem::size_of::<WindowedPlayerGameplayTraceRecord>())
                    as u64,
            ));
            let mut sample = ResourceMemorySample::new(1, 1, 0);
            report.observe_memory(&mut sample);
            assert_rows(&sample, &expected);
            let first = memory_totals(&sample.allocations).unwrap();
            report.observe_memory(&mut sample);
            assert_eq!(memory_totals(&sample.allocations).unwrap(), first);
            assert_eq!(
                sample.coverage.get("cpu.owner.instrumentation"),
                Some(&MemoryCoverage::Partial)
            );
            report.diagnostics = Vec::new();
            report.gameplay_trace_records = Vec::new();
            let mut after = ResourceMemorySample::new(2, 1, 0);
            report.observe_memory(&mut after);
            assert!(expected
                .iter()
                .all(|(id, _)| after.allocations.iter().all(|row| &row.allocation_id != id)));
        }
    }
    #[test]
    fn player_history_reserved_capacity_uses_original_backing_and_deduplicates_aliases() {
        for capacity in [256, 4096] {
            let timing = Vec::<u64>::with_capacity(capacity);
            let work = Vec::<AuiFrameWorkCounters>::with_capacity(capacity);
            let expected = [
                (
                    format!("cpu-storage:{:x}", timing.as_ptr() as usize),
                    (timing.capacity() * 8) as u64,
                ),
                (
                    format!("cpu-storage:{:x}", work.as_ptr() as usize),
                    (work.capacity() * std::mem::size_of::<AuiFrameWorkCounters>()) as u64,
                ),
            ];
            let mut sample = ResourceMemorySample::new(1, 1, 0);
            observe_measurement_history(
                &mut sample,
                [&timing; 5],
                (
                    work.as_ptr() as usize,
                    work.capacity() * std::mem::size_of::<AuiFrameWorkCounters>(),
                ),
            );
            assert_rows(&sample, &expected);
            let totals = memory_totals(&sample.allocations).unwrap();
            assert_eq!(
                totals.cpu_allocated,
                expected.iter().map(|(_, n)| n).sum::<u64>()
            );
            observe_measurement_history(
                &mut sample,
                [&timing; 5],
                (
                    work.as_ptr() as usize,
                    work.capacity() * std::mem::size_of::<AuiFrameWorkCounters>(),
                ),
            );
            assert_eq!(memory_totals(&sample.allocations).unwrap(), totals);
            let empty = Vec::new();
            let mut after = ResourceMemorySample::new(2, 1, 0);
            observe_measurement_history(&mut after, [&empty; 5], (0, 0));
            assert!(after.allocations.is_empty());
        }
    }
}
