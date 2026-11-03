# wgpu 26.0.1 lifetime backport

The vendored wgpu package is the unmodified cached crates.io 26.0.1 package except the two backports below.

`src/backend/wgpu_core.rs`: CoreRenderBundle now retains its ContextWgpuCore and
calls the existing global render_bundle_drop when its last dispatched Arc drops,
following the other native core handle destructors. The version, public API,
features and rendering behavior stay the same. Original licenses are retained.

This fixes a reproduced leak: dropping the final public RenderBundle alias left
its registry entry alive. Engine dynamic mesh recordings accumulated one bundle
per frame even after GPU completion and backend shutdown. A deterministic native
boundary regression and the original dynamic mesh / HostLoop consumers cover it.

The workspace patch applies to every engine consumer. Keeping this copy inside
engine_render_native also includes its Rust/TOML sources in the existing Compiler
Engine SDK source fingerprint. No global Cargo cache file is changed.

Provenance and the exact upstream file checksums, local diff, first-red output and
consumer receipts are in `.artifacts/374/gate-f-memory-20261024-033828` on main.

## WebGPU callback lifetime backport (375, 2026-10-25)

`wgpu/src/backend/webgpu.rs`, `set_device_lost_callback`: keep the one-shot
closure registered with `GPUDevice.lost` alive by transferring memory management
to JS GC with `closure.forget()` after `then`. Without it, the borrowed Rust
closure drops at function return; destroying the old device during restart
reproduced an asynchronous `closure invoked recursively or after being dropped`.
The version, API and native backport above are retained. This file is selected
only for the WebGPU backend.

Reference: wgpu upstream `wgpu/src/backend/webgpu.rs`, same function, lines
2597–2600 as retrieved 2026-10-25 from
https://raw.githubusercontent.com/gfx-rs/wgpu/trunk/wgpu/src/backend/webgpu.rs .
The original 26.0.1 source has no transfer after `then`, as confirmed in
https://raw.githubusercontent.com/gfx-rs/wgpu/v26.0.1/wgpu/src/backend/webgpu.rs .
Only this lifetime fix is backported; no upstream API or version migration.

First-red browser stack, pre-edit source hashes/backups, WASM lint and actual
restart/close/reopen verification are under
`.artifacts/375/web-runtime-20261025-061020`. No global Cargo registry is modified.
