# 374: official Spine backend

This standalone Cargo root provides the strict in-memory importer, official state
and ordered CPU geometry consumed by the engine workspace. Current construction
and qualification status belongs to the engine's 49/54 entries. This crate is not
an installed Provider or project SDK.

The official C++ core is pinned in `vendor/spine-cpp/UPSTREAM.json`; its original
file identities are retained there alongside the local PropertyId lookup patch.
That patch uses stable standard-map nodes in the original list order, with a
per-map standard raw-storage pool. Remove/clear destroy all logical nodes and
map destruction releases the entire pool. Full-key equality and all official
animation decisions remain. Release builds define NDEBUG; Debug retains native asserts.
All 159 upstream baseline files were checked against Git blob IDs. Keep `LICENSE` and source
notices. No editor, license credentials, or third-party example artwork is included.
The project-authored mathematical fixtures are described in `tests/fixtures/README.md`.

Run from this directory on Windows x64 with Rust 1.96.0 and MSVC 19.44:

```powershell
cargo +1.96.0 test --offline --locked -j1 --target-dir I:/gameEngin-build/374/cargo-target --tests -- --test-threads=1
cargo +1.96.0 check --offline --locked -j1 --target-dir I:/gameEngin-build/374/cargo-target --lib
cargo +1.96.0 clippy --offline --locked -j1 --target-dir I:/gameEngin-build/374/cargo-target --all-targets -- -D warnings
cargo +1.96.0 fmt --check
```

The single-thread test option is necessary for the allocator balance test. It
does not assert general multithread qualification. Instances, assets, and masks
are `!Send + !Sync`; official attachment-ID allocation is serialized during asset
construction. No per-frame global lock or I/O is added to evaluation.

`Asset::from_validated_json` is unsafe on purpose: the official parser assumes
valid exported structures. `import::validate` / `validate_declared` now check the
admitted reference/type/length/budget invariants before invoking it. Unknown fields,
duplicate keys, physics, sequence, two-color, unsupported formats/rotations and
linked-parent chains reject explicitly. `Instance::step` accepts a bounded delta only for
these feasibility tests. It is not the authoritative integer-tick controller.
Gate C owns tick-derived bounded phase/mix clocks, dependency-aware mask validation,
full supported timeline cloning, exact interruption goldens, and event publication.

Mask timelines are deep copied via public frame/curve APIs and fresh constructors.
Deform clones use the canonical attachment in the retained source data at
construction, so property IDs and apply-time attachment identity agree. Source
timelines never change. The probe whitelist is rotate/translate/deform; other
types reject. The 64-bit selection argument is a probe convenience, not the frozen
256-bone production mask representation. Gate B metadata retains source-f64 event
times, defaults/overrides, ordinals and stable event subasset IDs; session publication
and the original package consumer are pending.

For AI navigation: `src/import.rs` owns admission and attachment/timeline checks;
`src/import/constraints.rs` owns IK/transform/path checks; `metadata.rs` owns source
events, descriptor and dependency-aware Cook keys. `native/spine374.cpp` retains
official evaluation/clipping; `src/geometry.rs` returns ordered local vertices,
indices, per-page blend segments and bounds. No filesystem or GPU access exists here.

Admission safety budgets are 32MiB JSON, 1MiB atlas, 64KiB descriptor, and a 64MiB
conservative expanded-timeline estimate (including sparse deform expansion). The
official JSON parser converts every number to int, so numeric tokens outside the
int32 range reject before FFI even when the field is a float. Atlas admission uses
the 4.2 bounds/offsets form, nearest/linear clamp sampling, and 0/90 degree rotation.
The original engine Cook must verify actual texture dimensions, normalize PMA in
linear space and admit these capabilities before publishing a package.

CPU clipping calls the official implementation once per triangle: with at most
16 polygon vertices, scratch has a conservative bound of 266 vertices/714 indices
per call, below uint16. Identical vertex/UV values share indices only within their
own attachment; draw order is retained. Total output is bounded to 32768/98304.
Pixel equations and real GPU results remain Gate B exit requirements.

`contracts/` freezes proposed authoring schemas and qualification inputs. Their
presence does not mean the engine accepts them yet. See the primary-worktree
`374-B-实际合同与验收.md` for capability, error, coordinate, budget and Gate status.
