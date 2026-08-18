//! Tests for the panel-template command surface — see
//! `docs/done/xrds-widget-template-plan.md` §A3.
//!
//! The property worth pinning is that elements are addressed **by name**:
//! reordering or renaming must never silently re-point something.

use crate::bridge::EditorCommand;
use crate::editor_state::{EditorSession, EditorState};
use crate::panel_library::{apply_panel_library_command, build_panel_library_dto};
use xrds_scene_graph::XrdsPanelTemplateId;

fn fresh() -> (EditorSession, EditorState) {
    let session = EditorSession(
        xrds_scene_graph::XrdsSceneDocumentSession::new(
            xrds_scene_graph::XrdsSceneDocument::default(),
        )
        .expect("an empty document is valid"),
    );
    (session, EditorState::default())
}

fn apply(session: &mut EditorSession, state: &mut EditorState, cmd: EditorCommand) -> bool {
    apply_panel_library_command(&cmd, session, state)
}

fn create(session: &mut EditorSession, state: &mut EditorState, name: &str) -> u64 {
    apply(session, state, EditorCommand::CreatePanelTemplate { name: name.to_string() });
    session.0.document().panels.last().map(|t| t.id.0).expect("template created")
}

#[test]
fn creating_a_template_assigns_a_fresh_id_and_no_elements() {
    let (mut session, mut state) = fresh();
    let id = create(&mut session, &mut state, "Menu");
    let dto = build_panel_library_dto(session.0.document());
    assert_eq!(dto.len(), 1);
    assert_eq!(dto[0].id, id);
    assert_eq!(dto[0].name, "Menu");
    assert!(dto[0].elements.is_empty());
}

#[test]
fn a_duplicate_template_name_is_refused() {
    let (mut session, mut state) = fresh();
    create(&mut session, &mut state, "Menu");
    create(&mut session, &mut state, "Menu");
    assert_eq!(session.0.document().panels.len(), 1, "the second must not be created");
}

#[test]
fn template_and_element_names_go_through_the_naming_policy() {
    // Same validator as Track names — one policy, not three near-copies.
    let (mut session, mut state) = fresh();
    apply(&mut session, &mut state, EditorCommand::CreatePanelTemplate { name: "  ".to_string() });
    assert!(session.0.document().panels.is_empty(), "whitespace-only refused");
    assert!(state.pending_status.is_some(), "and refused *visibly*");

    state.pending_status = None;
    apply(
        &mut session,
        &mut state,
        EditorCommand::CreatePanelTemplate { name: "__reserved".to_string() },
    );
    assert!(session.0.document().panels.is_empty(), "reserved prefix refused");
    assert!(state.pending_status.is_some());

    // Trimming applies, so a padded name becomes the canonical key.
    state.pending_status = None;
    create(&mut session, &mut state, "  Menu  ");
    assert_eq!(session.0.document().panels[0].name, "Menu");
}

#[test]
fn adding_elements_requires_a_known_kind_and_a_unique_name() {
    let (mut session, mut state) = fresh();
    let id = create(&mut session, &mut state, "Menu");

    for kind in ["Label", "Button", "Image", "Slider", "Toggle"] {
        apply(
            &mut session,
            &mut state,
            EditorCommand::AddPanelElement {
                template_id: id,
                kind: kind.to_string(),
                name: kind.to_lowercase(),
            },
        );
    }
    assert_eq!(session.0.document().panels[0].elements.len(), 5, "all five kinds");

    // Unknown kind: nothing added.
    apply(
        &mut session,
        &mut state,
        EditorCommand::AddPanelElement {
            template_id: id,
            kind: "Hologram".to_string(),
            name: "h".to_string(),
        },
    );
    assert_eq!(session.0.document().panels[0].elements.len(), 5);

    // Duplicate name: nothing added, because the name is the addressing key.
    apply(
        &mut session,
        &mut state,
        EditorCommand::AddPanelElement {
            template_id: id,
            kind: "Button".to_string(),
            name: "button".to_string(),
        },
    );
    assert_eq!(session.0.document().panels[0].elements.len(), 5, "duplicate name refused");
}

#[test]
fn elements_are_addressed_by_name_so_order_does_not_matter() {
    // The reason commands take a name and not an index: an index-addressed
    // command would let a reorder silently hit the wrong element.
    let (mut session, mut state) = fresh();
    let id = create(&mut session, &mut state, "Menu");
    for name in ["a", "b", "c"] {
        apply(
            &mut session,
            &mut state,
            EditorCommand::AddPanelElement {
                template_id: id,
                kind: "Button".to_string(),
                name: name.to_string(),
            },
        );
    }

    // Remove the middle one. If addressing were positional, "c" would now be at
    // the index "b" used to occupy.
    apply(
        &mut session,
        &mut state,
        EditorCommand::RemovePanelElement { template_id: id, name: "b".to_string() },
    );
    let names: Vec<String> =
        session.0.document().panels[0].elements.iter().map(|e| e.name.clone()).collect();
    assert_eq!(names, vec!["a".to_string(), "c".to_string()]);

    // And renaming still finds the right one after the shift.
    apply(
        &mut session,
        &mut state,
        EditorCommand::RenamePanelElement {
            template_id: id,
            name: "c".to_string(),
            new_name: "third".to_string(),
        },
    );
    let names: Vec<String> =
        session.0.document().panels[0].elements.iter().map(|e| e.name.clone()).collect();
    assert_eq!(names, vec!["a".to_string(), "third".to_string()]);
}

#[test]
fn renaming_an_element_onto_an_existing_name_is_refused() {
    let (mut session, mut state) = fresh();
    let id = create(&mut session, &mut state, "Menu");
    for name in ["a", "b"] {
        apply(
            &mut session,
            &mut state,
            EditorCommand::AddPanelElement {
                template_id: id,
                kind: "Button".to_string(),
                name: name.to_string(),
            },
        );
    }
    apply(
        &mut session,
        &mut state,
        EditorCommand::RenamePanelElement {
            template_id: id,
            name: "a".to_string(),
            new_name: "b".to_string(),
        },
    );
    let names: Vec<String> =
        session.0.document().panels[0].elements.iter().map(|e| e.name.clone()).collect();
    assert_eq!(names, vec!["a".to_string(), "b".to_string()], "collision refused, both intact");
}

#[test]
fn renaming_a_template_needs_no_reference_fixups() {
    // Instances reference a template by **id**, which is exactly why they store
    // an id rather than a name — unlike a Track binding, which stores a name and
    // therefore has to be re-pointed on rename.
    let (mut session, mut state) = fresh();
    let id = create(&mut session, &mut state, "Menu");
    apply(
        &mut session,
        &mut state,
        EditorCommand::RenamePanelTemplate { id, name: "MainMenu".to_string() },
    );
    let t = &session.0.document().panels[0];
    assert_eq!(t.name, "MainMenu");
    assert_eq!(t.id, XrdsPanelTemplateId(id), "the id an instance holds is unchanged");
}

#[test]
fn the_dto_reports_which_triggers_an_element_can_emit() {
    // Sent from Rust so the picker cannot drift from the diagnostics.
    let (mut session, mut state) = fresh();
    let id = create(&mut session, &mut state, "Menu");
    for (kind, name) in [("Button", "b"), ("Label", "l"), ("Slider", "s"), ("Toggle", "t")] {
        apply(
            &mut session,
            &mut state,
            EditorCommand::AddPanelElement {
                template_id: id,
                kind: kind.to_string(),
                name: name.to_string(),
            },
        );
    }
    let dto = build_panel_library_dto(session.0.document());
    let by_name = |n: &str| {
        dto[0].elements.iter().find(|e| e.name == n).expect("present").emittable_triggers.clone()
    };

    assert_eq!(by_name("b"), vec!["ButtonPress".to_string(), "ButtonRelease".to_string()]);
    assert_eq!(by_name("s"), vec!["SliderChange".to_string()]);
    assert_eq!(by_name("t"), vec!["ToggleChange".to_string()]);
    assert!(by_name("l").is_empty(), "a Label emits nothing, so the picker offers nothing");
}

#[test]
fn template_edits_request_a_reimport_but_creation_does_not() {
    // Instances spawn their elements at import time, so an element edit is only
    // visible after a respawn. Creating an empty template changes nothing live.
    let (mut session, mut state) = fresh();
    let id = create(&mut session, &mut state, "Menu");
    assert!(
        !apply(&mut session, &mut state, EditorCommand::CreatePanelTemplate { name: "B".into() }),
        "an empty new template needs no reimport"
    );
    assert!(
        apply(
            &mut session,
            &mut state,
            EditorCommand::AddPanelElement {
                template_id: id,
                kind: "Button".to_string(),
                name: "go".to_string(),
            },
        ),
        "adding an element must reimport so instances respawn with it"
    );
}


// ---------------------------------------------------------------------------
// Instance element trigger bindings (§A6)
//
// Bindings live on the placed Panel node, not the template, so every command
// here is addressed by node id. That is what lets two instances of one template
// drive two different targets.
// ---------------------------------------------------------------------------

/// A template with one Button element, plus a Panel node instancing it.
/// Returns `(session, state, template_id, node_id)`.
fn with_button(name: &str) -> (EditorSession, EditorState, u64, u64) {
    let (mut session, mut state) = fresh();
    let template_id = create(&mut session, &mut state, "Menu");
    apply(
        &mut session,
        &mut state,
        EditorCommand::AddPanelElement {
            template_id,
            kind: "Button".to_string(),
            name: name.to_string(),
        },
    );
    spawn(&mut session, &mut state, "Panel");
    let node_id = session
        .0
        .document()
        .nodes
        .iter()
        .find(|n| matches!(n.payload, xrds_scene_graph::XrdsSceneNodePayload::Panel(_)))
        .expect("panel node")
        .id
        .0;
    (session, state, template_id, node_id)
}

fn wiring_of(
    session: &EditorSession,
    node_id: u64,
    element: &str,
) -> Vec<xrds_scene_graph::XrdsTriggerBinding> {
    let node = session
        .0
        .document()
        .node(xrds_scene_graph::XrdsSceneNodeId(node_id))
        .expect("node present");
    match &node.payload {
        xrds_scene_graph::XrdsSceneNodePayload::Panel(i) => i.triggers_for(element).to_vec(),
        _ => panic!("not a Panel node"),
    }
}

#[test]
fn a_new_binding_defaults_to_a_kind_the_element_can_emit() {
    // Seeding every element with ButtonPress would make a slider's first binding
    // silently inert. The default is resolved through the node's *template*,
    // since only the template knows the element's kind.
    use xrds_scene_graph::XrdsTriggerKind as K;
    let (mut session, mut state) = fresh();
    let template_id = create(&mut session, &mut state, "Menu");
    for (kind, name) in [("Button", "b"), ("Slider", "s"), ("Toggle", "t")] {
        apply(&mut session, &mut state, EditorCommand::AddPanelElement {
            template_id,
            kind: kind.to_string(),
            name: name.to_string(),
        });
    }
    spawn(&mut session, &mut state, "Panel");
    let node_id = session
        .0
        .document()
        .nodes
        .iter()
        .find(|n| matches!(n.payload, xrds_scene_graph::XrdsSceneNodePayload::Panel(_)))
        .expect("panel node")
        .id
        .0;

    for (name, expected) in
        [("b", K::ButtonPress), ("s", K::SliderChange), ("t", K::ToggleChange)]
    {
        apply(&mut session, &mut state, EditorCommand::AddPanelNodeTrigger {
            id: node_id,
            element: name.to_string(),
        });
        assert_eq!(wiring_of(&session, node_id, name)[0].trigger, expected, "{name} default");
    }
}

#[test]
fn bindings_can_be_added_edited_and_removed_on_a_node() {
    use xrds_scene_graph::XrdsTriggerKind as K;
    let (mut session, mut state, _t, node) = with_button("go");

    apply(&mut session, &mut state, EditorCommand::AddPanelNodeTrigger {
        id: node, element: "go".to_string(),
    });
    assert_eq!(wiring_of(&session, node, "go").len(), 1);

    apply(&mut session, &mut state, EditorCommand::SetPanelNodeTriggerTrack {
        id: node, element: "go".to_string(), index: 0, track: Some("Open".to_string()),
    });
    apply(&mut session, &mut state, EditorCommand::SetPanelNodeTriggerDisabled {
        id: node, element: "go".to_string(), index: 0, disabled: true,
    });
    apply(&mut session, &mut state, EditorCommand::SetPanelNodeTriggerHand {
        id: node, element: "go".to_string(), index: 0, hand: Some("Left".to_string()),
    });

    let b = &wiring_of(&session, node, "go")[0];
    assert_eq!(b.track.as_deref(), Some("Open"));
    assert!(b.disabled);
    assert!(b.hand.is_some());
    assert_eq!(b.trigger, K::ButtonPress);

    apply(&mut session, &mut state, EditorCommand::RemovePanelNodeTrigger {
        id: node, element: "go".to_string(), index: 0,
    });
    assert!(wiring_of(&session, node, "go").is_empty());
}

#[test]
fn removing_the_last_binding_drops_the_key_rather_than_leaving_an_empty_list() {
    // An empty entry reads like wiring in the saved document and is not, and it
    // would make the orphaned-key diagnostic fire on something already cleared.
    let (mut session, mut state, _t, node) = with_button("go");
    apply(&mut session, &mut state, EditorCommand::AddPanelNodeTrigger {
        id: node, element: "go".to_string(),
    });
    apply(&mut session, &mut state, EditorCommand::RemovePanelNodeTrigger {
        id: node, element: "go".to_string(), index: 0,
    });

    let node_ref = session
        .0
        .document()
        .node(xrds_scene_graph::XrdsSceneNodeId(node))
        .expect("node");
    match &node_ref.payload {
        xrds_scene_graph::XrdsSceneNodePayload::Panel(i) => {
            assert!(i.element_triggers.is_empty(), "{:?}", i.element_triggers)
        }
        _ => panic!("not a Panel"),
    }
}

#[test]
fn two_instances_of_one_template_wire_independently() {
    // The capability the whole model change is for. Under template-owned
    // bindings this was impossible: both nodes shared one list.
    let (mut session, mut state, _t, first) = with_button("go");
    spawn(&mut session, &mut state, "Panel");
    let second = session
        .0
        .document()
        .nodes
        .iter()
        .filter(|n| matches!(n.payload, xrds_scene_graph::XrdsSceneNodePayload::Panel(_)))
        .map(|n| n.id.0)
        .find(|id| *id != first)
        .expect("second panel node");

    for (node, track) in [(first, "OpenA"), (second, "OpenB")] {
        apply(&mut session, &mut state, EditorCommand::AddPanelNodeTrigger {
            id: node, element: "go".to_string(),
        });
        apply(&mut session, &mut state, EditorCommand::SetPanelNodeTriggerTrack {
            id: node, element: "go".to_string(), index: 0, track: Some(track.to_string()),
        });
    }

    assert_eq!(wiring_of(&session, first, "go")[0].track.as_deref(), Some("OpenA"));
    assert_eq!(wiring_of(&session, second, "go")[0].track.as_deref(), Some("OpenB"));
}

#[test]
fn an_out_of_range_binding_index_is_ignored_rather_than_panicking() {
    // A stale frontend can send one; the next snapshot corrects it.
    let (mut session, mut state, _t, node) = with_button("go");
    apply(&mut session, &mut state, EditorCommand::RemovePanelNodeTrigger {
        id: node, element: "go".to_string(), index: 9,
    });
    apply(&mut session, &mut state, EditorCommand::SetPanelNodeTriggerDisabled {
        id: node, element: "go".to_string(), index: 9, disabled: true,
    });
    assert!(wiring_of(&session, node, "go").is_empty(), "no crash, no phantom binding");
}

#[test]
fn a_command_naming_a_node_that_is_not_a_panel_is_ignored() {
    let (mut session, mut state, _t, node) = with_button("go");
    apply(&mut session, &mut state, EditorCommand::AddPanelNodeTrigger {
        id: 99999, element: "go".to_string(),
    });
    assert!(wiring_of(&session, node, "go").is_empty());
}

#[test]
fn wiring_an_element_the_template_lacks_is_stored_not_refused() {
    // Deliberate: this is the shape a deleted element leaves behind, and
    // refusing it would mean an author could never repoint recovered wiring.
    // `panel_diagnostics` reports it instead.
    let (mut session, mut state, _t, node) = with_button("go");
    apply(&mut session, &mut state, EditorCommand::AddPanelNodeTrigger {
        id: node, element: "ghost".to_string(),
    });
    assert_eq!(wiring_of(&session, node, "ghost").len(), 1);
    let titles: Vec<String> =
        session.0.document().panel_diagnostics().into_iter().map(|d| d.title).collect();
    assert!(
        titles.iter().any(|t| t == "Binding names an element the template does not have"),
        "{titles:?}"
    );
}

#[test]
fn renaming_a_template_element_moves_every_instances_wiring() {
    // Renames propagate because the intent is unambiguous — the element still
    // exists and is still the thing that was wired. Without this, renaming an
    // element in a 20-instance template would silently break 20 panels.
    let (mut session, mut state, template_id, first) = with_button("go");
    spawn(&mut session, &mut state, "Panel");
    let second = session
        .0
        .document()
        .nodes
        .iter()
        .filter(|n| matches!(n.payload, xrds_scene_graph::XrdsSceneNodePayload::Panel(_)))
        .map(|n| n.id.0)
        .find(|id| *id != first)
        .expect("second panel node");

    for node in [first, second] {
        apply(&mut session, &mut state, EditorCommand::AddPanelNodeTrigger {
            id: node, element: "go".to_string(),
        });
        apply(&mut session, &mut state, EditorCommand::SetPanelNodeTriggerTrack {
            id: node, element: "go".to_string(), index: 0, track: Some("Open".to_string()),
        });
    }

    apply(&mut session, &mut state, EditorCommand::RenamePanelElement {
        template_id, name: "go".to_string(), new_name: "start".to_string(),
    });

    for node in [first, second] {
        assert!(wiring_of(&session, node, "go").is_empty(), "old key must be gone");
        let moved = wiring_of(&session, node, "start");
        assert_eq!(moved.len(), 1, "the binding moved with its element");
        assert_eq!(moved[0].track.as_deref(), Some("Open"));
    }
    // Nothing is left dangling. The bindings name no Track, which is its own
    // (expected) warning, so this checks only for orphaned keys.
    let titles: Vec<String> =
        session.0.document().panel_diagnostics().into_iter().map(|d| d.title).collect();
    assert!(
        !titles.iter().any(|t| t.contains("does not have")),
        "no orphaned wiring after a rename: {titles:?}"
    );
}

#[test]
fn a_refused_rename_does_not_move_any_wiring() {
    // Renaming onto an existing name is refused; the propagation must not run
    // anyway, or the document ends up with wiring for a name nothing has.
    let (mut session, mut state, template_id, node) = with_button("go");
    apply(&mut session, &mut state, EditorCommand::AddPanelElement {
        template_id, kind: "Button".to_string(), name: "taken".to_string(),
    });
    apply(&mut session, &mut state, EditorCommand::AddPanelNodeTrigger {
        id: node, element: "go".to_string(),
    });

    apply(&mut session, &mut state, EditorCommand::RenamePanelElement {
        template_id, name: "go".to_string(), new_name: "taken".to_string(),
    });

    assert_eq!(wiring_of(&session, node, "go").len(), 1, "wiring must stay put");
    assert!(wiring_of(&session, node, "taken").is_empty());
}

// ---------------------------------------------------------------------------
// Head-locked panel placement
// ---------------------------------------------------------------------------

/// Adds a `PlayerAnchor` node and returns its id.
fn add_anchor(session: &mut EditorSession) -> u64 {
    session
        .0
        .edit(|doc| {
            doc.nodes.push(xrds_scene_graph::XrdsSceneNode {
                id: xrds_scene_graph::XrdsSceneNodeId(500),
                parent_id: None,
                name: "Anchor".to_string(),
                enabled: true,
                visible: true,
                grabbable: false,
                transform: Default::default(),
                payload: xrds_scene_graph::XrdsSceneNodePayload::PlayerAnchor(
                    Default::default(),
                ),
                editor: Default::default(),
                triggers: Vec::new(),
                watchers: Vec::new(),
            });
        })
        .expect("adding an anchor is a valid edit");
    500
}

// ---------------------------------------------------------------------------
// Scene-placed Panel nodes (§A4b-2a)
// ---------------------------------------------------------------------------

fn spawn(session: &mut EditorSession, state: &mut EditorState, kind: &str) -> bool {
    crate::palette::apply_palette_command(
        &EditorCommand::SpawnPrimitive { kind: kind.to_string(), parent_id: None },
        session,
        state,
    )
}

fn panel_nodes(session: &EditorSession) -> Vec<u64> {
    session
        .0
        .document()
        .nodes
        .iter()
        .filter_map(|n| match &n.payload {
            xrds_scene_graph::XrdsSceneNodePayload::Panel(i) => Some(i.template_id.0),
            _ => None,
        })
        .collect()
}

#[test]
fn spawning_a_panel_into_an_empty_library_creates_a_starter_template() {
    // Refusing the spawn would make the palette entry look broken to anyone who
    // has not opened the Panels workspace yet, so it bootstraps instead.
    let (mut session, mut state) = fresh();
    assert!(session.0.document().panels.is_empty(), "precondition");

    spawn(&mut session, &mut state, "Panel");

    assert_eq!(session.0.document().panels.len(), 1, "a template must be created");
    let tid = session.0.document().panels[0].id.0;
    assert_eq!(panel_nodes(&session), vec![tid], "the node must point at it");
}

#[test]
fn spawning_a_panel_reuses_an_existing_template_rather_than_multiplying_them() {
    // Otherwise placing four panels leaves four near-identical library entries.
    let (mut session, mut state) = fresh();
    let tid = create(&mut session, &mut state, "Console");

    spawn(&mut session, &mut state, "Panel");
    spawn(&mut session, &mut state, "Panel");

    assert_eq!(session.0.document().panels.len(), 1, "no extra templates");
    assert_eq!(panel_nodes(&session), vec![tid, tid]);
}

#[test]
fn a_spawned_panel_is_diagnostic_clean() {
    // The bootstrap path must not produce a document that immediately warns —
    // in particular the starter template's name has to pass the naming policy.
    let (mut session, mut state) = fresh();
    spawn(&mut session, &mut state, "Panel");
    let d = session.0.document().panel_diagnostics();
    assert!(d.is_empty(), "{:?}", d.iter().map(|x| &x.title).collect::<Vec<_>>());
}

#[test]
fn repointing_a_panel_instance_switches_template() {
    let (mut session, mut state) = fresh();
    let first = create(&mut session, &mut state, "One");
    let second = create(&mut session, &mut state, "Two");
    spawn(&mut session, &mut state, "Panel");
    assert_eq!(panel_nodes(&session), vec![first]);

    let node_id = session
        .0
        .document()
        .nodes
        .iter()
        .find(|n| matches!(n.payload, xrds_scene_graph::XrdsSceneNodePayload::Panel(_)))
        .expect("panel node")
        .id
        .0;

    let reimport = apply(&mut session, &mut state, EditorCommand::SetPanelInstanceTemplate {
        id: node_id,
        template_id: second,
    });
    assert!(reimport, "instances spawn at import, so this must reimport");
    assert_eq!(panel_nodes(&session), vec![second]);
}

#[test]
fn repointing_at_a_missing_template_is_refused() {
    // A dangling instance spawns nothing at all with no visible cause.
    let (mut session, mut state) = fresh();
    let first = create(&mut session, &mut state, "One");
    spawn(&mut session, &mut state, "Panel");

    let node_id = session
        .0
        .document()
        .nodes
        .iter()
        .find(|n| matches!(n.payload, xrds_scene_graph::XrdsSceneNodePayload::Panel(_)))
        .expect("panel node")
        .id
        .0;

    let reimport = apply(&mut session, &mut state, EditorCommand::SetPanelInstanceTemplate {
        id: node_id, template_id: 404,
    });
    assert!(!reimport);
    assert_eq!(panel_nodes(&session), vec![first], "must be left alone");
}

#[test]
fn deleting_a_template_leaves_panel_nodes_dangling_and_diagnosed() {
    // Deliberate: a Panel node's whole payload is the reference, so there is no
    // empty state to fall back to, and silently deleting the author's node is
    // worse than a reported dangle. Contrast anchors, whose link is cleared.
    let (mut session, mut state) = fresh();
    let tid = create(&mut session, &mut state, "Doomed");
    spawn(&mut session, &mut state, "Panel");

    apply(&mut session, &mut state, EditorCommand::DeletePanelTemplate { id: tid });

    assert_eq!(panel_nodes(&session), vec![tid], "the node survives, still pointing at it");
    let titles: Vec<String> = session
        .0
        .document()
        .panel_diagnostics()
        .into_iter()
        .map(|d| d.title)
        .collect();
    assert!(
        titles.iter().any(|t| t == "Panel instance names a missing template"),
        "{titles:?}"
    );
}

// ---------------------------------------------------------------------------
// Head-locked placement by parenting (§A6)
// ---------------------------------------------------------------------------

fn spawn_under(
    session: &mut EditorSession,
    state: &mut EditorState,
    kind: &str,
    parent: u64,
) -> bool {
    crate::palette::apply_palette_command(
        &EditorCommand::SpawnPrimitive { kind: kind.to_string(), parent_id: Some(parent) },
        session,
        state,
    )
}

fn transform_of(session: &EditorSession, id: u64) -> [f32; 3] {
    session
        .0
        .document()
        .node(xrds_scene_graph::XrdsSceneNodeId(id))
        .expect("node present")
        .transform
        .translation
}

#[test]
fn a_panel_under_an_anchor_defaults_to_camera_local_placement() {
    // A head-locked panel's transform is read as **camera-local**, so the
    // world-space default (eye height, 1 m forward) would put it 1.5 m above the
    // viewer's own eye. 1.5 m straight ahead instead — see `palette.rs` for why
    // that value and not the panel's own backdrop size.
    let (mut session, mut state) = fresh();
    create(&mut session, &mut state, "Hud");
    let anchor = add_anchor(&mut session);

    spawn_under(&mut session, &mut state, "Panel", anchor);
    let panel = session
        .0
        .document()
        .nodes
        .iter()
        .find(|n| matches!(n.payload, xrds_scene_graph::XrdsSceneNodePayload::Panel(_)))
        .expect("panel node")
        .id
        .0;

    assert_eq!(transform_of(&session, panel), [0.0, 0.0, -1.5]);
}

#[test]
fn a_panel_at_the_scene_root_keeps_its_world_space_placement() {
    // The other half of the same rule: without an anchor ancestor the transform is
    // world-space, so eye height and a metre forward is the useful default.
    let (mut session, mut state) = fresh();
    create(&mut session, &mut state, "Wall");
    spawn(&mut session, &mut state, "Panel");
    let panel = session
        .0
        .document()
        .nodes
        .iter()
        .find(|n| matches!(n.payload, xrds_scene_graph::XrdsSceneNodePayload::Panel(_)))
        .expect("panel node")
        .id
        .0;

    assert_eq!(transform_of(&session, panel), [0.0, 1.5, -1.0]);
}

#[test]
fn head_locked_placement_looks_through_intermediate_parents() {
    // Attachment is decided by the whole ancestor chain, not just the immediate
    // parent — grouping a panel under an Empty beneath an anchor must not silently
    // turn it back into a world panel.
    let (mut session, mut state) = fresh();
    create(&mut session, &mut state, "Hud");
    let anchor = add_anchor(&mut session);
    spawn_under(&mut session, &mut state, "Empty", anchor);
    let group = session
        .0
        .document()
        .nodes
        .iter()
        .find(|n| n.name == "Empty")
        .expect("group node")
        .id
        .0;

    spawn_under(&mut session, &mut state, "Panel", group);
    let panel = session
        .0
        .document()
        .nodes
        .iter()
        .find(|n| matches!(n.payload, xrds_scene_graph::XrdsSceneNodePayload::Panel(_)))
        .expect("panel node")
        .id
        .0;

    assert_eq!(transform_of(&session, panel), [0.0, 0.0, -1.5]);
}
