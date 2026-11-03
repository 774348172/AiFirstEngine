#include "spine374.h"
#include <spine/spine.h>
#include <atomic>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <memory_resource>
#include <mutex>
#include <stdexcept>
#include <vector>
#include <cmath>
#include <algorithm>
#include <array>
#include <unordered_map>
#include <limits>

using namespace spine;
namespace {
std::atomic<uint64_t> live_allocations{0};
// All runtime objects are confined to one Rust owner thread. Initialization is
// nevertheless synchronized because independent tests may load concurrently.
class Extension final : public DefaultSpineExtension {
    void* _alloc(size_t n, const char*, int) override {
        if (!n) return nullptr;
        void* p = std::malloc(n);
        if (!p) std::abort(); // Official containers do not support recoverable OOM.
        ++live_allocations;
        return p;
    }
    void* _calloc(size_t n, const char* f, int l) override {
        void* p = _alloc(n, f, l);
        if (p) std::memset(p, 0, n);
        return p;
    }
    void* _realloc(void* p, size_t n, const char* f, int l) override {
        if (!n) { _free(p, f, l); return nullptr; }
        if (!p) return _alloc(n, f, l);
        void* result = std::realloc(p, n);
        if (!result) std::abort();
        return result;
    }
    void _free(void* p, const char*, int) override {
        if (p) { --live_allocations; std::free(p); }
    }
    char* _readFile(const String&, int*) override { return nullptr; }
};
Extension extension;
std::once_flag initialized;
std::mutex asset_creation;
void initialize() { std::call_once(initialized, [] { SpineExtension::setInstance(&extension); }); }
void error_text(char* out, const char* message) { std::snprintf(out, 256, "%s", message); }
bool included(const uint64_t* bits, int index) { return index >= 0 && index < 256 && (bits[index/64] & (uint64_t(1) << (index%64))); }
void copy_curve(CurveTimeline& from, CurveTimeline& to) {
    to.getFrames().clearAndAddAll(from.getFrames());
    to.getCurves().clearAndAddAll(from.getCurves());
}
}
namespace spine { SpineExtension* getDefaultExtension() { return &extension; } }

struct S374Asset {
    // Reverse destruction order: SkeletonData (including attachments) before Atlas.
    std::unique_ptr<Atlas> atlas;
    std::unique_ptr<SkeletonData> data;
};
struct S374Mask {
    std::unique_ptr<Animation> animation;
};
struct S374Instance {
    S374Asset* asset;
    std::unique_ptr<Skeleton> skeleton;
    std::unique_ptr<AnimationStateData> state_data;
    std::unique_ptr<AnimationState> state;
    uint32_t callback_events = 0;
};
namespace {
bool finite_pose(Skeleton& skeleton) {
    auto& bones = skeleton.getBones();
    for (size_t i = 0; i < bones.size(); ++i) {
        auto& b = *bones[i];
        if (!std::isfinite(b.getWorldX()) || !std::isfinite(b.getWorldY()) ||
            !std::isfinite(b.getA()) || !std::isfinite(b.getB()) ||
            !std::isfinite(b.getC()) || !std::isfinite(b.getD())) return false;
    }
    return true;
}
}

extern "C" int s374_asset_create(const char* json, const char* atlas, S374Asset** out, char* error) noexcept {
    *out = nullptr;
    try {
        initialize();
        // Official VertexAttachment::getNextID has process-global mutable state.
        // Serialize asset construction; evaluated instances remain thread-confined.
        std::lock_guard<std::mutex> lock(asset_creation);
        auto asset = std::make_unique<S374Asset>();
        asset->atlas = std::make_unique<Atlas>(atlas, int(std::strlen(atlas)), "", nullptr, false);
        if (asset->atlas->getPages().size() == 0) { error_text(error, "empty atlas"); return 1; }
        SkeletonJson reader(asset->atlas.get());
        asset->data.reset(reader.readSkeletonData(json));
        if (!asset->data) { error_text(error, reader.getError().buffer()); return 2; }
        *out = asset.release();
        return 0;
    } catch (const std::bad_alloc&) { std::abort(); }
      catch (const std::exception& e) { error_text(error, e.what()); return 3; }
      catch (...) { error_text(error, "native load failure"); return 3; }
}
extern "C" void s374_asset_destroy(S374Asset* p) noexcept { delete p; }
extern "C" uint32_t s374_atlas_pages(S374Asset* p) noexcept { return uint32_t(p->atlas->getPages().size()); }
extern "C" float s374_duration(S374Asset* p, const char* name) noexcept {
    auto* animation = p->data->findAnimation(name);
    return animation ? animation->getDuration() : -1;
}
extern "C" uint32_t s374_event_keys(S374Asset* p, const char* name, S374EventKey* out, uint32_t capacity) noexcept {
    auto* animation = p->data->findAnimation(name);
    if (!animation) return 0;
    uint32_t count = 0;
    auto& timelines = animation->getTimelines();
    for (size_t i = 0; i < timelines.size(); ++i) {
        if (!timelines[i]->getRTTI().isExactly(EventTimeline::rtti)) continue;
        auto& events = static_cast<EventTimeline*>(timelines[i])->getEvents();
        for (size_t j = 0; j < events.size(); ++j) {
            if (count < capacity) out[count] = {events[j]->getTime(), count};
            ++count;
        }
    }
    return count;
}
extern "C" int s374_mask_create256(S374Asset* p, const char* name, const uint64_t* bits, S374Mask** out) noexcept {
    *out = nullptr;
    try {
        auto* source = p->data->findAnimation(name);
        if (!source) return 1;
        // Each clone owns fresh timeline/frame/curve arrays. Deform deliberately
        // references the SAME immutable canonical attachment, preserving its ID.
        // Never use setAttachment to rebind IDs after construction.
        Vector<Timeline*> clones;
        struct Cleanup {
            Vector<Timeline*>& values;
            ~Cleanup() { for (size_t i = 0; i < values.size(); ++i) delete values[i]; }
        } cleanup{clones};
        auto& timelines = source->getTimelines();
        for (size_t i = 0; i < timelines.size(); ++i) {
            Timeline* value = timelines[i];
            CurveTimeline* clone = nullptr;
            if (value->getRTTI().isExactly(RotateTimeline::rtti)) {
                auto& t = *static_cast<RotateTimeline*>(value);
                if (!included(bits, t.getBoneIndex())) continue;
                clone = new RotateTimeline(t.getFrameCount(), 0, t.getBoneIndex());
            } else if (value->getRTTI().isExactly(TranslateTimeline::rtti)) {
                auto& t = *static_cast<TranslateTimeline*>(value);
                if (!included(bits, t.getBoneIndex())) continue;
                clone = new TranslateTimeline(t.getFrameCount(), 0, t.getBoneIndex());
            } else if (value->getRTTI().isExactly(TranslateXTimeline::rtti)) {
                auto& t = *static_cast<TranslateXTimeline*>(value);
                if (!included(bits, t.getBoneIndex())) continue;
                clone = new TranslateXTimeline(t.getFrameCount(), 0, t.getBoneIndex());
            } else if (value->getRTTI().isExactly(TranslateYTimeline::rtti)) {
                auto& t = *static_cast<TranslateYTimeline*>(value);
                if (!included(bits, t.getBoneIndex())) continue;
                clone = new TranslateYTimeline(t.getFrameCount(), 0, t.getBoneIndex());
            } else if (value->getRTTI().isExactly(ScaleTimeline::rtti)) {
                auto& t = *static_cast<ScaleTimeline*>(value);
                if (!included(bits, t.getBoneIndex())) continue;
                clone = new ScaleTimeline(t.getFrameCount(), 0, t.getBoneIndex());
            } else if (value->getRTTI().isExactly(ScaleXTimeline::rtti)) {
                auto& t = *static_cast<ScaleXTimeline*>(value);
                if (!included(bits, t.getBoneIndex())) continue;
                clone = new ScaleXTimeline(t.getFrameCount(), 0, t.getBoneIndex());
            } else if (value->getRTTI().isExactly(ScaleYTimeline::rtti)) {
                auto& t = *static_cast<ScaleYTimeline*>(value);
                if (!included(bits, t.getBoneIndex())) continue;
                clone = new ScaleYTimeline(t.getFrameCount(), 0, t.getBoneIndex());
            } else if (value->getRTTI().isExactly(ShearTimeline::rtti)) {
                auto& t = *static_cast<ShearTimeline*>(value);
                if (!included(bits, t.getBoneIndex())) continue;
                clone = new ShearTimeline(t.getFrameCount(), 0, t.getBoneIndex());
            } else if (value->getRTTI().isExactly(ShearXTimeline::rtti)) {
                auto& t = *static_cast<ShearXTimeline*>(value);
                if (!included(bits, t.getBoneIndex())) continue;
                clone = new ShearXTimeline(t.getFrameCount(), 0, t.getBoneIndex());
            } else if (value->getRTTI().isExactly(ShearYTimeline::rtti)) {
                auto& t = *static_cast<ShearYTimeline*>(value);
                if (!included(bits, t.getBoneIndex())) continue;
                clone = new ShearYTimeline(t.getFrameCount(), 0, t.getBoneIndex());
            } else if (value->getRTTI().isExactly(RGBATimeline::rtti)) {
                auto& t = *static_cast<RGBATimeline*>(value);
                if (!included(bits, p->data->getSlots()[t.getSlotIndex()]->getBoneData().getIndex())) continue;
                clone = new RGBATimeline(t.getFrameCount(), 0, t.getSlotIndex());
            } else if (value->getRTTI().isExactly(RGBTimeline::rtti)) {
                auto& t = *static_cast<RGBTimeline*>(value);
                if (!included(bits, p->data->getSlots()[t.getSlotIndex()]->getBoneData().getIndex())) continue;
                clone = new RGBTimeline(t.getFrameCount(), 0, t.getSlotIndex());
            } else if (value->getRTTI().isExactly(AlphaTimeline::rtti)) {
                auto& t = *static_cast<AlphaTimeline*>(value);
                if (!included(bits, p->data->getSlots()[t.getSlotIndex()]->getBoneData().getIndex())) continue;
                clone = new AlphaTimeline(t.getFrameCount(), 0, t.getSlotIndex());
            } else if (value->getRTTI().isExactly(IkConstraintTimeline::rtti)) {
                auto& t = *static_cast<IkConstraintTimeline*>(value);
                auto* constraint = p->data->getIkConstraints()[t.getIkConstraintIndex()];
                if (!included(bits, constraint->getBones()[0]->getIndex())) continue;
                clone = new IkConstraintTimeline(t.getFrameCount(), 0, t.getIkConstraintIndex());
            } else if (value->getRTTI().isExactly(TransformConstraintTimeline::rtti)) {
                auto& t = *static_cast<TransformConstraintTimeline*>(value);
                auto* constraint = p->data->getTransformConstraints()[t.getTransformConstraintIndex()];
                if (!included(bits, constraint->getBones()[0]->getIndex())) continue;
                clone = new TransformConstraintTimeline(t.getFrameCount(), 0, t.getTransformConstraintIndex());
            } else if (value->getRTTI().isExactly(PathConstraintPositionTimeline::rtti)) {
                auto& t = *static_cast<PathConstraintPositionTimeline*>(value);
                auto* constraint = p->data->getPathConstraints()[t.getPathConstraintIndex()];
                if (!included(bits, constraint->getBones()[0]->getIndex())) continue;
                clone = new PathConstraintPositionTimeline(t.getFrameCount(), 0, t.getPathConstraintIndex());
            } else if (value->getRTTI().isExactly(PathConstraintSpacingTimeline::rtti)) {
                auto& t = *static_cast<PathConstraintSpacingTimeline*>(value);
                auto* constraint = p->data->getPathConstraints()[t.getPathConstraintIndex()];
                if (!included(bits, constraint->getBones()[0]->getIndex())) continue;
                clone = new PathConstraintSpacingTimeline(t.getFrameCount(), 0, t.getPathConstraintIndex());
            } else if (value->getRTTI().isExactly(PathConstraintMixTimeline::rtti)) {
                auto& t = *static_cast<PathConstraintMixTimeline*>(value);
                auto* constraint = p->data->getPathConstraints()[t.getPathConstraintIndex()];
                if (!included(bits, constraint->getBones()[0]->getIndex())) continue;
                clone = new PathConstraintMixTimeline(t.getFrameCount(), 0, t.getPathConstraintIndex());
            } else if (value->getRTTI().isExactly(AttachmentTimeline::rtti)) {
                auto& t = *static_cast<AttachmentTimeline*>(value);
                if (!included(bits, p->data->getSlots()[t.getSlotIndex()]->getBoneData().getIndex())) continue;
                auto* d = new AttachmentTimeline(t.getFrameCount(), t.getSlotIndex());
                d->getFrames().clearAndAddAll(t.getFrames());
                d->getAttachmentNames().clearAndAddAll(t.getAttachmentNames());
                clones.add(d); continue;
            } else if (value->getRTTI().isExactly(DrawOrderTimeline::rtti)) {
                auto& t = *static_cast<DrawOrderTimeline*>(value);
                auto* d = new DrawOrderTimeline(t.getFrameCount());
                d->getFrames().clearAndAddAll(t.getFrames());
                d->getDrawOrders().clearAndAddAll(t.getDrawOrders());
                clones.add(d); continue;
            } else if (value->getRTTI().isExactly(DeformTimeline::rtti)) {
                auto& t = *static_cast<DeformTimeline*>(value);
                int bone = p->data->getSlots()[t.getSlotIndex()]->getBoneData().getIndex();
                if (!included(bits, bone)) continue;
                auto* d = new DeformTimeline(t.getFrameCount(), 0, t.getSlotIndex(), t.getAttachment());
                d->getVertices().clearAndAddAll(t.getVertices());
                clone = d;
            } else if (value->getRTTI().isExactly(EventTimeline::rtti)) {
                continue; // Content events come from the source index, once per active action.
            } else {
                return 2; // Unqualified timeline types never silently disappear.
            }
            copy_curve(*static_cast<CurveTimeline*>(value), *clone);
            clones.add(clone);
        }
        if (clones.size() == 0) return 2;
        auto mask = std::make_unique<S374Mask>();
        mask->animation.reset(new Animation("374-owned-mask", clones, source->getDuration()));
        clones.clear(); // Animation copied the pointer vector; transfer sole ownership.
        *out = mask.release();
        return 0;
    } catch (const std::bad_alloc&) { std::abort(); }
      catch (...) { return 3; }
}
extern "C" int s374_mask_create(S374Asset* p, const char* name, uint64_t bits, S374Mask** out) noexcept {
    const uint64_t selection[4] = {bits,0,0,0};
    return s374_mask_create256(p,name,selection,out);
}
extern "C" void s374_mask_destroy(S374Mask* p) noexcept { delete p; }
extern "C" uint32_t s374_mask_timelines(S374Mask* p) noexcept { return uint32_t(p->animation->getTimelines().size()); }
extern "C" int s374_mask_property(S374Asset* p, S374Mask* mask, const char* name, uint32_t index) noexcept {
    auto* source = p->data->findAnimation(name);
    if (!source || index >= source->getTimelines().size()) return -1;
    return mask->animation->hasTimeline(source->getTimelines()[index]->getPropertyIds()) ? 1 : 0;
}
extern "C" int s374_instance_create(S374Asset* asset, S374Instance** out) noexcept {
    *out = nullptr;
    try {
        auto instance = std::make_unique<S374Instance>();
        instance->asset = asset;
        instance->skeleton = std::make_unique<Skeleton>(asset->data.get());
        instance->state_data = std::make_unique<AnimationStateData>(asset->data.get());
        instance->state = std::make_unique<AnimationState>(instance->state_data.get());
        instance->state->setRendererObject(instance.get());
        instance->state->setListener([](AnimationState* state, EventType type, TrackEntry*, Event*) {
            if (type == EventType_Event) ++static_cast<S374Instance*>(state->getRendererObject())->callback_events;
        });
        instance->skeleton->updateWorldTransform(Physics_None);
        if (!finite_pose(*instance->skeleton)) return 3;
        *out = instance.release();
        return 0;
    } catch (const std::bad_alloc&) { std::abort(); }
      catch (...) { return 3; }
}
extern "C" void s374_instance_destroy(S374Instance* p) noexcept { delete p; }
extern "C" int s374_play(S374Instance* p, uint32_t track, const char* name, S374Mask* mask, float mix, int loop, int additive) noexcept {
    auto* animation = mask ? mask->animation.get() : p->asset->data->findAnimation(name);
    if (!animation) return 1;
    if (additive) {
        auto& timelines = animation->getTimelines();
        for (size_t i = 0; i < timelines.size(); ++i) {
            if (!timelines[i]->getRTTI().isExactly(RotateTimeline::rtti) &&
                !timelines[i]->getRTTI().isExactly(TranslateTimeline::rtti)) return 2;
        }
    }
    try {
        auto* previous = p->state->getCurrent(track);
        if (previous) p->state_data->setMix(previous->getAnimation(), animation, mix);
        auto* entry = p->state->setAnimation(track, animation, loop != 0);
        entry->setMixBlend(additive ? MixBlend_Add : MixBlend_Replace);
        return 0;
    } catch (const std::bad_alloc&) { std::abort(); }
      catch (...) { return 3; }
}
extern "C" int s374_additive(S374Asset* p, const char* name, S374Mask* mask) noexcept {
    auto* animation = mask ? mask->animation.get() : p->data->findAnimation(name);
    if (!animation) return 1;
    auto& timelines = animation->getTimelines();
    for (size_t i = 0; i < timelines.size(); ++i) {
        auto* t = timelines[i];
        if (!t->getRTTI().isExactly(RotateTimeline::rtti) &&
            !t->getRTTI().isExactly(TranslateTimeline::rtti)) return 2;
    }
    return 0;
}
extern "C" int s374_tag(S374Instance* p, uint32_t track, uint64_t id, float alpha) noexcept {
    static_assert(sizeof(uintptr_t) == sizeof(uint64_t));
    auto* entry = p->state->getCurrent(track);
    if (!entry) return 1;
    entry->setRendererObject(reinterpret_cast<void*>(static_cast<uintptr_t>(id)));
    entry->setTimeScale(0);
    entry->setTrackEnd(std::numeric_limits<float>::max());
    entry->setAlpha(alpha);
    return 0;
}
extern "C" uint32_t s374_entries(S374Instance* p, uint64_t* ids) noexcept {
    uint32_t count = 0;
    for (uint32_t track = 0; track < 4; ++track) {
        for (auto* e = p->state->getCurrent(track); e; e = e->getMixingFrom()) {
            if (count == 16) return 17;
            ids[count++] = static_cast<uint64_t>(reinterpret_cast<uintptr_t>(e->getRendererObject()));
        }
    }
    return count;
}
extern "C" int s374_prepare_sample(S374Instance* p, const S374Sample* samples, uint32_t count) noexcept {
        if (count > 16) return 4;
        TrackEntry* entries[16];
        const S374Sample* bound[16];
        uint32_t live = 0;
        // Rust owns integer time. Only bounded local phase/mix values reach
        // official TrackEntry; no delta accumulation or second pose mixer.
        for (uint32_t track = 0; track < 4; ++track) {
            for (auto* e = p->state->getCurrent(track); e; e = e->getMixingFrom()) {
                uint64_t id = static_cast<uint64_t>(reinterpret_cast<uintptr_t>(e->getRendererObject()));
                const S374Sample* sample = nullptr;
                for (uint32_t i = 0; i < count; ++i) if (samples[i].id == id) sample = &samples[i];
                if (!sample || live == 16) return 4;
                entries[live] = e; bound[live++] = sample;
            }
        }
        // Bind the whole array before any assignment. These values must already
        // be current when setCurrent snapshots the interrupted mix percentage.
        for (uint32_t i = 0; i < live; ++i) {
            entries[i]->setTrackTime(bound[i]->phase);
            entries[i]->setMixTime(bound[i]->mix);
        }
        return 0;
}
extern "C" int s374_sample(S374Instance* p, const S374Sample* samples, uint32_t count) noexcept {
    try {
        int status = s374_prepare_sample(p,samples,count);
        if (status) return status;
        p->callback_events = 0;
        p->state->update(0);
        // Absolute samples must start from a stable reference. In particular,
        // additive properties absent from lower tracks must not feed their last
        // committed value back into the next sample. Official state still owns
        // every timeline/hold/mix application and the full ordered layer stack.
        p->skeleton->setToSetupPose();
        p->state->apply(*p->skeleton);
        p->skeleton->updateWorldTransform(Physics_None);
        return finite_pose(*p->skeleton) ? 0 : 3;
    } catch (const std::bad_alloc&) { std::abort(); }
      catch (...) { return 3; }
}
extern "C" int s374_clear(S374Instance* p, uint32_t track) noexcept {
    try {
        p->state->clearTrack(track);
        p->skeleton->setToSetupPose();
        p->state->apply(*p->skeleton);
        p->skeleton->updateWorldTransform(Physics_None);
        return finite_pose(*p->skeleton) ? 0 : 3;
    } catch (const std::bad_alloc&) { std::abort(); }
      catch (...) { return 3; }
}
extern "C" int s374_empty(S374Instance* p, uint32_t track, float mix) noexcept {
    try { p->state->setEmptyAnimation(track, mix); return 0; }
    catch (const std::bad_alloc&) { std::abort(); }
    catch (...) { return 3; }
}
extern "C" int s374_step(S374Instance* p, float dt) noexcept {
    try {
        p->callback_events = 0;
        p->state->update(dt);
        p->state->apply(*p->skeleton);
        p->skeleton->updateWorldTransform(Physics_None);
        if (!finite_pose(*p->skeleton)) return 3;
        return 0;
    } catch (const std::bad_alloc&) { std::abort(); }
      catch (...) { return 3; }
}
extern "C" int s374_bone(S374Instance* p, const char* name, S374Pose* out) noexcept {
    auto* bone = p->skeleton->findBone(name);
    if (!bone) return 1;
    *out = {bone->getWorldX(), bone->getWorldY(), bone->getWorldRotationX()};
    return 0;
}
extern "C" int s374_bone_transform(S374Instance* p, uint32_t index, S374BoneTransform* out) noexcept {
    auto& bones = p->skeleton->getBones();
    if (index >= bones.size()) return 1;
    auto& b = *bones[index];
    *out = {b.getWorldX(),b.getWorldY(),b.getA(),b.getB(),b.getC(),b.getD()};
    return 0;
}
extern "C" int s374_vertices(S374Instance* p, const char* name, float* out, uint32_t capacity) noexcept {
    auto* slot = p->skeleton->findSlot(name);
    if (!slot || !slot->getAttachment() || !slot->getAttachment()->getRTTI().instanceOf(VertexAttachment::rtti)) return -1;
    auto* attachment = static_cast<VertexAttachment*>(slot->getAttachment());
    auto count = attachment->getWorldVerticesLength();
    if (count > capacity) return -2;
    attachment->computeWorldVertices(*slot, 0, count, out, 0, 2);
    return int(count);
}
extern "C" float s374_clock(S374Instance* p, uint32_t track) noexcept {
    auto* current = p->state->getCurrent(track);
    return current ? current->getTrackTime() : -1;
}
extern "C" float s374_local_clock(S374Instance* p, uint32_t track) noexcept {
    auto* current = p->state->getCurrent(track);
    return current ? current->getAnimationTime() : -1;
}
extern "C" uint32_t s374_callback_events(S374Instance* p) noexcept { return p->callback_events; }
extern "C" int s374_mask_used(S374Instance* p, S374Mask* mask) noexcept {
    for (uint32_t track = 0; track < 8; ++track)
        for (auto* e = p->state->getCurrent(track); e; e = e->getMixingFrom())
            if (e->getAnimation() == mask->animation.get()) return 1;
    return 0;
}
extern "C" uint64_t s374_live_allocations() noexcept { return live_allocations.load(); }

extern "C" int s374_skin(S374Instance* p, const char* name) noexcept {
    auto* skin = p->asset->data->findSkin(name);
    if (!skin) return 1;
    try {
        p->skeleton->setSkin(skin);
        // Clear old skin deform/attachment state before reapplying this pose.
        p->skeleton->setSlotsToSetupPose();
        p->state->apply(*p->skeleton);
        p->skeleton->updateWorldTransform(Physics_None);
        if (!finite_pose(*p->skeleton)) return 3;
        return 0;
    } catch (const std::bad_alloc&) { std::abort(); }
      catch (...) { return 3; }
}

namespace {
struct GeometryInputWriter {
    uint8_t* out;
    size_t capacity, size = 0;
    bool valid = true;
    const uint8_t* expected = nullptr;
    bool matches = true;
    bool has_values() const { return out || expected; }
    void bytes(const void* value, size_t count) {
        if (count > capacity - size) { valid = false; return; }
        if (out && valid && count) std::memcpy(out + size, value, count);
        if (expected && matches && count && std::memcmp(expected + size, value, count)) matches = false;
        size += count;
    }
    template<class T> void value(T v) { bytes(&v, sizeof(v)); }
    void color(const Color& c) { value(c.r); value(c.g); value(c.b); value(c.a); }
};
}
static int geometry_input(S374Instance* p, float ppu, GeometryInputWriter& w) noexcept {
    if (!std::isfinite(ppu) || ppu <= 0) return 4;
    w.value(ppu);
    auto& skeleton = *p->skeleton;
    w.color(skeleton.getColor());
    auto& bones = skeleton.getBones();
    w.value(uint32_t(bones.size()));
    // Sizing needs the fixed byte count, not a second read of every matrix.
    constexpr size_t bone_input_bytes = 6 * sizeof(float) + sizeof(uint8_t);
    if (bones.size() > w.capacity / bone_input_bytes) return 4;
    if (!w.has_values()) w.bytes(nullptr, bones.size() * bone_input_bytes);
    for (size_t i = 0; w.has_values() && i < bones.size() && w.valid && w.matches; ++i) {
        auto& b = *bones[i];
        w.value(b.getWorldX()); w.value(b.getWorldY());
        w.value(b.getA()); w.value(b.getB()); w.value(b.getC()); w.value(b.getD());
        w.value(uint8_t(b.isActive()));
    }
    auto& order = skeleton.getDrawOrder();
    w.value(uint32_t(order.size()));
    for (size_t i = 0; i < order.size() && w.valid && w.matches; ++i) {
        auto& slot = *order[i];
        auto* attachment = slot.getAttachment();
        // Sequences mutate shared attachment UV/offset state during extraction.
        // Keep their original path rather than assuming immutable asset inputs.
        if (attachment && attachment->getRTTI().isExactly(RegionAttachment::rtti)
            && static_cast<RegionAttachment*>(attachment)->getSequence()) return 2;
        if (attachment && attachment->getRTTI().isExactly(MeshAttachment::rtti)
            && static_cast<MeshAttachment*>(attachment)->getSequence()) return 2;
        if (w.has_values()) {
            w.value(slot.getData().getIndex());
            w.value(reinterpret_cast<uintptr_t>(attachment));
            w.color(slot.getColor());
        } else w.bytes(nullptr, sizeof(int) + sizeof(uintptr_t) + 4 * sizeof(float));
        auto& deform = slot.getDeform();
        w.value(uint32_t(deform.size()));
        if (deform.size() > w.capacity / sizeof(float)) return 4;
        w.bytes(deform.buffer(), deform.size() * sizeof(float));
    }
    if (!w.valid) return 4;
    return 0;
}

extern "C" int s374_geometry_input(S374Instance* p, float ppu,
    uint8_t* out, uint32_t capacity, uint32_t* written) noexcept {
    *written = 0;
    // Even sizing has a strict bound, without a native container allocation.
    GeometryInputWriter w{out, std::min(size_t(capacity), size_t(1024 * 1024))};
    const int status = geometry_input(p, ppu, w);
    if (!status) *written = uint32_t(w.size);
    return status;
}

extern "C" int s374_geometry_input_matches(S374Instance* p, float ppu,
    const uint8_t* expected, uint32_t length) noexcept {
    if (!expected || length > 1024 * 1024) return 4;
    GeometryInputWriter w{nullptr, length};
    w.expected = expected;
    const int status = geometry_input(p, ppu, w);
    // The shared encoder compares every byte; a prefix is never a match.
    return status ? status : (w.matches && w.size == length ? 0 : 1);
}

extern "C" int s374_geometry(S374Instance* p, float ppu,
    S374Vertex* out_vertices, uint32_t vertex_capacity,
    uint32_t* out_indices, uint32_t index_capacity,
    S374Batch* out_batches, uint32_t batch_capacity, S374GeometryCounts* counts) noexcept {
    *counts = {};
    if (!std::isfinite(ppu) || ppu <= 0) return 4;
    if (!finite_pose(*p->skeleton)) return 3;
    try {
        std::vector<S374Vertex> result_vertices;
        std::vector<uint32_t> result_indices;
        std::vector<S374Batch> result_batches;
        result_vertices.reserve(std::min(vertex_capacity, 32768u));
        result_indices.reserve(std::min(index_capacity, 98304u));
        result_batches.reserve(std::min(batch_capacity, 256u));
        std::vector<float> xy;
        std::vector<uint32_t> remap;
        SkeletonClipping clipper;
        auto& order = p->skeleton->getDrawOrder();
        size_t raw_vertices = 0, raw_indices = 0;
        struct KeyHash {
            size_t operator()(const std::array<uint32_t,4>& key) const {
                size_t h = 0;
                for (auto x : key) h ^= std::hash<uint32_t>{}(x) + size_t(0x9e3779b9) + (h<<6) + (h>>2);
                return h;
            }
        };
        // All scratch storage belongs to this extraction. Reuse node blocks
        // between slots/batches, then release everything on return or error.
        std::pmr::unsynchronized_pool_resource vertex_nodes;
        std::pmr::unordered_map<std::array<uint32_t,4>,uint32_t,KeyHash> attachment_vertices{&vertex_nodes};
        attachment_vertices.reserve(std::min(vertex_capacity, 8192u));
        auto append = [&](const float* xy, const float* uv, size_t vertex_count,
                          const unsigned short* indices, size_t index_count,
                          AtlasRegion* region, BlendMode blend, const Color& color) {
            if (result_indices.size() + index_count > std::min(index_capacity, 98304u)) return false;
            remap.resize(vertex_count);
            auto first = static_cast<uint32_t>(result_indices.size());
            for (size_t i = 0; i < vertex_count; ++i) {
                S374Vertex v = {xy[i*2]/ppu, xy[i*2+1]/ppu, uv[i*2], uv[i*2+1],color.r,color.g,color.b,color.a};
                if (!std::isfinite(v.x) || !std::isfinite(v.y) || !std::isfinite(v.u) || !std::isfinite(v.v) ||
                    !std::isfinite(v.r) || !std::isfinite(v.g) || !std::isfinite(v.b) || !std::isfinite(v.a)) return false;
                std::array<uint32_t,4> key;
                const float values[] = {v.x,v.y,v.u,v.v};
                std::memcpy(key.data(),values,sizeof(values));
                auto inserted = attachment_vertices.try_emplace(key, static_cast<uint32_t>(result_vertices.size()));
                remap[i] = inserted.first->second;
                if (inserted.second) {
                    if (result_vertices.size() >= std::min(vertex_capacity,32768u)) return false;
                    result_vertices.push_back(v);
                }
            }
            for (size_t i = 0; i < index_count; ++i) {
                if (indices[i] >= vertex_count) return false;
                result_indices.push_back(remap[indices[i]]);
            }
            auto page = static_cast<uint32_t>(region->page->index);
            auto mode = static_cast<uint32_t>(blend);
            // Merge only adjacent segments; never sort transparent draws by texture.
            if (!result_batches.empty() && result_batches.back().page == page && result_batches.back().blend == mode) {
                result_batches.back().index_count += static_cast<uint32_t>(index_count);
            } else {
                if (result_batches.size() >= batch_capacity) return false;
                result_batches.push_back({page,mode,first,static_cast<uint32_t>(index_count)});
            }
            return true;
        };
        for (size_t si = 0; si < order.size(); ++si) {
            Slot& slot = *order[si];
            attachment_vertices.clear(); // Never merge vertices across slot tint/material.
            auto* attachment = slot.getAttachment();
            if (!attachment || !slot.getBone().isActive()) {clipper.clipEnd(slot); continue;}
            if (attachment->getRTTI().isExactly(PathAttachment::rtti)) {clipper.clipEnd(slot); continue;}
            if (attachment->getRTTI().isExactly(ClippingAttachment::rtti)) {
                auto* clip = static_cast<ClippingAttachment*>(attachment);
                if (clipper.isClipping() || clip->getWorldVerticesLength() < 6 || clip->getWorldVerticesLength() > 32) return 4;
                float polygon[32];
                clip->computeWorldVertices(slot,0,clip->getWorldVerticesLength(),polygon,0,2);
                for (size_t i = 0; i < clip->getWorldVerticesLength(); ++i) if (!std::isfinite(polygon[i])) return 3;
                clipper.clipStart(slot, clip);
                clipper.clipEnd(slot);
                continue;
            }
            xy.clear();
            float* uvs = nullptr;
            unsigned short* triangles = nullptr;
            size_t triangle_count = 0;
            unsigned short quad[] = {0,1,2,2,3,0};
            TextureRegion* texture_region = nullptr;
            Color* attachment_color = nullptr;
            if (attachment->getRTTI().isExactly(RegionAttachment::rtti)) {
                auto* region = static_cast<RegionAttachment*>(attachment);
                xy.resize(8); region->computeWorldVertices(slot,xy.data(),0,2);
                uvs = region->getUVs().buffer(); triangles = quad; triangle_count = 6;
                texture_region = region->getRegion(); attachment_color = &region->getColor();
            } else if (attachment->getRTTI().isExactly(MeshAttachment::rtti)) {
                auto* mesh = static_cast<MeshAttachment*>(attachment);
                if (mesh->getWorldVerticesLength() > 16384 || mesh->getTriangles().size() > 49152) return 4;
                xy.resize(mesh->getWorldVerticesLength());
                mesh->computeWorldVertices(slot,0,static_cast<int>(xy.size()),xy.data(),0,2);
                if (mesh->getUVs().size() != xy.size()) return 4;
                uvs = mesh->getUVs().buffer(); triangles = mesh->getTriangles().buffer(); triangle_count = mesh->getTriangles().size();
                texture_region = mesh->getRegion(); attachment_color = &mesh->getColor();
            } else return 2;
            raw_vertices += xy.size()/2; raw_indices += triangle_count;
            for (float value : xy) if (!std::isfinite(value)) return 3;
            if (raw_vertices > 8192 || raw_indices > 49152 || triangle_count%3 || !texture_region) return 4;
            auto* region = static_cast<AtlasRegion*>(texture_region);
            auto& sc = p->skeleton->getColor(); auto& sl = slot.getColor(); auto& ac = *attachment_color;
            Color color(sc.r*sl.r*ac.r,sc.g*sl.g*ac.g,sc.b*sl.b*ac.b,sc.a*sl.a*ac.a);
            auto blend = slot.getData().getBlendMode();
            if (blend < BlendMode_Normal || blend > BlendMode_Screen) return 2;
            if (clipper.isClipping()) {
                // Bound BEFORE every native call. A <=16-vertex clip has <=14
                // convex pieces: each input triangle can produce <=266 vertices
                // and <=714 indices. A batch of 64 is therefore <=17024 vertices
                // and <=45696 indices, below uint16 and the frame output limits.
                // Official iteration and append keep the original triangle order.
                constexpr size_t batch_triangles = 64;
                static_assert(batch_triangles * 266 < 65536);
                static_assert(batch_triangles * 266 <= 32768);
                static_assert(batch_triangles * 714 <= 98304);
                for (size_t ti = 0; ti < triangle_count; ti += batch_triangles * 3) {
                    const size_t count = std::min(batch_triangles * 3, triangle_count - ti);
                    for (size_t k = 0; k < count; ++k) if (triangles[ti+k] >= xy.size()/2) return 4;
                    clipper.clipTriangles(xy.data(),triangles+ti,count,uvs,2);
                    auto& cv = clipper.getClippedVertices(); auto& cu = clipper.getClippedUVs(); auto& ct = clipper.getClippedTriangles();
                    if (cv.size()/2 > count/3*266 || ct.size() > count/3*714 || cv.size()!=cu.size()) return 4;
                    if (!ct.size()) continue;
                    if (!append(cv.buffer(),cu.buffer(),cv.size()/2,ct.buffer(),ct.size(),region,blend,color)) return 4;
                }
            } else if (!append(xy.data(),uvs,xy.size()/2,triangles,triangle_count,region,blend,color)) return 4;
            clipper.clipEnd(slot);
        }
        clipper.clipEnd();
        // Transactional copy: no partial geometry escapes on any earlier error.
        std::copy(result_vertices.begin(),result_vertices.end(),out_vertices);
        std::copy(result_indices.begin(),result_indices.end(),out_indices);
        std::copy(result_batches.begin(),result_batches.end(),out_batches);
        *counts = {static_cast<uint32_t>(result_vertices.size()),static_cast<uint32_t>(result_indices.size()),static_cast<uint32_t>(result_batches.size())};
        return 0;
    } catch (const std::bad_alloc&) { std::abort(); }
      catch (...) { return 3; }
}
