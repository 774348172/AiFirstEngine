#pragma once
#include <stdint.h>

// Internal backend ABI. No STL, exception, TrackEntry, or borrowed data crosses it.
struct S374Asset;
struct S374Mask;
struct S374Instance;
struct S374Pose { float x, y, rotation; };
struct S374BoneTransform { float x, y, a, b, c, d; };
struct S374EventKey { float seconds; uint32_t ordinal; };
struct S374Vertex { float x, y, u, v, r, g, b, a; };
struct S374Batch { uint32_t page, blend, first_index, index_count; };
struct S374GeometryCounts { uint32_t vertices, indices, batches; };
struct S374Sample { uint64_t id; float phase, mix; };

extern "C" {
int s374_asset_create(const char* json, const char* atlas, S374Asset** out, char* error) noexcept;
void s374_asset_destroy(S374Asset*) noexcept;
uint32_t s374_atlas_pages(S374Asset*) noexcept;
uint32_t s374_event_keys(S374Asset*, const char* animation, S374EventKey* out, uint32_t capacity) noexcept;
float s374_duration(S374Asset*, const char* animation) noexcept;
int s374_mask_create(S374Asset*, const char* animation, uint64_t bone_bits, S374Mask** out) noexcept;
int s374_mask_create256(S374Asset*, const char*, const uint64_t*, S374Mask**) noexcept;
void s374_mask_destroy(S374Mask*) noexcept;
uint32_t s374_mask_timelines(S374Mask*) noexcept;
int s374_mask_property(S374Asset*, S374Mask*, const char* animation, uint32_t timeline) noexcept;
int s374_instance_create(S374Asset*, S374Instance** out) noexcept;
void s374_instance_destroy(S374Instance*) noexcept;
int s374_play(S374Instance*, uint32_t track, const char* animation, S374Mask*, float mix, int loop, int additive) noexcept;
int s374_empty(S374Instance*, uint32_t track, float mix) noexcept;
int s374_step(S374Instance*, float dt) noexcept;
int s374_tag(S374Instance*, uint32_t track, uint64_t id, float alpha) noexcept;
int s374_sample(S374Instance*, const S374Sample*, uint32_t count) noexcept;
int s374_prepare_sample(S374Instance*, const S374Sample*, uint32_t count) noexcept;
uint32_t s374_entries(S374Instance*, uint64_t* ids) noexcept;
int s374_clear(S374Instance*, uint32_t track) noexcept;
int s374_additive(S374Asset*, const char*, S374Mask*) noexcept;
int s374_mask_used(S374Instance*, S374Mask*) noexcept;
int s374_bone(S374Instance*, const char* bone, S374Pose* out) noexcept;
int s374_bone_transform(S374Instance*, uint32_t bone, S374BoneTransform*) noexcept;
int s374_vertices(S374Instance*, const char* slot, float* out, uint32_t capacity) noexcept;
float s374_clock(S374Instance*, uint32_t track) noexcept;
float s374_local_clock(S374Instance*, uint32_t track) noexcept;
uint32_t s374_callback_events(S374Instance*) noexcept;
uint64_t s374_live_allocations() noexcept;
int s374_skin(S374Instance*, const char* name) noexcept;
// Bounded, call-local equality input. Capacity/unsupported means use extraction.
int s374_geometry_input(S374Instance*, float ppu, uint8_t*, uint32_t capacity, uint32_t* written) noexcept;
int s374_geometry(S374Instance*, float pixels_per_unit,
    S374Vertex*, uint32_t vertex_capacity, uint32_t*, uint32_t index_capacity,
    S374Batch*, uint32_t batch_capacity, S374GeometryCounts*) noexcept;
}
