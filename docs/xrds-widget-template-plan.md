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

- [ ] Attachment points — camera `panel_template_id`/`panel_depth`, scene
      `XrdsSceneNodePayload::Panel { template_id }`. Moved to **A2**, where the
      runtime that spawns them lands, so the schema and its consumer arrive
      together.

### A4b — retire the old vocabulary (after A2 and A3)

- [ ] Delete `XrdsHudTemplate`, `XrdsHudItemDef`, `HudItemDefId`; a HUD text
      item becomes a `Label` element.
- [ ] Retire `XrdsSceneWorldPanel::widgets` in favour of `template_id`.
- [ ] Delete `hud_library.rs` and the 12 HUD commands.
- [ ] Only once the runtime and editor are proven on the new types — this is
      the step risk #1 is about.

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

### A2c — camera attachment and HUD migration (next)

The half where risk #1 actually bites — everything above is additive and the
working HUD is untouched.

- [ ] Camera: `XrdsScenePlayerAnchor { panel_template_id, panel_depth }`, with
      `depth` on the *attachment* (§3), alongside `hud_template_id` at first.
- [ ] Spawn the camera path from a template (`reimport.rs:472` currently reads
      `hud_template`), reusing `spawn_panel_element_in_world`.
- [ ] Keep `set_hud_item(name, …)` working (`src/xrds_api/context.rs`) by
      resolving to a `Label` element of that name. Its contract is
      name-addressed and unification preserves name-addressing.
- [ ] Tests: `set_hud_item` still updates a migrated Label; an element's Track
      participates in the conflict guard.

### A2 — original scope, for reference
- [ ] Spawn elements from a template for **both** attachments: the world path
      in `import_runtime_nodes`, and the camera path in `reimport.rs:472`
      (which currently reads `hud_template`).
- [ ] Tag element entities with `XrdsTriggerBindings` from the element's own
      authored `triggers`, mirroring `tag_trigger_binding_entities`
      (`reimport.rs:536`) — **including its remove-when-empty behaviour**, so
      unchecking the last binding actually detaches the component.
- [ ] Confirm no change is needed in `consume_triggers` (§4). If one turns out
      to be needed, stop and re-read §4 — the analysis says otherwise.
- [ ] Keep `set_hud_item(name, …)` working (`src/xrds_api/context.rs`) by
      resolving to a `Label` element of that name. Its contract is
      name-addressed and unification preserves name-addressing.
- [ ] Tests:
  - [ ] pressing a button element fires the Track its binding names
  - [ ] an element with no triggers gets no `XrdsTriggerBindings`
  - [ ] removing the last binding detaches the component
  - [ ] `set_hud_item` still updates a migrated Label
  - [ ] a Track fired by an element participates in the conflict guard

### A3 — bridge + editor

- [ ] DTOs in `src-tauri/src/bridge.rs`: template, element, element kind;
      snapshot `panel_library` (generalizing `hud_library`).
- [ ] Replace the two command surfaces with one element-addressed set. Today:
      12 HUD commands (`CreateHudTemplate` … `LinkHudTemplate`) and 7 world
      commands (`SetWorldPanelParams`, `AddWorldPanelWidget`,
      `RemoveWorldPanelWidget`, `MoveWorldPanelWidget`, `SetWorldPanelWidget`,
      `SetWorldPanelWidgets`, `SetWorldPanelLayout`).
- [ ] Address elements by **name**, not index — `MoveWorldPanelWidget` reorders
      today, and an index-addressed command would silently re-point bindings.
- [ ] **Bump `BRIDGE_VERSION`** on both sides. It is 5 now.
- [ ] Generalize `HudLibraryPanel.tsx` into the template library.
- [ ] Converge `HudCanvasOverlay.tsx` and `WorldPanelCanvasOverlay.tsx` into
      one canvas; attachment differs, canvas editing does not.
- [ ] Element inspector reusing `TriggersSection` from `Inspector.tsx` — a
      binding is a binding.
- [ ] `validKindsFor` in `src/lib/sequencer.ts`: `ButtonPress` /
      `ButtonRelease` / `SliderChange` / `ToggleChange` become **available** on
      elements. They are hard-coded `false` today with a comment explaining
      they are unreachable — that comment is what this plan retires.
- [ ] Delete `src-tauri/src/hud_library.rs` once its commands are folded in.
- [ ] Tests: vitest for element-row labelling and kind availability.

### A4 — the instance hazard (§5)

- [ ] Diagnostic pass can see, per template, how many nodes instance it.
- [ ] Warn when a Track fired from a template-authored element trigger has a
      row targeting a fixed `Node(id)` while the template has >1 instance.
- [ ] Wording must **not** suggest `TriggerSource` as the fix — it resolves to
      the element, not its neighbours (§5). State the consequence only.
- [ ] Test: 1 instance → quiet; 2 instances + fixed row → warns; 2 instances +
      `TriggerSource` row → quiet (each drives its own element).

### A5 — authorable stop (§6b)

Unblocked: §6b settles stop as asset-keyed, reusing start's resolution.

- [ ] `XrdsTriggerEffect { Fire, Stop }` on `XrdsTriggerBinding`
      (`#[serde(default)]` = `Fire`, so existing documents are unchanged).
- [ ] Runtime: `consume_triggers` branches on the effect. Stop routes through
      `despawn_agents_releasing_locks` — the one choke point, or it leaks locks.
- [ ] Stop resolves its asset set via the **same** `schedule_track_keys` call
      start uses, then despawns the holders — never a name-keyed sweep. A
      name-keyed stop repeats the §5b mistake.
- [ ] Two bindings on one element (Stop X, then Fire Y) fire in authored order,
      which is what gives start/stop buttons their reset without a conditional.
- [ ] Diagnostic: a `Stop` binding naming a Track nothing ever fires.
- [ ] Editor: the Fires picker gains a Fire/Stop mode.
- [ ] Tests: stop releases locks so the Track can be re-fired; stop on a
      not-running Track is a harmless no-op; **stopping one instance's run
      leaves another instance's disjoint run alive**; two bindings on one
      element both fire, in order.
- [ ] `BRIDGE_VERSION` bump.

### B — elements as action targets (§6, separately stoppable)

- [ ] `XrdsActionTarget::Element { panel: XrdsSceneNodeId, name: String }`.
- [ ] **New index resource** mapping `(panel entity, element name) → element
      entity`. Nothing tracks widget entities today, so there is no lookup to
      reuse — closest precedent is `XrdsIdIndex`.
- [ ] Actions: `SetElementText`, `SetElementValue`, `SetElementEnabled`. All
      instant, so no scheduler change.
- [ ] Element entities participate in `XrdsTrackAssetLocks` — wanted, not
      incidental: two Tracks writing one element should conflict exactly as two
      Tracks writing one node do.
- [ ] Sequencer: element rows selectable as Track assets.
- [ ] Tests: targeting resolves to the right instance's element; two Tracks on
      one element conflict; a deleted element's target is diagnosed.

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
