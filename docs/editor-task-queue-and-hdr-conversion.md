# Environment authoring: sky, conversion, and a task queue

**Status:** planned, not started. Written 2026-08-20, reordered the same day.

## Read this first: the order changed

This began as "convert a downloaded HDR panorama into something usable", with a
task queue underneath it so the editor would not freeze. Both are still worth
building. Neither should be built first.

Surveying what Bevy already offers turned up two things the editor does not expose,
and one of them makes the conversion pipeline optional for a large class of scenes:

1. **`bevy_pbr::atmosphere` — a procedural sky.** Hillaire 2020 atmospheric
   scattering: add `Atmosphere` to a 3D camera and get a physically-based sky with
   dynamic time of day, driven by the scene's own directional lights, plus
   scattering in front of distant geometry. LUT-based and "fairly cheap" per its
   own documentation. `bevy_pbr` is already in our feature set.

   For an outdoor scene this is *better* than a downloaded panorama, not merely
   cheaper: the sun matches the light that casts the shadows, which a static image
   can never do. It also pairs naturally with terrain, since it handles aerial
   perspective over distance.

2. **`Skybox::rotation`** — a `Quat` on the component, unexposed. This is not
   polish. A converted panorama arrives in whatever orientation it was shot, and
   aiming the sun *is* rotation. Shipping conversion without it delivers a sky the
   author cannot point.

So the original framing — "an author cannot use a downloaded sky" — was answering
the wrong question first. For many scenes the better answer is "you do not need
one". Conversion still matters for authors who bring a specific environment, and
for IBL, but it is no longer the only path to a sky.

**Revised order:**

| Step | Why it comes first |
| --- | --- |
| 0a. Expose `Skybox::rotation` | Trivial; conversion is incomplete without it, and it improves the two shipped maps today |
| 0b. Atmosphere spike, verified on Quest | May remove the need for conversion in most outdoor scenes. Cheap to try, and **unverified on Adreno** — the `bevy_hanabi` lesson says check before committing |
| A. Task queue | Independently useful regardless of what follows |
| B. Equirect → cubemap | Only for authors bringing their own environment |
| C. IBL prefilter | Real graphics work; own pass |

Two caveats on the atmosphere before leaning on it: upstream states it is untested
with volumetric fog, and nobody has run it on an Adreno GPU. That is a device check
before commitment, not after.

Also cheap and adjacent: **fog is linear-only** here
(`XrdsSceneFogEnvironment` is colour/start/end), while Bevy's `FogFalloff` offers
exponential, squared-exponential and atmospheric. The last pairs with step 0b.

---

The original plan follows, still accurate for steps A–C.

## Why

An author who downloads an environment — the ordinary way to get one, e.g. a
stitched HDR panorama from ambientCG — cannot use it. The SDK ships two pre-baked
cubemaps in `assets/environment_maps/` and offers no way to make more.

The obstacle is a format one, and it is absolute rather than a matter of tagging:
Bevy's `Skybox` and `EnvironmentMapLight` both require a **cube** texture, while a
downloaded panorama is a single **equirectangular** 2-D image. Radiance `.hdr` has
no cubemap concept at all, so no import-time classification can make one work. The
missing piece is a conversion, and it does not exist anywhere in the tree.

This surfaced while adding the Skybox section to the editor
(`docs/small-phases-plan.md`, S6-adjacent work): asset-kind detection was fixed to
read the KTX2 header — `faceCount == 6` means cubemap — which correctly identifies
the SDK's own maps, and equally correctly refuses to pretend a panorama is one.

## Part 1 — a general task queue

Conversion is slow enough to be visible (seconds), and it must not freeze the
editor. Rather than a third bespoke background job, this generalises what already
exists.

### What is there now

`ExportJob` and `ApkExportJob` in `editor_state.rs`: a `std::thread::spawn` plus
`Arc<Mutex<Option<Result<String, String>>>>`, polled each frame with `try_lock`,
each with its own snapshot boolean (`is_exporting`, `is_exporting_apk`) and its own
UI. The pattern works; it just does not scale to a third caller, and neither job
can report progress — only "running" or "done".

### Model

```rust
struct EditorTask {
    id: u64,
    label: String,          // "Converting sky.hdr"
    state: TaskState,       // Queued | Running | Done | Failed | Cancelled
    progress: Option<f32>,  // None = indeterminate, else 0.0..=1.0
    detail: Option<String>, // current step, or the error when Failed
    cancel: Arc<AtomicBool>,// checked by the worker at its own granularity
}
```

- **A queue, not just a set.** Conversions are CPU- and memory-heavy; running six
  at once because someone dropped a folder in would be worse than running them in
  order. A small concurrency cap, default 1 for conversions.
- **Progress is `Option`.** An APK build cannot honestly report a fraction, and a
  fake progress bar that jumps to 90% and sits there is worse than a spinner.
  Conversion *can* report real progress (faces, then mip levels), so the model must
  express both rather than forcing one.
- **Finished tasks linger** until dismissed, so a failure that lands while the
  author is looking elsewhere is still there when they look back. This is the
  common failure of status-bar-only reporting.

### Scope: author-initiated jobs only

**This queue is for visible, author-initiated work.** Slow, few, and something the
author is waiting on: conversion, export, baking. Anything high-frequency or
system-initiated must not report here.

Written down because the boundary is not obvious and the wrong side of it is
ruinous. Terrain is the case that makes it concrete — it is on the Gap Analysis,
and it brings two kinds of work that look alike and are not:

| | Author-initiated | System-initiated |
| --- | --- | --- |
| Terrain | import a heightmap, bake chunks, generate collision | stream chunks as the player moves |
| LOD | bake the LOD chain | select a level per frame |
| Environment | convert a panorama | — |

The right-hand column would break this design in three specific ways: "finished
tasks linger until dismissed" buries a real failure under thousands of successes;
FIFO is wrong when the chunk ahead of you matters more than the one behind; and
there is no cancellation, which is streaming's most common event.

A shared thread pool underneath is fine. A shared queue *with a UI* is not. If
streaming ever needs scheduling, it needs its own — priority-ordered, cancellable,
and silent.

### Cancellation

In the model from the start, not added later. Immediately useful — an author who
starts converting a 16K panorama and realises it is the wrong file should be able
to stop it — and it is the single thing a future scheduling mechanism would most
need if any of this is ever shared. Retrofitting cancellation through a thread
that only reports a final `Result` means rewriting every job.

### UI

A task strip in the status bar showing the active task and a count, expanding to a
list. Failures stay until dismissed and carry their error text.

**Existing jobs migrate onto it**, so APK export and app export report through the
same surface. That is the test of whether the abstraction is right: if they do not
fit, it is the wrong shape.

## Part 2 — the conversion

### Phase order matters here

Equirect → cubemap and IBL prefiltering are different jobs of very different size,
and the plan should not pretend otherwise.

**Phase B — skybox cubemap.** Project the panorama onto six faces. One render pass
per face, or a straightforward CPU loop; no filtering. Sufficient for `Skybox`,
which is what an author sees first and asks for by name.

**Phase C — IBL prefiltering.** `EnvironmentMapLight` needs a diffuse irradiance
map and a specular chain prefiltered by roughness (the shipped `specular.ktx2` has
11 mip levels, `diffuse.ktx2` has 1). This is GGX importance sampling — real
graphics work, and worth its own pass rather than being smuggled into Phase B.

Until Phase C, a converted panorama lights nothing; it is a backdrop. That must be
said in the UI, or it becomes another silent half-feature.

### How to convert — decide before building

1. **In-process with Bevy's renderer.** No external dependency, works on every
   platform the editor runs on, and the GPU is already initialised. Costs a
   render-to-cubemap path the editor does not currently have.
2. **Khronos `ktx` CLI.** `ktx create --cubemap` and friends do this properly and
   are battle-tested. But it is a tool the author must install, and the editor
   would need the same prerequisite-check dance as the APK export — which is
   precisely the friction this feature exists to remove.
3. **A Rust crate.** Needs a survey; none is known-good here yet. Unverified.

Leaning (1) for the skybox phase, since the editor already owns a GPU context and a
tool dependency defeats the purpose. Phase C may still be easier via (2) offline.

**Do not start Phase B before this is settled.** It is the whole cost of the
feature.

## Phasing

| Phase | Scope | Notes |
| --- | --- | --- |
| 0a | Expose `Skybox::rotation` | Trivial. Do first — see the reordering at the top |
| 0b | ~~Procedural atmosphere, verified on Quest~~ | **Done, and measured — see below** |
| A | ~~Task queue + UI, existing export jobs migrated~~ | **Done — see below** |
| B | Equirect → cubemap for skybox | Gated on the conversion-method decision above |
| C | IBL prefilter (diffuse + specular chain) | Real graphics work; own pass |

Phase A is worth doing regardless of whether B and C ever happen: it removes two
bespoke job implementations and gives long operations somewhere honest to report.

**B and C may never be needed.** If the atmosphere covers outdoor scenes and the
two shipped cubemaps cover indoor ones, conversion serves only the author who wants
one *specific* environment. That is a real use, and a much weaker reason to build a
GPU cubemap pipeline than "you cannot have a sky at all" was. Re-judge after 0b
rather than treating this plan as committed.

## 0b result: works on Adreno, costs more than a whole frame budget

Measured on a Quest 3, 2026-08-20. Two APKs from the same generated scene — a
200×200 ground plane, one shadow-casting directional light, one cube — differing
only in whether `atmosphere` was set. Frame time taken from the
`[XR-DIAG] update_view_proj#N` counter, which logs every 90 frames, so consecutive
timestamps give the period of 90 frames directly.

| | Frame time | Rate |
| --- | --- | --- |
| Without atmosphere | ~13.0 ms | ~77 fps (at display cap) |
| With atmosphere | ~31.3 ms | ~32 fps |
| **Cost** | **+18.3 ms/frame** | |

The 72 Hz budget is 13.9 ms. **Atmosphere alone costs about 1.3× the entire frame
budget**, while everything else in the scene fits comfortably inside it. That is not
a tuning gap; it is a different order of magnitude.

**It renders correctly** — sky, horizon, aerial haze and the sun disc all confirmed
by eye on device, with no validation errors and no panics. This is not the
`bevy_hanabi` outcome, where the feature simply produced nothing on Adreno. It
works; it is unaffordable at default settings.

Two things were needed to get it working at all, both worth keeping in mind if this
is revisited:

- `Atmosphere` requires Bevy's `Hdr`, adding a float intermediate render target —
  paid once per eye.
- The `render_sky` pass **samples the depth texture**, and `Camera3d::default`
  creates depth as `RENDER_ATTACHMENT` only. Bevy adds `TEXTURE_BINDING` itself for
  occlusion-culling views and *not* for atmosphere, so the bind group fails
  validation without an explicit fix on our side.

### Decision: ship it as an advanced, opt-in feature

Not removed — it works, and desktop and exported desktop apps have the budget for
it. Not defaulted on — an XR scene that enables it drops to under half the target
frame rate, and a default that is pleasant on desktop and unusable on a headset is
the wrong default for this SDK. Unreal defaults its sky atmosphere on because it
targets hardware where the intermediate is nearly free; we do not have that.

The editor states the measured cost rather than a vague warning, so the choice is
informed at the point it is made.

**Not attempted, and the obvious next question:** `AtmosphereSettings` exposes LUT
sizes and per-ray sample counts, and the defaults are desktop-tuned. Whether 18 ms
can be brought under ~3 ms that way is unknown and would be its own measurement.
Recorded as a lead, not a plan.

### Consequence for B and C

The premise that "the atmosphere may make conversion unnecessary for outdoor
scenes" **does not hold for XR**. On a headset an author still needs a cubemap, so
conversion keeps whatever priority it had. On desktop the atmosphere is a genuine
alternative.

## Phase A result: the migration paid for itself before conversion exists

`apps/xrds-editor/src-tauri/src/task_queue.rs`, plus a `TaskStrip` in the
frontend. Both export jobs are now tasks; `ExportJob` and `ApkExportJob` are gone,
along with the two `Arc<Mutex<Option<Result<..>>>>` pairs and — importantly — the
*two separate polling sites* they had grown (`bevy_scene.rs` and
`bevy_bridge.rs` were both watching the desktop export).

The abstraction test in this plan was "if the existing jobs do not fit, it is the
wrong shape". They fit, and both got things they did not have:

- **Cancellation.** `TaskContext::run_child` polls `try_wait` rather than
  blocking on `wait`, so a cancel actually kills the compiler. An APK build runs
  for minutes and previously could not be abandoned — the dialog's "Cancel" was
  greyed out while building, so realising you had picked the wrong folder meant
  waiting it out.
- **Streamed output for the desktop export**, which previously wrote to the
  editor's own stdout, i.e. nowhere an author could read it.

Three things worth recording:

**Lanes, not a global cap.** `Build` is capped at 1 because cargo locks the target
directory: a desktop export and an APK build started together did not run in
parallel before, they ran one at a time while both claimed to be running. Making
the second one visibly `Queued` describes what was already happening.

**`Cancelling` is a state.** A cargo build does not stop when asked, and reporting
"Cancelled" while it still holds the target-dir lock would make the next queued
build look stuck for no visible reason. Relatedly, a killed build exits non-zero;
the queue reports that as cancelled rather than failed, because telling an author
their own stop request "failed" is a lie about their own action.

**It fixed the reported dialog bug as a side effect.** "After clicking Export APK
the popup disappears" was `phase === "building" && !snapshot.is_exporting_apk`:
`phase` was set synchronously, but the next snapshot still had the flag `false`
because the backend had not drained the command yet. *A boolean going low cannot
distinguish "not started" from "finished".* The dialog now watches its own task by
monotonic id, which answers the question that was actually being asked.

Also removed: `EditorState::is_selected`, which evaluated `contains(id)`, discarded
it, and returned `false`. Dead — every caller used `selection.contains` directly —
but a trap with a correct-looking name.

### What the first real run caught: cancellation wedged the lane

Verified in the editor — export and cancel both work for the desktop app and the
APK. Getting there took three fixes, and the first is the one worth remembering.

**`kill()` does not kill a process tree.** Killing cargo leaves its rustc children
alive, and they inherited the stdout/stderr pipe handles. `run_child` joined its
reader threads unconditionally, so the readers blocked on a pipe a surviving
grandchild still held; `run_child` never returned, the task sat in `Cancelling`
forever, and — because a task in `Cancelling` still occupies its lane — every later
build was refused with "Busy". The symptom was an APK export stuck at "Starting…"
*after an application export had been cancelled*, which points at the wrong feature
entirely.

The drain is now bounded (2s), after which the readers are left detached; they hold
an `Arc`, so late lines still land in the log. The test that guards this asserts the
invariant rather than the mechanism: **a cancelled build must release its lane.**
Killing the whole process tree would be better still, and is platform-specific work
nobody needs yet — a wedged lane was the actual damage.

**A refusal behind a modal is a hang.** `ExportApk` has five guards that reject
before any task exists, each setting a status toast — which renders *behind* the
modal dialog. The dialog set `phase = "building"` optimistically and waited forever
for a task that was never created. It now shows the refusal and returns to "ready",
with a frame-count backstop for any future guard that forgets to set a message.
Generalisable: **optimistic UI needs a path for "the backend said no".**

**A guard mirrored in the UI drifts from the one that enforces it.** The dialog's
`canExport` checked `!is_dirty` but not `has_save_path`, while the backend checks
both, so a never-saved scene left the button enabled and the export silently
refused.

## Done when

An author drops a downloaded panorama into the editor, watches a progress entry
finish without the editor freezing, and picks the result in the Skybox section —
and, after Phase C, in IBL too.
