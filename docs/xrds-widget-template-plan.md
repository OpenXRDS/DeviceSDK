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
**the same asset regardless of which instance was pressed**. Only
`TriggerSource`-relative rows behave per-instance.

This is the bug class that works perfectly while there is one instance and
breaks silently on the second. It needs an author-time diagnostic:

> *This Track is fired from template "MainMenu", which has 3 instances, but
> targets fixed node "door_left". Every instance will drive that same node —
> use a Trigger-source row for per-instance behaviour.*

Severity: warning, not error. It is legitimate when the target really is
global (one shared door, many call buttons).

Note this hazard is about **targeting, not timing**. One press is enough to
show it: press the third-floor button and the ground-floor door opens. The
timing problem is separate, and worse — next section.

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

## 9. Phasing

Each phase ends with the tree building and tests passing.

- **A1 — schema.** `XrdsPanelTemplate`/`XrdsPanelElement`, the registry, both
  attachment points, `depth` moved. Diagnostics: duplicate element name,
  dangling template reference, element trigger on a non-interactive kind.
- **A2 — runtime.** Spawn elements from templates for both attachments; tag
  element entities with `XrdsTriggerBindings`. Keep `set_hud_item` working.
- **A3 — bridge + editor.** Generalize the library panel and the two canvases
  into one workspace; element inspector with triggers. Bump `BRIDGE_VERSION`.
- **A4 — the instance-hazard diagnostic** (§5), once instance counts are
  known to the diagnostic pass.
- **B — elements as action targets** (§6). Separately stoppable.

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
