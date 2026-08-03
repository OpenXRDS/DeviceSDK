# Trigger-action sequencing — remaining plan

What is still **ahead**. The completed work (Phases 0-5, 7, 8, 9, 9a, 10) has
moved to [`done/xrds-trigger-action-v1.md`](done/xrds-trigger-action-v1.md);
the design rationale lives in
[`xrds-scenegraph-trigger-action-sequencing.md`](xrds-scenegraph-trigger-action-sequencing.md),
and unscheduled action variants in
[`xrds-trigger-action-backlog.md`](xrds-trigger-action-backlog.md).

Phase numbers are stable (code comments reference them), so they are
non-contiguous here — 0-5, 7, 8, 9, 9a and 10 are done and live in the
record doc. Only Phase 6 (editor integration) remains.

Two things raised during Phase 10's review — multiplayer authority and
`XrdsSceneNode::enabled` semantics — were deliberately scoped **out** of
trigger-action rather than left as open questions for it: multiplayer
authority isn't a trigger-action concern (it's a broader `xrds-net`
question, if and when that's taken up), and `enabled` needs a pass across
every node type, not just this one. See Phase 10 in
[`done/xrds-trigger-action-v1.md`](done/xrds-trigger-action-v1.md) for the
full reasoning behind both. Neither blocks Phase 6.

## Priority

Per the priority call recorded in the design doc: this is being built
**before** other new SDK components, as foundational plumbing other
planned features will hook into. Nothing here is blocked on unrelated
work.

## Non-goals (carried over from the design doc)

- No scripting language, no visual node-graph editor, no codegen.
- No general branching/conditional logic inside `Action` — that's the
  expert-layer escape hatch's job (`FireCustomEvent` → real Bevy system).
- No parallel execution via `bevy-sequential-actions` tuple-add — already
  confirmed sequential-only by the spike; genuine concurrency needs
  separate agents/queues (this is what the Phase 9 timeline scheduler is,
  built outside `bevy-sequential-actions` for exactly that reason).
- No editor UI designed here — tracked as Phase 6 below.

## Phase 6 — editor integration (tracked, not designed here)

- [ ] `xrds-editor` property-panel UI for authoring `XrdsTriggerBinding`/
      `XrdsSequence` on interaction zones. **List-based** (add/remove/
      reorder steps, pick trigger kind and action from dropdowns) — not
      a node-graph, per the "no Blueprint-shaped authoring surface"
      decision in the design doc. Deliberately not scoped in detail here.
      Phase 9a has now settled how bindings reference what they run
      (`runnable: Option<String>` through the document registry, falling
      back to inline `sequence`), so the data shape this UI would edit is
      finally stable.

      Two things this UI specifically needs, identified along the way: a
      **picker rather than free text** for `Custom(String)` trigger names
      and runnable names, since string matching means a typo silently never
      fires; and an **instance list** — a view enumerating bindings across
      all nodes, which is what makes the template/instance split usable
      (see Phase 10 in the record doc).
