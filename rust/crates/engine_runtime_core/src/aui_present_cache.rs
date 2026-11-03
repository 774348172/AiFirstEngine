//! Shared Player/Editor presentation scheduling; resources are mounted for one package lifetime.
use crate::aui::*;
use crate::aui_viewport::{AuiViewportInput, AuiViewportResolver};
use crate::aui_virtual_list::AuiVirtualListPresentState;
use crate::game_view_presentation::ResolvedGameViewPresentation;
use crate::runtime_package::RuntimePackage;
use crate::world::World;
use std::collections::{BTreeSet, VecDeque};

pub const MAX_AUI_EXTENSION_STABILIZATION_ROUNDS: usize = 8;
const MAX_STATIC_PRESENT_VARIANTS: usize = 4;
const MAX_STATIC_PRESENT_VARIANT_NODES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq)]
struct StaticPresentVariantKey {
    theme: Option<crate::runtime_package::RuntimeAssetRef>,
    viewport: Option<String>,
}

pub trait AuiExtensionFrameHost {
    fn advance_ui_animations(
        &mut self,
        _frame: u64,
        _delta_us: u64,
    ) -> Result<(), project_runtime_sdk::aui::UiError> {
        Ok(())
    }
    fn cancel_drag_animations(&mut self, _document: &str, _interaction: &AuiInteractionResult) {}
    fn retained_ui(&self) -> Option<&crate::aui_session::AuiSession>;
    fn publish_ui_geometry(&mut self, _output: &AuiRuntimePresentOutput) {}
    fn refresh_ui_extensions_for_frame(
        &mut self,
        frame: &crate::aui_extension::AuiCustomFrame,
        allow_publish: bool,
    ) -> Result<crate::aui_extension::AuiCustomFrameResult, String>;
}

impl AuiExtensionFrameHost for crate::engine_host_loop::EngineHostLoop {
    fn advance_ui_animations(
        &mut self,
        frame: u64,
        delta_us: u64,
    ) -> Result<(), project_runtime_sdk::aui::UiError> {
        self.advance_ui_animations(frame, delta_us)
    }
    fn cancel_drag_animations(&mut self, document: &str, interaction: &AuiInteractionResult) {
        self.cancel_drag_animations(document, interaction);
    }
    fn publish_ui_geometry(&mut self, output: &AuiRuntimePresentOutput) {
        self.publish_ui_geometry(output);
    }
    fn retained_ui(&self) -> Option<&crate::aui_session::AuiSession> {
        self.retained_ui()
    }

    fn refresh_ui_extensions_for_frame(
        &mut self,
        frame: &crate::aui_extension::AuiCustomFrame,
        allow_publish: bool,
    ) -> Result<crate::aui_extension::AuiCustomFrameResult, String> {
        self.refresh_ui_extensions_for_frame(frame, allow_publish)
    }
}

pub struct AuiPresentContext<'a> {
    presentation: Option<&'a ResolvedGameViewPresentation>,
    viewport: Option<&'a AuiViewportInput>,
    interaction: &'a mut AuiInteractionState,
    pub retained_instance: Option<&'a crate::aui_instance::AuiInstance>,
    feedback_cache: Option<&'a mut crate::aui_control_feedback::AuiControlFeedbackState>,
}

/// Inputs captured for one presentation frame, after input dispatch and before feedback.
pub struct AuiAnimationPresentContext<'a> {
    pub frame_index: u64,
    pub delta_us: u64,
    pub interaction_result: Option<&'a AuiInteractionResult>,
    pub presentation: AuiPresentContext<'a>,
}

impl<'a> AuiPresentContext<'a> {
    pub fn new(
        presentation: Option<&'a ResolvedGameViewPresentation>,
        viewport: Option<&'a AuiViewportInput>,
        interaction: &'a mut AuiInteractionState,
    ) -> Self {
        Self {
            presentation,
            viewport,
            interaction,
            retained_instance: None,
            feedback_cache: None,
        }
    }

    pub fn with_feedback_cache(
        mut self,
        feedback: &'a mut crate::aui_control_feedback::AuiControlFeedbackState,
    ) -> Self {
        self.feedback_cache = Some(feedback);
        self
    }

    pub fn with_retained_ui(
        mut self,
        retained_ui: Option<&'a crate::aui_session::AuiSession>,
        package: &RuntimePackage,
    ) -> Self {
        self.retained_instance = package
            .aui_manifest
            .documents
            .first()
            .and_then(|entry| retained_ui.and_then(|ui| ui.document(&entry.document_id)));
        self
    }
}

pub struct AuiPresentCache {
    snapshot_cache: ProjectUiStateSnapshotCache,
    c2_runtime: AuiC2Runtime,
    last_present: Option<AuiRuntimePresentOutput>,
    pub rebuild_count: u64,
    pub hit_count: u64,
    pub presentation_revision: u64,
    last_snapshot_output: Option<ProjectUiStateSnapshotOutput>,
    last_viewport_identity: Option<String>,
    last_theme_identity: Option<crate::runtime_package::RuntimeAssetRef>,
    last_virtual_interaction_identity: Option<String>,
    virtual_lists: AuiVirtualListPresentState,
    uses_viewport_layout: bool,
    last_content_key: Option<(u64, u64)>,
    last_binding_set_key: Option<(u64, u64)>,
    static_variants: VecDeque<(StaticPresentVariantKey, AuiRuntimePresentOutput)>,
}

impl AuiPresentCache {
    pub fn set_measurement_enabled(&mut self, enabled: bool) {
        self.c2_runtime.measurement_enabled = enabled;
    }

    pub fn update_timings(&self) -> AuiUpdateTimings {
        self.c2_runtime.timings
    }
    pub fn for_package(package: &RuntimePackage) -> Self {
        let document = package
            .aui_manifest
            .documents
            .first()
            .and_then(|entry| package.aui_documents.get(&entry.document_id));
        let active_binding_paths = document.map(active_aui_binding_paths).unwrap_or_default();
        let uses_viewport_layout = document.is_some_and(AuiViewportResolver::requires_viewport);
        Self {
            snapshot_cache: ProjectUiStateSnapshotCache::new(active_binding_paths),
            c2_runtime: AuiC2Runtime::default(),
            last_present: None,
            rebuild_count: 0,
            hit_count: 0,
            presentation_revision: 0,
            last_snapshot_output: None,
            last_viewport_identity: None,
            last_theme_identity: None,
            last_virtual_interaction_identity: None,
            virtual_lists: AuiVirtualListPresentState::default(),
            uses_viewport_layout,
            last_content_key: None,
            last_binding_set_key: None,
            static_variants: VecDeque::new(),
        }
    }

    pub fn take_or_rebuild(
        &mut self,
        package: &RuntimePackage,
        world: &World,
        frame_index: u64,
        producer: &mut dyn ProjectUiStateSnapshotProducer,
        context: AuiPresentContext<'_>,
    ) -> Option<AuiRuntimePresentOutput> {
        let production_started = self
            .c2_runtime
            .measurement_enabled
            .then(crate::platform_time::Instant::now);
        let AuiPresentContext {
            presentation,
            viewport,
            interaction,
            retained_instance,
            feedback_cache,
        } = context;
        let document_id = package
            .aui_manifest
            .documents
            .first()
            .map(|entry| entry.document_id.as_str())?;
        let package_document = package.aui_documents.get(document_id)?;
        let retained_instance =
            retained_instance.filter(|instance| instance.source().document_id == document_id);
        let document = retained_instance
            .map(crate::aui_instance::AuiInstance::source)
            .unwrap_or(package_document);
        let binding_set_key =
            retained_instance.and_then(crate::aui_instance::AuiInstance::binding_set_key);
        if self.last_binding_set_key != binding_set_key {
            self.snapshot_cache
                .set_active_binding_paths(active_aui_binding_paths(document));
            self.last_binding_set_key = binding_set_key;
        }
        let retained_changed = self.c2_runtime.retained_change_pending(retained_instance);
        let content_key = retained_instance.and_then(crate::aui_instance::AuiInstance::content_key);
        let content_changed = self.last_content_key != content_key;
        let same_retained_instance = self
            .last_content_key
            .zip(content_key)
            .is_some_and(|(old, new)| old.0 == new.0);
        let viewport_identity = self.uses_viewport_layout.then(|| {
            viewport
                .and_then(|viewport| viewport.compact_identity().ok())
                .unwrap_or_else(|| "missing-or-invalid-viewport-input".to_string())
        });
        let viewport_changed = self.last_viewport_identity != viewport_identity;
        let theme_identity = document.theme.clone();
        let theme_changed = self.last_theme_identity != theme_identity;
        let variant_key = StaticPresentVariantKey {
            theme: theme_identity.clone(),
            viewport: viewport_identity.clone(),
        };
        let virtual_interaction_identity =
            AuiVirtualListPresentState::interaction_identity(document, interaction);
        let virtual_interaction_changed =
            self.last_virtual_interaction_identity != virtual_interaction_identity;
        let snapshot_checkpoint = self.snapshot_cache.checkpoint();
        let reused_snapshot;
        let replaced_snapshot;
        let mut changed_binding_paths = Vec::new();
        let snapshot_result = self.snapshot_cache.resolve(
            producer,
            frame_index,
            package,
            world,
            ProjectUiStateReportMode::Summary,
        );
        self.c2_runtime.timings.production_exchange_diff_ns += aui_elapsed_ns(production_started);
        let diff_started = self
            .c2_runtime
            .measurement_enabled
            .then(crate::platform_time::Instant::now);
        let mut snapshot_output = match snapshot_result {
            Ok(ProjectUiStateSnapshotCacheResult::Reuse) | Err(_)
                if viewport_changed
                    || theme_changed
                    || virtual_interaction_changed
                    || retained_changed =>
            {
                let mut output = self.last_snapshot_output.clone()?;
                reused_snapshot = true;
                replaced_snapshot = false;
                output.snapshot.frame_index = frame_index;
                output.report.frame_index = frame_index;
                output
            }
            Ok(ProjectUiStateSnapshotCacheResult::Reuse) | Err(_) => {
                if let Some(present) = self.last_present.take() {
                    self.c2_runtime.observe_retained(retained_instance);
                    self.hit_count = self.hit_count.saturating_add(1);
                    return Some(present);
                }
                return None;
            }
            Ok(ProjectUiStateSnapshotCacheResult::Replace(output)) => {
                reused_snapshot = self.last_snapshot_output.as_ref().is_some_and(|previous| {
                    for path in self.snapshot_cache.active_binding_paths() {
                        if self.c2_runtime.measurement_enabled {
                            self.c2_runtime.work.active_binding_paths_compared += 1;
                        }
                        if previous.snapshot.values.get(path) != output.snapshot.values.get(path) {
                            changed_binding_paths.push(path.clone());
                        }
                    }
                    previous.snapshot.virtual_lists == output.snapshot.virtual_lists
                        && changed_binding_paths.is_empty()
                });
                replaced_snapshot = !reused_snapshot;
                output
            }
        };
        let equivalent_report = self.last_snapshot_output.as_ref().is_some_and(|previous| {
            snapshot_output
                .report
                .equivalent_except_frame(&previous.report)
        });
        self.c2_runtime.timings.production_exchange_diff_ns += aui_elapsed_ns(diff_started);
        if reused_snapshot
            && equivalent_report
            && !retained_changed
            && !content_changed
            && !viewport_changed
            && !theme_changed
            && !virtual_interaction_changed
        {
            if let Some(mut present) = self.last_present.take() {
                self.c2_runtime.observe_retained(retained_instance);
                present.report.frame_index = frame_index;
                if let Some(report) = present.report.ui_state_snapshot_report.as_mut() {
                    report.frame_index = frame_index;
                }
                self.last_snapshot_output = Some(snapshot_output);
                self.hit_count = self.hit_count.saturating_add(1);
                return Some(present);
            }
        }
        self.c2_runtime.observe_retained(retained_instance);
        if replaced_snapshot || retained_changed || content_changed || virtual_interaction_changed {
            self.static_variants.clear();
        }
        if reused_snapshot
            && !retained_changed
            && !content_changed
            && !virtual_interaction_changed
            && (viewport_changed || theme_changed)
        {
            if let Some(index) = self
                .static_variants
                .iter()
                .position(|(key, _)| key == &variant_key)
            {
                let (key, mut output) = self
                    .static_variants
                    .remove(index)
                    .expect("located static AUI variant");
                output.report.frame_index = frame_index;
                self.static_variants.push_front((key, output.clone()));
                self.last_present = None;
                self.last_snapshot_output = Some(snapshot_output);
                self.last_viewport_identity = viewport_identity;
                self.last_theme_identity = theme_identity;
                self.last_virtual_interaction_identity = virtual_interaction_identity;
                self.last_content_key = content_key;
                let measurement_enabled = self.c2_runtime.measurement_enabled;
                let timings = self.c2_runtime.timings;
                let work = self.c2_runtime.work;
                self.c2_runtime = AuiC2Runtime::default();
                self.c2_runtime.measurement_enabled = measurement_enabled;
                self.c2_runtime.timings = timings;
                self.c2_runtime.work = work;
                self.c2_runtime.observe_retained(retained_instance);
                self.hit_count = self.hit_count.saturating_add(1);
                return Some(output);
            }
        }
        // The scheduler has committed to rebuilding.
        let previous_snapshot_output = self.last_snapshot_output.take();
        let previous_viewport_identity = self.last_viewport_identity.clone();
        let previous_theme_identity = self.last_theme_identity.clone();
        let previous_virtual_interaction_identity = self.last_virtual_interaction_identity.clone();
        let previous_content_key = self.last_content_key;
        snapshot_output.snapshot.frame_index = frame_index;
        snapshot_output.report.frame_index = frame_index;
        self.last_snapshot_output = Some(snapshot_output.clone());
        self.last_viewport_identity = viewport_identity;
        self.last_theme_identity = theme_identity;
        self.rebuild_count = self.rebuild_count.saturating_add(1);
        self.presentation_revision = self.presentation_revision.saturating_add(1);
        // Package resources are immutable for this cache lifetime. Only this scheduler can
        // prove that snapshot, retained content, viewport and materialized structure agree.
        let preflight_started = self
            .c2_runtime
            .measurement_enabled
            .then(crate::platform_time::Instant::now);
        self.c2_runtime.reuse_prepared_input_once = reused_snapshot
            && !content_changed
            && !viewport_changed
            && !virtual_interaction_changed
            && document.virtual_lists.is_empty();
        let bindings_unchanged = changed_binding_paths.is_empty();
        self.c2_runtime.changed_binding_paths_once = if replaced_snapshot
            && (!content_changed
                || (same_retained_instance
                    && !self.c2_runtime.retained_value_removed()
                    && !self.c2_runtime.retained_full_prepare_required()))
            && (!retained_changed || !self.c2_runtime.retained_full_prepare_required())
            && !viewport_changed
            && !theme_changed
            && !virtual_interaction_changed
            && document.virtual_lists.is_empty()
            && self.c2_runtime.can_apply_changed_bindings(
                document,
                &snapshot_output.snapshot,
                &changed_binding_paths,
                &package.font_bundles,
            ) {
            changed_binding_paths
        } else {
            Vec::new()
        };
        self.c2_runtime
            .apply_retained_changes_to_prepared_input_once = (reused_snapshot
            || (replaced_snapshot && bindings_unchanged)
            || !self.c2_runtime.changed_binding_paths_once.is_empty())
            && content_changed
            && same_retained_instance
            && !self.c2_runtime.retained_value_removed()
            && !self.c2_runtime.retained_full_prepare_required()
            && !viewport_changed
            && !virtual_interaction_changed
            && document.virtual_lists.is_empty();
        // A verified local edit must release the old Arc before mutating the prepared input.
        let safe_local_edit = (!self.c2_runtime.changed_binding_paths_once.is_empty()
            || self
                .c2_runtime
                .apply_retained_changes_to_prepared_input_once)
            && self
                .c2_runtime
                .local_retained_edits_are_safe(&package.font_bundles);
        let previous_present = if safe_local_edit {
            if let Some(feedback) = feedback_cache {
                feedback.release_source_products();
            }
            self.last_present = None;
            None
        } else {
            self.last_present.take()
        };
        self.last_content_key = content_key;
        self.c2_runtime.timings.prepare_style_preflight_ns += aui_elapsed_ns(preflight_started);
        let base = self.present(
            document,
            snapshot_output.clone(),
            package,
            presentation,
            viewport,
        );
        let mut output = if document.virtual_lists.is_empty() {
            base
        } else {
            match self.virtual_lists.materialize(
                &base.resolved_document,
                &base.layout,
                &snapshot_output.snapshot.virtual_lists,
                interaction,
            ) {
                Ok(materialized) => {
                    let mut present = self.present(
                        &materialized.document,
                        snapshot_output,
                        package,
                        presentation,
                        viewport,
                    );
                    present.virtual_lists = materialized.windows;
                    present
                }
                Err(error) => {
                    let mut present = base;
                    present.report.status = AuiRuntimePresentStatus::Partial;
                    present
                        .report
                        .diagnostics
                        .extend(error.diagnostics.into_iter().map(|diagnostic| {
                            crate::aui::AuiRuntimePresentDiagnostic {
                                severity: crate::aui::AuiRuntimePresentDiagnosticSeverity::Error,
                                code: diagnostic.code,
                                message: diagnostic.message,
                                path: diagnostic.item_key,
                            }
                        }));
                    present
                }
            }
        };
        if output.report.status == AuiRuntimePresentStatus::Failed {
            self.snapshot_cache.restore(snapshot_checkpoint);
            self.last_snapshot_output = previous_snapshot_output;
            self.last_viewport_identity = previous_viewport_identity;
            self.last_theme_identity = previous_theme_identity;
            self.last_virtual_interaction_identity = previous_virtual_interaction_identity;
            self.last_content_key = previous_content_key;
            self.c2_runtime = AuiC2Runtime::default();
            if let Some(previous) = previous_present {
                let mut fallback = previous.clone();
                fallback.report = output.report;
                self.last_present = Some(previous);
                return Some(fallback);
            }
            return Some(output);
        }
        self.last_virtual_interaction_identity =
            AuiVirtualListPresentState::interaction_identity(document, interaction);
        output.report.frame_index = frame_index;
        self.remember_static_variant(variant_key, &output);
        Some(output)
    }

    pub fn take_or_rebuild_stabilized(
        &mut self,
        package: &RuntimePackage,
        world: &World,
        frame_index: u64,
        producer: &mut dyn ProjectUiStateSnapshotProducer,
        host: &mut impl AuiExtensionFrameHost,
        context: AuiPresentContext<'_>,
    ) -> Option<AuiRuntimePresentOutput> {
        let AuiPresentContext {
            presentation,
            viewport,
            interaction,
            retained_instance: _,
            mut feedback_cache,
        } = context;
        for round in 0..MAX_AUI_EXTENSION_STABILIZATION_ROUNDS {
            let rebuild_count_before = self.rebuild_count;
            let mut output = self.take_or_rebuild(
                package,
                world,
                frame_index,
                producer,
                AuiPresentContext {
                    feedback_cache: feedback_cache.as_deref_mut(),
                    ..AuiPresentContext::new(presentation, viewport, &mut *interaction)
                        .with_retained_ui(host.retained_ui(), package)
                },
            )?;
            let custom_work_pending = host
                .retained_ui()
                .is_some_and(crate::aui_session::AuiSession::has_custom_work);
            if self.rebuild_count == rebuild_count_before && !custom_work_pending {
                host.publish_ui_geometry(&output);
                return Some(output);
            }
            let frame = self.custom_extension_frame(&output, package);
            if frame.nodes.is_empty() {
                host.publish_ui_geometry(&output);
                return Some(output);
            }
            match host.refresh_ui_extensions_for_frame(
                &frame,
                round + 1 < MAX_AUI_EXTENSION_STABILIZATION_ROUNDS,
            ) {
                Ok(result) if result.stabilization_limit_exceeded => {
                    output.report.status = AuiRuntimePresentStatus::Partial;
                    output.report.diagnostics.push(AuiRuntimePresentDiagnostic {
                        severity: AuiRuntimePresentDiagnosticSeverity::Error,
                        code: "aui_extension.stabilization_limit_exceeded".to_string(),
                        message: "Custom AUI root did not stabilize within 8 update rounds. The last valid geometry remains published.".to_string(),
                        path: Some(frame.document_id),
                    });
                    return Some(output);
                }
                Ok(result) if result.changed => continue,
                Ok(_) => {
                    host.publish_ui_geometry(&output);
                    return Some(output);
                }
                Err(message) => {
                    let mut fallback = self.take_or_rebuild(
                        package,
                        world,
                        frame_index,
                        producer,
                        AuiPresentContext {
                            feedback_cache: feedback_cache.as_deref_mut(),
                            ..AuiPresentContext::new(presentation, viewport, &mut *interaction)
                                .with_retained_ui(host.retained_ui(), package)
                        },
                    )?;
                    fallback.report.status = AuiRuntimePresentStatus::Partial;
                    fallback
                        .report
                        .diagnostics
                        .push(AuiRuntimePresentDiagnostic {
                            severity: AuiRuntimePresentDiagnosticSeverity::Error,
                            code: "aui_extension.frame_update_failed".to_string(),
                            message,
                            path: Some(frame.document_id),
                        });
                    return Some(fallback);
                }
            }
        }
        unreachable!("bounded custom AUI stabilization loop always returns")
    }

    /// Input has read the last published frame. Advance once, rebuild only on sparse changes,
    /// then let the caller apply feedback and publish the shared final geometry.
    pub fn advance_animations_for_present(
        &mut self,
        mut present: Option<AuiRuntimePresentOutput>,
        package: &RuntimePackage,
        world: &World,
        producer: &mut dyn ProjectUiStateSnapshotProducer,
        host: &mut impl AuiExtensionFrameHost,
        context: AuiAnimationPresentContext<'_>,
    ) -> Option<AuiRuntimePresentOutput> {
        let AuiAnimationPresentContext {
            frame_index: frame,
            delta_us,
            interaction_result,
            presentation: context,
        } = context;
        let document_id = present
            .as_ref()
            .map(|p| p.resolved_document.document_id.clone());
        let key = document_id
            .as_deref()
            .and_then(|id| host.retained_ui()?.document(id)?.presentation_key());
        if let (Some(id), Some(interaction)) = (document_id.as_deref(), interaction_result) {
            host.cancel_drag_animations(id, interaction);
        }
        if let Err(error) = host.advance_ui_animations(frame, delta_us) {
            if let Some(present) = &mut present {
                present.report.status = AuiRuntimePresentStatus::Failed;
                present
                    .report
                    .diagnostics
                    .push(AuiRuntimePresentDiagnostic {
                        severity: AuiRuntimePresentDiagnosticSeverity::Error,
                        code: "aui_animation.clock_failed".into(),
                        message: format!("{error:?}"),
                        path: document_id,
                    });
            }
            return present;
        }
        let next = document_id
            .as_deref()
            .and_then(|id| host.retained_ui()?.document(id)?.presentation_key());
        let instance = document_id
            .as_deref()
            .and_then(|id| host.retained_ui()?.document(id));
        if key == next && !self.c2_runtime.retained_change_pending(instance) {
            return present;
        }
        self.store(present.take());
        self.take_or_rebuild_stabilized(package, world, frame, producer, host, context)
    }

    fn present(
        &mut self,
        document: &crate::aui::AuiDocument,
        snapshot_output: ProjectUiStateSnapshotOutput,
        package: &RuntimePackage,
        presentation: Option<&ResolvedGameViewPresentation>,
        viewport: Option<&AuiViewportInput>,
    ) -> AuiRuntimePresentOutput {
        if let Some(viewport) = viewport {
            AuiRuntimePresenter::present_project_snapshot_with_fonts_for_presentation_cached_with_themes_and_viewport(
                document,
                snapshot_output,
                &package.font_atlases,
                &package.font_bundles,
                &package.aui_themes,
                presentation,
                viewport,
                &mut self.c2_runtime,
            )
        } else {
            AuiRuntimePresenter::present_project_snapshot_with_fonts_for_presentation_cached_with_themes(
                document,
                snapshot_output,
                &package.font_atlases,
                &package.font_bundles,
                &package.aui_themes,
                presentation,
                &mut self.c2_runtime,
            )
        }
    }

    pub fn apply_virtual_list_actions(
        &mut self,
        present: &mut AuiRuntimePresentOutput,
        actions: &[crate::aui::AuiAction],
    ) {
        if self
            .virtual_lists
            .apply_actions(actions, &mut present.virtual_lists)
        {
            self.last_virtual_interaction_identity = None;
        }
    }

    pub fn current(&self) -> Option<&AuiRuntimePresentOutput> {
        self.last_present.as_ref()
    }

    pub fn c2_runtime(&self) -> &AuiC2Runtime {
        &self.c2_runtime
    }

    pub fn virtual_list_lifecycle_stats(
        &self,
    ) -> crate::aui_virtual_list::AuiVirtualListLifecycleStats {
        self.virtual_lists.lifecycle_stats()
    }

    pub fn custom_extension_frame(
        &self,
        present: &AuiRuntimePresentOutput,
        package: &RuntimePackage,
    ) -> crate::aui_extension::AuiCustomFrame {
        let computed = present
            .layout
            .computed_nodes
            .iter()
            .map(|node| (node.node_id.as_str(), node))
            .collect::<std::collections::HashMap<_, _>>();
        let nodes = present
            .resolved_document
            .nodes
            .iter()
            .filter(|node| node.custom.is_some())
            .filter_map(|node| {
                let geometry = computed.get(node.node_id.as_str())?;
                let children = node
                    .children
                    .iter()
                    .filter_map(|child| {
                        let child_geometry = computed.get(child.as_str())?;
                        Some((
                            child.clone(),
                            [child_geometry.rect.width, child_geometry.rect.height],
                        ))
                    })
                    .collect();
                Some(crate::aui_extension::AuiCustomFrameNode {
                    node_id: node.node_id.clone(),
                    constraints: self
                        .c2_runtime
                        .custom_measure_inputs()
                        .get(&node.node_id)
                        .copied(),
                    size: [geometry.rect.width, geometry.rect.height],
                    children,
                })
            })
            .collect();
        crate::aui_extension::AuiCustomFrame {
            document_id: present.resolved_document.document_id.clone(),
            resource_identity: package.manifest.content_hash.clone().unwrap_or_else(|| {
                package
                    .manifest
                    .project
                    .runtime_module
                    .aot_content_digest
                    .clone()
            }),
            nodes,
        }
    }

    pub fn store(&mut self, present: Option<AuiRuntimePresentOutput>) {
        if !present
            .as_ref()
            .is_some_and(|output| output.report.status == AuiRuntimePresentStatus::Failed)
        {
            self.last_present = present;
        }
    }

    fn remember_static_variant(
        &mut self,
        key: StaticPresentVariantKey,
        output: &AuiRuntimePresentOutput,
    ) {
        if !static_present_variant_is_bounded(output.resolved_document.nodes.len()) {
            return;
        }
        if let Some(index) = self
            .static_variants
            .iter()
            .position(|(candidate, _)| candidate == &key)
        {
            self.static_variants.remove(index);
        }
        self.static_variants.push_front((key, output.clone()));
        self.static_variants.truncate(MAX_STATIC_PRESENT_VARIANTS);
    }
}

fn static_present_variant_is_bounded(node_count: usize) -> bool {
    node_count <= MAX_STATIC_PRESENT_VARIANT_NODES
}

fn active_aui_binding_paths(document: &crate::aui::AuiDocument) -> Vec<String> {
    document
        .nodes
        .iter()
        .flat_map(|node| node.binding_refs.iter().map(|binding| binding.path.clone()))
        .chain(
            document
                .virtual_lists
                .iter()
                .map(|declaration| declaration.data_path.clone()),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aui_instance::AuiInstance;
    use crate::aui_layout_schema::{LayoutMode, LayoutSpec, AUI_LAYOUT_SCHEMA_VERSION};
    use crate::font_bundle::*;
    use crate::runtime_asset::{RuntimeAssetIndex, RuntimePackageMountTable};
    use crate::runtime_package::*;
    use project_runtime_sdk::aui::{UiProperty, UiPropertyOwner, UiValue, UiVisibility, UiWrite};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn text_test_font_registry() -> RuntimeFontBundleRegistry {
        let bytes = include_bytes!("../../../resources/editor/fonts/NotoSansSC-VF.ttf").to_vec();
        let source_digest = crate::canonical_digest::sha256_prefixed(&bytes);
        let metadata = CookedFontBundleAsset {
            schema_version: COOKED_FONT_BUNDLE_SCHEMA_VERSION.into(),
            font_bundle_id: "test-font".into(),
            font_stack_id: "test-stack".into(),
            generation: 1,
            max_bitmap_pages: 1,
            max_msdf_pages: 1,
            legacy_mode: false,
            fallback_used: false,
            quality_gate_eligible: true,
            pages: Vec::new(),
            glyphs: Vec::new(),
            kerning_adjustments: Vec::new(),
            font_faces: vec![RuntimeFontFaceDescriptor {
                font_face_id: "noto".into(),
                face_index: 0,
                source_digest: source_digest.clone(),
                payload_path: "noto.ttf".into(),
            }],
            shaping_recipe: Some(crate::text_shaping::SHAPING_RECIPE.into()),
            bundle_digest: "sha256:test-font-bundle".into(),
        };
        let mut registry = RuntimeFontBundleRegistry {
            default_ui_font_bundle_id: Some("test-font".into()),
            ..Default::default()
        };
        Arc::make_mut(&mut registry.bundles_by_id).insert(
            "test-font".into(),
            RuntimeLoadedFontBundle {
                metadata,
                page_payloads: Vec::new(),
                font_face_sources: vec![RuntimePackageSourceFontFace {
                    font_face_id: "noto".into(),
                    face_index: 0,
                    source_digest,
                    bytes,
                }],
            },
        );
        registry
    }

    struct ReusingProducer {
        fail_next: bool,
    }

    fn render_test_font_registry(text: &str) -> RuntimeFontBundleRegistry {
        let mut registry = text_test_font_registry();
        let bundle = Arc::make_mut(&mut registry.bundles_by_id)
            .get_mut("test-font")
            .unwrap();
        bundle.metadata.pages.push(CookedFontBundlePage {
            page_index: 0,
            render_mode: FontBundleRenderMode::BitmapR8,
            format: "r8Unorm".into(),
            width: 64,
            height: 64,
            byte_len: 4096,
            sha256: "sha256:gate-d-test-page".into(),
            payload_path: "page.r8".into(),
        });
        bundle.page_payloads.push(vec![255; 4096]);
        let glyph_ids: BTreeSet<_> =
            crate::text_shaping::shape_text("noto", &bundle.font_face_sources[0].bytes, text)
                .unwrap()
                .items
                .into_iter()
                .filter(|glyph| glyph.has_outline)
                .map(|glyph| glyph.glyph_id as u16)
                .collect();
        bundle.metadata.glyphs = glyph_ids
            .into_iter()
            .enumerate()
            .map(|(i, glyph_id)| CookedFontBundleGlyph {
                font_family_id: "family-ui".into(),
                font_face_id: "noto".into(),
                style: FontBundleStyle::Normal,
                weight: 400,
                glyph_id,
                codepoint: u32::MAX,
                render_mode: FontBundleRenderMode::BitmapR8,
                pixel_size: 12,
                page_index: 0,
                pixel_rect: [(i as u32 % 7) * 8, (i as u32 / 7) * 12, 8, 12],
                bearing_x: 0,
                bearing_y: 12,
                advance_per_em_millionths: 600_000,
                plane_bounds_per_em_millionths: None,
            })
            .collect();
        registry
    }

    #[test]
    fn gate_d_single_text_and_translation_keep_drawing_glyph_and_hover_work_local() {
        for count in [400, 1_000, 10_000] {
            let mut package = retained_package();
            package.font_bundles = render_test_font_registry("firstsecond…");
            let document = package.aui_documents.documents_by_id.get_mut("ui").unwrap();
            let unit = &mut document.nodes[1];
            unit.kind = AuiNodeKind::Button;
            unit.text = Some("first".into());
            unit.interactable = true;
            unit.feedback = AuiFeedbackSelection::none();
            unit.rect = AuiRect::fixed_position(10., 10., 80., 30.);
            unit.slot = Some(crate::aui_layout_schema::SlotSpec {
                width: crate::aui_layout_schema::Dimension::Fixed { value: 80. },
                height: crate::aui_layout_schema::Dimension::Fixed { value: 30. },
                participation: crate::aui_layout_schema::Participation::OutOfFlow,
                ..Default::default()
            });
            unit.style = Some(AuiStyle {
                color: None,
                text_color: None,
                font_size: Some(12.),
                font: None,
            });
            for i in 2..count {
                let id = format!("text-{i}");
                document.nodes[0].children.push(id.clone());
                let mut text = document.nodes[1].clone();
                text.node_id = id;
                text.kind = AuiNodeKind::Text;
                text.interactable = false;
                document.nodes.push(text);
            }
            let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
            let unit = instance.node("unit").unwrap();
            let text = instance.node("text-2").unwrap();
            let mut cache = AuiPresentCache::for_package(&package);
            let mut producer = ReusingProducer { fail_next: false };
            let mut state = crate::aui_control_feedback::AuiControlFeedbackState::default();
            let mut interaction_state = AuiInteractionState::default();
            let mut interaction = AuiInteractionResult::default();
            interaction.control_snapshot.hovered_node = Some("unit".into());
            let mut first =
                present_retained(&mut cache, &package, &instance, &mut producer, 1).unwrap();
            assert_eq!(
                first.report.status,
                AuiRuntimePresentStatus::Success,
                "{count}: {:?}",
                first.report.diagnostics
            );
            AuiRuntimePresenter::apply_control_feedback_with_fonts_and_themes(
                &mut first,
                &interaction,
                &mut state,
                250_000,
                &package.font_atlases,
                &package.font_bundles,
                &package.aui_themes,
            );
            let old_overlay = first.overlay.clone();
            cache.store(Some(first));
            for frame in 2..=3 {
                let base_before = cache.c2_runtime().drawing_stats();
                let feedback_before = state.drawing_stats();
                let mut edit = instance.edit();
                if frame == 2 {
                    edit.write(
                        unit,
                        UiProperty::Translation,
                        UiValue::Translation([10., 0.]),
                    )
                    .unwrap();
                } else {
                    edit.write(text, UiProperty::Text, UiValue::Text("second".into()))
                        .unwrap();
                }
                edit.commit().unwrap();
                let mut context = AuiPresentContext::new(None, None, &mut interaction_state)
                    .with_feedback_cache(&mut state);
                context.retained_instance = Some(&instance);
                let mut changed = cache
                    .take_or_rebuild(&package, &World::new(), frame, &mut producer, context)
                    .unwrap();
                assert_eq!(changed.report.status, AuiRuntimePresentStatus::Success);
                assert_eq!(
                    cache.c2_runtime().prepared_document_nodes_copied(),
                    0,
                    "{count}/{frame}"
                );
                let delta = cache.c2_runtime().drawing_stats().since(base_before);
                assert_eq!(delta.nodes_visited, 1, "{count}/{frame}: {delta:?}");
                assert_eq!(delta.items_visited, if frame == 2 { 2 } else { 1 });
                assert_eq!(delta.text_items_visited, 1);
                interaction.control_snapshot.frame_id = frame;
                let feedback = AuiRuntimePresenter::apply_control_feedback_with_fonts_and_themes(
                    &mut changed,
                    &interaction,
                    &mut state,
                    250_000,
                    &package.font_atlases,
                    &package.font_bundles,
                    &package.aui_themes,
                );
                let delta = state.drawing_stats().since(feedback_before);
                assert_eq!(feedback.report.document_nodes_scanned, 1);
                assert_eq!(
                    delta.nodes_visited,
                    if frame == 2 { 1 } else { 2 },
                    "{count}/{frame}: {delta:?}"
                );
                assert_eq!(delta.items_visited, if frame == 2 { 2 } else { 1 });
                assert_eq!(delta.text_items_visited, 1);
                let mut fresh_cache = AuiPresentCache::for_package(&package);
                let mut fresh =
                    present_retained(&mut fresh_cache, &package, &instance, &mut producer, frame)
                        .unwrap();
                AuiRuntimePresenter::apply_control_feedback_with_fonts_and_themes(
                    &mut fresh,
                    &interaction,
                    &mut Default::default(),
                    250_000,
                    &package.font_atlases,
                    &package.font_bundles,
                    &package.aui_themes,
                );
                assert_eq!(changed.overlay, fresh.overlay, "{count}/{frame}");
                assert_eq!(changed.composition, fresh.composition, "{count}/{frame}");
                cache.store(Some(changed));
            }
            assert_eq!(
                old_overlay
                    .draw_items
                    .iter()
                    .find(|item| item.node_id == "unit:label")
                    .unwrap()
                    .text
                    .as_deref(),
                Some("first")
            );
            assert_eq!(
                old_overlay
                    .draw_items
                    .iter()
                    .find(|item| item.node_id == "unit")
                    .unwrap()
                    .rect
                    .x,
                10.
            );
        }
    }

    #[test]
    fn gate_d_measurement_is_opt_in_and_reuse_still_counts_production_boundary() {
        let package = retained_package();
        let mut instance =
            AuiInstance::mount(Arc::new(package.aui_documents.get("ui").unwrap().clone())).unwrap();
        let mut cache = AuiPresentCache::for_package(&package);
        let mut producer = ReusingProducer { fail_next: false };
        let first = present_retained(&mut cache, &package, &instance, &mut producer, 1).unwrap();
        cache.store(Some(first));
        assert_eq!(cache.update_timings(), AuiUpdateTimings::default());
        cache.set_measurement_enabled(true);
        let clean = present_retained(&mut cache, &package, &instance, &mut producer, 2).unwrap();
        cache.store(Some(clean));
        let before = cache.update_timings();
        let before_work = cache.c2_runtime().update_work();
        assert!(before.production_exchange_diff_ns > 0);
        assert_eq!(before.draw_projection_glyph_ns, 0);
        let unit = instance.node("unit").unwrap();
        let mut edit = instance.edit();
        edit.write(
            unit,
            UiProperty::Translation,
            UiValue::Translation([1., 0.]),
        )
        .unwrap();
        edit.commit().unwrap();
        let moved = present_retained(&mut cache, &package, &instance, &mut producer, 3).unwrap();
        cache.store(Some(moved));
        let delta = cache.update_timings().since(before);
        assert!(delta.production_exchange_diff_ns > 0);
        assert!(delta.prepare_style_preflight_ns > 0);
        assert!(delta.transform_clip_finalize_ns > 0);
        assert!(delta.draw_projection_glyph_ns > 0);
        let work = cache.c2_runtime().update_work().since(before_work);
        assert_eq!(
            work.layout_nodes_visited, 0,
            "translation must not repeat old solver work"
        );
        assert_eq!(work.geometry_nodes_updated, 1);
        cache.set_measurement_enabled(false);
        let before = cache.update_timings();
        present_retained(&mut cache, &package, &instance, &mut producer, 4).unwrap();
        assert_eq!(cache.update_timings(), before);
    }

    struct ReplacingProducer {
        visible: bool,
        fail_next: bool,
    }

    struct BindingValueProducer {
        value: Option<AuiBindingValue>,
        revision: u64,
        previous_seen: Option<u64>,
    }

    struct OrderedBindingProducer {
        first: bool,
        last: bool,
    }

    impl ProjectUiStateSnapshotProducer for OrderedBindingProducer {
        fn producer_id(&self) -> &str {
            "ordered-binding-test"
        }
        fn produce(
            &mut self,
            context: ProjectUiStateProducerContext<'_>,
        ) -> ProjectUiStateSnapshotOutput {
            ProjectUiStateSnapshotOutput::new(
                self.producer_id(),
                AuiSnapshotSource::ProjectProducer,
                ProjectUiStateSnapshot::new(context.frame_index)
                    .with_value("test.first", AuiBindingValue::Bool(self.first))
                    .with_value("test.last", AuiBindingValue::Bool(self.last)),
            )
        }
        fn resolve(
            &mut self,
            context: ProjectUiStateProducerContext<'_>,
        ) -> Result<ProjectUiStateResolve, ProjectUiStateResolveError> {
            Ok(ProjectUiStateResolve::Replace {
                identity: ProjectUiStateIdentity {
                    producer_epoch: 1,
                    visible_revision: context.frame_index,
                    binding_set: context.binding_set.identity().clone(),
                },
                output: self.produce(context),
            })
        }
    }

    impl ProjectUiStateSnapshotProducer for BindingValueProducer {
        fn producer_id(&self) -> &str {
            "aui-binding-value-test"
        }

        fn produce(
            &mut self,
            context: ProjectUiStateProducerContext<'_>,
        ) -> ProjectUiStateSnapshotOutput {
            let mut snapshot = ProjectUiStateSnapshot::new(context.frame_index);
            if let Some(value) = &self.value {
                snapshot.values.insert("test.value".into(), value.clone());
            }
            ProjectUiStateSnapshotOutput::new(
                self.producer_id(),
                AuiSnapshotSource::ProjectProducer,
                snapshot,
            )
        }

        fn resolve(
            &mut self,
            context: ProjectUiStateProducerContext<'_>,
        ) -> Result<ProjectUiStateResolve, ProjectUiStateResolveError> {
            self.previous_seen = context
                .previous_identity
                .as_ref()
                .map(|id| id.visible_revision);
            if self.previous_seen == Some(self.revision) {
                return Ok(ProjectUiStateResolve::Reuse {
                    identity: context
                        .previous_identity
                        .expect("matching previous identity"),
                });
            }
            Ok(ProjectUiStateResolve::Replace {
                identity: ProjectUiStateIdentity {
                    producer_epoch: 1,
                    visible_revision: self.revision,
                    binding_set: context.binding_set.identity().clone(),
                },
                output: self.produce(context),
            })
        }
    }

    impl ProjectUiStateSnapshotProducer for ReplacingProducer {
        fn producer_id(&self) -> &str {
            "aui-replacing-producer-test"
        }

        fn produce(
            &mut self,
            context: ProjectUiStateProducerContext<'_>,
        ) -> ProjectUiStateSnapshotOutput {
            ProjectUiStateSnapshotOutput::new(
                self.producer_id(),
                AuiSnapshotSource::ProjectProducer,
                ProjectUiStateSnapshot::new(context.frame_index)
                    .with_value("test.unit_visible", AuiBindingValue::Bool(self.visible))
                    .with_value(
                        "test.unused",
                        AuiBindingValue::Number(context.frame_index as f32),
                    ),
            )
        }

        fn resolve(
            &mut self,
            context: ProjectUiStateProducerContext<'_>,
        ) -> Result<ProjectUiStateResolve, ProjectUiStateResolveError> {
            if self.fail_next {
                self.fail_next = false;
                return Err(ProjectUiStateResolveError::new(
                    "test.failure",
                    "intentional replacement failure",
                ));
            }
            Ok(ProjectUiStateResolve::Replace {
                identity: ProjectUiStateIdentity {
                    producer_epoch: 1,
                    visible_revision: context.frame_index,
                    binding_set: context.binding_set.identity().clone(),
                },
                output: self.produce(context),
            })
        }
    }

    impl ProjectUiStateSnapshotProducer for ReusingProducer {
        fn producer_id(&self) -> &str {
            "aui-present-cache-test"
        }

        fn produce(
            &mut self,
            context: ProjectUiStateProducerContext<'_>,
        ) -> ProjectUiStateSnapshotOutput {
            ProjectUiStateSnapshotOutput::new(
                self.producer_id(),
                AuiSnapshotSource::ProjectProducer,
                ProjectUiStateSnapshot::new(context.frame_index),
            )
        }

        fn resolve(
            &mut self,
            context: ProjectUiStateProducerContext<'_>,
        ) -> Result<ProjectUiStateResolve, ProjectUiStateResolveError> {
            if self.fail_next {
                self.fail_next = false;
                return Err(ProjectUiStateResolveError::new(
                    "test.failure",
                    "intentional failure",
                ));
            }
            let identity = ProjectUiStateIdentity {
                producer_epoch: 1,
                visible_revision: 1,
                binding_set: context.binding_set.identity().clone(),
            };
            if context.previous_identity.as_ref() == Some(&identity) {
                Ok(ProjectUiStateResolve::Reuse { identity })
            } else {
                Ok(ProjectUiStateResolve::Replace {
                    identity,
                    output: self.produce(context),
                })
            }
        }
    }

    fn retained_package() -> RuntimePackage {
        let mut root = AuiNode::new("root", AuiNodeKind::Panel, AuiRect::stretch_full());
        root.layout = Some(LayoutSpec {
            mode: LayoutMode::Overlay,
            ..Default::default()
        });
        root.children = vec!["unit".into()];
        let mut unit = AuiNode::new(
            "unit",
            AuiNodeKind::Panel,
            AuiRect::fixed_position(10.0, 10.0, 30.0, 30.0),
        );
        unit.parent = Some("root".into());
        let mut document = AuiDocument::new(
            "ui",
            vec![AuiCanvas::screen_overlay("main", 160.0, 120.0, "root")],
            vec![root, unit],
        );
        document.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
        let mut aui_documents = RuntimeAuiDocumentRegistry::empty("test");
        aui_documents.documents_by_id.insert("ui".into(), document);
        let assets = RuntimeAssetManifest {
            schema_version: RUNTIME_ASSET_MANIFEST_SCHEMA_VERSION.into(),
            assets: Vec::new(),
            runtime_asset_index: Vec::new(),
            bundle_table: Vec::new(),
            cooked_asset_table: Vec::new(),
            dependency_table: Vec::new(),
        };
        RuntimePackage {
            package_dir: PathBuf::new(),
            source: crate::runtime_package_source::RuntimePackageSource::directory(PathBuf::new()),
            manifest: RuntimePackageManifest {
                schema_version: RUNTIME_PACKAGE_SCHEMA_VERSION.into(),
                package_mode: RUNTIME_PACKAGE_MODE.into(),
                project: RuntimeProjectInfo::explicit_empty("test", "Test", "1"),
                active_scene_id: "scene".into(),
                scenes: Vec::new(),
                assets: RuntimeManifestAssetIndex {
                    path: String::new(),
                    asset_count: 0,
                },
                rules: RuntimeManifestRuleIndex {
                    path: String::new(),
                    mode: "none".into(),
                },
                input: RuntimeManifestInputIndex {
                    path: String::new(),
                    default_mapping_id: String::new(),
                    mapping_count: 0,
                },
                aui: None,
                font_atlases: None,
                font_bundles: None,
                animator2d: None,
                spine_animator: None,
                observation_contract: None,
                content_hash: None,
            },
            active_scene: RuntimeScene {
                main_camera: None,
                schema_version: RUNTIME_SCENE_SCHEMA_VERSION.into(),
                id: "scene".into(),
                name: "Scene".into(),
                gravity: 0.0,
                background: "#000000".into(),
                sky_color: "#000000".into(),
                entities: Vec::new(),
            },
            runtime_asset_index: RuntimeAssetIndex::from_manifest(&assets, &[], &[], &[]),
            runtime_asset_mount_table: RuntimePackageMountTable::from_manifest(&assets),
            assets,
            rules: RuntimeRuleManifest {
                schema_version: RUNTIME_RULE_MANIFEST_SCHEMA_VERSION.into(),
                mode: "none".into(),
                rules: Vec::new(),
                modules: Vec::new(),
            },
            aui_manifest: RuntimeAuiManifest {
                schema_version: RUNTIME_AUI_MANIFEST_SCHEMA_VERSION.into(),
                documents: vec![RuntimeAuiManifestEntry {
                    document_id: "ui".into(),
                    path: String::new(),
                    canvas_count: 1,
                    node_count: 2,
                    binding_count: 0,
                    action_count: 0,
                    asset_refs: Vec::new(),
                }],
            },
            aui_documents,
            aui_themes: crate::aui_theme::AuiThemeRegistry::builtin(),
            font_atlas_manifest: RuntimeFontAtlasManifest::empty(),
            font_atlases: RuntimeAuiFontAtlasRegistry::empty("test"),
            font_bundle_manifest: crate::font_bundle::RuntimeFontBundleManifest::empty(),
            font_bundles: crate::font_bundle::RuntimeFontBundleRegistry::default(),
            animator2d_registry: crate::animator2d::CookedAnimator2DRegistry::empty(),
            spine_animator_registry: None,
            input_manifest: RuntimeInputManifest {
                schema_version: RUNTIME_INPUT_MANIFEST_SCHEMA_VERSION.into(),
                default_mapping_id: String::new(),
                mappings: Vec::new(),
            },
            input_mappings: Vec::new(),
            default_input_mapping: None,
        }
    }

    fn present_retained(
        cache: &mut AuiPresentCache,
        package: &RuntimePackage,
        instance: &AuiInstance,
        producer: &mut dyn ProjectUiStateSnapshotProducer,
        frame: u64,
    ) -> Option<AuiRuntimePresentOutput> {
        let mut interaction = AuiInteractionState::default();
        let mut context = AuiPresentContext::new(None, None, &mut interaction);
        context.retained_instance = Some(instance);
        cache.take_or_rebuild(package, &World::new(), frame, producer, context)
    }

    #[test]
    fn classic_retained_animation_updates_only_affected_geometry_and_drawing() {
        for count in [601, 10_000] {
            let mut package = retained_package();
            let document = package.aui_documents.documents_by_id.get_mut("ui").unwrap();
            document.schema_version = "aui-document.v2".into();
            document.nodes[0].layout = None;
            for i in 2..count {
                let id = format!("pool-{i}");
                document.nodes[0].children.push(id.clone());
                document.nodes.push(
                    AuiNode::new(
                        id,
                        AuiNodeKind::Panel,
                        AuiRect::fixed_position(60., 10., 20., 20.),
                    )
                    .with_parent("root"),
                );
            }
            let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
            let unit = instance.node("unit").unwrap();
            let mut cache = AuiPresentCache::for_package(&package);
            let mut producer = ReusingProducer { fail_next: false };
            let initial =
                present_retained(&mut cache, &package, &instance, &mut producer, 1).unwrap();
            assert_eq!(initial.report.status, AuiRuntimePresentStatus::Success);
            let initial_layout = initial.layout.clone();
            cache.store(Some(initial));
            for frame in 2..=4 {
                let before = cache.c2_runtime().drawing_stats();
                let mut edit = instance.edit();
                edit.write(
                    unit,
                    UiProperty::Translation,
                    UiValue::Translation([10. * (frame - 1) as f32, 0.]),
                )
                .unwrap();
                edit.commit().unwrap();
                let changed =
                    present_retained(&mut cache, &package, &instance, &mut producer, frame)
                        .unwrap();
                let delta = cache.c2_runtime().drawing_stats().since(before);
                assert_eq!(delta.nodes_visited, 1, "{count}/{frame}: {delta:?}");
                assert!(cache.c2_runtime().geometry_update_stats().base_reused);
                assert_eq!(cache.c2_runtime().geometry_update_stats().nodes_updated, 1);
                let mut fresh_cache = AuiPresentCache::for_package(&package);
                let fresh =
                    present_retained(&mut fresh_cache, &package, &instance, &mut producer, frame)
                        .unwrap();
                assert_eq!(changed.layout.computed_nodes, fresh.layout.computed_nodes);
                assert_eq!(changed.overlay, fresh.overlay);
                assert_eq!(changed.composition, fresh.composition);
                assert_eq!(initial_layout.computed_nodes[1].rect.x, 10.);
                cache.store(Some(changed));
            }
        }
    }

    #[test]
    fn classic_retained_parent_transforms_and_content_invalidation_match_full_rebuild() {
        use project_runtime_sdk::aui::UiVisibility;
        let mut package = retained_package();
        let document = package.aui_documents.documents_by_id.get_mut("ui").unwrap();
        document.schema_version = "aui-document.v2".into();
        document.nodes[0].layout = None;
        document.nodes[1].clip_policy = AuiClipPolicy::Rect;
        document.nodes[1].children.push("child".into());
        document.nodes.push(
            AuiNode::new(
                "child",
                AuiNodeKind::Panel,
                AuiRect::fixed_position(20., 5., 30., 20.),
            )
            .with_parent("unit"),
        );
        let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
        let root = instance.node("root").unwrap();
        let unit = instance.node("unit").unwrap();
        let mut cache = AuiPresentCache::for_package(&package);
        let mut producer = ReusingProducer { fail_next: false };
        let initial = present_retained(&mut cache, &package, &instance, &mut producer, 1).unwrap();
        let old_overlay = initial.overlay.clone();
        cache.store(Some(initial));
        let changes = [
            (
                unit,
                UiProperty::Translation,
                UiValue::Translation([20., 0.]),
                true,
                2,
            ),
            (
                unit,
                UiProperty::Scale,
                UiValue::Vector([1.5, 0.8]),
                true,
                2,
            ),
            (unit, UiProperty::Rotation, UiValue::Number(0.2), true, 2),
            (unit, UiProperty::Opacity, UiValue::Number(0.3), true, 2),
            (
                unit,
                UiProperty::Translation,
                UiValue::Translation([0., 0.]),
                true,
                2,
            ),
            (
                unit,
                UiProperty::Visibility,
                UiValue::Visibility(UiVisibility::Hidden),
                false,
                3,
            ),
            (
                unit,
                UiProperty::Visibility,
                UiValue::Visibility(UiVisibility::Visible),
                false,
                3,
            ),
            (
                unit,
                UiProperty::Translation,
                UiValue::Translation([-10., 0.]),
                true,
                2,
            ),
            (
                root,
                UiProperty::Translation,
                UiValue::Translation([5., 5.]),
                true,
                3,
            ),
            (unit, UiProperty::Rotation, UiValue::Number(0.), true, 2),
        ];
        for (index, (handle, property, value, reused, affected)) in changes.into_iter().enumerate()
        {
            let frame = index as u64 + 2;
            let mut edit = instance.edit();
            edit.write(handle, property, value).unwrap();
            edit.commit().unwrap();
            let changed =
                present_retained(&mut cache, &package, &instance, &mut producer, frame).unwrap();
            let stats = cache.c2_runtime().geometry_update_stats();
            assert_eq!(stats.base_reused, reused, "frame {frame}");
            assert_eq!(stats.nodes_updated, affected, "frame {frame}");
            let mut fresh_cache = AuiPresentCache::for_package(&package);
            let fresh =
                present_retained(&mut fresh_cache, &package, &instance, &mut producer, frame)
                    .unwrap();
            assert_eq!(changed.layout, fresh.layout, "frame {frame}");
            assert_eq!(changed.overlay, fresh.overlay, "frame {frame}");
            assert_eq!(changed.composition, fresh.composition, "frame {frame}");
            cache.store(Some(changed));
        }
        assert_eq!(
            old_overlay
                .draw_items
                .iter()
                .find(|item| item.node_id == "unit")
                .unwrap()
                .rect
                .x,
            10.
        );
    }

    #[test]
    fn affine_leaf_update_reuses_preparation_sibling_drawing_and_clean_publication() {
        use project_runtime_sdk::aui::UiNodeRef;
        let mut package = retained_package();
        let document = package.aui_documents.documents_by_id.get_mut("ui").unwrap();
        for i in 0..1000 {
            let id = format!("sibling-{i}");
            document.nodes[0].children.push(id.clone());
            document.nodes.push(
                AuiNode::new(
                    id,
                    AuiNodeKind::Panel,
                    AuiRect::fixed_position(60., 10., 20., 20.),
                )
                .with_parent("root"),
            );
        }
        let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
        let mut cache = AuiPresentCache::for_package(&package);
        let mut producer = ReusingProducer { fail_next: false };
        let before = present_retained(&mut cache, &package, &instance, &mut producer, 1).unwrap();
        cache.store(Some(before.clone()));
        let handle = instance.node("unit").unwrap();
        let mut edit = instance.edit();
        UiNodeRef::new(handle).set_rotation(&mut edit, 0.3).unwrap();
        edit.commit().unwrap();
        let after = present_retained(&mut cache, &package, &instance, &mut producer, 2).unwrap();
        assert_eq!(cache.c2_runtime().geometry_update_stats().nodes_updated, 1);
        assert_eq!(cache.c2_runtime().prepared_binding_nodes_rebuilt(), 0);
        assert_eq!(
            before.layout.computed_nodes[2],
            after.layout.computed_nodes[2]
        );
        let old = before
            .overlay
            .draw_items
            .iter()
            .find(|i| i.node_id == "sibling-999")
            .unwrap();
        let new = after
            .overlay
            .draw_items
            .iter()
            .find(|i| i.node_id == "sibling-999")
            .unwrap();
        assert!(Arc::ptr_eq(old, new));
        cache.store(Some(after.clone()));
        let rebuilds = cache.rebuild_count;
        let clean = present_retained(&mut cache, &package, &instance, &mut producer, 3).unwrap();
        assert_eq!(cache.rebuild_count, rebuilds);
        assert!(Arc::ptr_eq(&after.layout, &clean.layout));
    }

    struct AnimationHost {
        ui: crate::aui_session::AuiSession,
    }
    impl AuiExtensionFrameHost for AnimationHost {
        fn retained_ui(&self) -> Option<&crate::aui_session::AuiSession> {
            Some(&self.ui)
        }
        fn publish_ui_geometry(&mut self, output: &AuiRuntimePresentOutput) {
            self.ui.publish_geometry(output);
        }
        fn advance_ui_animations(
            &mut self,
            frame: u64,
            delta_us: u64,
        ) -> Result<(), project_runtime_sdk::aui::UiError> {
            self.ui.advance_animations(frame, delta_us)
        }
        fn cancel_drag_animations(&mut self, document: &str, interaction: &AuiInteractionResult) {
            self.ui.cancel_drag_animations(document, interaction);
        }
        fn refresh_ui_extensions_for_frame(
            &mut self,
            _: &crate::aui_extension::AuiCustomFrame,
            _: bool,
        ) -> Result<crate::aui_extension::AuiCustomFrameResult, String> {
            Ok(Default::default())
        }
    }

    #[test]
    fn aui_animation_binding_changes_remain_base_while_temporarily_overridden() {
        use project_runtime_sdk::aui::{UiAccess, UiRead, UiValue};
        use project_runtime_sdk::aui_animation::*;
        let mut package = retained_package();
        package
            .aui_documents
            .documents_by_id
            .get_mut("ui")
            .unwrap()
            .nodes[1]
            .binding_refs
            .push(AuiBindingRef::new(
                "opacity",
                AuiBindingTarget::Opacity,
                "test.value",
                None,
            ));
        let mut host = AnimationHost {
            ui: crate::aui_session::AuiSession::mount(&package.aui_documents).unwrap(),
        };
        let target = host.ui.resolve("ui", "unit").unwrap();
        let world = World::new();
        let mut cache = AuiPresentCache::for_package(&package);
        let mut input = AuiInteractionState::default();
        let mut producer = BindingValueProducer {
            value: Some(AuiBindingValue::Number(0.8)),
            revision: 1,
            previous_seen: None,
        };
        let initial = cache
            .take_or_rebuild_stabilized(
                &package,
                &world,
                1,
                &mut producer,
                &mut host,
                AuiPresentContext::new(None, None, &mut input),
            )
            .unwrap();
        assert_eq!(
            host.ui.read(target, UiProperty::Opacity).unwrap(),
            UiValue::Number(0.8)
        );
        host.ui.begin();
        let reply = host
            .ui
            .animation_request(AnimationRequest::Play {
                request: AnimationPlay::new(
                    Animation2D {
                        version: ANIMATION_2D_VERSION,
                        root: AnimationStep::Tween {
                            track: TweenTrack {
                                target,
                                property: TweenProperty::Opacity,
                                from: TweenValue::Scalar(0.5),
                                to: TweenValue::Scalar(0.5),
                                duration_us: 100,
                                delay_us: 0,
                                easing: Easing::Linear,
                            },
                        },
                        repeat: Repeat::Once,
                        direction: PlaybackDirection::Forward,
                        completion: CompletionPolicy::Hold,
                    },
                    target,
                ),
            })
            .unwrap();
        let AnimationReply::Handle(handle) = reply else {
            panic!("expected playback")
        };
        host.ui.finish(true);
        let animated = cache
            .advance_animations_for_present(
                Some(initial),
                &package,
                &world,
                &mut producer,
                &mut host,
                AuiAnimationPresentContext {
                    frame_index: 2,
                    delta_us: 50,
                    interaction_result: None,
                    presentation: AuiPresentContext::new(None, None, &mut input),
                },
            )
            .unwrap();
        assert_eq!(
            animated.layout.computed_nodes[1]
                .presentation
                .unwrap()
                .modulation[3],
            0.5
        );
        assert_eq!(
            host.ui.read(target, UiProperty::Opacity).unwrap(),
            UiValue::Number(0.8)
        );
        cache.store(Some(animated));
        producer.value = Some(AuiBindingValue::Number(0.3));
        producer.revision = 2;
        let updated = cache
            .take_or_rebuild_stabilized(
                &package,
                &world,
                3,
                &mut producer,
                &mut host,
                AuiPresentContext::new(None, None, &mut input),
            )
            .unwrap();
        assert_eq!(
            updated.layout.computed_nodes[1]
                .presentation
                .unwrap()
                .modulation[3],
            0.5
        );
        assert_eq!(
            host.ui.read(target, UiProperty::Opacity).unwrap(),
            UiValue::Number(0.3)
        );
        host.ui.begin();
        host.ui
            .animation_request(AnimationRequest::Cancel { handle })
            .unwrap();
        host.ui.finish(true);
        let restored = cache
            .advance_animations_for_present(
                Some(updated),
                &package,
                &world,
                &mut producer,
                &mut host,
                AuiAnimationPresentContext {
                    frame_index: 4,
                    delta_us: 0,
                    interaction_result: None,
                    presentation: AuiPresentContext::new(None, None, &mut input),
                },
            )
            .unwrap();
        assert_eq!(
            restored.layout.computed_nodes[1]
                .presentation
                .unwrap()
                .modulation[3],
            0.3
        );
    }

    #[test]
    fn aui_animation_production_phase_shares_geometry_feedback_and_sparse_cache() {
        use project_runtime_sdk::aui::{UiAccess, UiGeometryAccess, UiNodeRef, UiRead};
        use project_runtime_sdk::aui_animation::*;
        let mut package = retained_package();
        let document = package.aui_documents.documents_by_id.get_mut("ui").unwrap();
        document.nodes[1].kind = AuiNodeKind::Button;
        document.nodes[1].interactable = true;
        document.nodes[1].slot = Some(crate::aui_layout_schema::SlotSpec {
            width: crate::aui_layout_schema::Dimension::Fixed { value: 30. },
            height: crate::aui_layout_schema::Dimension::Fixed { value: 30. },
            participation: crate::aui_layout_schema::Participation::OutOfFlow,
            ..Default::default()
        });
        for index in 0..1000 {
            let id = format!("clean-{index}");
            document.nodes[0].children.push(id.clone());
            document.nodes.push(
                AuiNode::new(
                    id,
                    AuiNodeKind::Panel,
                    AuiRect::fixed_position(120., 80., 10., 10.),
                )
                .with_parent("root"),
            );
        }
        let mut host = AnimationHost {
            ui: crate::aui_session::AuiSession::mount(&package.aui_documents).unwrap(),
        };
        let target = host.ui.resolve("ui", "unit").unwrap();
        let mut cache = AuiPresentCache::for_package(&package);
        let world = World::new();
        let mut producer = ReusingProducer { fail_next: false };
        let mut input = AuiInteractionState::default();
        let mut feedback = crate::aui_control_feedback::AuiControlFeedbackState::default();
        let initial = cache
            .take_or_rebuild_stabilized(
                &package,
                &world,
                1,
                &mut producer,
                &mut host,
                AuiPresentContext::new(None, None, &mut input).with_feedback_cache(&mut feedback),
            )
            .unwrap();
        let initial_point = host.ui.local_to_screen(target, [15.; 2]).unwrap();
        let sibling = initial
            .overlay
            .draw_items
            .iter()
            .find(|i| i.node_id == "clean-999")
            .unwrap()
            .clone();
        host.ui.begin();
        host.ui
            .animation_request(AnimationRequest::Play {
                request: AnimationPlay {
                    owner: target,
                    scope: project_runtime_sdk::aui::UiSubscriptionScope::Active,
                    clock: AnimationClock::Ui,
                    conflict: TrackConflictPolicy::Reject,
                    callback_id: None,
                    animation: Animation2D {
                        version: ANIMATION_2D_VERSION,
                        root: AnimationStep::Tween {
                            track: TweenTrack {
                                target,
                                property: TweenProperty::Translation,
                                from: TweenValue::Vec2([0.; 2]),
                                to: TweenValue::Vec2([80., 0.]),
                                duration_us: 100,
                                delay_us: 0,
                                easing: Easing::Linear,
                            },
                        },
                        repeat: Repeat::Once,
                        direction: PlaybackDirection::Forward,
                        completion: CompletionPolicy::Hold,
                    },
                },
            })
            .unwrap();
        host.ui.finish(true);
        assert_eq!(
            host.ui.local_to_screen(target, [15.; 2]).unwrap(),
            initial_point,
            "input reads previous published geometry until advancement"
        );
        let mut present = cache
            .advance_animations_for_present(
                Some(initial),
                &package,
                &world,
                &mut producer,
                &mut host,
                AuiAnimationPresentContext {
                    frame_index: 2,
                    delta_us: 50,
                    interaction_result: None,
                    presentation: AuiPresentContext::new(None, None, &mut input)
                        .with_feedback_cache(&mut feedback),
                },
            )
            .unwrap();
        let moved = host.ui.local_to_screen(target, [15.; 2]).unwrap();
        assert_eq!(moved, [initial_point[0] + 40., initial_point[1]]);
        assert_eq!(cache.c2_runtime().geometry_update_stats().nodes_updated, 1);
        assert_eq!(cache.c2_runtime().prepared_binding_nodes_rebuilt(), 0);
        assert!(Arc::ptr_eq(
            &sibling,
            present
                .overlay
                .draw_items
                .iter()
                .find(|i| i.node_id == "clean-999")
                .unwrap()
        ));
        assert_eq!(
            AuiInteractionSystem::hit_test(
                &present.resolved_document,
                &present.layout,
                moved[0],
                moved[1]
            )
            .hit_node
            .as_deref(),
            Some("unit"),
            "point {moved:?}, unit {:?}, source {:?}",
            present.layout.computed_nodes[1],
            present.resolved_document.nodes[1]
        );
        assert_eq!(
            host.ui.read(target, UiProperty::Translation).unwrap(),
            UiValue::Translation([0.; 2])
        );
        // A committed base change forces the immediate matrix path; it must not add animation twice.
        host.ui.begin();
        let mut writes = project_runtime_sdk::aui_wire::UiWriteBuffer::new(&host.ui);
        UiNodeRef::new(target)
            .set_opacity(&mut writes, 0.8)
            .unwrap();
        host.ui.stage(writes.finish().unwrap()).unwrap();
        host.ui.finish(true);
        assert_eq!(host.ui.local_to_screen(target, [15.; 2]).unwrap(), moved);
        let mut interaction = AuiInteractionResult::default();
        interaction.control_snapshot.hovered_node = Some("unit".into());
        AuiRuntimePresenter::apply_control_feedback_with_fonts_and_themes(
            &mut present,
            &interaction,
            &mut feedback,
            250_000,
            &package.font_atlases,
            &package.font_bundles,
            &package.aui_themes,
        );
        host.ui.publish_geometry(&present);
        let feedback_point = host.ui.local_to_screen(target, [15.; 2]).unwrap();
        assert_eq!(
            AuiInteractionSystem::hit_test(
                &present.resolved_document,
                &present.layout,
                feedback_point[0],
                feedback_point[1]
            )
            .hit_node
            .as_deref(),
            Some("unit")
        );
        // Repeated frame IDs cannot move the animation again.
        let repeated = cache
            .advance_animations_for_present(
                Some(present),
                &package,
                &world,
                &mut producer,
                &mut host,
                AuiAnimationPresentContext {
                    frame_index: 2,
                    delta_us: 50,
                    interaction_result: None,
                    presentation: AuiPresentContext::new(None, None, &mut input)
                        .with_feedback_cache(&mut feedback),
                },
            )
            .unwrap();
        assert_eq!(
            host.ui.local_to_screen(target, [15.; 2]).unwrap(),
            feedback_point
        );
        let layout = repeated.layout.clone();
        let clean = cache
            .advance_animations_for_present(
                Some(repeated),
                &package,
                &world,
                &mut producer,
                &mut host,
                AuiAnimationPresentContext {
                    frame_index: 2,
                    delta_us: 50,
                    interaction_result: None,
                    presentation: AuiPresentContext::new(None, None, &mut input)
                        .with_feedback_cache(&mut feedback),
                },
            )
            .unwrap();
        assert!(Arc::ptr_eq(&layout, &clean.layout));
    }

    #[test]
    fn local_style_replacement_and_release_match_fresh_theme_without_layout_rebuild() {
        use project_runtime_sdk::aui::{UiLocalStyle, UiNodeRef};
        let mut package = retained_package();
        let document = package.aui_documents.documents_by_id.get_mut("ui").unwrap();
        document.nodes[1].binding_refs.push(AuiBindingRef::new(
            "style",
            AuiBindingTarget::LocalStyle,
            "test.value",
            None,
        ));
        let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
        let mut cache = AuiPresentCache::for_package(&package);
        let mut producer = BindingValueProducer {
            value: Some(AuiBindingValue::LocalStyle(UiLocalStyle {
                radius: Some(11.),
                ..Default::default()
            })),
            revision: 1,
            previous_seen: None,
        };
        let base = present_retained(&mut cache, &package, &instance, &mut producer, 1).unwrap();
        assert_eq!(
            base.resolved_document.nodes[1]
                .resolved_style
                .as_ref()
                .unwrap()
                .paint
                .radius,
            11.
        );
        cache.store(Some(base));
        let handle = instance.node("unit").unwrap();
        let node = UiNodeRef::new(handle);
        for (frame, style) in [
            (
                2,
                UiLocalStyle {
                    background: Some([1., 0., 0., 1.]),
                    radius: Some(9.),
                    ..Default::default()
                },
            ),
            (3, UiLocalStyle::default()),
        ] {
            let mut writes = project_runtime_sdk::aui_wire::UiWriteBuffer::new(&instance);
            node.take_property_ownership(&mut writes, UiProperty::LocalStyle)
                .unwrap();
            node.set_local_style(&mut writes, style).unwrap();
            instance
                .apply_update_batch(&writes.finish().unwrap())
                .unwrap();
            let changed =
                present_retained(&mut cache, &package, &instance, &mut producer, frame).unwrap();
            let mut fresh_cache = AuiPresentCache::for_package(&package);
            let fresh =
                present_retained(&mut fresh_cache, &package, &instance, &mut producer, frame)
                    .unwrap();
            assert_eq!(changed.resolved_document, fresh.resolved_document);
            assert_eq!(changed.overlay, fresh.overlay);
            cache.store(Some(changed));
        }
        let mut writes = project_runtime_sdk::aui_wire::UiWriteBuffer::new(&instance);
        node.release_property_to_binding(&mut writes, UiProperty::LocalStyle)
            .unwrap();
        instance
            .apply_update_batch(&writes.finish().unwrap())
            .unwrap();
        let restored = present_retained(&mut cache, &package, &instance, &mut producer, 4).unwrap();
        assert_eq!(
            restored.resolved_document.nodes[1]
                .resolved_style
                .as_ref()
                .unwrap()
                .paint
                .radius,
            11.
        );
    }

    #[test]
    fn gate_e_binding_and_retained_content_changes_share_local_preparation() {
        let mut package = retained_package();
        let document = package.aui_documents.documents_by_id.get_mut("ui").unwrap();
        document.nodes[1].kind = AuiNodeKind::ProgressBar;
        document.nodes[1].binding_refs.push(AuiBindingRef::new(
            "health",
            AuiBindingTarget::ProgressBarValue,
            "test.value",
            None,
        ));
        let mut actor = AuiNode::new(
            "actor",
            AuiNodeKind::Panel,
            AuiRect::fixed_position(50.0, 10.0, 20.0, 20.0),
        );
        actor.parent = Some("root".into());
        actor.visible = false;
        document.nodes[0].children.push("actor".into());
        document.nodes.push(actor);
        let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
        let actor = instance.node("actor").unwrap();
        let mut producer = BindingValueProducer {
            value: Some(AuiBindingValue::Number(0.8)),
            revision: 1,
            previous_seen: None,
        };
        let mut cache = AuiPresentCache::for_package(&package);
        cache.set_measurement_enabled(true);
        let first = present_retained(&mut cache, &package, &instance, &mut producer, 1).unwrap();
        cache.store(Some(first));
        let before = cache.c2_runtime().work.full_prepare_count;
        for frame in 2..=3 {
            let mut edit = instance.edit();
            edit.write(
                actor,
                UiProperty::Visibility,
                UiValue::Visibility(if frame == 2 {
                    UiVisibility::Visible
                } else {
                    UiVisibility::Hidden
                }),
            )
            .unwrap();
            edit.write(
                actor,
                UiProperty::Translation,
                UiValue::Translation([5.0 * frame as f32, 0.0]),
            )
            .unwrap();
            edit.commit().unwrap();
            // The second Replace changes a binding; the third has equal binding values.
            producer.value = Some(AuiBindingValue::Number(0.4));
            producer.revision = frame;
            let output =
                present_retained(&mut cache, &package, &instance, &mut producer, frame).unwrap();
            assert!(cache.c2_runtime().prepared_input_reused());
            assert_eq!(cache.c2_runtime().work.full_prepare_count, before);
            assert_eq!(cache.c2_runtime().prepared_document_nodes_copied(), 0);
            let mut fresh = AuiPresentCache::for_package(&package);
            let expected =
                present_retained(&mut fresh, &package, &instance, &mut producer, frame).unwrap();
            assert_eq!(output.resolved_document, expected.resolved_document);
            assert_eq!(output.layout, expected.layout);
            assert_eq!(output.overlay, expected.overlay);
            cache.store(Some(output));
        }
        let previous = cache.last_present.as_ref().unwrap().clone();
        let mut edit = instance.edit();
        edit.write(
            actor,
            UiProperty::Visibility,
            UiValue::Visibility(UiVisibility::Visible),
        )
        .unwrap();
        edit.commit().unwrap();
        producer.value = Some(AuiBindingValue::Bool(true)); // invalid ProgressBar input
        producer.revision = 4;
        let failed = present_retained(&mut cache, &package, &instance, &mut producer, 4).unwrap();
        assert_eq!(failed.report.status, AuiRuntimePresentStatus::Failed);
        assert_eq!(failed.resolved_document, previous.resolved_document);
        assert_eq!(failed.layout, previous.layout);
        cache.store(Some(failed));
        producer.value = Some(AuiBindingValue::Number(0.6));
        producer.revision = 5;
        let recovered =
            present_retained(&mut cache, &package, &instance, &mut producer, 5).unwrap();
        let mut fresh = AuiPresentCache::for_package(&package);
        let expected = present_retained(&mut fresh, &package, &instance, &mut producer, 5).unwrap();
        assert_eq!(recovered.resolved_document, expected.resolved_document);
        assert_eq!(recovered.layout, expected.layout);
        assert_eq!(recovered.overlay, expected.overlay);
    }

    #[test]
    fn production_presenter_consumes_one_translation_and_recovers_from_failed_snapshot() {
        let package = retained_package();
        let document = package.aui_documents.get("ui").unwrap();
        let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
        let unit = instance.node("unit").unwrap();
        let mut edit = instance.edit();
        edit.write(
            unit,
            UiProperty::Translation,
            UiValue::Translation([5.0, 0.0]),
        )
        .unwrap();
        edit.commit().unwrap();
        let mut cache = AuiPresentCache::for_package(&package);
        let mut producer = ReusingProducer { fail_next: true };
        assert!(present_retained(&mut cache, &package, &instance, &mut producer, 1).is_none());
        let first = present_retained(&mut cache, &package, &instance, &mut producer, 2);
        assert!(first.is_some());
        assert_eq!(
            cache.c2_runtime().retained_properties_read(),
            1,
            "failed snapshot must not consume the retained baseline"
        );
        cache.store(first);
        let clean = present_retained(&mut cache, &package, &instance, &mut producer, 3);
        let initial_x = clean.as_ref().unwrap().layout.computed_nodes[1].rect.x;
        cache.store(clean);
        let mut edit = instance.edit();
        edit.write(
            unit,
            UiProperty::Translation,
            UiValue::Translation([15.0, 0.0]),
        )
        .unwrap();
        edit.commit().unwrap();
        let mut edit = instance.edit();
        edit.write(
            unit,
            UiProperty::Translation,
            UiValue::Translation([20.0, 0.0]),
        )
        .unwrap();
        edit.commit().unwrap();
        let moved = present_retained(&mut cache, &package, &instance, &mut producer, 4).unwrap();
        assert_eq!(cache.c2_runtime().retained_properties_read(), 1);
        assert_eq!(moved.layout.computed_nodes[1].rect.x, initial_x + 15.0);
    }

    #[test]
    fn equivalent_replace_reuses_prepared_input_without_copying_document_nodes() {
        let mut package = retained_package();
        package
            .aui_documents
            .documents_by_id
            .get_mut("ui")
            .unwrap()
            .nodes[1]
            .binding_refs
            .push(AuiBindingRef::new(
                "unit-visible",
                AuiBindingTarget::PanelVisible,
                "test.unit_visible",
                None,
            ));
        let mut cache = AuiPresentCache::for_package(&package);
        let mut producer = ReplacingProducer {
            visible: true,
            fail_next: false,
        };
        let mut interaction = AuiInteractionState::default();
        let first = cache
            .take_or_rebuild(
                &package,
                &World::new(),
                1,
                &mut producer,
                AuiPresentContext::new(None, None, &mut interaction),
            )
            .unwrap();
        cache.store(Some(first));
        let second = cache
            .take_or_rebuild(
                &package,
                &World::new(),
                2,
                &mut producer,
                AuiPresentContext::new(None, None, &mut interaction),
            )
            .unwrap();
        assert_eq!(second.report.frame_index, 2);
        assert_eq!(cache.rebuild_count, 1);
        assert_eq!(cache.hit_count, 1);
        assert!(second.resolved_document.nodes[1].visible);
        cache.store(Some(second));
        producer.visible = false;
        let changed = cache
            .take_or_rebuild(
                &package,
                &World::new(),
                3,
                &mut producer,
                AuiPresentContext::new(None, None, &mut interaction),
            )
            .unwrap();
        assert!(cache.c2_runtime().prepared_input_reused());
        assert_eq!(cache.c2_runtime().prepared_document_nodes_copied(), 0);
        assert!(!changed.resolved_document.nodes[1].visible);
        assert!(!changed.layout.computed_nodes[1].effective_visible);
    }

    #[test]
    fn invalid_binding_does_not_advance_successful_snapshot_identity() {
        let mut package = retained_package();
        package
            .aui_documents
            .documents_by_id
            .get_mut("ui")
            .unwrap()
            .nodes[1]
            .binding_refs
            .push(AuiBindingRef::new(
                "unit-visible",
                AuiBindingTarget::PanelVisible,
                "test.value",
                None,
            ));
        let mut producer = BindingValueProducer {
            value: Some(AuiBindingValue::Bool(false)),
            revision: 1,
            previous_seen: None,
        };
        let mut cache = AuiPresentCache::for_package(&package);
        let mut interaction = AuiInteractionState::default();
        let first = cache
            .take_or_rebuild(
                &package,
                &World::new(),
                1,
                &mut producer,
                AuiPresentContext::new(None, None, &mut interaction),
            )
            .unwrap();
        cache.store(Some(first));
        producer.value = Some(AuiBindingValue::String("invalid".into()));
        producer.revision = 2;
        let invalid = cache
            .take_or_rebuild(
                &package,
                &World::new(),
                2,
                &mut producer,
                AuiPresentContext::new(None, None, &mut interaction),
            )
            .unwrap();
        assert_eq!(invalid.report.status, AuiRuntimePresentStatus::Failed);
        assert!(!invalid.resolved_document.nodes[1].visible);
        cache.store(Some(invalid));
        assert!(!cache.current().unwrap().resolved_document.nodes[1].visible);
        producer.value = Some(AuiBindingValue::Bool(false));
        producer.revision = 1;
        let reused = cache
            .take_or_rebuild(
                &package,
                &World::new(),
                3,
                &mut producer,
                AuiPresentContext::new(None, None, &mut interaction),
            )
            .unwrap();
        assert_eq!(reused.report.status, AuiRuntimePresentStatus::Success);
        assert!(!reused.resolved_document.nodes[1].visible);
        cache.store(Some(reused));
        producer.value = Some(AuiBindingValue::Bool(true));
        producer.revision = 4;
        let recovered = cache
            .take_or_rebuild(
                &package,
                &World::new(),
                4,
                &mut producer,
                AuiPresentContext::new(None, None, &mut interaction),
            )
            .unwrap();
        assert_eq!(producer.previous_seen, Some(1));
        assert!(recovered.resolved_document.nodes[1].visible);
    }

    #[test]
    fn missing_binding_fallback_and_recovery_use_full_prepare() {
        let mut package = retained_package();
        package
            .aui_documents
            .documents_by_id
            .get_mut("ui")
            .unwrap()
            .nodes[1]
            .binding_refs
            .push(AuiBindingRef::new(
                "unit-visible",
                AuiBindingTarget::PanelVisible,
                "test.value",
                Some(AuiBindingValue::Bool(true)),
            ));
        let mut producer = BindingValueProducer {
            value: Some(AuiBindingValue::Bool(false)),
            revision: 1,
            previous_seen: None,
        };
        let mut cache = AuiPresentCache::for_package(&package);
        let mut interaction = AuiInteractionState::default();
        let first = cache
            .take_or_rebuild(
                &package,
                &World::new(),
                1,
                &mut producer,
                AuiPresentContext::new(None, None, &mut interaction),
            )
            .unwrap();
        cache.store(Some(first));
        producer.value = None;
        producer.revision = 2;
        let fallback = cache
            .take_or_rebuild(
                &package,
                &World::new(),
                2,
                &mut producer,
                AuiPresentContext::new(None, None, &mut interaction),
            )
            .unwrap();
        assert!(!cache.c2_runtime().prepared_input_reused());
        assert!(fallback.resolved_document.nodes[1].visible);
        cache.store(Some(fallback));
        producer.value = Some(AuiBindingValue::Bool(false));
        producer.revision = 3;
        let recovered = cache
            .take_or_rebuild(
                &package,
                &World::new(),
                3,
                &mut producer,
                AuiPresentContext::new(None, None, &mut interaction),
            )
            .unwrap();
        assert!(!cache.c2_runtime().prepared_input_reused());
        assert!(!recovered.resolved_document.nodes[1].visible);
    }

    #[test]
    fn binding_order_controller_override_and_release_match_fresh_output() {
        let mut package = retained_package();
        let node = &mut package
            .aui_documents
            .documents_by_id
            .get_mut("ui")
            .unwrap()
            .nodes[1];
        for (id, path) in [("first", "test.first"), ("last", "test.last")] {
            node.binding_refs.push(AuiBindingRef::new(
                id,
                AuiBindingTarget::PanelVisible,
                path,
                None,
            ));
        }
        let mut instance =
            AuiInstance::mount(Arc::new(package.aui_documents.get("ui").unwrap().clone())).unwrap();
        let unit = instance.node("unit").unwrap();
        let mut producer = OrderedBindingProducer {
            first: true,
            last: true,
        };
        let mut cache = AuiPresentCache::for_package(&package);
        let first = present_retained(&mut cache, &package, &instance, &mut producer, 1).unwrap();
        cache.store(Some(first));
        producer.first = false;
        let overridden =
            present_retained(&mut cache, &package, &instance, &mut producer, 2).unwrap();
        assert!(overridden.resolved_document.nodes[1].visible);
        assert_eq!(
            cache
                .c2_runtime()
                .geometry_update_stats()
                .source_nodes_compared,
            0
        );
        assert_eq!(cache.c2_runtime().prepared_document_nodes_copied(), 0);
        cache.store(Some(overridden));
        let mut edit = instance.edit();
        edit.transfer_property_ownership(unit, UiProperty::Visibility, UiPropertyOwner::Controller)
            .unwrap();
        edit.write(
            unit,
            UiProperty::Visibility,
            UiValue::Visibility(UiVisibility::Hidden),
        )
        .unwrap();
        edit.commit().unwrap();
        let hidden = present_retained(&mut cache, &package, &instance, &mut producer, 3).unwrap();
        assert!(!hidden.layout.computed_nodes[1].effective_visible);
        assert_eq!(cache.c2_runtime().prepared_document_nodes_copied(), 0);
        cache.store(Some(hidden));
        producer.first = true;
        let masked = present_retained(&mut cache, &package, &instance, &mut producer, 4).unwrap();
        assert!(!masked.resolved_document.nodes[1].visible);
        assert_eq!(
            cache
                .c2_runtime()
                .geometry_update_stats()
                .source_nodes_compared,
            0
        );
        cache.store(Some(masked));
        let mut edit = instance.edit();
        edit.transfer_property_ownership(unit, UiProperty::Visibility, UiPropertyOwner::Binding)
            .unwrap();
        edit.commit().unwrap();
        let restored = present_retained(&mut cache, &package, &instance, &mut producer, 5).unwrap();
        let mut fresh_cache = AuiPresentCache::for_package(&package);
        let fresh =
            present_retained(&mut fresh_cache, &package, &instance, &mut producer, 5).unwrap();
        assert!(restored.layout.computed_nodes[1].effective_visible);
        assert_eq!(restored.resolved_document, fresh.resolved_document);
        assert_eq!(restored.layout, fresh.layout);
        assert_eq!(restored.overlay, fresh.overlay);
    }

    #[test]
    fn external_old_frame_is_immutable_during_local_visibility_and_rect_bindings() {
        for target in [
            AuiBindingTarget::PanelVisible,
            AuiBindingTarget::RectOffsetX,
            AuiBindingTarget::RectOffsetY,
        ] {
            let mut package = retained_package();
            package
                .aui_documents
                .documents_by_id
                .get_mut("ui")
                .unwrap()
                .nodes[1]
                .binding_refs
                .push(AuiBindingRef::new("value", target, "test.value", None));
            let (old, new) = if target == AuiBindingTarget::PanelVisible {
                (AuiBindingValue::Bool(true), AuiBindingValue::Bool(false))
            } else {
                (AuiBindingValue::Number(10.0), AuiBindingValue::Number(20.0))
            };
            let mut producer = BindingValueProducer {
                value: Some(old),
                revision: 1,
                previous_seen: None,
            };
            let mut cache = AuiPresentCache::for_package(&package);
            let mut interaction = AuiInteractionState::default();
            let first = cache
                .take_or_rebuild(
                    &package,
                    &World::new(),
                    1,
                    &mut producer,
                    AuiPresentContext::new(None, None, &mut interaction),
                )
                .unwrap();
            let old_document = first.resolved_document.as_ref().clone();
            let old_layout = first.layout.as_ref().clone();
            cache.store(Some(first.clone()));
            producer.value = Some(new.clone());
            producer.revision = 2;
            let changed = cache
                .take_or_rebuild(
                    &package,
                    &World::new(),
                    2,
                    &mut producer,
                    AuiPresentContext::new(None, None, &mut interaction),
                )
                .unwrap();
            assert_eq!(*first.resolved_document, old_document);
            assert_eq!(*first.layout, old_layout);
            assert!(cache.c2_runtime().prepared_input_reused());
            assert_eq!(cache.c2_runtime().prepared_document_nodes_copied(), 2);
            assert_eq!(cache.c2_runtime().solver_build_count(), 1);
            let mut fresh = AuiPresentCache::for_package(&package);
            let expected = fresh
                .take_or_rebuild(
                    &package,
                    &World::new(),
                    2,
                    &mut producer,
                    AuiPresentContext::new(None, None, &mut interaction),
                )
                .unwrap();
            assert_eq!(changed.resolved_document, expected.resolved_document);
            assert_eq!(changed.layout, expected.layout);
            assert_eq!(changed.overlay, expected.overlay);
        }
    }

    #[test]
    fn root_visibility_failure_preserves_successful_output() {
        let mut package = retained_package();
        package
            .aui_documents
            .documents_by_id
            .get_mut("ui")
            .unwrap()
            .nodes[0]
            .binding_refs
            .push(AuiBindingRef::new(
                "root-visible",
                AuiBindingTarget::PanelVisible,
                "test.value",
                None,
            ));
        let mut producer = BindingValueProducer {
            value: Some(AuiBindingValue::Bool(true)),
            revision: 1,
            previous_seen: None,
        };
        let mut cache = AuiPresentCache::for_package(&package);
        let mut interaction = AuiInteractionState::default();
        let first = cache
            .take_or_rebuild(
                &package,
                &World::new(),
                1,
                &mut producer,
                AuiPresentContext::new(None, None, &mut interaction),
            )
            .unwrap();
        let old_document = first.resolved_document.as_ref().clone();
        let old_layout = first.layout.as_ref().clone();
        cache.store(Some(first));
        producer.value = Some(AuiBindingValue::Bool(false));
        producer.revision = 2;
        let failed = cache
            .take_or_rebuild(
                &package,
                &World::new(),
                2,
                &mut producer,
                AuiPresentContext::new(None, None, &mut interaction),
            )
            .unwrap();
        assert_eq!(failed.report.status, AuiRuntimePresentStatus::Failed);
        assert_eq!(*failed.resolved_document, old_document);
        assert_eq!(*failed.layout, old_layout);
    }

    #[test]
    fn retained_text_uses_local_copy_path_and_preserves_picture_on_paragraph_failure() {
        let mut package = retained_package();
        package.font_bundles = text_test_font_registry();
        let node = &mut package
            .aui_documents
            .documents_by_id
            .get_mut("ui")
            .unwrap()
            .nodes[1];
        node.kind = AuiNodeKind::Text;
        node.text = Some("valid".into());
        let mut instance =
            AuiInstance::mount(Arc::new(package.aui_documents.get("ui").unwrap().clone())).unwrap();
        let unit = instance.node("unit").unwrap();
        let mut cache = AuiPresentCache::for_package(&package);
        let mut producer = ReusingProducer { fail_next: false };
        let first = present_retained(&mut cache, &package, &instance, &mut producer, 1).unwrap();
        cache.store(Some(first));
        let mut edit = instance.edit();
        edit.write(unit, UiProperty::Text, UiValue::Text("edited".into()))
            .unwrap();
        edit.commit().unwrap();
        let changed = present_retained(&mut cache, &package, &instance, &mut producer, 2).unwrap();
        assert_ne!(changed.report.status, AuiRuntimePresentStatus::Failed);
        assert_eq!(
            changed.resolved_document.nodes[1].text.as_deref(),
            Some("edited")
        );
        assert_eq!(cache.c2_runtime().prepared_document_nodes_copied(), 0);
        let mut fresh_cache = AuiPresentCache::for_package(&package);
        let fresh =
            present_retained(&mut fresh_cache, &package, &instance, &mut producer, 2).unwrap();
        assert_eq!(changed.resolved_document, fresh.resolved_document);
        assert_eq!(changed.layout, fresh.layout);
        assert_eq!(changed.overlay, fresh.overlay);
        let old_document = changed.resolved_document.as_ref().clone();
        let old_layout = changed.layout.as_ref().clone();
        let old_overlay = changed.overlay.clone();
        cache.store(Some(changed));
        let mut edit = instance.edit();
        edit.write(
            unit,
            UiProperty::Text,
            UiValue::Text("x".repeat(crate::aui_paragraph::MAX_PARAGRAPH_SOURCE_BYTES + 1)),
        )
        .unwrap();
        edit.commit().unwrap();
        let failed = present_retained(&mut cache, &package, &instance, &mut producer, 3).unwrap();
        assert_eq!(failed.report.status, AuiRuntimePresentStatus::Failed);
        assert_eq!(*failed.resolved_document, old_document);
        assert_eq!(*failed.layout, old_layout);
        assert_eq!(failed.overlay, old_overlay);
        cache.store(Some(failed));
        let mut edit = instance.edit();
        edit.write(unit, UiProperty::Text, UiValue::Text("recovered".into()))
            .unwrap();
        edit.commit().unwrap();
        let recovered =
            present_retained(&mut cache, &package, &instance, &mut producer, 4).unwrap();
        assert_ne!(recovered.report.status, AuiRuntimePresentStatus::Failed);
        assert_eq!(
            recovered.resolved_document.nodes[1].text.as_deref(),
            Some("recovered")
        );
    }

    #[test]
    fn local_text_failure_preserves_successful_picture_and_identity() {
        let mut package = retained_package();
        package.font_bundles = text_test_font_registry();
        let node = &mut package
            .aui_documents
            .documents_by_id
            .get_mut("ui")
            .unwrap()
            .nodes[1];
        node.kind = AuiNodeKind::Text;
        node.text = Some("valid".into());
        node.binding_refs.push(AuiBindingRef::new(
            "text",
            AuiBindingTarget::TextText,
            "test.value",
            None,
        ));
        let mut producer = BindingValueProducer {
            value: Some(AuiBindingValue::String("valid".into())),
            revision: 1,
            previous_seen: None,
        };
        let mut cache = AuiPresentCache::for_package(&package);
        let mut interaction = AuiInteractionState::default();
        let first = cache
            .take_or_rebuild(
                &package,
                &World::new(),
                1,
                &mut producer,
                AuiPresentContext::new(None, None, &mut interaction),
            )
            .unwrap();
        assert_ne!(
            first.report.status,
            AuiRuntimePresentStatus::Failed,
            "{:?}",
            first.report
        );
        let old_document = first.resolved_document.clone();
        let old_layout = first.layout.clone();
        let old_overlay = first.overlay.clone();
        // Keep values rather than Arc aliases, so this exercises the scheduler's local branch.
        let old_document = old_document.as_ref().clone();
        let old_layout = old_layout.as_ref().clone();
        cache.store(Some(first));
        producer.value = Some(AuiBindingValue::String(
            "x".repeat(crate::aui_paragraph::MAX_PARAGRAPH_SOURCE_BYTES + 1),
        ));
        producer.revision = 2;
        let failed = cache
            .take_or_rebuild(
                &package,
                &World::new(),
                2,
                &mut producer,
                AuiPresentContext::new(None, None, &mut interaction),
            )
            .unwrap();
        assert_eq!(failed.report.status, AuiRuntimePresentStatus::Failed);
        assert_eq!(*failed.resolved_document, old_document);
        assert_eq!(*failed.layout, old_layout);
        assert_eq!(failed.overlay, old_overlay);
        assert_eq!(
            cache.last_snapshot_output.as_ref().unwrap().snapshot.values["test.value"],
            AuiBindingValue::String("valid".into())
        );
    }

    #[test]
    fn gate_b_non_layout_binding_targets_match_fresh_presentation() {
        let cases = [
            (
                AuiNodeKind::ProgressBar,
                AuiBindingTarget::ProgressBarValue,
                AuiBindingValue::Number(0.1),
                AuiBindingValue::Number(0.8),
            ),
            (
                AuiNodeKind::Toggle,
                AuiBindingTarget::ToggleChecked,
                AuiBindingValue::Bool(false),
                AuiBindingValue::Bool(true),
            ),
            (
                AuiNodeKind::Slider,
                AuiBindingTarget::SliderValue,
                AuiBindingValue::Number(0.2),
                AuiBindingValue::Number(0.9),
            ),
            (
                AuiNodeKind::Image,
                AuiBindingTarget::ImageAssetRef,
                AuiBindingValue::AssetRef(AuiAssetRef::new("old-image")),
                AuiBindingValue::AssetRef(AuiAssetRef::new("new-image")),
            ),
            (
                AuiNodeKind::Button,
                AuiBindingTarget::Interactable,
                AuiBindingValue::Bool(true),
                AuiBindingValue::Bool(false),
            ),
            (
                AuiNodeKind::Text,
                AuiBindingTarget::TextColor,
                AuiBindingValue::Color("#ff0000".into()),
                AuiBindingValue::String("#00ff00".into()),
            ),
            (
                AuiNodeKind::Text,
                AuiBindingTarget::TextText,
                AuiBindingValue::String("same".into()),
                AuiBindingValue::String("next".into()),
            ),
            (
                AuiNodeKind::InputField,
                AuiBindingTarget::InputFieldText,
                AuiBindingValue::String("input".into()),
                AuiBindingValue::String("edited".into()),
            ),
            (
                AuiNodeKind::Panel,
                AuiBindingTarget::PanelVisible,
                AuiBindingValue::Bool(true),
                AuiBindingValue::Bool(false),
            ),
            (
                AuiNodeKind::Image,
                AuiBindingTarget::ImageVisible,
                AuiBindingValue::Bool(true),
                AuiBindingValue::Bool(false),
            ),
            (
                AuiNodeKind::Text,
                AuiBindingTarget::PanelVisible,
                AuiBindingValue::Bool(false),
                AuiBindingValue::Bool(true),
            ),
        ];
        for (kind, target, old, new) in cases {
            let mut package = retained_package();
            if matches!(kind, AuiNodeKind::Text | AuiNodeKind::InputField) {
                package.font_bundles = text_test_font_registry();
            }
            let node = &mut package
                .aui_documents
                .documents_by_id
                .get_mut("ui")
                .unwrap()
                .nodes[1];
            node.kind = kind;
            node.interactable = true;
            if matches!(kind, AuiNodeKind::Text | AuiNodeKind::InputField) {
                node.text = Some("same".into());
            }
            if kind == AuiNodeKind::Slider {
                node.slider_value = Some(0.2);
                node.slider_min = Some(0.0);
                node.slider_max = Some(1.0);
                node.slider_step = Some(0.1);
            }
            node.binding_refs
                .push(AuiBindingRef::new("value", target, "test.value", None));
            let mut producer = BindingValueProducer {
                value: Some(old),
                revision: 1,
                previous_seen: None,
            };
            let mut cache = AuiPresentCache::for_package(&package);
            let mut interaction = AuiInteractionState::default();
            let first = cache
                .take_or_rebuild(
                    &package,
                    &World::new(),
                    1,
                    &mut producer,
                    AuiPresentContext::new(None, None, &mut interaction),
                )
                .unwrap();
            assert_ne!(
                first.report.status,
                AuiRuntimePresentStatus::Failed,
                "{target:?}: {:?}",
                first.report
            );
            cache.store(Some(first));
            producer.value = Some(new.clone());
            producer.revision = 2;
            let changed = cache
                .take_or_rebuild(
                    &package,
                    &World::new(),
                    2,
                    &mut producer,
                    AuiPresentContext::new(None, None, &mut interaction),
                )
                .unwrap();
            assert!(cache.c2_runtime().prepared_input_reused(), "{target:?}");
            assert_eq!(
                cache.c2_runtime().prepared_document_nodes_copied(),
                0,
                "{target:?}"
            );
            let mut fresh_cache = AuiPresentCache::for_package(&package);
            let mut fresh_producer = BindingValueProducer {
                value: Some(new),
                revision: 2,
                previous_seen: None,
            };
            let fresh = fresh_cache
                .take_or_rebuild(
                    &package,
                    &World::new(),
                    2,
                    &mut fresh_producer,
                    AuiPresentContext::new(None, None, &mut interaction),
                )
                .unwrap();
            assert_eq!(
                changed.resolved_document, fresh.resolved_document,
                "{target:?}"
            );
            assert_eq!(changed.layout, fresh.layout, "{target:?}");
            assert_eq!(
                changed.paragraph_layouts, fresh.paragraph_layouts,
                "{target:?}"
            );
            assert_eq!(changed.overlay, fresh.overlay, "{target:?}");
            assert_eq!(changed.composition, fresh.composition, "{target:?}");
        }
    }

    #[test]
    fn single_leaf_replace_copy_and_solver_counts_are_bounded_across_sizes() {
        for node_count in [400, 1_000, 10_000] {
            for target in [
                AuiBindingTarget::PanelVisible,
                AuiBindingTarget::RectOffsetX,
                AuiBindingTarget::RectOffsetY,
            ] {
                let mut package = retained_package();
                let document = package.aui_documents.documents_by_id.get_mut("ui").unwrap();
                if target != AuiBindingTarget::PanelVisible {
                    document.nodes[1].slot = Some(crate::aui_layout_schema::SlotSpec {
                        participation: crate::aui_layout_schema::Participation::OutOfFlow,
                        ..Default::default()
                    });
                }
                document.nodes[1].binding_refs.push(AuiBindingRef::new(
                    "unit-visible",
                    target,
                    "test.value",
                    None,
                ));
                for slot in 2..node_count {
                    let node_id = format!("panel-{slot}");
                    document.nodes[0].children.push(node_id.clone());
                    let mut node = AuiNode::new(
                        node_id,
                        AuiNodeKind::Panel,
                        AuiRect::fixed_position(0.0, 0.0, 10.0, 10.0),
                    );
                    node.parent = Some("root".into());
                    document.nodes.push(node);
                }
                package.aui_manifest.documents[0].node_count = node_count;
                let mut producer = BindingValueProducer {
                    value: Some(if target == AuiBindingTarget::PanelVisible {
                        AuiBindingValue::Bool(true)
                    } else {
                        AuiBindingValue::Number(10.)
                    }),
                    revision: 1,
                    previous_seen: None,
                };
                let mut cache = AuiPresentCache::for_package(&package);
                let mut interaction = AuiInteractionState::default();
                let first = cache
                    .take_or_rebuild(
                        &package,
                        &World::new(),
                        1,
                        &mut producer,
                        AuiPresentContext::new(None, None, &mut interaction),
                    )
                    .unwrap();
                assert_eq!(
                    first.report.status,
                    AuiRuntimePresentStatus::Success,
                    "{node_count}"
                );
                cache.store(Some(first));
                let solver_builds = cache.c2_runtime().solver_build_count();
                let drawing_before = cache.c2_runtime().drawing_stats();
                producer.value = Some(if target == AuiBindingTarget::PanelVisible {
                    AuiBindingValue::Bool(false)
                } else {
                    AuiBindingValue::Number(20.)
                });
                producer.revision = 2;
                let changed = cache
                    .take_or_rebuild(
                        &package,
                        &World::new(),
                        2,
                        &mut producer,
                        AuiPresentContext::new(None, None, &mut interaction),
                    )
                    .unwrap();
                if target == AuiBindingTarget::PanelVisible {
                    assert!(!changed.resolved_document.nodes[1].visible);
                }
                let drawing_after = cache.c2_runtime().drawing_stats();
                assert_eq!(
                    drawing_after.nodes_visited - drawing_before.nodes_visited,
                    1
                );
                if target != AuiBindingTarget::PanelVisible {
                    assert_eq!(
                        drawing_after.items_visited - drawing_before.items_visited,
                        1
                    );
                    assert_eq!(
                        drawing_after.item_payloads_built - drawing_before.item_payloads_built,
                        1
                    );
                }
                assert!(cache.c2_runtime().prepared_input_reused(), "{node_count}");
                assert_eq!(
                    cache.c2_runtime().prepared_binding_nodes_rebuilt(),
                    1,
                    "{node_count}"
                );
                assert_eq!(
                    cache.c2_runtime().solver_build_count(),
                    solver_builds,
                    "{node_count}"
                );
                assert_eq!(
                    cache.c2_runtime().prepared_document_nodes_copied(),
                    0,
                    "{node_count}"
                );
                assert_eq!(
                    cache
                        .c2_runtime()
                        .geometry_update_stats()
                        .source_nodes_compared,
                    1,
                    "{node_count}"
                );
                let stats = cache.c2_runtime().layout_stats("main").unwrap();
                assert_eq!(stats.source_nodes_loaded, 0, "{node_count}: {stats:?}");
                if target == AuiBindingTarget::PanelVisible {
                    assert!(stats.clean_snapshot_reused, "{node_count}: {stats:?}");
                } else {
                    assert!(
                        stats.visited_nodes <= 4,
                        "{node_count}/{target:?}: {stats:?}"
                    );
                    let mut fresh = AuiPresentCache::for_package(&package);
                    let expected = fresh
                        .take_or_rebuild(
                            &package,
                            &World::new(),
                            2,
                            &mut producer,
                            AuiPresentContext::new(None, None, &mut interaction),
                        )
                        .unwrap();
                    assert_eq!(changed.layout, expected.layout);
                    assert_eq!(changed.resolved_document, expected.resolved_document);
                    assert_eq!(changed.overlay, expected.overlay);
                }
            }
        }
    }

    #[test]
    fn gate_c_rect_text_hud_and_translation_commit_together_and_invalid_rect_preserves_picture() {
        struct MixedProducer {
            revision: u64,
            invalid: bool,
        }
        impl ProjectUiStateSnapshotProducer for MixedProducer {
            fn producer_id(&self) -> &str {
                "mixed-layout-test"
            }
            fn produce(
                &mut self,
                context: ProjectUiStateProducerContext<'_>,
            ) -> ProjectUiStateSnapshotOutput {
                ProjectUiStateSnapshotOutput::new(
                    self.producer_id(),
                    AuiSnapshotSource::ProjectProducer,
                    ProjectUiStateSnapshot::new(context.frame_index)
                        .with_value(
                            "test.x",
                            AuiBindingValue::Number(if self.invalid {
                                f32::NAN
                            } else {
                                self.revision as f32 * 10.
                            }),
                        )
                        .with_value(
                            "test.text",
                            AuiBindingValue::String(
                                if self.revision == 1 {
                                    "first"
                                } else {
                                    "second"
                                }
                                .into(),
                            ),
                        )
                        .with_value(
                            "test.hud",
                            AuiBindingValue::Number(self.revision as f32 * 0.2),
                        ),
                )
            }
            fn resolve(
                &mut self,
                context: ProjectUiStateProducerContext<'_>,
            ) -> Result<ProjectUiStateResolve, ProjectUiStateResolveError> {
                Ok(ProjectUiStateResolve::Replace {
                    identity: ProjectUiStateIdentity {
                        producer_epoch: 1,
                        visible_revision: self.revision,
                        binding_set: context.binding_set.identity().clone(),
                    },
                    output: self.produce(context),
                })
            }
        }
        let mut package = retained_package();
        package.font_bundles = text_test_font_registry();
        let bundle = Arc::make_mut(&mut package.font_bundles.bundles_by_id)
            .get_mut("test-font")
            .unwrap();
        bundle.metadata.pages.push(CookedFontBundlePage {
            page_index: 0,
            render_mode: FontBundleRenderMode::BitmapR8,
            format: "r8Unorm".into(),
            width: 64,
            height: 64,
            byte_len: 4096,
            sha256: "sha256:mixed-test-page".into(),
            payload_path: "page.r8".into(),
        });
        bundle.page_payloads.push(vec![0; 4096]);
        let glyph_ids: BTreeSet<_> = crate::text_shaping::shape_text(
            "noto",
            &bundle.font_face_sources[0].bytes,
            "firstsecond…",
        )
        .unwrap()
        .items
        .into_iter()
        .filter(|glyph| glyph.has_outline)
        .map(|glyph| glyph.glyph_id as u16)
        .collect();
        bundle.metadata.glyphs = glyph_ids
            .into_iter()
            .enumerate()
            .map(|(i, glyph_id)| CookedFontBundleGlyph {
                font_family_id: "family-ui".into(),
                font_face_id: "noto".into(),
                style: FontBundleStyle::Normal,
                weight: 400,
                glyph_id,
                codepoint: u32::MAX,
                render_mode: FontBundleRenderMode::BitmapR8,
                pixel_size: 12,
                page_index: 0,
                pixel_rect: [(i as u32 % 7) * 8, (i as u32 / 7) * 12, 8, 12],
                bearing_x: 0,
                bearing_y: 12,
                advance_per_em_millionths: 600_000,
                plane_bounds_per_em_millionths: None,
            })
            .collect();
        let document = package.aui_documents.documents_by_id.get_mut("ui").unwrap();
        let unit = &mut document.nodes[1];
        unit.kind = AuiNodeKind::Text;
        unit.text = Some("first".into());
        unit.slot = Some(crate::aui_layout_schema::SlotSpec {
            participation: crate::aui_layout_schema::Participation::OutOfFlow,
            ..Default::default()
        });
        unit.binding_refs = vec![
            AuiBindingRef::new("x", AuiBindingTarget::RectOffsetX, "test.x", None),
            AuiBindingRef::new("text", AuiBindingTarget::TextText, "test.text", None),
        ];
        let mut hud = AuiNode::new(
            "hud",
            AuiNodeKind::ProgressBar,
            AuiRect::fixed_position(60., 70., 80., 20.),
        )
        .with_parent("root");
        hud.binding_refs.push(AuiBindingRef::new(
            "hud",
            AuiBindingTarget::ProgressBarValue,
            "test.hud",
            None,
        ));
        document.nodes[0].children.push("hud".into());
        document.nodes.push(hud);
        let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
        let unit = instance.node("unit").unwrap();
        let mut cache = AuiPresentCache::for_package(&package);
        let mut producer = MixedProducer {
            revision: 1,
            invalid: false,
        };
        let first = present_retained(&mut cache, &package, &instance, &mut producer, 1).unwrap();
        assert_eq!(first.report.status, AuiRuntimePresentStatus::Success);
        cache.store(Some(first));
        let builds = cache.c2_runtime().solver_build_count();
        let mut edit = instance.edit();
        edit.write(
            unit,
            UiProperty::Translation,
            UiValue::Translation([7., 0.]),
        )
        .unwrap();
        edit.commit().unwrap();
        producer.revision = 2;
        let changed = present_retained(&mut cache, &package, &instance, &mut producer, 2).unwrap();
        assert_eq!(changed.layout.computed_nodes[1].rect.x, 27.);
        assert_eq!(cache.c2_runtime().prepared_document_nodes_copied(), 0);
        assert_eq!(cache.c2_runtime().prepared_binding_nodes_rebuilt(), 2);
        assert_eq!(cache.c2_runtime().solver_build_count(), builds);
        assert_eq!(
            cache
                .c2_runtime()
                .geometry_update_stats()
                .translation_entries_compared,
            1
        );
        let mut fresh_cache = AuiPresentCache::for_package(&package);
        let fresh =
            present_retained(&mut fresh_cache, &package, &instance, &mut producer, 2).unwrap();
        assert_eq!(changed.resolved_document, fresh.resolved_document);
        assert_eq!(changed.layout, fresh.layout);
        assert_eq!(changed.paragraph_layouts, fresh.paragraph_layouts);
        assert_eq!(changed.overlay, fresh.overlay);
        let old_document = changed.resolved_document.as_ref().clone();
        let old_layout = changed.layout.as_ref().clone();
        let old_overlay = changed.overlay.clone();
        cache.store(Some(changed));
        producer.revision = 3;
        producer.invalid = true;
        let failed = present_retained(&mut cache, &package, &instance, &mut producer, 3).unwrap();
        assert_eq!(failed.report.status, AuiRuntimePresentStatus::Failed);
        assert_eq!(*failed.resolved_document, old_document);
        assert_eq!(*failed.layout, old_layout);
        assert_eq!(failed.overlay, old_overlay);
        cache.store(Some(failed));
        producer.invalid = false;
        let recovered =
            present_retained(&mut cache, &package, &instance, &mut producer, 4).unwrap();
        let fresh = present_retained(
            &mut AuiPresentCache::for_package(&package),
            &package,
            &instance,
            &mut producer,
            4,
        )
        .unwrap();
        assert_eq!(recovered.report.status, AuiRuntimePresentStatus::Success);
        assert_eq!(recovered.layout, fresh.layout);
        assert_eq!(recovered.overlay, fresh.overlay);
    }

    #[test]
    fn text_replace_and_retained_translation_share_local_prepared_input() {
        let mut package = retained_package();
        package.font_bundles = text_test_font_registry();
        let node = &mut package
            .aui_documents
            .documents_by_id
            .get_mut("ui")
            .unwrap()
            .nodes[1];
        node.kind = AuiNodeKind::Text;
        node.text = Some("old".into());
        node.binding_refs.push(AuiBindingRef::new(
            "unit-text",
            AuiBindingTarget::TextText,
            "test.value",
            None,
        ));
        let document = package.aui_documents.get("ui").unwrap();
        let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
        let unit = instance.node("unit").unwrap();
        let mut cache = AuiPresentCache::for_package(&package);
        let mut producer = BindingValueProducer {
            value: Some(AuiBindingValue::String("first".into())),
            revision: 1,
            previous_seen: None,
        };
        let first = present_retained(&mut cache, &package, &instance, &mut producer, 1).unwrap();
        assert_eq!(first.layout.computed_nodes.len(), 2, "{:#?}", first.report);
        let initial_x = first.layout.computed_nodes[1].rect.x;
        cache.store(Some(first));

        let mut edit = instance.edit();
        edit.write(
            unit,
            UiProperty::Translation,
            UiValue::Translation([12.0, 0.0]),
        )
        .unwrap();
        edit.commit().unwrap();
        producer.value = Some(AuiBindingValue::String("second".into()));
        producer.revision = 2;
        let changed = present_retained(&mut cache, &package, &instance, &mut producer, 2).unwrap();
        assert!(cache.c2_runtime().prepared_input_reused());
        assert_eq!(cache.c2_runtime().prepared_document_nodes_copied(), 0);
        assert_eq!(
            changed.resolved_document.nodes[1].text.as_deref(),
            Some("second")
        );
        assert_eq!(changed.layout.computed_nodes[1].rect.x, initial_x + 12.0);
        let mut fresh_cache = AuiPresentCache::for_package(&package);
        let mut fresh_producer = BindingValueProducer {
            value: Some(AuiBindingValue::String("second".into())),
            revision: 2,
            previous_seen: None,
        };
        let fresh = present_retained(
            &mut fresh_cache,
            &package,
            &instance,
            &mut fresh_producer,
            2,
        )
        .unwrap();
        assert_eq!(changed.resolved_document, fresh.resolved_document);
        assert_eq!(changed.layout, fresh.layout);
    }

    #[test]
    fn failed_replace_keeps_last_successful_binding_baseline() {
        let mut package = retained_package();
        package
            .aui_documents
            .documents_by_id
            .get_mut("ui")
            .unwrap()
            .nodes[1]
            .binding_refs
            .push(AuiBindingRef::new(
                "unit-visible",
                AuiBindingTarget::PanelVisible,
                "test.unit_visible",
                None,
            ));
        let mut cache = AuiPresentCache::for_package(&package);
        let mut producer = ReplacingProducer {
            visible: true,
            fail_next: false,
        };
        let mut interaction = AuiInteractionState::default();
        let first = cache
            .take_or_rebuild(
                &package,
                &World::new(),
                1,
                &mut producer,
                AuiPresentContext::new(None, None, &mut interaction),
            )
            .unwrap();
        cache.store(Some(first));
        producer.visible = false;
        producer.fail_next = true;
        let failed = cache
            .take_or_rebuild(
                &package,
                &World::new(),
                2,
                &mut producer,
                AuiPresentContext::new(None, None, &mut interaction),
            )
            .unwrap();
        assert!(failed.resolved_document.nodes[1].visible);
        cache.store(Some(failed));
        let accepted = cache
            .take_or_rebuild(
                &package,
                &World::new(),
                3,
                &mut producer,
                AuiPresentContext::new(None, None, &mut interaction),
            )
            .unwrap();
        assert!(!accepted.resolved_document.nodes[1].visible);
    }

    #[test]
    fn dynamic_template_adds_its_binding_path_to_the_producer_set() {
        let package = retained_package();
        let document = package.aui_documents.get("ui").unwrap();
        let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
        let mut cache = AuiPresentCache::for_package(&package);
        let mut producer = ReplacingProducer {
            visible: true,
            fail_next: false,
        };
        let first = present_retained(&mut cache, &package, &instance, &mut producer, 1).unwrap();
        cache.store(Some(first));

        let root = instance.node("root").unwrap();
        let mut extra = AuiNode::new(
            "extra",
            AuiNodeKind::Panel,
            AuiRect::fixed_position(50.0, 10.0, 20.0, 20.0),
        );
        extra.binding_refs.push(AuiBindingRef::new(
            "extra-visible",
            AuiBindingTarget::PanelVisible,
            "test.extra_visible",
            Some(AuiBindingValue::Bool(true)),
        ));
        let mut template = AuiDocument::new(
            "template",
            vec![AuiCanvas::screen_overlay("template", 160.0, 120.0, "extra")],
            vec![extra],
        );
        template.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
        let added = instance
            .instantiate_template(&template, root, None)
            .unwrap();
        let next = present_retained(&mut cache, &package, &instance, &mut producer, 2).unwrap();
        assert!(next.resolved_document.nodes.len() > 2);
        assert!(cache
            .snapshot_cache
            .active_binding_paths()
            .contains(&"test.extra_visible".to_string()));
        cache.store(Some(next));
        instance.delete_subtree(added.root).unwrap();
        let removed = present_retained(&mut cache, &package, &instance, &mut producer, 3).unwrap();
        assert!(removed
            .resolved_document
            .nodes
            .iter()
            .any(|node| node.node_id.starts_with("__dead_") && !node.visible));
        assert!(!cache
            .snapshot_cache
            .active_binding_paths()
            .contains(&"test.extra_visible".to_string()));
    }

    #[test]
    fn production_presenter_uses_committed_structure_after_revision_gap() {
        let package = retained_package();
        let document = package.aui_documents.get("ui").unwrap();
        let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
        let mut cache = AuiPresentCache::for_package(&package);
        let mut producer = ReusingProducer { fail_next: false };
        let first = present_retained(&mut cache, &package, &instance, &mut producer, 1).unwrap();
        cache.store(Some(first));
        let root = instance.node("root").unwrap();
        let mut template = AuiDocument::new(
            "template",
            vec![AuiCanvas::screen_overlay("template", 160.0, 120.0, "extra")],
            vec![AuiNode::new(
                "extra",
                AuiNodeKind::Panel,
                AuiRect::fixed_position(50.0, 10.0, 20.0, 20.0),
            )],
        );
        template.schema_version = AUI_LAYOUT_SCHEMA_VERSION.into();
        let added = instance
            .instantiate_template(&template, root, None)
            .unwrap();
        let output = present_retained(&mut cache, &package, &instance, &mut producer, 2).unwrap();
        let id = instance.source().nodes[added.root.slot as usize]
            .node_id
            .clone();
        assert!(output
            .resolved_document
            .nodes
            .iter()
            .any(|node| node.node_id == id));
        assert!(output
            .layout
            .computed_nodes
            .iter()
            .any(|node| node.node_id == id));
        cache.store(Some(output));
        instance.delete_subtree(added.root).unwrap();
        let removed = present_retained(&mut cache, &package, &instance, &mut producer, 3).unwrap();
        assert!(!removed
            .resolved_document
            .nodes
            .iter()
            .any(|node| node.node_id == id));
        cache.store(Some(removed));
        let replacement = instance
            .instantiate_template(&template, root, None)
            .unwrap();
        assert_eq!(replacement.root.slot, added.root.slot);
        assert_ne!(replacement.root.generation, added.root.generation);
        let reused = present_retained(&mut cache, &package, &instance, &mut producer, 4).unwrap();
        let replacement_id = &instance.source().nodes[replacement.root.slot as usize].node_id;
        assert!(reused
            .resolved_document
            .nodes
            .iter()
            .any(|node| &node.node_id == replacement_id));
        assert!(!reused
            .resolved_document
            .nodes
            .iter()
            .any(|node| node.node_id == id));
    }

    #[test]
    fn custom_paint_edit_reuses_preparation_and_layout_with_400_nodes() {
        use project_runtime_sdk::aui_extension::*;
        let mut package = retained_package();
        let document = package.aui_documents.documents_by_id.get_mut("ui").unwrap();
        document.nodes[1].kind = AuiNodeKind::Custom;
        document.nodes[1].custom = Some(crate::aui_extension::AuiCustomNodeSpec {
            type_id: "test.paint".into(),
            schema_version: 1,
            properties: Default::default(),
        });
        for i in 0..398 {
            let id = format!("sibling-{i}");
            document.nodes[0].children.push(id.clone());
            document.nodes.push(
                AuiNode::new(
                    id,
                    AuiNodeKind::Panel,
                    AuiRect::fixed_position(60., 10., 20., 20.),
                )
                .with_parent("root"),
            );
        }
        let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
        let unit = instance.node("unit").unwrap();
        let paint = |red| UiExtensionOutput::Paint {
            output: UiExtensionPaintOutput {
                vertices: Vec::new(),
                indices: Vec::new(),
                asset_id: None,
                primitives: vec![UiExtensionPaintPrimitive::Rect {
                    rect: [0., 0., 20., 20.],
                    color: [red, 0., 0., 1.],
                }],
            },
        };
        instance.install_custom_output(unit, paint(0.2)).unwrap();
        let mut cache = AuiPresentCache::for_package(&package);
        cache.set_measurement_enabled(true);
        let mut producer = ReusingProducer { fail_next: false };
        let first = present_retained(&mut cache, &package, &instance, &mut producer, 1).unwrap();
        let old_overlay = first.overlay.clone();
        cache.store(Some(first));
        let before = cache.c2_runtime().update_work();
        let descriptor = UiExtensionTypeDescriptor {
            type_id: "test.paint".into(),
            schema_version: 1,
            callback_id: 1,
            properties: vec![UiExtensionPropertyDescriptor {
                name: "red".into(),
                property_type: UiExtensionPropertyType::Float,
                invalidates: vec![UiExtensionInvalidation::Paint],
            }],
            measure: false,
            layout: false,
            paint: true,
        };
        instance
            .apply_custom_property(unit, "red", UiExtensionValue::Float(0.8), &descriptor)
            .unwrap();
        instance.install_custom_output(unit, paint(0.8)).unwrap();
        let changed = present_retained(&mut cache, &package, &instance, &mut producer, 2).unwrap();
        let work = cache.c2_runtime().update_work().since(before);
        assert_eq!(
            work.full_prepare_count, 0,
            "a paint edit must not prepare 400 nodes"
        );
        assert_eq!(
            work.layout_nodes_visited, 0,
            "paint must not invalidate measurement"
        );
        assert_eq!(work.prepared_document_nodes_copied, 0);
        assert_ne!(
            changed.overlay, old_overlay,
            "the new paint must be visible"
        );
        let mut fresh = AuiPresentCache::for_package(&package);
        let expected = present_retained(&mut fresh, &package, &instance, &mut producer, 2).unwrap();
        assert_eq!(changed.overlay, expected.overlay);
        assert_eq!(changed.layout, expected.layout);
    }

    #[test]
    fn custom_measure_and_layout_products_still_invalidate_layout() {
        use crate::aui_layout_schema::{Dimension, SlotSpec};
        use project_runtime_sdk::aui_extension::*;
        let mut package = retained_package();
        let doc = package.aui_documents.documents_by_id.get_mut("ui").unwrap();
        doc.nodes[0].layout.as_mut().unwrap().mode = LayoutMode::Row;
        doc.nodes[0].layout.as_mut().unwrap().align_items =
            crate::aui_layout_schema::AlignItems::Start;
        doc.nodes[0].kind = AuiNodeKind::Custom;
        doc.nodes[0].custom = Some(crate::aui_extension::AuiCustomNodeSpec {
            type_id: "test.layout".into(),
            schema_version: 1,
            properties: Default::default(),
        });
        doc.nodes[1].kind = AuiNodeKind::Custom;
        doc.nodes[1].slot = Some(SlotSpec {
            width: Dimension::Content,
            height: Dimension::Content,
            flex_shrink: 0.,
            ..Default::default()
        });
        doc.nodes[1].custom = Some(crate::aui_extension::AuiCustomNodeSpec {
            type_id: "test.measure".into(),
            schema_version: 1,
            properties: Default::default(),
        });
        let mut instance = AuiInstance::mount(Arc::new(doc.clone())).unwrap();
        let unit = instance.node("unit").unwrap();
        let root = instance.node("root").unwrap();
        for handle in [root, unit] {
            instance
                .install_custom_output(
                    handle,
                    UiExtensionOutput::Paint {
                        output: UiExtensionPaintOutput {
                            vertices: Vec::new(),
                            indices: Vec::new(),
                            asset_id: None,
                            primitives: vec![UiExtensionPaintPrimitive::Rect {
                                rect: [0., 0., 10., 10.],
                                color: [0.2, 0.3, 0.4, 1.],
                            }],
                        },
                    },
                )
                .unwrap();
        }
        let mut cache = AuiPresentCache::for_package(&package);
        cache.set_measurement_enabled(true);
        let mut producer = ReusingProducer { fail_next: false };
        let initial = present_retained(&mut cache, &package, &instance, &mut producer, 1).unwrap();
        assert_eq!(
            initial.report.status,
            AuiRuntimePresentStatus::Success,
            "{:?}",
            initial.report.diagnostics
        );
        cache.store(Some(initial));
        for (frame, size) in [(2, [23., 17.]), (3, [41., 29.])] {
            let before = cache.c2_runtime().update_work();
            instance
                .install_custom_output(
                    unit,
                    UiExtensionOutput::Measure {
                        output: UiExtensionMeasureOutput {
                            size,
                            first_baseline: None,
                        },
                    },
                )
                .unwrap();
            let changed =
                present_retained(&mut cache, &package, &instance, &mut producer, frame).unwrap();
            assert!(
                cache
                    .c2_runtime()
                    .update_work()
                    .since(before)
                    .layout_nodes_visited
                    > 0,
                "frame={frame}, work={:?}, output={:?}, layout={:?}",
                cache.c2_runtime().update_work().since(before),
                changed.resolved_document.nodes[1].custom_runtime,
                changed.layout.computed_nodes,
            );
            let geometry = changed
                .layout
                .computed_nodes
                .iter()
                .find(|node| node.node_id == "unit")
                .unwrap();
            assert_eq!([geometry.rect.width, geometry.rect.height], size);
            let mut fresh = AuiPresentCache::for_package(&package);
            let expected =
                present_retained(&mut fresh, &package, &instance, &mut producer, frame).unwrap();
            assert_eq!(changed.layout, expected.layout);
            assert_eq!(changed.overlay, expected.overlay);
            cache.store(Some(changed));
        }
        for (frame, x) in [(4, 2.), (5, 7.)] {
            instance
                .install_custom_output(
                    root,
                    UiExtensionOutput::Layout {
                        children: vec![UiExtensionChildRect {
                            handle: unit,
                            rect: [x, 3., 41., 29.],
                        }],
                    },
                )
                .unwrap();
            let changed =
                present_retained(&mut cache, &package, &instance, &mut producer, frame).unwrap();
            let geometry = changed
                .layout
                .computed_nodes
                .iter()
                .find(|node| node.node_id == "unit")
                .unwrap();
            assert_eq!([geometry.rect.x, geometry.rect.y], [x, 3.]);
            let mut fresh = AuiPresentCache::for_package(&package);
            let expected =
                present_retained(&mut fresh, &package, &instance, &mut producer, frame).unwrap();
            assert_eq!(changed.layout, expected.layout);
            cache.store(Some(changed));
        }
    }

    #[test]
    fn production_presenter_rebuilds_custom_output_after_revision_gap() {
        let package = retained_package();
        let document = package.aui_documents.get("ui").unwrap();
        let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
        let mut cache = AuiPresentCache::for_package(&package);
        let mut producer = ReusingProducer { fail_next: false };
        let first = present_retained(&mut cache, &package, &instance, &mut producer, 1).unwrap();
        cache.store(Some(first));

        let unit = instance.node("unit").unwrap();
        // Overflow the bounded journal: readers must rebuild from committed source.
        for i in 0..8193 {
            let mut edit = instance.edit();
            edit.write(
                unit,
                UiProperty::Translation,
                UiValue::Translation([i as f32, 0.]),
            )
            .unwrap();
            edit.commit().unwrap();
        }
        instance
            .install_custom_output(
                unit,
                project_runtime_sdk::aui_extension::UiExtensionOutput::Measure {
                    output: project_runtime_sdk::aui_extension::UiExtensionMeasureOutput {
                        size: [23.0, 17.0],
                        first_baseline: None,
                    },
                },
            )
            .unwrap();
        let updated = present_retained(&mut cache, &package, &instance, &mut producer, 2).unwrap();
        assert!(!cache.c2_runtime().prepared_input_reused());
        assert_eq!(
            updated.resolved_document.nodes[1]
                .custom_runtime
                .as_ref()
                .unwrap()
                .measure
                .as_ref()
                .unwrap()
                .size,
            [23.0, 17.0]
        );
    }

    #[test]
    fn production_presenter_restores_binding_after_controller_releases_property() {
        let mut package = retained_package();
        let document = package.aui_documents.documents_by_id.get_mut("ui").unwrap();
        document.nodes[1].binding_refs.push(AuiBindingRef::new(
            "unit-visible",
            AuiBindingTarget::PanelVisible,
            "test.unit_visible",
            Some(AuiBindingValue::Bool(true)),
        ));
        let mut instance = AuiInstance::mount(Arc::new(document.clone())).unwrap();
        let mut cache = AuiPresentCache::for_package(&package);
        let mut producer = ReusingProducer { fail_next: false };
        let first = present_retained(&mut cache, &package, &instance, &mut producer, 1).unwrap();
        assert!(first.resolved_document.nodes[1].visible);
        cache.store(Some(first));

        let unit = instance.node("unit").unwrap();
        let mut edit = instance.edit();
        edit.transfer_property_ownership(unit, UiProperty::Visibility, UiPropertyOwner::Controller)
            .unwrap();
        edit.write(
            unit,
            UiProperty::Visibility,
            UiValue::Visibility(UiVisibility::Hidden),
        )
        .unwrap();
        edit.commit().unwrap();
        let hidden = present_retained(&mut cache, &package, &instance, &mut producer, 2).unwrap();
        assert!(!hidden.resolved_document.nodes[1].visible);
        assert!(!hidden.layout.computed_nodes[1].effective_visible);
        assert_eq!(cache.c2_runtime().prepared_document_nodes_copied(), 0);
        cache.store(Some(hidden));

        let mut edit = instance.edit();
        edit.transfer_property_ownership(unit, UiProperty::Visibility, UiPropertyOwner::Binding)
            .unwrap();
        edit.commit().unwrap();
        let restored = present_retained(&mut cache, &package, &instance, &mut producer, 3).unwrap();
        assert!(restored.resolved_document.nodes[1].visible);
        assert!(restored.layout.computed_nodes[1].effective_visible);
    }
    #[test]
    fn shared_present_cache_receives_declared_binding_paths() {
        use crate::aui_virtual_list::{AuiVirtualListDeclaration, AuiVirtualListExtentMode};

        let mut document = AuiDocument::new(
            "player-bindings",
            Vec::new(),
            vec![
                AuiNode::new(
                    "grain",
                    AuiNodeKind::Text,
                    AuiRect::fixed_position(0.0, 0.0, 1.0, 1.0),
                )
                .with_binding(AuiBindingRef::new(
                    "grain-binding",
                    AuiBindingTarget::TextText,
                    "tower.military_grain_text",
                    None,
                )),
                AuiNode::new(
                    "reserve",
                    AuiNodeKind::Text,
                    AuiRect::fixed_position(0.0, 0.0, 1.0, 1.0),
                )
                .with_binding(AuiBindingRef::new(
                    "reserve-binding",
                    AuiBindingTarget::TextText,
                    "tower.reserve.0.text",
                    None,
                ))
                .with_binding(AuiBindingRef::new(
                    "grain-binding-duplicate",
                    AuiBindingTarget::TextText,
                    "tower.military_grain_text",
                    None,
                )),
            ],
        );
        document.virtual_lists.push(AuiVirtualListDeclaration {
            list_node_id: "reserve".into(),
            data_path: "tower.reserve.items".into(),
            item_template_node_id: "reserve-template".into(),
            overscan: 64.0,
            max_live_instances: 16,
            extent_mode: AuiVirtualListExtentMode::Fixed { item_extent: 32.0 },
        });

        assert_eq!(
            active_aui_binding_paths(&document),
            vec![
                "tower.military_grain_text".to_string(),
                "tower.reserve.0.text".to_string(),
                "tower.reserve.items".to_string(),
            ]
        );
    }

    #[test]
    fn static_present_variants_exclude_large_documents() {
        assert!(static_present_variant_is_bounded(4_096));
        assert!(!static_present_variant_is_bounded(4_097));
        assert!(!static_present_variant_is_bounded(10_000));
    }
}
