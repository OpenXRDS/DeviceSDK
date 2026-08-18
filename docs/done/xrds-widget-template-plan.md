# Unified panel templates — widget identity and trigger binding

Plan doc, same role as `docs/done/xrds-track-model-plan.md`: decisions and the reasoning
behind them, written before the code so the *why* survives.

## 1. Scope

**In scope now**

1. One unified panel-template model. HUD and WorldPanel become the same thing;
   the only difference is *attachment*.
2. Every panel is a named template, instanced by reference.
3. Elements get a stable identity, scoped to their template.
4. Elements can **fire** triggers, which fire Tracks like any other binding.
5. A panel-authoring workspace in the Sequencer's design language, with a
   template library.

**Phase B, designed here but stoppable before it**

6. Elements as **action targets** — "some elements display something as a
   consequence of the trigger". Discrete, event-driven writes only.

**Explicitly out of scope**

- **Live data binding** (an element reflecting a value re-asserted every frame).
  A Track is authored keys at fixed times; per-frame updates from a live source
  are not choreography, and expressing them as keys means either thousands of
  keys or a zero-duration looping Track — the degenerate case
  `docs/done/xrds-track-model-plan.md` already documents as pathological. Deferred with
  the streaming scenario. Recorded because the shape is known: the read-side
  vocabulary already exists as `XrdsObservable` (which
  `XrdsThresholdWatcher` reads every frame), so a binding is that same read
  *minus the threshold* — an extension, not new scripting. It would also create
  a conflict class `XrdsTrackAssetLocks` does not cover: a bound element
  re-asserts every frame, so a Track key writing it is overwritten on the next.
- **Media surfaces / streaming.** `xrds-net` is already a runtime dependency
  (ws/wss, quic, mqtt) and `xrds-media` has a `video` module, but `xrds-media`
  is **not a runtime dependency at all**, and there is no authorable media
  element. When it does land: the Track fires a *one-shot start* and frame
  delivery is a runtime system, never Track keys. And unlike a local animation,
  a stream has connect latency and can fail mid-play, so it needs
  `MediaReady`/`MediaFailed`/`MediaEnded` trigger kinds (precedent:
  `AnimationComplete`) or "start the video, *then* show the caption" is
  unauthorable and a dead stream looks identical to a working one.

## 2. Why unify — the two sides are complementary

Each side already has exactly what the other lacks:

| | HUD (`XrdsHudItemDef`) | World (`XrdsSceneWorldWidget`) |
|---|---|---|
| Element kinds | 1 — text only | 5 — Label, Button, Image, Slider, Toggle |
| Element identity | **has** `id` + `name` | **none** |
| Addressable at runtime | **yes**, `set_hud_item(name, …)` | no |
| Template registry + reuse | **has**, plus a library panel | none |
| Canvas editor | `HudCanvasOverlay` | `WorldPanelCanvasOverlay` |

So unification is not a merge of two similar things — it is taking the world
side's **element vocabulary** and the HUD side's **identity and reuse model**.
Neither side loses anything, and both gain: HUD gets buttons/sliders/toggles/
images, world panels get templates and name-addressing.

This is also what makes "every panel is a template" load-bearing rather than
cosmetic: the HUD already works that way, so it is the model both sides
converge *on*, not a new invention imposed on both.

**Attachment is the only remaining difference**, which is the whole claim being
tested: HUD is attached to the player's camera, a world panel sits in the
scene.

## 3. Target schema

```rust
/// Document-level registry, mirroring `XrdsSceneDocument::tracks`.
pub struct XrdsPanelTemplate {
    pub id: PanelTemplateId,
    pub name: String,
    /// Canvas size in metres.
    pub size: [f32; 2],
    pub background: XrdsPanelBackground,   // colour, corner_radius, opacity
    pub layout: XrdsSceneWorldLayout,
    pub elements: Vec<XrdsPanelElement>,
}

pub struct XrdsPanelElement {
    /// Stable, unique within its template. **The addressing key.**
    pub name: String,
    /// Canvas-local position: X right, Y up (metres).
    pub local_position: [f32; 2],
    pub kind: XrdsPanelElementKind,        // Label | Button | Image | Slider | Toggle
    /// Triggers this element fires. Empty for non-interactive kinds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<XrdsTriggerBinding>,
}
```

### Attachment

```rust
// Camera-attached. `depth` lives HERE, not on the template — see below.
XrdsScenePlayerAnchor { panel_template_id: Option<PanelTemplateId>, panel_depth: f32, … }

// Scene-placed. Positioned by the node's own transform.
XrdsSceneNodePayload::Panel { template_id: PanelTemplateId }
```

**`depth` moves off the template.** `XrdsHudTemplate` carries `depth: f32`
today, which quietly prevents instancing one template at two depths. Depth is
an attachment property; the template holds content only. This falls directly
out of "the difference is only attachment" and is the first place that claim
bites.

### Identity is a name, not an index

Reordering elements in the editor's canvas must not silently re-point a
binding or an action target. Names also preserve the contract `set_hud_item`
already has. Uniqueness is enforced per template and diagnosed, not assumed.

## 3b. Naming policy — required, because names are the keys

If a name is the identity, loose naming is a correctness problem, not a style
one. **This is already a live gap for Tracks**, not just a future concern for
elements:

- `CreateTrack` checks uniqueness but never trims or rejects empty, so `""` and
  `"Door "` are both authorable today. `"Door "` and `"Door"` are different keys
  that render identically — an author cannot see why the binding "does nothing".
- `RenameTrack` is sound (it checks uniqueness *and* re-points every binding),
  so the fix is at creation, not rename.
- **Six frontend sentinels would collide**: `__none__`, `__any__`, `__add__`,
  `__clear__`, `__add_asset__`, `__no_texture__`. A Track literally named
  `__none__` reads as "nothing selected" in the Fires picker — it would appear
  to unwire itself.

### The rules

One shared validator, applied to Track names, template names and element names
— not three near-copies that drift.

1. **Trim** leading/trailing whitespace on input. Prevents invisible duplicates.
2. **Reject empty** after trimming (error).
3. **Restrict the character set**, and compose Hangul. Allowed: printable
   ASCII, precomposed Hangul syllables (가–힣), and the standalone
   compatibility jamo on a Korean keyboard (ㄱ–ㆎ). Everything else — accented
   Latin, Han, emoji, control characters — is refused.

   **Korean is why this needs care.** Hangul has two encodings that render
   identically: precomposed syllables (한) and conjoining jamo (ᄒ + ᅡ + ᆫ).
   Restricting characters does *not* remove that — plain English has no
   decomposed forms, so admitting Korean is what introduces it. Handled by
   composing jamo before the character check.

   Done with `unicode-normalization` (NFC), not hand-rolled Hangul arithmetic.
   A hand-written composer was tried first and had a real hole: UAX #15 also
   composes a *precomposed* LV syllable followed by a conjoining trailing
   consonant (하 U+D558 + ᆫ U+11AB → 한), not only sequences starting from
   conjoining jamo. Missing that left such input as two code points, rendering
   as 한 but then refused for containing a conjoining jamo. Cost: **one** new
   lockfile entry — `tinyvec` was already present via Bevy — and names are
   validated when a human types one, so this is not a hot path.
4. **Uniqueness by exact match** after 1–3, within scope: Track and template
   names document-wide, element names within their template.
5. **Warn** on names differing only by case within a scope. Deliberately *not*
   case-insensitive uniqueness: case-folding drags in locale edge cases (Turkish
   dotless i) for a trap a diagnostic catches just as well.
6. **Reserve the `__` prefix**, refused *with a visible notification that says
   why and suggests a replacement* — not a silent rejection. The whole point of
   this policy is that a name which "does nothing" is undebuggable, so failing
   invisibly here would reproduce the very bug being fixed. Cheaper than making
   six pickers collision-proof, and no author wants that prefix anyway.
7. **Do not otherwise restrict characters.** Names are display strings, never
   paths or identifiers, so an identifier-only rule would only annoy authors.
8. **Rename re-points references.** Tracks already do this; elements must too,
   for both bindings and (Phase B) action targets.

Applying this to Tracks is a small independent fix and can land first, the same
way §5b did.

## 4. Element triggers — nearly free, and why

`consume_triggers` needs **no changes at all**. The chain, verified:

- The four widget events already target the element's own entity:
  `XrdsTriggerRef::Entity(self.button_entity)`.
- `XrdsTriggerRef::Entity(e).resolve(index)` returns `Some(e)` — a straight
  pass-through, no id lookup.
- `consume_triggers` then requires `XrdsTriggerBindings` on that entity.

The *only* missing piece is that component. Element entities never receive it,
because it is inserted solely by `tag_trigger_binding_entities`, which walks
`document.nodes` — and elements are not nodes.

So the work is: give elements authored `triggers`, and tag their entities at
spawn, mirroring `tag_trigger_binding_entities` (including its
remove-when-empty behaviour, so toggling the last binding off actually
detaches).

This also explains why `HoverEnter`/`HoverExit` already work while the four
widget kinds never did: those two return `XrdsTriggerRef::Id(self.panel_id)`,
a real document id, so they land on the panel node — which *is* tagged.

Elements do **not** appear in the scene hierarchy. They are addressed as
`(panel instance, element name)`, never as scene nodes.

## 5. The instance hazard that "every panel is a template" introduces

A template instanced three times shares one authored set of element triggers.
A Track fired from such a trigger, with a row hard-targeting `Node(7)`, drives
**the same asset regardless of which instance was pressed**.

This is the bug class that works perfectly while there is one instance and
breaks silently on the second. It needs an author-time diagnostic:

> *This Track is fired from template "MainMenu", which has 3 instances, but
> targets fixed node "door_left". Every instance will drive that same node.*

Severity: warning, not error. It is legitimate when the target really is
global (one shared door, many call buttons).

Note this hazard is about **targeting, not timing**. One press is enough to
show it: press the third-floor button and the ground-floor door opens. The
timing problem is separate, and worse — §5b.

### There is no per-instance mitigation today

An earlier draft of this section said `TriggerSource` rows solve it. **That was
wrong**, and it matters because it made the hazard look handled.

The widget events do not override `XrdsTriggerEvent::source()`, and the trait
defaults it to `target()`:

```rust
fn source(&self) -> XrdsTriggerRef { self.target() }   // trait default
```

For a button press, `target` is `Entity(self.button_entity)` — so `source` *is
the button*. A `TriggerSource` row therefore drives **the button itself**, never
"the door next to the button".

So the model has **no relative addressing**: no way to express "the asset
belonging to my instance". The two things an author can say are "this exact
node" (shared by every instance) or "the element that fired" (the button). The
useful middle — my instance's door — is inexpressible.

This is a real gap, not a diagnostic-able mistake, and it is the thing that
would make templates genuinely reusable for anything that drives scene
geometry. Candidate shapes, none chosen:

- **Per-instance override table** on the panel node: `{ "door": Node(7) }`, with
  rows targeting a named slot the instance fills in. Most explicit; the instance
  says what it is wired to.
- **Relative-to-panel addressing**: `XrdsActionTarget::SiblingOf { … }` or a
  parent-scoped lookup. Cheaper to author, but depends on scene layout matching
  the intent, which is fragile.

Out of scope for this plan; recorded because §5's diagnostic is now honest about
having no fix to suggest, and that is a worse place to leave an author than a
warning with a remedy.

## 5b. The concurrency gap: one Track cannot run per-instance

Three call buttons firing Track `OpenDoor`, pressed at about the same time.
Today each press **kills the previous agent and restarts**, because
`spawn_track_agent_in_world`'s same-Track re-fire rule is keyed on the Track's
**name** alone:

```rust
.filter(|(_, agent)| agent.name == name)   // ← name, nothing else
```

So only one agent ever exists. The last press wins and the first two are cut
off mid-animation. Not a crash or a race — a silent "only the last button did
anything", which is exactly the sort of thing that reads as flaky input.

**The model already had the right machinery; one rule was keyed too coarsely.**
The two mechanisms disagreed about identity:

| | keyed on |
|---|---|
| same-Track restart | Track **name** |
| `XrdsTrackAssetLocks` conflict guard | resolved **entities** |

If those three buttons use `TriggerSource`-relative rows they resolve to three
*different* doors — disjoint lock sets — so the conflict guard would happily
run all three concurrently. Only the name-based restart forbade it.

### Decided and landed: first run has priority

Policy: **a running Track is never preempted except by an explicit stop.**

The fix was to *delete* the same-name special case rather than re-key it. Once
gone, the existing entity-keyed guard answers all three cases with no new
mechanism:

| Situation | Before | Now |
|---|---|---|
| same button pressed twice | restart, first cut off | **refused**, first keeps running |
| 3 buttons → 3 different doors | last wins, 2 cut off | **3 run concurrently** |
| 3 buttons → the same door | last wins | **first wins**, rest refused |

Simpler than the `(name, source)` re-keying first proposed here: that would
have kept a special case for "same source restarts", which contradicts
first-run priority. One uniform rule is easier to explain and was less code.

**Preempting is still possible, but must be explicit** — which is the point of
the wording. `preview_play_track_in_world` stops the current preview before
starting, and that is exactly what makes the editor's ⏮ restart button work;
the expert path has `stop_sequences_on`/`stop_all_sequences`.

**Gap worth naming: there is no *authorable* stop.** `XrdsAction` has no
`StopTrack`, so content cannot halt a Track — only Rust can. A door that opens
on press and should re-open on a second press within its own run is therefore
not authorable today. Not blocking, and deliberately not invented here, but
this policy is what makes it matter.

**This changed behaviour for plain nodes too**, not only elements: two
different nodes firing one Track used to restart each other. Decided
deliberately, not slipped in.

Tests: `re_firing_a_running_track_restarts_it_rather_than_conflicting_with_itself`
was replaced by `..._is_refused_so_the_first_run_keeps_priority`, plus the
disjoint-concurrency case, the shared-asset case, the explicit-stop case, and
`previewing_the_same_track_twice_restarts_it_rather_than_being_refused` —
which guards the ⏮ path, since removing the restart made that stop
load-bearing and there had been **no preview tests at all**. Mutation-verified:
restoring the old restart fails 4 of them.

Sequencing note: this was a **Track-model change, not a widget change**, and
landed independently ahead of this plan. Templates only made it easier to hit,
because N instances naturally share one Track.

## 6. Phase B — elements as action targets

For "some elements display something as a consequence", a Track must be able
to address an element. Today `XrdsActionTarget` only addresses
`XrdsSceneNodeId`.

```rust
XrdsActionTarget::Element { panel: XrdsSceneNodeId, name: String }
```

**`panel` is the instance node, not the template**, and that is the whole
point: a template instanced N times has N sets of live elements, so a template
reference would be ambiguous about which one to write.

Actions needed are small and discrete — `SetElementText`, `SetElementValue`,
`SetElementEnabled`. Each applies instantly, so they fit the Track model with
no scheduler change.

Note this makes element entities participate in `XrdsTrackAssetLocks`, since
locks key on resolved `Entity`. That is correct and wanted: two Tracks writing
the same element should conflict exactly as two Tracks writing the same node
do.

## 6b. Authorable stop — in scope

Motivating case: a start button and a stop button, both in the content. First-run
priority (§5b) makes this necessary rather than nice — without a stop, a running
Track cannot be interrupted from authored content at all, only from Rust
(`stop_sequences_on` / `stop_all_sequences`).

**Recommended shape: a mode on the binding, not a new action.**

```rust
pub enum XrdsTriggerEffect { Fire, Stop }   // on XrdsTriggerBinding
```

A stop button then binds directly to "stop Track X". The alternative — an
`XrdsAction::StopTrack { name }` on a Track row — would force a dummy Track
whose only purpose is to stop another, for the plainest case there is.

**This does not reintroduce what killed `Run`.** `Run` was deleted because
starting chains: depth limits, runaway detection, a Track able to launch a Track
launching a Track. Stopping is monotonic — it only removes running work, so it
cannot recurse or fan out. A Track stopping itself is odd but bounded.

An action-level `StopTrack` may still be wanted later, for stopping something
partway through a choreography. Not needed for the button case, so not now.

### State as visibility, not as conditionals

A single button that toggles start/stop needs to know whether the Track is
running — which means a condition, which is the first step toward the
Blueprint-style branching this whole design exists to avoid.

**Two buttons, and the state is which one is visible.** Start visible / Stop
hidden means "not running"; the reverse means "running". Nothing evaluates a
condition; the scene *is* the state. This is the same discipline as the rest of
the model: closed vocabulary, no branching.

The Track flips them as part of its own choreography — which is Phase B
(`SetElementVisible`), and is the strongest argument for B not being optional.

**The hole, and why it already closes.** On natural completion the Track's last
key flips the buttons back. On an *early* stop the remaining keys never fire, so
the flip-back would be skipped and the UI would sit showing "Stop" for a Track
that is not running.

No new machinery needed: `XrdsSceneNode::triggers` is a `Vec`, and
`consume_triggers` iterates **every** matching binding, not the first. So the
stop button carries two bindings on one `ButtonPress`:

1. `Stop` Track "PlayVideo"
2. `Fire` Track "ResetButtons"  *(flips visibility back)*

They run in authored order, deterministically. "Stop this and start that"
without a conditional, out of parts that already exist.

### What "stop Track X" stops: by resolved assets, not by name

An earlier draft posed this as an open question between "stop every agent of X"
and "stop only my panel instance's agent", and invented a panel-instance scope
to make the second work. Both were wrong turns.

**Stop resolves its assets exactly the way start does.** Start computes the
Track's resolved asset set and refuses if any are held; stop computes the *same*
set and despawns whoever holds them. Same `schedule_track_keys` call, same
resolved-entity set, one code path.

That is right for the same reason §5b was: **name-keyed operations are the wrong
granularity, entity-keyed ones are correct.** Reaching for a name-keyed stop
would have repeated the exact mistake this plan already corrected once.

It also makes the supposed ambiguity vanish rather than needing a new scope:

- Rows target fixed nodes → all instances contend for one asset, so only ever
  one agent exists (§5b case 3). Nothing to disambiguate.
- Rows target `TriggerSource` → each instance drives its own element, genuinely
  disjoint, and asset-keyed stop reaches only the right one.

No panel-instance concept is needed in either case. Not blocking anything.

## 6c. Continuous controls are read, not bound

Found while sanity-checking what element triggers make possible. Two shipped
features interact in a way neither shows on its own:

- `world_ui_slider_system` fires `XrWorldSliderChangeEvent` **every frame the
  value changes during a drag** (its own doc comment says so).
- First-run priority (§5b) refuses a re-fire while a Track still runs.

So a slider bound to a Track yields roughly **one run per drag**, with every
later frame refused and logged as a conflict. Arguably useful debouncing, but
surprising, and the conflict log gets noisy.

**And the Track never receives the slider's value.** `XrdsActionValue::
FromTriggerSource` reads an `XrdsTriggerValue` component off the source entity,
and the slider system never populates one.

So the division is: **discrete controls bind to Tracks; continuous controls get
read.** A throttle or trim wheel belongs to gameplay code via the expert path.
That is not a gap to close so much as the right boundary — a Track is authored
choreography, not a control loop. Making continuous controls *authorable* is the
deferred data-binding subsystem (§1), and it would need the read direction too
(element → value), not only value → element.

Worth a diagnostic eventually: a `SliderChange` binding on an element is almost
always a mistake unless the Track is short and idempotent.

## 7. Editor — a third workspace, same language as the Sequencer

The Sequencer's shell is: named registry on the left, the thing being edited
in the middle, inspector on the right. Panels get the same shell:

- **Left** — template library. `HudLibraryPanel` already does this (list,
  rename, "edit template"); generalize it rather than write a second one.
- **Middle** — 2D canvas. `HudCanvasOverlay` and `WorldPanelCanvasOverlay`
  already exist; they converge into one.
- **Right** — element inspector, including that element's triggers. Reuse the
  `TriggersSection` built for nodes: a binding is a binding, and its
  kind-availability hints (`unavailableReasonFor`) apply unchanged — except
  that on an element, `ButtonPress`/`SliderChange`/… become *available* for the
  first time, which is the visible payoff of this whole plan.

## 8. Migration

**No document migration.** Verified: no `.xrds` or scene `.json` documents are
tracked in the repo — only config files. Same situation as the Track rework, so
again no compatibility shims, deliberately.

**There is real code-level breakage**, unlike the Track rework:
`XrdsHudTemplate`, `XrdsHudItemDef`, `set_hud_item`, `hud_library.rs`'s command
surface, and the HUD DTOs. Eight Rust files reference them. No *example* does,
which bounds the blast radius.

**`set_hud_item(name, …)` should keep working verbatim.** Its contract is
name-addressed, and unification preserves name-addressing — a HUD text item
becomes a `Label` element with the same name. Keeping a public SDK API stable
through this is cheap, and worth it.

## 9. Phasing and checklist

Every phase ends green: `cargo check --workspace --all-targets`, the crate's
own tests, and for A3 also `tsc` + vitest + `vite build`. Phases are
dependency-ordered; within a phase, order is a suggestion.

### A0 — prerequisite (landed)

- [x] **First-run priority** — delete the name-keyed same-Track restart so the
      entity-keyed guard decides. Landed as `41402dd`, ahead of this plan and
      independent of it. See §5b for why templates make it matter.

### A0b — naming policy (§3b) — landed

- [x] One shared validator — `crates/xrds-scene-graph/src/naming.rs`:
      `normalize_authored_name`, `XrdsNameError` (typed, carries `message()`
      *and* `suggestion()`), `RESERVED_NAME_PREFIX`, `name_case_fold`,
      `names_differing_only_by_case`.
- [x] Applied at `CreateTrack` and `RenameTrack` via `reject_bad_name`, which
      refuses **visibly** — `pending_status` gets the reason and a concrete
      alternative, because a silent refusal reproduces the bug being fixed.
- [x] Diagnostics in `track_diagnostics()`, so a document built in Rust or
      hand-edited is reported rather than trusted: surrounding whitespace
      (error), unusable name (error), two Tracks differing only by case
      (warning — legal, just invisible in review).
- [x] Tests: 14 in `tests/naming.rs`, 3 in the editor. Mutation-verified —
      removing the trim fails 6 scene-graph tests and 3 editor tests.
- [x] **Character allowlist + NFC.** `is_allowed_char` (printable ASCII +
      가–힣 + ㄱ–ㆎ) and `compose_hangul` over `unicode-normalization`.
      Restricting the set is what keeps the normalization surface to Hangul —
      but Korean is exactly why NFC is still needed, since plain English has no
      decomposed forms. Tested: decomposed and precomposed Korean canonicalize
      to one key; **precomposed-LV + trailing jamo composes too** (the case a
      hand-rolled composer got wrong); an unpaired conjoining jamo is refused
      with a message pointing at the keyboard forms.

Fixed a live bug: `CreateTrack` accepted `""` and `"Door "`, and `"Door "`
renders identically to `"Door"` while hashing differently.

### A1 — the panel vocabulary, additively — landed

**Sequencing corrected while doing this.** A1 as first written deleted
`XrdsHudTemplate` and retired `XrdsSceneWorldPanel::widgets`, which breaks 8
files across runtime and editor — so it could not end with the workspace green,
contradicting this section's own rule. Since risk #1 is *breaking a working
system*, A1 is now purely **additive**: the new vocabulary lands and is
validated before anything migrates onto it. Deletion moves to A4b, after the
runtime and editor are on the new types. The whole workspace stays green
throughout.

- [x] `src/scene/panel.rs`: `XrdsPanelTemplate`, `XrdsPanelElement`,
      `XrdsPanelTemplateId`, `XrdsPanelBackground`.
- [x] **`kind` reuses `XrdsSceneWorldWidget`** rather than declaring a parallel
      five-variant `XrdsPanelElementKind` as originally planned. The widget
      structs already carry `local_position`, per-kind sizing and colours, so a
      second enum would be a duplicate that drifts — and it makes migration a
      matter of giving each existing widget a name. An element is exactly *a
      named widget with triggers*, which is the honest description.
- [x] Element carries `name` and `triggers` (`#[serde(default)]`, skipped when
      empty) — precisely the two things the world side lacked.
- [x] `XrdsSceneDocument::panels` + `panel_template(id)`,
      `panel_template_mut(id)`, `panel_template_by_name(name)`,
      `next_available_panel_template_id()`. Both lookups exist because the two
      halves address differently: instances store an id (stable across renames),
      authors pick by name.
- [x] `XrdsPanelElement::can_emit` / `is_interactive` / `kind_name`. `Custom`
      and `RunawayDetected` are **not** emittable by an element: both dispatch
      to a node's `XrdsId`, and an element has no id.
- [x] Template carries **no placement**, asserted by a test that fails if
      `depth`/`translation`/`anchor` ever appear in its serialized form. That is
      the `XrdsHudTemplate::depth` bug prevented structurally rather than by
      comment.
- [x] `panel_diagnostics()` in `src/document/panel_diagnostics.rs`, kept
      separate from `track_diagnostics()` so the editor can show panel problems
      in the panel workspace: duplicate element name (**error** — breaks
      addressing), element cannot emit its trigger (**warning** — inert, most
      likely a leftover after changing kind), binding naming a missing Track
      (**error**), binding running nothing (**warning**), plus the §3b naming
      policy applied to panel *and* element names.
- [x] 20 tests in `src/tests/panel.rs`. Mutation-verified: breaking duplicate
      detection fails 2, breaking `can_emit` fails 3.

Deferred out of A1 (was in scope, now sequenced later):

- [x] Attachment points — scene `XrdsSceneNodePayload::Panel { template_id }`.
      Moved to **A2**, where the runtime that spawns them landed. The camera
      half shipped as `panel_template_id`/`panel_depth` in A2c, then was itself
      retired in favour of parenting a `Panel` node under the anchor — see A6
      and A6b-lite, whose fields and `link_panel` were later deleted outright.

### A4b — retire the old vocabulary (after A2 and A3)

Split into three, because the original single step could not end green: it mixed
a latent data-loss bug, a vocabulary deletion, and a payload redesign that
reaches into glTF export.

#### A4b-0 — the panels export round-trip — landed

Found while surveying A4b, **not** by a test: `hud_library` and `tracks` both
round-trip through export, and `panels` did not. Since a `Panel` node carries
only a `template_id`, the registry *is* the content — so `export_scene_document`
produced documents whose panels were empty shells, and any save/load cycle
silently deleted every panel template an author had made. A latent bug in A2,
and A4b would have replaced the registry that *does* survive with the one that
doesn't.

- [x] `XrdsImportedPanelLibrary` resource + `sync_panel_registry`, mirroring
      `XrdsTrackRegistry`/`sync_track_registry`.
- [x] Wired into **both** import paths. `reimport_scene_in_world` and
      `XrdsAPI::import_scene_document` share no body, and `hud_library` was in
      fact only ever stored by the first — the same asymmetry that once left
      `tag_player_anchor_entities` off the import path.
- [x] Exported alongside `hud_library`/`tracks` in `export_scene_document`.
- [x] Replaced wholesale rather than merged, so deleting a template in the editor
      does not resurrect it on the next import. Tested.
- [x] 3 tests, one per path plus the clearing rule. Written failing first
      (`left: []`). Mutation-verified: removing either call fails exactly the
      test for that path, so neither is coasting on the other.

#### A4b-1 — retire the HUD template vocabulary — landed

- [x] Deleted `XrdsHudTemplate`, `XrdsHudItemDef`, `HudItemDefId`,
      `XrdsSceneDocument::hud_library`, `hud_template()`/`hud_template_mut()`,
      `next_available_template_id()`; a HUD text item is now a `Label` element.
      Unblocked by A3b-3 — before that this step *removed* authoring ability.
- [x] Deleted a **second, entirely dead copy** of `XrdsHudTemplate` /
      `XrdsHudItemDef` in `xrds-components/src/primitives/hud_panel.rs`. Nothing
      referenced it; it had been shadowed by the scene-graph pair the whole time.
- [x] Deleted `XrdsScenePlayerAnchor::hud_template_id`, and with it the "anchor
      links both template kinds" diagnostic and its two tests — with one kind of
      template left, the ambiguity cannot be expressed. Replaced by one test that
      the common no-panel case stays quiet.
- [x] `XrdsImportedHudLibrary` and `spawn_hud_instance_for_anchor` gone; the
      anchor path keeps only `panel_template_id`, with no precedence rule.
- [x] `link_hud` → `link_panel(anchor, Option<&XrdsPanelTemplate>, depth)`. It
      could not survive its own parameter type.
- [x] **`set_hud_item` keeps its name and contract** (§8), and its doc comment
      now says why: it resolves against `XrdsStoredHudInstance`, which was always
      name-keyed and is exactly what the panel path populates.
- [x] Deleted `hud_library.rs` and its 12 commands, `HudTemplateDto` /
      `HudItemDefDto`, the `hud_library` snapshot field, `HudLibraryPanel` and
      `HudCanvasOverlay`. Plus 8 of 10 now-dead `.hud-library-*` CSS rules; the
      2 survivors keep their names, since renaming a shared class is a silent
      unstyling if a call site is missed.
- [x] **`LinkPanelTemplate` added, not just renamed.** The anchor DTO exposed
      `hud_template_id` and never `panel_template_id`, so head-locked panels were
      not authorable from the editor at all — deleting the old command without
      this would have been a straight regression. Carries `depth`, so the
      Inspector gained a depth slider that `XrdsHudTemplate::depth` made
      impossible. 6 tests; mutation-verified on the dangling-link guard.
- [x] Panel commands added to `is_structural_command`, where they had been
      missing — panel authoring logged at `trace!` while comparable edits logged
      at `info!`. (Investigated as a possible missed-reimport bug first; it is
      log-level only. `apply_panel_library_command` returns its own reimport
      flag.)
- [x] Corrected an inaccurate comment on `DeletePanelTemplate`: it clears
      *anchor* links but leaves `Panel` **nodes** dangling, because a Panel node's
      whole payload is the reference and there is no empty state to fall back to.
      Diagnosed rather than silently deleting the author's node.
- [x] `BRIDGE_VERSION` 7 → 8, both sides.
- [x] Not in scope, as planned: `XrdsSceneHudText`/`XrdsStoredHudText`/
      `SetHudText`. A *node payload*, unrelated to templates despite the name.

#### A4b-2a — make scene-placed panels authorable — landed

Reordered ahead of the deletion after scoping turned up two compounding gaps.
These are the reason A4b-2 is a **bug fix**, not a cleanup:

1. **`XrdsSceneWorldWidget` has no `triggers` field.** Only `XrdsPanelElement`
   adds one, and the WorldPanel spawn path (`api.rs`, `reimport.rs`) discards the
   entity `spawn_world_widget_from_scene` returns. So **every button, slider and
   toggle on a WorldPanel node is dead** — it renders, it highlights on hover, and
   it can never run anything.
2. **Nothing in the editor could create a `Panel` node.** The palette offered only
   `WorldPanel`; `XrdsSceneNodePayload::Panel` appeared solely in tests. So a
   template with working triggers could be head-locked (A4b-1) but never placed on
   a wall — the missing half of "attachment is the only difference".

Doing this before the deletion also means a stall leaves a working feature rather
than a half-migration.

- [x] Palette `Panel` entry. Bootstraps a starter template when the library is
      empty rather than refusing the spawn, which would make the entry look broken
      to anyone who has not opened the Panels workspace; reuses the first existing
      template otherwise, so placing four panels does not leave four near-identical
      library entries. Both branches mutation-verified.
- [x] Same default placement as `WorldPanel` (eye height, 1 m forward), so
      migrating one in A4b-2b does not move it.
- [x] `NodePayloadDto::Panel { template_id }` — id only. The frontend resolves the
      name from `panel_library`, which is already in the snapshot, so a rename
      cannot leave a stale copy behind.
- [x] `SetPanelInstanceTemplate`. **Not** `Option<u64>` unlike
      `LinkPanelTemplate`: an anchor can have no panel, but a Panel node *is* its
      template reference, so clearing it would leave a node that can never render.
- [x] Inspector section, deliberately thin — template picker plus element count
      and size. No per-instance size/background overrides: that would be
      `XrdsHudTemplate::depth`'s mistake in reverse, with instances quietly
      diverging from the shared definition.
- [x] A missing template shows as a red note *and* keeps a placeholder option, so
      the select cannot silently snap to another template and hide the problem.
- [x] Corrected the `HudText` palette tip, which still pointed at the deleted HUD
      Library panel, and made the `WorldPanel` tip state that its buttons cannot
      fire triggers.
- [x] 6 tests, including that a bootstrapped panel is diagnostic-clean (the
      starter name has to pass the naming policy) and that deleting a template
      leaves Panel nodes dangling-and-diagnosed rather than silently removing the
      author's node.
- [x] `BRIDGE_VERSION` 9 → 10.

#### A4b-2b — retire `XrdsSceneWorldPanel` — landed

`XrdsSceneWorldPanel` held background + size + layout + inline `widgets` — field
for field an `XrdsPanelTemplate` fused to its own attachment. The unified model
already splits those into `XrdsPanelTemplate` + `XrdsSceneNodePayload::Panel`, so
the payload was not *missing* a `template_id`, it was **wholly superseded**.

Planned as a migration ("convert WorldPanel nodes to Panel instances + a generated
template"). **No migration was needed**: the user confirmed no scene uses it, which
removed the only reason to keep the payload loadable. Straight deletion instead.

- [x] `XrdsSceneWorldPanel`, `XrdsSceneNodePayload::WorldPanel`,
      `XrdsSceneRuntimeComponent::WorldPanel` and both runtime spawn arms.
- [x] The 7 editor commands, `NodePayloadDto::WorldPanel`, `WorldPanelSection`,
      `WorldPanelCanvasOverlay`, and App/Inspector's `onEditWorldPanel` wiring.
- [x] `world_panel_gizmo_system` — it drew a wireframe rect because that payload
      had no mesh to select against. A `Panel` node needs no replacement: its
      backdrop is a real mesh, so `update_selection_outline` already outlines it
      through the same generic path every primitive uses.
- [x] `crates/xrds-scene-graph/src/tests/world_ui.rs` deleted whole.
- [x] `BRIDGE_VERSION` 15 → 16.

**Kept, deliberately** — the naming collision makes these easy to delete by
mistake. `xrds_components::XrdsWorldPanel` (runtime component),
`spawn_world_panel_descriptor` and `registry.rs`'s registration back the *scripting*
API (`XrdsAPI::spawn_world_panel`), which is a separate live feature.
`XrdsSceneWorldLayout` and `XrdsSceneWorldWidget` are `XrdsPanelTemplate::layout`
and `XrdsPanelElement::kind` — the replacement's own vocabulary.
`spawn_world_widget_from_scene` looked orphaned and is not: `spawn_panel_element_in_world`
calls it, which is the live element path.

Two things this exposed, both fixed here:

- **`HoverEnter`/`HoverExit` availability** pointed at `WorldPanel`, the only
  payload that used to get a pointer surface. It now points at `Panel`, which
  qualifies for the same reason.
- **`Panel` had no `KIND_ICON` entry**, so a placed panel showed no icon in the
  hierarchy tree. `WorldPanel`'s icon moved over rather than being lost.

### A2a — the trigger mechanism — landed

Split from A2 so the *mechanism* is proven by test before any attachment wiring
depends on it. A2b below does the wiring.

- [x] **`spawn_world_widget_from_scene` returns the spawned `Entity`.** It
      discarded it before, which is the single reason authored widget triggers
      could never fire. Correction to this checklist as first written: the five
      `spawn_world_*_entity` helpers *already* returned theirs — only the
      wrapper threw it away.
- [x] `spawn_panel_element_in_world(world, panel_entity, element)` — spawns an
      element and tags it with its own authored triggers, returning its entity.
- [x] `set_element_trigger_bindings` split out so re-authoring uses the same
      **remove-when-empty** rule as spawn. Two paths disagreeing about that is
      how an "unbound" element keeps firing.
- [x] Confirmed: **`consume_triggers` needed no change at all**, as §4 predicted.
- [x] 5 tests. The load-bearing one — `pressing_a_panel_element_fires_the_track_its_binding_names`
      — is the plan's premise made executable. Mutation-verified: skipping the
      tagging fails 3, always-insert-never-remove fails 2.

### A2b — scene attachment — landed

- [x] `XrdsSceneNodePayload::Panel(XrdsScenePanelInstance { template_id })`. A
      struct rather than a bare id so per-instance data has somewhere to go
      later — the obvious candidate being §5's relative-addressing override
      table, letting an instance say *which* door its button opens.
- [x] **Resolved by a document pass, not by `to_runtime_node`.** A Panel node
      carries only a `template_id`, and resolving it needs the document — which
      that per-node conversion deliberately does not have. So `Panel` emits a
      bare node and `spawn_panel_instances` fills it in, exactly the shape
      `PlayerSpawnZone`/`tag_spawn_zone_entities` already established. Threading
      a template through the conversion would have pushed document lookup into a
      method built without one.
- [x] Wired into **both** import paths (`reimport_scene_in_world` and
      `XrdsAPI::import_scene_document`) — mutation-verified, since only one is
      exercised by tests and a single-path wiring would have looked fine.
- [x] Elements spawn **per instance**, so a template instanced twice yields two
      independent element sets. Asserted, because sharing entities would make
      two panels unable to behave independently.
- [x] A dangling `template_id` logs and yields an empty node rather than failing
      the scene load. The reference is diagnosed at author time by
      `panel_diagnostics`; refusing to load over it would be worse.
- [x] 4 tests, including `an_element_on_an_instance_fires_its_track_end_to_end`
      — the full authored path with no hand-spawned entities: document →
      template → instance → element → binding → Track.

### A2c — camera attachment — landed

**Superseded by A6/A6b-lite below.** Everything in this section describes
`panel_template_id`/`panel_depth`/`link_panel`/`spawn_panel_template_head_locked`,
all since deleted — a head-locked panel is now authored by parenting a `Panel`
node under the anchor, which is what A6 exists to explain. Kept as the
historical record of the step that made "attachment is the only difference"
literally true, even though the specific mechanism it used did not last.

**"Attachment is the only difference" now holds literally.** One template, two
attachment points, and the only thing that differs is placement: a scene `Panel`
node parents elements to itself, the camera path parents them to the anchor with
`XrdsHeadLocked` at `-depth` in camera-local space. The elements themselves spawn
through the same `spawn_panel_element_in_world` either way.

- [x] `XrdsScenePlayerAnchor { panel_template_id, panel_depth }`, alongside
      `hud_template_id` — additive, so the working HUD is untouched.
- [x] **`depth` on the attachment, not the template.** Tested by instancing one
      template at two depths, which `XrdsHudTemplate::depth` made impossible.
      `panel_depth` defaults to `0.5` to match the old HUD default, so migrating
      a template does not silently move it.
- [x] `spawn_panel_template_head_locked` returns
      **`XrdsStoredHudInstance`** — the very component `set_hud_item` already
      resolves by name. So a public API predating all of this keeps working
      against a migrated template with **zero changes**. That was the cheapest
      part of the migration precisely because both models address by name, which
      is the payoff of §2's "identity" column.
- [x] `panel_template_id` wins when both are linked, because honouring both
      would stack two overlapping panels on one anchor — a rendering bug rather
      than a legible authoring mistake. Warned about at author time instead.
- [x] Closed a gap left in A1: `panel_diagnostics` now also checks that a
      `Panel` instance's and an anchor's `template_id` **resolve**. A1 listed
      that and only did it for element→Track.
- [x] `XrdsPanelElement::local_position()` reaches across the five widget
      variants, so an attachment can place an element without matching on kind.
- [x] 12 tests (8 schema, 4 runtime). Mutation-verified: hardcoding depth fails
      1, dropping the name map fails 3 — including `set_hud_item`.
- [x] A head-locked panel can carry a **button**, tested. That is what
      unification buys the HUD side, which had one element kind and no triggers.

Not done, deliberately: `XrdsAPI::set_hud_template_for_anchor` (`api.rs:212`) is
the expert-path API taking an explicit `XrdsHudTemplate`. It is not
document-driven, so it needs no panel branch; a panel equivalent can be added
when something asks for one.

### A2 — original scope, for reference

**Cancelled as written — superseded by the A2a/A2b/A2c split below**, which
shipped every capability listed here (and landed it in three independently
testable steps instead of one). Kept only so the split is legible against the
original ask; none of these bullets describe outstanding work.

- [ ] ~~Spawn elements from a template for **both** attachments: the world path
      in `import_runtime_nodes`, and the camera path in `reimport.rs:472`
      (which currently reads `hud_template`).~~
- [ ] ~~Tag element entities with `XrdsTriggerBindings` from the element's own
      authored `triggers`, mirroring `tag_trigger_binding_entities`
      (`reimport.rs:536`) — **including its remove-when-empty behaviour**, so
      unchecking the last binding actually detaches the component.~~
- [ ] ~~Confirm no change is needed in `consume_triggers` (§4). If one turns out
      to be needed, stop and re-read §4 — the analysis says otherwise.~~
- [ ] ~~Keep `set_hud_item(name, …)` working (`src/xrds_api/context.rs`) by
      resolving to a `Label` element of that name. Its contract is
      name-addressed and unification preserves name-addressing.~~
- [ ] ~~Tests:~~
  - [ ] ~~pressing a button element fires the Track its binding names~~
  - [ ] ~~an element with no triggers gets no `XrdsTriggerBindings`~~
  - [ ] ~~removing the last binding detaches the component~~
  - [ ] ~~`set_hud_item` still updates a migrated Label~~
  - [ ] ~~a Track fired by an element participates in the conflict guard~~

### A3a — panel bridge (Rust) — landed

Additive, like everything before it: `hud_library.rs` and its 12 commands are
untouched, so the working HUD keeps running while the frontend has not moved yet.

- [x] `PanelTemplateDto` / `PanelElementDto` in `bridge.rs`, plus snapshot
      `panel_library` and `panel_diagnostics` (separate from
      `track_diagnostics`, so the panel workspace shows its own).
- [x] `PanelElementDto.widget` **reuses `WorldWidgetDto`**, same reasoning as the
      schema reusing `XrdsSceneWorldWidget` — a parallel five-kind DTO would
      drift, and an element genuinely is a named widget with triggers.
- [x] `emittable_triggers` is resolved **server-side** from
      `XrdsPanelElement::can_emit`, not re-derived in TypeScript. Reachability is
      a runtime fact, and a second copy of the rule would drift from the Rust
      diagnostics that use the original. A `Label` reports `[]`, so the picker
      offers nothing.
- [x] `panel_library.rs`: template CRUD + element CRUD, every name routed
      through §3b's validator so panels and elements get the same policy as
      Tracks.
- [x] **Elements addressed by name, never index** — tested by removing a middle
      element and then renaming a later one, which an index-addressed command
      would get wrong.
- [x] Renaming a template needs **no** reference fixups, because instances store
      an *id*. That is the deliberate difference from Track bindings, which store
      a name and therefore must be re-pointed on rename.
- [x] Deleting a template clears anchors that linked it, rather than leaving a
      dangling reference — same choice as `DeleteTrack`.
- [x] Template edits request a reimport (instances spawn elements at import
      time), but creating an empty template does not.
- [x] `BRIDGE_VERSION` 5 → 6, both sides.
- [x] 10 tests. Mutation-verified: dropping the duplicate-name guard fails 1,
      always-reporting-all-triggers fails 1.

### A3b-1 — element trigger editing + the finish line — landed

**The plan's measurable definition of done is met.** `lib/sequencer.ts` no longer
claims the four widget kinds are `"not reachable — authored widgets have no
bindable node"`. That was true when written and is not any more, and a test now
fails if it comes back.

- [x] Six element-binding commands: add / remove / set kind / track / hand /
      disabled. Addressed by `(template, element **name**, binding index)` — the
      element by name so reordering cannot re-point a binding, the binding by
      index because nothing references one positionally.
- [x] All of them funnel through one `with_element` helper, so
      address-by-name-and-tolerate-a-miss is written once. A miss is a stale
      frontend, not a panic — the next snapshot corrects it.
- [x] A new binding **defaults to a kind the element can actually emit**
      (`SliderChange` on a slider, not `ButtonPress`). Seeding everything with
      `ButtonPress` would make a slider's first binding silently inert, which is
      exactly the confusion `emittable_triggers` exists to prevent.
- [x] Frontend rule split in two, disjoint on purpose:
      `unavailableReasonFor` (nodes) now says *"set this on a panel element, not
      on a node"* — a "wrong place" message rather than a "nowhere" one — and
      `elementUnavailableReasonFor` (elements) reads `emittable_triggers` from
      the snapshot rather than re-deriving the rule.
- [x] `validKindsForElement`, `elementRowLabel`, `elementKindName` for the
      workspace to come.
- [x] `BRIDGE_VERSION` 6 → 7.
- [x] 5 Rust tests, 9 vitest. Mutation-verified both ways: restoring the old
      "not reachable" string fails a test; dropping the duplicate/emittable
      guards fails others.

### A3b-2 — Panels workspace — landed

A third workspace next to Scene and Sequencer, in the Sequencer's language (§7):
**library | elements | canvas | inspector**.

- [x] `Panels` entry in the toolbar switcher. `Workspace` moved to
      `types/bridge.ts` as a shared union so Toolbar and App cannot drift.
- [x] **It hides the 3D viewport rather than resizing it**, which is what makes
      it focused and is the real difference from the Sequencer (which keeps the
      viewport live on purpose). Panel design is 2D, so the viewport contributes
      nothing.
- [x] **Closes the Bevy viewport hole while mounted**, via the same
      `set_viewport_hole` IPC the modal overlays use. Not cosmetic: the editor
      punches a real hole in its own window for Bevy, so a full-screen React
      layout that left it open would have clicks land on the 3D scene instead of
      the canvas — the same class of bug as an absolutely-positioned overlay
      swallowing input.
- [x] Library: create / rename (double-click) / delete, with a confirm that
      names the consequence (anchors linking it get cleared).
- [x] Elements: add by kind with an auto-generated non-colliding name, select,
      remove.
- [x] Canvas: elements drawn **to scale** on the template's authored dimensions,
      with a centre crosshair — `local_position` is measured from the centre, so
      without a visible origin the inspector's numbers have no anchor.
- [x] Element inspector: rename, plus trigger bindings with kind / Fires /
      hand / disabled.
- [x] **The kind picker is driven by `validKindsForElement`**, i.e. by the
      `emittable_triggers` Rust computed. A Label therefore offers nothing and
      its "+ Add binding" is disabled with a reason, rather than offering
      `ButtonPress` and silently never firing. A binding whose kind the element
      cannot emit is kept in the list and flagged, so an existing document is
      never silently rewritten.

Deliberately not done in this pass, with reasons — all three later landed,
folded into A3b-3 below rather than tracked as their own phase:

- [x] **Drag-to-move on the canvas.** Position lives *inside* the widget variant,
      so moving one element means rebuilding its whole DTO — per pointer-move
      that needs the live-preview/commit split the node Inspector's transform
      fields already use. Shipped in A3b-3 (`dragRef` in `PanelWorkspace.tsx`).
- [x] **Per-widget property editing** (label text, colours, sizes). The
      `SetPanelElementWidget` command exists and is wired; only the form was
      missing. Shipped in A3b-3 as the data-driven `widgetFields`/`withField`
      table in `lib/panelWidget.ts`.
- [x] Retiring `HudCanvasOverlay`/`WorldPanelCanvasOverlay` — was A4b, after
      this workspace had been used enough to trust. Both files are deleted.

### A3 — original scope, for reference

**Cancelled as written — superseded by A3a/A3b-1/A3b-2/A3b-3**, which shipped
every capability listed here across four landed steps instead of one. Kept
only so the split is legible against the original ask; none of these bullets
describe outstanding work.

- [ ] ~~DTOs in `src-tauri/src/bridge.rs`: template, element, element kind;
      snapshot `panel_library` (generalizing `hud_library`).~~
- [ ] ~~Replace the two command surfaces with one element-addressed set. Today:
      12 HUD commands (`CreateHudTemplate` … `LinkHudTemplate`) and 7 world
      commands (`SetWorldPanelParams`, `AddWorldPanelWidget`,
      `RemoveWorldPanelWidget`, `MoveWorldPanelWidget`, `SetWorldPanelWidget`,
      `SetWorldPanelWidgets`, `SetWorldPanelLayout`).~~
- [ ] ~~Address elements by **name**, not index — `MoveWorldPanelWidget` reorders
      today, and an index-addressed command would silently re-point bindings.~~
- [ ] ~~**Bump `BRIDGE_VERSION`** on both sides. It is 5 now.~~
- [ ] ~~Generalize `HudLibraryPanel.tsx` into the template library.~~
- [ ] ~~Converge `HudCanvasOverlay.tsx` and `WorldPanelCanvasOverlay.tsx` into
      one canvas; attachment differs, canvas editing does not.~~
- [ ] ~~Element inspector reusing `TriggersSection` from `Inspector.tsx` — a
      binding is a binding.~~
- [ ] ~~`validKindsFor` in `src/lib/sequencer.ts`: `ButtonPress` /
      `ButtonRelease` / `SliderChange` / `ToggleChange` become **available** on
      elements. They are hard-coded `false` today with a comment explaining
      they are unreachable — that comment is what this plan retires.~~
- [ ] ~~Delete `src-tauri/src/hud_library.rs` once its commands are folded in.~~
- [ ] ~~Tests: vitest for element-row labelling and kind availability.~~

### A4 — the instance hazard (§5)

**Done.**

- [x] Diagnostic pass can see, per template, how many nodes instance it —
      `XrdsSceneDocument::panel_instance_count`. Counts Panel nodes *and*
      head-locking anchors: the hazard is about how many live copies of an
      element exist, not how each one got there.
- [x] Warn when a Track fired from a template-authored element trigger has a
      row targeting a fixed `Node(id)` while the template has >1 instance.
- [x] Wording must **not** suggest `TriggerSource` as the fix — it resolves to
      the element, not its neighbours (§5). State the consequence only. A test
      asserts the detail never mentions it, so the corrected advice cannot
      quietly come back.
- [x] The detail names the instance count, the element, the Track, and the
      **node names** (not ids) it will drive — a warning that only says "this
      might be wrong" sends the author hunting.
- [x] A dangling Track reports only the dangling reference, not both: two
      diagnostics for one mistake reads as two mistakes.
- [x] Test: 1 instance → quiet; 2 instances + fixed row → warns; 2 instances +
      `TriggerSource` row → quiet (each drives its own element). Plus:
      no-Track binding, dangling Track, anchor-as-second-instance. Mutating the
      `< 2` guard and the target filter each fail tests.
- [x] Editor: surfaced with no bridge change — `build_panel_diagnostics_dto`
      already carries it. Fixed the inspector's filter along the way: it matched
      on element name alone, so two panels each with a "Go" button showed each
      other's warnings, and the duplicate-name error ("element named X") matched
      nothing at all and was invisible.

Not covered, deliberately: `SelfNode` rows. For an element-fired Track "the node
the sequence is authored on" has no meaning — there is no node. That is a gap in
the *model*, not a wording problem a warning can fix, and it belongs with Phase
B's `XrdsActionTarget::Element` rather than here.

### A3b-3 — element properties and canvas drag — landed

Promoted from "deferred polish" once it became clear these are **A4b's gate**,
not niceties: A4b turns a HUD text item into a `Label` element, and a `Label`
whose `text` cannot be set is not a replacement for `XrdsHudItemDef`. The old
vocabulary could not be retired while the new one was unauthorable.

- [x] `src/lib/panelWidget.ts` — a field table per `WorldWidget` kind, plus
      `withField`/`withVec2Component`/`movedTo`/`alphaOf`. Data-driven rather
      than five hand-written forms: the five variants overlap almost entirely,
      so bespoke forms are four chances to omit a field, and an omitted field is
      an unauthorable property with no error anywhere.
- [x] 19 tests, in a pure module for the same reason `sequencer.ts` is one. Two
      run **both directions**: every DTO key is offered by the form, and every
      offered key exists on the DTO (a stale key writes something Rust ignores,
      so the edit looks like it worked and did nothing). Mutation-verified by
      dropping `Label.text` from the table.
- [x] `alphaOf` exists because `<input type="color">` is RGB-only — rebuilding
      a colour without it forces every translucent element opaque on first touch.
      A test pins the fallback direction, since defaulting to 0 hides things.
- [x] `NaN` guard on every number input: a half-typed box serialises as `null`
      and is rejected by the Rust side.
- [x] An unknown widget kind yields position-only rather than throwing, so a
      document from a newer build does not blank the inspector.
- [x] Canvas drag-to-move, following `WorldPanelCanvasOverlay`: local state
      while moving, exactly **one** `SetPanelElementWidget` on pointer-up. Not
      merely about traffic — the editor has an undo stack, and per-move commands
      would bury the author's previous action under a hundred one-pixel nudges.
      Same 4px click/drag threshold, so selection feels identical in both
      canvases. `touch-action: none; user-select: none` per the existing
      draggable precedent.
- [x] Fixed the inspector's diagnostic filter (see A4) — found while wiring the
      properties form next to it.
- [x] No bridge change: `SetPanelElementWidget` was already wired, which is why
      this was a UI-only pass. `BRIDGE_VERSION` stays at 7.

Correction to a claim made while doing this: I reported the canvas was drawing
elements half-a-size off-target because it positioned by top-left. It was not —
`.panel-ws-el` already carried `translate(-50%, -50%)`. No bug, and nothing to
fix.

### A6 — bindings move to the instance — landed

**Supersedes §5 and deletes §A4.** The user's proposal: a template designs a panel
and carries no triggers; the placed *instance* carries the wiring.

This is strictly better than what §A1–A4 built, for one reason. §5's hazard —
one elevator template on three floors, every floor's button firing the same Track
at the same fixed door — could only ever be *warned* about, because
`XrdsActionTarget::TriggerSource` resolves to the button that fired, not to
anything near it, so there was no fix to suggest. With bindings on the instance
each floor wires its own door: the hazard is **unrepresentable**, not diagnosed.
§A4's warning is therefore deleted rather than disabled, along with its 8 tests —
a warning for a condition that cannot occur is noise.

The cost, accepted: twenty instances wired alike is twenty sets of bindings, where
a shared template needed one edit. That is the price of per-instance targets.

- [x] `XrdsPanelElement` loses `triggers`. A serialisation test asserts no
      `trigger` key can reappear on a template — the load-bearing property.
- [x] `XrdsScenePanelInstance` gains
      `element_triggers: BTreeMap<String, Vec<XrdsTriggerBinding>>`. A map, not a
      `Vec` of pairs: duplicate keys become structurally impossible and the order
      is deterministic, so two saves produce identical JSON. The struct's original
      doc comment predicted exactly this use ("letting an instance say *which*
      door its button opens").
- [x] `set_triggers` removes the key when the list is empty; `rename_element`
      moves it. Renames propagate, deletes are diagnosed — per the user's call.
- [x] **Attachment is parenting.** A Panel node under a `PlayerAnchor` is
      head-locked, resolved by walking the document's `parent_id` chain (Bevy's
      hierarchy is still queued at that point in import), depth-bounded so a
      hand-authored cycle cannot hang the load.
- [x] `panel_depth` is superseded by the node's own `transform`, composed with the
      element's canvas position — an author gets offset *and rotation*, not one
      scalar. Care taken: this is the node's **local** transform. Using a world
      position as a camera-local offset is the recorded anchor-offset mistake.
- [x] **`set_hud_item` keeps its exact signature.** It takes an anchor id and
      resolves `XrdsStoredHudInstance` on the anchor, so a Panel node under an
      anchor *contributes* to that component — extending, not replacing, so two
      panels under one anchor both stay addressable. Mutating that away fails 3
      tests, so the public API is genuinely covered, not just claimed.
- [x] Diagnostics rewritten as a per-instance pass, attributed to `node_id` so the
      scene Inspector shows each Panel node its own problems — template-owned
      bindings had nowhere to point. New error: **"Binding names an element the
      template does not have"**, the hazard that replaces the one this removed. It
      keeps the bindings rather than dropping them, so authored wiring stays
      recoverable, and it suppresses the redundant Track check on the same row
      (two diagnostics for one mistake reads as two mistakes).
- [x] The 6 element-trigger commands became node-scoped
      (`AddPanelNodeTrigger`, …). Wiring a name the template lacks is **stored,
      not refused** — that is the shape a deleted element leaves behind, and
      refusing it would make recovered wiring impossible to repoint.
- [x] Editor: trigger authoring moved out of the Panels workspace into the scene
      Inspector (`PanelInstanceTriggers.tsx`), driven by a server-side join of
      template element + instance wiring so the UI never cross-references
      `panel_library` itself. Orphaned rows are shown, not hidden — hiding them
      would make the UI disagree with the saved file.
- [x] `elementRowLabel` no longer shows a binding count; a count against a
      template would be right for one instance and wrong for the next. Its test
      was replaced rather than deleted.
- [x] `TRACK_NONE_SENTINEL`/`HAND_ANY_SENTINEL` exported from `bridge.ts` instead
      of being redeclared per component — two copies that drift silently stop
      matching.
- [x] `BRIDGE_VERSION` 10 → 11.
- [x] Mutation-verified: skipping the head-lock insertion fails the depth test;
      skipping the `XrdsStoredHudInstance` contribution fails all three
      `set_hud_item` tests; the rename propagation and its refusal path are both
      covered.

Still open from this change: `XrdsScenePlayerAnchor::panel_template_id` /
`panel_depth` and `link_panel` remain as a second, wiring-less attachment path.
They work but cannot carry bindings, and both call sites pass an empty map with a
comment saying so. Retiring them is A6b.

### A6b-lite — stop *using* the anchor-link path — landed

Per the user: leave `panel_template_id`/`panel_depth`/`link_panel` working as
reference, clean the deprecates out later. But "don't use them" had a live
consequence — the PlayerAnchor Inspector still *drove* that path, so an author
picking "Head-locked Panel" there got a panel with dead buttons, which is exactly
the trap A6 removed everywhere else.

**"Later" arrived.** The whole anchor-link path — both fields, `link_panel`,
`spawn_panel_template_head_locked`, its diagnostic, and the Inspector's warning
below — was deleted outright in a follow-up cleanup pass, once the user
confirmed no scene relied on it. Nothing below is still working "as reference";
the bullets are kept as the historical record of the interim state.

- [x] `#[deprecated]` on `link_panel`; doc notes on both anchor fields saying why
      (they cannot carry `element_triggers`) and what to do instead. *(Fields and
      method since deleted — see above.)*
- [x] The Inspector's picker + depth slider became **"+ Add panel child"**, which
      sends `SpawnPrimitive { kind: "Panel", parent_id: anchor }` — the working
      path, reusing a command that already existed. *(Still the only path; this
      part did not change.)*
- [x] An anchor still holding the old link shows a warning naming the template and
      depth, so an existing document explains itself rather than silently
      under-performing. *(The warning itself was deleted along with the fields it
      read — there is nothing left to warn about.)*
- [x] A Panel node under an anchor defaults to `[0, 0, -0.5]`, not
      `[0, 1.5, -1]`. Its transform is read as *camera-local*, so the world-space
      default would have put a HUD a metre and a half above the viewer's own eye.
      Matches the depth the retired `panel_depth` used, so migrating moved
      nothing at the time. **Superseded:** once a panel gained an opaque backdrop
      (A2c predates that), 0.5 m turned the default template into a blindfold —
      moved to `-1.5` in the same cleanup pass, matching where Quest places its
      own windows.
- [x] The ancestor walk mirrors the runtime's, checking the **whole chain** — a
      panel grouped under an Empty beneath an anchor is still head-locked.
      Mutation-verified (immediate-parent-only fails the test).
- [x] 3 tests.

### A5 — authorable stop (§6b) — landed

- [x] `XrdsTriggerEffect { Fire, Stop }` on `XrdsTriggerBinding`, defaulting to
      `Fire` **and skipped when serialising** — so documents predating stop keep
      their exact meaning *and* adding the field churns no saved file. Both halves
      asserted.
- [x] Runtime: `consume_triggers` branches on the effect. Stop routes through
      `despawn_agents_releasing_locks`, the single choke point — mutating the
      release away fails 5 tests, two of them new.
- [x] Stop resolves its asset set via the **same** `schedule_track_keys` call
      start uses, then despawns the holders. Mutating it into a name-keyed sweep
      fails exactly the disjoint-runs test, which is the §5b mistake it avoids.
- [x] Bindings are processed in authored order, so `Stop X` then `Fire X` on one
      element restarts a Track from the top with no conditional.
- [x] Diagnostic: "Stop binding for a Track nothing fires". Warning, not error,
      and only when *nothing anywhere* fires it — `XrdsAPI` can start a Track from
      Rust, so a Stop-only binding is legitimate in a partly code-driven scene.
      Suppressed when the Track is missing entirely, which is already an error.
- [x] `XrdsSceneDocument::all_trigger_bindings` — one iterator over **both**
      binding sources (node triggers *and* panel element wiring). A caller walking
      only `node.triggers` would warn that nothing fires a Track a panel button
      starts perfectly well; mutation-verified against exactly that.
- [x] Editor: the Fires picker gained a Fire/Stop mode on both node and panel
      element bindings, and the row label follows the effect ("Stops → Open")
      so it reads as one sentence.
- [x] 6 runtime tests, 6 scene-graph tests.
- [x] `BRIDGE_VERSION` 11 → 12.

Correction worth recording: the first two runtime tests were written against
`XrdsCustomTriggerEvent`, which has **no `consume_triggers` registration** — Custom
is only reachable via threshold crossings. They passed nothing until switched to
`ButtonPress`, which is both wired and the actual motivating case. A test that
fires an unconsumed event asserts nothing.

### B1 — elements as action targets: addressing — landed

- [x] `XrdsActionTarget::Element { panel: XrdsSceneNodeId, name: String }`. The
      *panel node* is named explicitly rather than taken from `self`, so a wall
      switch can drive a display panel across the room. Two instances of one
      template are two different targets, which falls out of the addressing
      instead of needing a rule.
- [x] **`XrdsActionTarget` gave up `Copy`** — a `String` forbids it. Every call
      site takes it by reference, which is what it wanted anyway.
- [x] `XrdsPanelElementIndex` — `(panel entity, element name) → element entity`.
      Needed because nothing else tracks widget entities: elements are not
      document nodes, so `XrdsIdIndex` cannot hold them, and
      `XrdsStoredHudInstance` is per-anchor and head-locked-only.
- [x] Rebuilt, not merged, on every import: element entities are respawned
      wholesale, so a surviving entry would point at a dead — or recycled —
      entity. Tested.
- [x] Element entities participate in `XrdsTrackAssetLocks` for free, because
      resolution returns an ordinary entity. Two Tracks on one element conflict;
      two Tracks on the *same element of two instances* do not. Both tested, and
      mutating the index to key on name alone fails exactly those two.
- [x] Diagnostics distinguish **three** dangling cases, because the fix differs:
      the panel node is missing, the panel exists but has no such element (a
      rename), or the addressed node is not a Panel at all (a wrong reference).
- [x] Editor: `AddTrackElementAsset` (separate from `AddTrackAsset` — an element
      target needs two values, and one command where "the second field only
      sometimes matters" invites a half-filled payload). Rows label themselves
      "PanelName · elementName", joined server-side so the UI never
      cross-references `panel_library`.
- [x] `BRIDGE_VERSION` 12 → 13.
- [x] 6 runtime tests, 6 scene-graph tests, 3 frontend tests.

**Known gap, pinned by a test rather than hidden.** The start guard counts
*authored* keys, not resolved ones, so a Track whose only row cannot resolve still
starts an agent — one holding no locks and driving nothing. This is pre-existing
and shared with `Node(missing)` rows; refusing to start here would change node-row
behaviour as a side effect, so it was left alone and asserted instead. What the
test does pin is that an unresolved row locks *nothing* — not a neighbouring
element, not the panel — since one typo would otherwise block every Track sharing
that panel.

### B2 — element-specific actions — landed

- [x] `SetElementText`, `SetElementValue`, `SetElementEnabled`. All instant, so no
      scheduler change, and all added to `KNOWN_ACTION_KINDS` so an older build
      degrades them to `Unknown` rather than failing the document.
- [x] `SetElementText` finds `Text3d` **wherever it is** — self first, then one
      level of children — because a Label carries it directly while a Button keeps
      its caption on a child. Branching on widget kind would need editing for
      every future text-bearing widget. One level only, deliberately: a recursive
      descent could reach into a nested widget the author never addressed.
- [x] `SetElementValue` covers Slider *and* Toggle, with a Toggle as the
      degenerate scalar (non-zero is checked). One action means an author cannot
      pick the wrong one for the element they have. Clamped to the slider's
      authored range — mutating the clamp away fails a test.
- [x] `SetElementEnabled` is **present-but-dead, not hidden**, via a
      `XrdsWorldElementDisabled` marker. A marker rather than a field on each
      widget: the three interaction systems exclude it with one `Without<…>`
      filter, and the widget `*Params` structs describe *appearance* while this is
      runtime state. A test asserts visibility is untouched.
- [x] Editor: DTO both directions, `summarizeAction` shows the value (on a panel
      row the value *is* the point), and `actionUnavailableReasonFor` greys
      element actions on a node row **with a reason** rather than hiding them —
      the same principle the trigger pickers use. A test guards drift between the
      offered list and the set that gates it.
- [x] `BRIDGE_VERSION` 13 → 14.
- [x] 6 runtime tests, 4 frontend tests.

**Correction worth recording.** The disabled filter first went *only* where the
button system emits press events — so a directly-written event still woke a
disabled button, and the test that was supposed to prove otherwise passed for the
wrong reason. Fixed properly: `consume_triggers` now skips a disabled target too,
so "disabled" means the element does not act **whatever the event source**, not
merely that one emitter skips it.

Two tests also initially asserted only that a text component *existed* — true
before the Track ran, so removing the child search left them green. Both now
assert the content changed (via `Text3d`'s `Debug`, the only handle available),
and the mutation fails them.

### B3 — element rows in the Sequencer picker — landed

Closes the loop: a panel element is now addable as a Track row by clicking, so the
whole panel/trigger/Track story is authorable without touching the bridge.

- [x] `panel_instances` snapshot field — every placed Panel node with its
      template's element names and kinds. A whole-document summary for the same
      reason `all_node_bindings` is one: the picker needs *every* panel's elements,
      and the snapshot otherwise carries only the selected node's payload.
      `hierarchy` cannot serve it — a node's kind is there but not its
      `template_id`, so it cannot say which elements a Panel has.
- [x] Deliberately thinner than `PanelInstanceElementDto`: no wiring, no
      `emittable_triggers`. Those are per-selection detail, and computing them for
      every panel every frame would be work nothing reads.
- [x] A panel whose template is missing contributes an **empty element list rather
      than being omitted**, so a dangling reference shows as a panel with nothing
      to offer instead of looking like no panel at all.
- [x] One picker offers both kinds. `addableAssets` returns a tagged union and
      `encodeAddableAsset`/`decodeAddableAsset` carry it through the `<Select>`
      value, so the encoding lives in one tested place rather than being parsed
      inline at the call site.
- [x] Elements are excluded per `(panel, name)`, not by name — the same element
      name on a different panel is a different asset, which is the whole point of
      the addressing. Mutation-verified: name-only scoping fails exactly the
      "same name, different panel" test.
- [x] 10 frontend tests.

Two real bugs the tests caught, both in code I had just written:

- **`"node:"` decoded to node 0.** `Number("")` is `0`, not `NaN`, so a bare
  prefix resolved to a real id and the picker would have sent a command addressing
  whatever node that is. Now a strict digits-only parse.
- **Splitting the encoded value on the last colon** truncated element names
  containing one — which the naming policy permits — so the command would silently
  address a different element. Fixed to split on the first two only, with a test
  using `a:b:c`.

### Deliberately not in this plan

- [ ] ~~Live data binding~~ — see §1. Do not reach for a looping Track with
      `SetElementText` as a substitute; that is the pathological zero-duration
      loop.
- [ ] ~~Media surfaces / streaming~~ — see §1.
- [ ] ~~Action-level `XrdsAction::StopTrack`~~ — for stopping something partway
      through a choreography. The button case is covered by A5's binding-level
      effect without it, so not now.

*(Authorable stop is no longer deferred — it moved to A5.)*

## 10. Risks

1. **Breaking a working system.** The Track rework replaced something with no
   users; this one replaces HUD, which works and is wired through the editor.
   Mitigation: keep `set_hud_item` stable, and do A2 before touching the
   editor so the runtime path is proven first.
2. **Element identity churn.** Renaming an element must re-point, or at least
   diagnose, every binding and action target naming it. Renames are cheap in
   the editor and easy to do without noticing the fallout.
3. **The instance hazard (§5)** is silent until the second instance exists.
   The diagnostic is the mitigation and should not be deferred past A4.
4. **Scope creep toward data binding.** Deferred in §1 for a reason. If an
   element needs to reflect a live value, that is the deferred subsystem, not
   a quick `SetElementText` on a looping Track — which would reintroduce the
   pathological zero-duration loop.
