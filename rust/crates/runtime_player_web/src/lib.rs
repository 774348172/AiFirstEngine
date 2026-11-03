//! Browser host for the original Rust runtime and wgpu backend.
#![cfg(target_arch = "wasm32")]
use core::aui::*;
use core::aui_present_cache::{AuiPresentCache, AuiPresentContext};
use core::engine_host_loop::{EngineFrameInput, EngineHostLoop, EngineHostMode};
use core::game_view_presentation::*;
use core::project_runtime_module::ProjectRuntimeBootstrap;
use core::runtime_package::{load_runtime_package_from_source, RuntimePackage};
use core::runtime_scene_hydration::RuntimeSceneHydrator;
use core::runtime_texture::{
    load_runtime_texture_payload_from_source, runtime_texture_render_handle,
    RuntimeTextureBindingContext,
};
use core::sprite2d_render_pipeline::Sprite2DTextureBindingContext;
use core::world::World;
use engine_content::web_package::WebPackageManifest;
use engine_input::{
    InputDeviceState, InputMappingAsset, InputResolver, RawInputEvent, RuntimePointerButton,
};
use engine_render_native::wgpu_backend::real::RealWgpuBackend;
use engine_runtime_core as core;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

fn error(value: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&value.to_string())
}

#[wasm_bindgen]
pub struct WebPlayer {
    canvas: web_sys::HtmlCanvasElement,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    output_format: wgpu::TextureFormat,
    device: wgpu::Device,
    gpu_error: Arc<std::sync::Mutex<Option<String>>>,
    backend: RealWgpuBackend,
    package: RuntimePackage,
    hydrator: RuntimeSceneHydrator,
    world: World,
    host: EngineHostLoop,
    producer: Box<dyn ProjectUiStateSnapshotProducer>,
    mapping: InputMappingAsset,
    input: InputDeviceState,
    raw: Vec<RawInputEvent>,
    interaction: AuiInteractionState,
    feedback: core::aui_control_feedback::AuiControlFeedbackState,
    aui: AuiPresentCache,
    sprites: Sprite2DTextureBindingContext,
    textures: RuntimeTextureBindingContext,
    frames: u64,
    fixed_ticks: u64,
    last_timestamp: Option<f64>,
    remainder: f64,
    pressed: std::collections::BTreeMap<String, engine_input::InputActionState>,
    draw_count: usize,
    closed: bool,
    paused: bool,
}

pub async fn create_player(
    canvas: web_sys::HtmlCanvasElement,
    manifest: &str,
    files: js_sys::Map,
    api: project_runtime_abi::ProjectRuntimeApi,
) -> Result<WebPlayer, JsValue> {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&JsValue::from_str(&format!("web.panic:{info}")))
    }));
    let receipt: WebPackageManifest = serde_json::from_str(manifest).map_err(error)?;
    let mut downloaded = Vec::new();
    for pair in js_sys::try_iter(&files)?.ok_or_else(|| error("web.package.files_not_iterable"))? {
        let pair = js_sys::Array::from(&pair?);
        let key = pair
            .get(0)
            .as_string()
            .ok_or_else(|| error("web.package.path_not_string"))?;
        downloaded.push((key, js_sys::Uint8Array::new(&pair.get(1)).to_vec()));
    }
    let source = receipt.verified_source(downloaded).map_err(error)?;
    let loaded = load_runtime_package_from_source(source);
    let package = loaded
        .value
        .ok_or_else(|| error(format!("web.package.decode:{:?}", loaded.diagnostics)))?;
    if package.manifest.project.project_id != receipt.project_id
        || package.manifest.project.runtime_module.module_id != receipt.module_id
        || package.manifest.content_hash.as_deref() != Some(&receipt.package_content_hash)
    {
        return Err(error("web.package.identity_mismatch"));
    }
    let linked = core::project_runtime_native_adapter::linked_project_runtime_set_from_api(api)
        .map_err(error)?;
    let parts = ProjectRuntimeBootstrap::bind(&package, &linked)
        .map_err(error)?
        .into_parts();
    let mut world = World::new();
    let mut hydrator = RuntimeSceneHydrator::from_package(&package);
    let hydration = hydrator.hydrate_active_scene(&package, &mut world);
    if hydration.has_errors() {
        return Err(error(format!(
            "web.scene.hydration:{:?}",
            hydration.instantiate_report.diagnostics
        )));
    }
    let mut host = EngineHostLoop::with_project_runtime_session(
        &package.active_scene.id,
        parts.project_logic,
        parts.project_runtime_session,
    );
    host.set_external_rhi_enabled(true);
    host.set_game_view_target(package.manifest.project.game_view_target.unwrap_or(
        GameViewTargetSpec::new(1280, 720, GameViewScalePolicy::Contain),
    ));
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::BROWSER_WEBGPU,
        ..Default::default()
    });
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .map_err(error)?;
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
        })
        .await
        .map_err(error)?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("aife-web-player"),
            required_features: wgpu::Features::empty(),
            required_limits: engine_render_native::particle_gpu::renderer_device_limits(
                adapter.limits(),
            ),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(error)?;
    let gpu_error = Arc::new(std::sync::Mutex::new(None));
    let captured_error = Arc::clone(&gpu_error);
    device.on_uncaptured_error(Box::new(move |error| {
        let message = format!("web.gpu.validation:{error}");
        web_sys::console::error_1(&JsValue::from_str(&message));
        if let Ok(mut current) = captured_error.lock() {
            if current.is_none() {
                *current = Some(message);
            }
        }
    }));
    let mut config = surface
        .get_default_config(&adapter, canvas.width().max(1), canvas.height().max(1))
        .ok_or_else(|| error("web.gpu.surface_config_unavailable"))?;
    config.present_mode = wgpu::PresentMode::Fifo;
    // Core AUI and texture tint values are linear. WebGPU canvas formats are
    // unorm; render through the compatible sRGB view to encode for display.
    let output_format = config.format.add_srgb_suffix();
    if output_format != config.format {
        config.view_formats.push(output_format);
    }
    surface.configure(&device, &config);
    let mut backend = RealWgpuBackend::from_device_queue(
        device.clone(),
        queue,
        output_format,
        config.width,
        config.height,
        "BrowserWebGPU",
    );
    if !package.font_bundles.bundles_by_id.is_empty() {
        for bundle in package.font_bundles.bundles_by_id.values() {
            backend
                .register_font_texture_arrays(bundle)
                .map_err(error)?;
        }
    } else if let Some(atlas) = package.font_atlases.default_atlas() {
        backend
            .register_alpha8_texture(
                core::runtime_renderer::font_atlas_render_handle(&atlas.metadata.font_atlas_id),
                atlas.metadata.atlas_width,
                atlas.metadata.atlas_height,
                &atlas.atlas_alpha,
                "nearestClamp",
            )
            .map_err(error)?;
    }
    let mut textures = RuntimeTextureBindingContext::default();
    let mut sprites = Sprite2DTextureBindingContext::new();
    for record in &package.assets.runtime_asset_index {
        if record.asset_type != "texture" {
            continue;
        }
        let reference = core::runtime_package::RuntimeAssetRef {
            id: record.asset_id.clone(),
            asset_type: "texture".into(),
            guid: Some(record.asset_guid.clone()),
            sub_asset: None,
        };
        let texture = load_runtime_texture_payload_from_source(
            &package.source,
            &package.runtime_asset_index,
            &reference,
        )
        .map_err(|e| error(format!("{e:?}")))?;
        let handle = runtime_texture_render_handle(&record.asset_id);
        backend
            .register_cooked_texture(handle, &texture)
            .map_err(error)?;
        textures.insert_with_extent(
            record.asset_id.clone(),
            handle,
            texture.sampler.clone(),
            texture.width,
            texture.height,
        );
        sprites.insert_texture_handle(record.asset_id.clone(), handle, texture.sampler);
    }
    let aui = AuiPresentCache::for_package(&package);
    Ok(WebPlayer {
        canvas,
        surface,
        config,
        output_format,
        device,
        gpu_error,
        backend,
        package,
        hydrator,
        world,
        host,
        producer: parts.ui_state_producer,
        mapping: parts.default_input_mapping,
        input: InputDeviceState::new(),
        raw: Vec::new(),
        interaction: AuiInteractionState::default(),
        feedback: Default::default(),
        aui,
        sprites,
        textures,
        frames: 0,
        fixed_ticks: 0,
        last_timestamp: None,
        remainder: 0.0,
        pressed: Default::default(),
        draw_count: 0,
        closed: false,
        paused: false,
    })
}

#[wasm_bindgen]
impl WebPlayer {
    pub fn pointer(&mut self, x: f32, y: f32, phase: &str) {
        if self.closed || self.raw.len() >= 1024 {
            return;
        }
        self.raw.push(RawInputEvent::mouse_move(
            self.frames + 1,
            "web-canvas",
            x,
            y,
        ));
        match phase {
            "down" => self.raw.push(RawInputEvent::mouse_button_down(
                self.frames + 1,
                "web-canvas",
                RuntimePointerButton::Primary,
            )),
            "up" => self.raw.push(RawInputEvent::mouse_button_up(
                self.frames + 1,
                "web-canvas",
                RuntimePointerButton::Primary,
            )),
            "cancel" => self
                .raw
                .push(RawInputEvent::focus_lost(self.frames + 1, "web-canvas")),
            _ => {}
        }
    }
    pub fn key(&mut self, key: &str, down: bool) {
        if self.closed || self.raw.len() >= 1024 {
            return;
        }
        let event = if down {
            RawInputEvent::keyboard_down(self.frames + 1, "web-canvas", key)
        } else {
            RawInputEvent::keyboard_up(self.frames + 1, "web-canvas", key)
        };
        self.raw.push(event);
    }
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        self.last_timestamp = None;
        self.remainder = 0.0;
        self.pressed.clear();
        if paused {
            self.raw.clear();
            self.raw
                .push(RawInputEvent::focus_lost(self.frames + 1, "web-canvas"));
        }
        self.host.dispatch_focus_changed(!paused, &mut self.world);
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        if self.closed || width == 0 || height == 0 {
            return;
        }
        let width = width.min(4096);
        let height = height.min(4096);
        if self.config.width == width && self.config.height == height {
            return;
        }
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }
    pub fn frame(&mut self, timestamp: f64) -> Result<(), JsValue> {
        if self.closed || self.paused {
            return Ok(());
        }
        if let Some(message) = self.gpu_error.lock().map_err(error)?.as_ref() {
            return Err(error(message));
        }
        if !timestamp.is_finite() {
            return Err(error("web.clock.invalid_timestamp"));
        }
        let elapsed = self
            .last_timestamp
            .replace(timestamp)
            .map_or(1.0 / 60.0, |last| {
                ((timestamp - last) / 1000.0).clamp(0.0, 0.1)
            });
        self.remainder += elapsed;
        let ticks = ((self.remainder * 60.0).floor() as usize).min(6);
        self.remainder -= ticks as f64 / 60.0;
        let frame = self.frames + 1;
        let target =
            self.package
                .manifest
                .project
                .game_view_target
                .unwrap_or(GameViewTargetSpec::new(
                    1280,
                    720,
                    GameViewScalePolicy::Contain,
                ));
        let references = self
            .package
            .aui_manifest
            .documents
            .first()
            .and_then(|entry| self.package.aui_documents.get(&entry.document_id))
            .map(|doc| {
                doc.canvases
                    .iter()
                    .map(|c| {
                        CanvasReferenceFact::new(
                            &c.canvas_id,
                            c.reference_resolution.x as u32,
                            c.reference_resolution.y as u32,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let presentation = Arc::new(
            GameViewPresentationModule::resolve(GameViewPresentationSpec {
                session_id: "web-player".into(),
                target_id: "web-surface".into(),
                target_extent: target.extent,
                display_rect: GameViewRect::from_extent(GameViewExtent::new(
                    self.config.width,
                    self.config.height,
                )),
                scale_policy: target.scale_policy,
                surface_generation: 1,
                presentation_revision: frame,
                canvas_references: references,
            })
            .map_err(|e| error(format!("{e:?}")))?,
        );
        // UI is composed into the logical GameView before the final surface
        // contain transform. Surface bars and DPR must not become UI layout size.
        let viewport = core::aui_viewport::AuiViewportInput::new(
            target.extent.width,
            target.extent.height,
            1.0,
        );
        let mut present = self.aui.take_or_rebuild_stabilized(
            &self.package,
            &self.world,
            frame,
            self.producer.as_mut(),
            &mut self.host,
            AuiPresentContext::new(Some(&presentation), Some(&viewport), &mut self.interaction)
                .with_feedback_cache(&mut self.feedback),
        );
        // Browser coordinates are canvas physical pixels; map once to the engine target.
        for raw in &mut self.raw {
            if let engine_input::RawInputValue::Pointer { x, y } = &mut raw.value {
                if let Ok(p) = presentation.display_to_target(GameViewPoint::new(*x, *y)) {
                    *x = p.x;
                    *y = p.y;
                }
            }
        }
        self.input.begin_frame();
        self.input.apply_raw_events(&self.raw);
        self.raw.clear();
        let runtime_input = self.input.to_runtime_input_frame(frame, "web-canvas");
        let interaction = present.as_ref().map(|p| {
            AuiInteractionSystem::process_target_space_session_with_state(
                &p.resolved_document,
                &p.layout,
                &runtime_input,
                &mut self.interaction,
                AuiInteractionConfig::default(),
                &presentation,
                "web-canvas",
            )
        });
        let filtered = interaction.as_ref().map_or_else(
            || runtime_input.clone(),
            |i| runtime_input.filter_consumed_events(&i.consumed_event_indices),
        );
        let mut snapshot = InputResolver::resolve(&filtered, &self.mapping).action_snapshot;
        for action in &snapshot.actions {
            if matches!(
                action.value,
                engine_input::ActionValue::Button {
                    phase: engine_input::ActionPhase::Pressed
                }
            ) {
                self.pressed
                    .insert(action.action_id.clone(), action.clone());
            }
        }
        if ticks > 0 {
            for (id, action) in std::mem::take(&mut self.pressed) {
                snapshot.actions.retain(|a| a.action_id != id);
                snapshot.actions.push(action);
            }
        }
        present = self.aui.advance_animations_for_present(
            present,
            &self.package,
            &self.world,
            self.producer.as_mut(),
            &mut self.host,
            core::aui_present_cache::AuiAnimationPresentContext {
                frame_index: frame,
                delta_us: (elapsed * 1_000_000.0) as u64,
                interaction_result: interaction.as_ref(),
                presentation: AuiPresentContext::new(
                    Some(&presentation),
                    Some(&viewport),
                    &mut self.interaction,
                )
                .with_feedback_cache(&mut self.feedback),
            },
        );
        if let Some(p) = &present {
            if p.report.status == AuiRuntimePresentStatus::Failed {
                return Err(error(format!("web.aui.present:{:?}", p.report.diagnostics)));
            }
        }
        if let (Some(p), Some(i)) = (&mut present, &interaction) {
            AuiRuntimePresenter::apply_control_feedback_with_fonts_for_presentation_and_themes(
                p,
                i,
                &mut self.feedback,
                (elapsed * 1_000_000.0) as u64,
                &self.package.font_atlases,
                &self.package.font_bundles,
                &self.package.aui_themes,
                Some(&presentation),
            );
            self.host.publish_ui_geometry(p);
        }
        let mut input = EngineFrameInput::new(EngineHostMode::ExportedGame)
            .with_action_snapshot(snapshot)
            .with_fixed_step_count(ticks)
            .with_unscaled_delta_time(elapsed as f32)
            .with_deferred_render_submission();
        if let Some(i) = interaction {
            input = input.with_aui_interaction(i);
        }
        if let Some(p) = &present {
            input = input
                .with_aui_overlay(p.overlay.clone())
                .with_aui_composition(p.composition.clone());
        }
        let output = self.host.tick_with_runtime_context(
            input,
            &mut self.world,
            core::frame_loop::RuntimeFrameContext::new(
                &self.package,
                self.hydrator.instance_loader_mut(),
            ),
        );
        if output
            .project_runtime_session_report
            .as_ref()
            .is_some_and(|report| report.terminal_fault)
        {
            return Err(error(format!(
                "web.session.fault:{:?}",
                output.project_runtime_session_report
            )));
        }
        if output.host_control.is_some() {
            self.paused = true;
            return Ok(());
        }
        let rendered = self
            .host
            .render_thread_for_target_with_runtime_resources_and_presentation(
                core::runtime_renderer::RenderTarget::surface(
                    "web-surface",
                    self.config.width,
                    self.config.height,
                )
                .with_presentation_scale_policy(target.scale_policy),
                present.as_ref().map(|p| &p.overlay),
                present.as_ref().map(|p| &p.composition),
                Some(&self.sprites),
                Some(&self.textures),
                Some(presentation.clone()),
            );
        if rendered.report.rhi_status != "ok" {
            return Err(error(format!(
                "web.render.plan:{:?}",
                rendered.report.diagnostics
            )));
        }
        self.aui.store(present);
        let plan = &rendered.renderer_output.rhi_command_plan;
        if !self
            .backend
            .prepare_surface_resources(
                plan,
                [self.config.width, self.config.height],
                Some(presentation.display_content_rect),
            )
            .map_err(error)?
        {
            return Ok(());
        }
        let texture = match self.surface.get_current_texture() {
            Ok(texture) => texture,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            Err(wgpu::SurfaceError::Timeout) => return Ok(()),
            Err(e) => return Err(error(e)),
        };
        let view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.output_format),
            ..Default::default()
        });
        let report = self.backend.execute_plan_to_surface_view_in_rect(
            plan,
            &view,
            presentation.display_content_rect,
        );
        if report.submit_count == 0 || report.failed_resource_count > 0 {
            return Err(error(format!("web.render.submit:{:?}", report.diagnostics)));
        }
        texture.present();
        if let Some(token) = self.backend.last_gpu_submission() {
            self.backend.record_surface_present(token);
        }
        self.draw_count = report.draw_count;
        self.frames = frame;
        self.fixed_ticks += ticks as u64;
        Ok(())
    }
    /// Explicit low-frequency diagnostic; the normal frame path creates no JSON.
    pub fn diagnostics(&self) -> String {
        let texts:Vec<_>=self.aui.current().into_iter().flat_map(|p|p.resolved_document.nodes.iter()).filter(|n|n.visible).filter_map(|n|n.text.as_ref().map(|t|serde_json::json!({"id":n.node_id,"text":t.chars().take(256).collect::<String>()}))).take(64).collect();
        serde_json::json!({"auiText":texts,"gpuError":self.gpu_error.lock().ok().and_then(|e|e.clone()),"projectId":self.package.manifest.project.project_id,"packageHash":self.package.manifest.content_hash,"moduleId":self.package.manifest.project.runtime_module.module_id,"frames":self.frames,"fixedTicks":self.fixed_ticks,"draws":self.draw_count,"paused":self.paused,"closed":self.closed,"auiDocuments":self.package.aui_documents.documents_by_id.len(),"fontBundles":self.package.font_bundles.bundles_by_id.len(),"width":self.config.width,"height":self.config.height}).to_string()
    }
    pub fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.raw.clear();
        // Browser shutdown cannot block waiting for its own event loop.
        self.backend.abandon_device_epoch();
    }
}
impl Drop for WebPlayer {
    fn drop(&mut self) {
        self.close();
    }
}
