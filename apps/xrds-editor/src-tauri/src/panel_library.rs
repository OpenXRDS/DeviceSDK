//! Editor bridge for panel templates — the unified model behind HUD panels and
//! world-space panels. See `docs/xrds-widget-template-plan.md` §A3.
//!
//! Took the shape of the now-deleted `hud_library.rs` (snapshot serializer +
//! command dispatcher) and has replaced it outright: a HUD is a panel template
//! head-locked to an anchor, so its 12 commands collapsed into these.
//!
//! **Elements are addressed by name, never by index.** `MoveWorldPanelWidget`
//! reorders by index today, and an index-addressed element command would let a
//! reorder silently re-point a trigger binding — the same class of invisible
//! breakage the naming policy exists to prevent.

use bevy::log::error;
use xrds_scene_graph::{
    XrdsPanelElement, XrdsPanelTemplate, XrdsPanelTemplateId, XrdsSceneDocument,
    XrdsSceneNodeId, XrdsSceneNodePayload, XrdsSceneWorldWidget, XrdsTriggerBinding,
};

use crate::bridge::{EditorCommand, PanelElementDto, PanelTemplateDto, TriggerDiagnosticDto};
use crate::editor_state::{EditorSession, EditorState};
use crate::inspector::{world_layout_dto, world_widget_dto};

// ---------------------------------------------------------------------------
// Snapshot serializers
// ---------------------------------------------------------------------------

pub fn build_panel_library_dto(doc: &XrdsSceneDocument) -> Vec<PanelTemplateDto> {
    doc.panels
        .iter()
        .map(|t| PanelTemplateDto {
            id: t.id.0,
            name: t.name.clone(),
            size: t.size,
            color: t.background.color,
            corner_radius: t.background.corner_radius,
            opacity: t.background.opacity,
            layout: world_layout_dto(&t.layout),
            elements: t.elements.iter().map(element_dto).collect(),
        })
        .collect()
}

fn element_dto(e: &XrdsPanelElement) -> PanelElementDto {
    PanelElementDto {
        name: e.name.clone(),
        widget: world_widget_dto(&e.kind),
        // Resolved here rather than re-derived in TypeScript: which kinds an
        // element can emit is a runtime fact, and a second copy of the rule
        // would drift from the diagnostics that use the Rust one.
        emittable_triggers: emittable_trigger_names(e),
    }
}

/// Every element of a placed Panel node, template definition joined with this
/// instance's wiring.
///
/// Template order first so the list matches the Panels workspace, then any
/// orphaned keys appended — a binding whose element was deleted stays visible and
/// repointable instead of disappearing from the UI while remaining in the file.
pub fn build_panel_instance_elements_dto(
    doc: &XrdsSceneDocument,
    instance: &xrds_scene_graph::XrdsScenePanelInstance,
) -> Vec<crate::bridge::PanelInstanceElementDto> {
    use crate::bridge::PanelInstanceElementDto;

    let template = doc.panel_template(instance.template_id);
    let mut out: Vec<PanelInstanceElementDto> = Vec::new();

    if let Some(t) = template {
        for e in &t.elements {
            out.push(PanelInstanceElementDto {
                name: e.name.clone(),
                kind: e.kind_name().to_string(),
                emittable_triggers: emittable_trigger_names(e),
                triggers: crate::trigger_action::build_node_triggers_dto(
                    instance.triggers_for(&e.name),
                ),
                orphaned: false,
            });
        }
    }

    for (name, bindings) in &instance.element_triggers {
        let known = template.map_or(false, |t| t.element(name).is_some());
        if known {
            continue;
        }
        out.push(PanelInstanceElementDto {
            name: name.clone(),
            kind: "missing".to_string(),
            // Nothing is emittable by an element that does not exist, so the UI
            // offers no kinds rather than pretending this is fixable in place.
            emittable_triggers: Vec::new(),
            triggers: crate::trigger_action::build_node_triggers_dto(bindings),
            orphaned: true,
        });
    }

    out
}

/// Every placed `Panel` node with the elements its template defines.
///
/// Nodes whose template is missing contribute an **empty element list rather than
/// being omitted**: the Sequencer's picker then shows the panel with nothing to
/// offer, which is honest, where dropping the row entirely would make a dangling
/// reference look like no panel at all.
pub fn build_panel_instances_dto(
    doc: &XrdsSceneDocument,
) -> Vec<crate::bridge::PanelInstanceSummaryDto> {
    use crate::bridge::{PanelElementRefDto, PanelInstanceSummaryDto};

    doc.nodes
        .iter()
        .filter_map(|node| {
            let XrdsSceneNodePayload::Panel(instance) = &node.payload else { return None };
            let elements = doc
                .panel_template(instance.template_id)
                .map(|t| {
                    t.elements
                        .iter()
                        .map(|e| PanelElementRefDto {
                            name: e.name.clone(),
                            kind: e.kind_name().to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(PanelInstanceSummaryDto {
                node_id: node.id.0,
                node_name: node.name.clone(),
                elements,
            })
        })
        .collect()
}

/// Trigger kinds this element can actually emit, as wire strings.
fn emittable_trigger_names(e: &XrdsPanelElement) -> Vec<String> {
    use xrds_scene_graph::XrdsTriggerKind as K;
    [
        K::ButtonPress,
        K::ButtonRelease,
        K::SliderChange,
        K::ToggleChange,
    ]
    .into_iter()
    .filter(|k| e.can_emit(k))
    .map(|k| format!("{k:?}"))
    .collect()
}

pub fn build_panel_diagnostics_dto(doc: &XrdsSceneDocument) -> Vec<TriggerDiagnosticDto> {
    doc.panel_diagnostics().iter().map(crate::trigger_action::diagnostic_to_dto).collect()
}

// ---------------------------------------------------------------------------
// Command handlers
// ---------------------------------------------------------------------------

/// Returns true if a full scene reimport is needed after the command.
///
/// Any change to a template must reimport: instances spawn their elements at
/// import time, so an edit is only visible once they respawn. Cheaper
/// alternatives (patching live element entities in place) would need to
/// re-resolve trigger bindings and re-parent, which is exactly what import
/// already does correctly.
pub fn apply_panel_library_command(
    cmd: &EditorCommand,
    session: &mut EditorSession,
    state: &mut EditorState,
) -> bool {
    match cmd {
        EditorCommand::CreatePanelTemplate { name } => {
            let Some(name) = crate::trigger_action::reject_bad_name(state, name, "Panel") else {
                return false;
            };
            match session.0.edit(|doc| {
                if doc.panel_template_by_name(&name).is_some() {
                    error!("[panel] CreatePanelTemplate: {name:?} already exists");
                    return;
                }
                let id = doc.next_available_panel_template_id();
                doc.panels.push(XrdsPanelTemplate { id, name, ..XrdsPanelTemplate::default() });
            }) {
                Ok(_) => {}
                Err(e) => error!("[panel] CreatePanelTemplate failed: {:?}", e),
            }
            false
        }

        EditorCommand::RenamePanelTemplate { id, name } => {
            let Some(name) = crate::trigger_action::reject_bad_name(state, name, "Panel") else {
                return false;
            };
            let id = XrdsPanelTemplateId(*id);
            match session.0.edit(|doc| {
                if doc.panels.iter().any(|t| t.id != id && t.name == name) {
                    error!("[panel] RenamePanelTemplate: {name:?} already exists");
                    return;
                }
                if let Some(t) = doc.panel_template_mut(id) {
                    t.name = name;
                }
                // No reference re-pointing needed: instances reference a
                // template by *id*, which is exactly why they store the id
                // rather than the name.
            }) {
                Ok(_) => {}
                Err(e) => error!("[panel] RenamePanelTemplate failed: {:?}", e),
            }
            false
        }

        EditorCommand::DeletePanelTemplate { id } => {
            let id = XrdsPanelTemplateId(*id);
            // `Panel` nodes referencing this template are deliberately left
            // dangling. Their whole payload is the template reference — there is
            // no empty state to fall back to, so the alternatives are deleting the
            // author's node outright or leaving a diagnosable reference.
            // `panel_diagnostics` reports it as "Panel instance names a missing
            // template"; silently deleting scene nodes is worse.
            match session.0.edit(|doc| {
                doc.panels.retain(|t| t.id != id);
            }) {
                Ok(_) => {}
                Err(e) => error!("[panel] DeletePanelTemplate failed: {:?}", e),
            }
            true
        }

        EditorCommand::SetPanelTemplateParams { id, size, color, corner_radius, opacity } => {
            let id = XrdsPanelTemplateId(*id);
            let (size, color, corner_radius, opacity) = (*size, *color, *corner_radius, *opacity);
            match session.0.edit(|doc| {
                if let Some(t) = doc.panel_template_mut(id) {
                    t.size = size;
                    t.background.color = color;
                    t.background.corner_radius = corner_radius;
                    t.background.opacity = opacity;
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[panel] SetPanelTemplateParams failed: {:?}", e),
            }
            true
        }

        EditorCommand::AddPanelElement { template_id, kind, name } => {
            let Some(name) = crate::trigger_action::reject_bad_name(state, name, "Element") else {
                return false;
            };
            let id = XrdsPanelTemplateId(*template_id);
            let Some(widget) = default_widget_for_kind(kind) else {
                error!("[panel] AddPanelElement: unknown kind {kind:?}");
                return false;
            };
            match session.0.edit(|doc| {
                let Some(t) = doc.panel_template_mut(id) else { return };
                if t.element(&name).is_some() {
                    error!("[panel] AddPanelElement: {name:?} already exists in this template");
                    return;
                }
                t.elements.push(XrdsPanelElement::new(name, widget));
            }) {
                Ok(_) => {}
                Err(e) => error!("[panel] AddPanelElement failed: {:?}", e),
            }
            true
        }

        EditorCommand::RemovePanelElement { template_id, name } => {
            let id = XrdsPanelTemplateId(*template_id);
            let name = name.clone();
            match session.0.edit(|doc| {
                if let Some(t) = doc.panel_template_mut(id) {
                    t.elements.retain(|e| e.name != name);
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[panel] RemovePanelElement failed: {:?}", e),
            }
            true
        }

        EditorCommand::RenamePanelElement { template_id, name, new_name } => {
            let Some(new_name) =
                crate::trigger_action::reject_bad_name(state, new_name, "Element")
            else {
                return false;
            };
            let id = XrdsPanelTemplateId(*template_id);
            let name = name.clone();
            match session.0.edit(|doc| {
                let Some(t) = doc.panel_template_mut(id) else { return };
                if t.elements.iter().any(|e| e.name != name && e.name == new_name) {
                    error!("[panel] RenamePanelElement: {new_name:?} already exists");
                    return;
                }
                if t.element(&name).is_none() {
                    return;
                }
                if let Some(e) = t.element_mut(&name) {
                    e.name = new_name.clone();
                }
                // **Rename propagates to every instance's wiring.** The element
                // still exists and is still the thing that was wired, so the
                // intent is unambiguous — leaving the old key would silently break
                // every placed panel. Contrast a *delete*, which is diagnosed
                // rather than propagated, because there is no obvious new target
                // and dropping the bindings would discard authored work.
                for node in &mut doc.nodes {
                    if let XrdsSceneNodePayload::Panel(ref mut i) = node.payload {
                        if i.template_id == id {
                            i.rename_element(&name, new_name.clone());
                        }
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[panel] RenamePanelElement failed: {:?}", e),
            }
            true
        }

        EditorCommand::SetPanelElementWidget { template_id, name, widget } => {
            let id = XrdsPanelTemplateId(*template_id);
            let name = name.clone();
            let widget = crate::inspector::world_widget_from_dto(widget);
            match session.0.edit(|doc| {
                let Some(t) = doc.panel_template_mut(id) else { return };
                if let Some(e) = t.element_mut(&name) {
                    e.kind = widget;
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[panel] SetPanelElementWidget failed: {:?}", e),
            }
            true
        }

        // --- Instance element trigger bindings ---
        //
        // Addressed by `(Panel node id, element name)`, **not** by template: the
        // bindings live on the placed node so two instances of one template can
        // drive two different targets. That is the whole point of the model, and
        // it is why these six replaced the template-scoped versions.
        EditorCommand::AddPanelNodeTrigger { id, element } => {
            // The default kind is resolved from the *template's* element, since
            // that is what knows whether this is a Button or a Label. A freshly
            // added binding is therefore never inert unless the element genuinely
            // emits nothing, which `panel_diagnostics` then reports.
            let kind = element_default_kind(session, *id, element);
            with_instance(session, *id, element, |bindings| {
                bindings.push(XrdsTriggerBinding {
                    trigger: kind,
                    track: None,
                    effect: Default::default(),
                    disabled: false,
                    hand: None,
                });
            });
            true
        }

        EditorCommand::RemovePanelNodeTrigger { id, element, index } => {
            let index = *index;
            with_instance(session, *id, element, |bindings| {
                if index < bindings.len() {
                    bindings.remove(index);
                }
            });
            true
        }

        EditorCommand::SetPanelNodeTriggerKind { id, element, index, trigger } => {
            let index = *index;
            let kind = crate::trigger_action::trigger_kind_from_dto(trigger);
            with_instance(session, *id, element, |bindings| {
                if let Some(b) = bindings.get_mut(index) {
                    b.trigger = kind;
                }
            });
            true
        }

        EditorCommand::SetPanelNodeTriggerTrack { id, element, index, track } => {
            let index = *index;
            let track = track.clone();
            with_instance(session, *id, element, |bindings| {
                if let Some(b) = bindings.get_mut(index) {
                    b.track = track;
                }
            });
            true
        }

        EditorCommand::SetPanelNodeTriggerHand { id, element, index, hand } => {
            let index = *index;
            let hand = crate::trigger_action::hand_from_dto(hand);
            with_instance(session, *id, element, |bindings| {
                if let Some(b) = bindings.get_mut(index) {
                    b.hand = hand;
                }
            });
            true
        }

        EditorCommand::SetPanelNodeTriggerEffect { id, element, index, effect } => {
            let index = *index;
            let effect = crate::trigger_action::effect_from_dto(effect);
            with_instance(session, *id, element, |bindings| {
                if let Some(b) = bindings.get_mut(index) {
                    b.effect = effect;
                }
            });
            true
        }

        EditorCommand::SetPanelNodeTriggerDisabled { id, element, index, disabled } => {
            let index = *index;
            let disabled = *disabled;
            with_instance(session, *id, element, |bindings| {
                if let Some(b) = bindings.get_mut(index) {
                    b.disabled = disabled;
                }
            });
            true
        }

        EditorCommand::SetPanelInstanceTemplate { id, template_id } => {
            let tid = XrdsPanelTemplateId(*template_id);
            let node_id = XrdsSceneNodeId(*id);
            // A dangling reference renders nothing with no visible cause, so it is
            // refused rather than stored.
            if session.0.document().panel_template(tid).is_none() {
                error!("[panel] SetPanelInstanceTemplate: template {tid:?} does not exist");
                return false;
            }
            match session.0.edit(|doc| {
                let Some(node) = doc.node_mut(node_id) else {
                    error!("[panel] SetPanelInstanceTemplate: node {id} not found");
                    return;
                };
                if let XrdsSceneNodePayload::Panel(ref mut i) = node.payload {
                    i.template_id = tid;
                } else {
                    error!("[panel] SetPanelInstanceTemplate: node {id} is not a Panel");
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[panel] SetPanelInstanceTemplate failed: {:?}", e),
            }
            true
        }

        _ => false,
    }
}

/// Runs `edit` against one named element, or logs and does nothing.
///
/// Every element command funnels through this so the "address by name, tolerate
/// a miss" behaviour is written once. A miss is a stale frontend, not a bug
/// worth panicking over — the next snapshot corrects it.
/// Runs `edit` against one Panel node's bindings for one element name.
///
/// Reads the current list, hands it over, then writes it back through
/// `set_triggers` — which is what applies the remove-when-empty rule, so removing
/// the last binding deletes the key instead of leaving an empty list that reads
/// like wiring and is not.
///
/// The element name is **not** checked against the template here. Wiring a name
/// the template lacks is possible (an element deleted after the fact) and is
/// reported by `panel_diagnostics` rather than refused, so the authored bindings
/// stay recoverable.
fn with_instance(
    session: &mut EditorSession,
    node_id: u64,
    element: &str,
    edit: impl FnOnce(&mut Vec<XrdsTriggerBinding>),
) {
    let node_id = XrdsSceneNodeId(node_id);
    let element = element.to_string();
    match session.0.edit(|doc| {
        let Some(node) = doc.node_mut(node_id) else {
            error!("[panel] no node {node_id:?}");
            return;
        };
        let XrdsSceneNodePayload::Panel(ref mut instance) = node.payload else {
            error!("[panel] node {node_id:?} is not a Panel");
            return;
        };
        let mut bindings = instance.triggers_for(&element).to_vec();
        edit(&mut bindings);
        instance.set_triggers(element, bindings);
    }) {
        Ok(_) => {}
        Err(e) => error!("[panel] instance binding edit failed: {:?}", e),
    }
}

/// The trigger kind a new binding on this node's element should default to.
///
/// Resolved through the node's template, because only the template knows the
/// element's kind — a Button defaults to `ButtonPress`, a Slider to
/// `SliderChange`. Falls back to `ButtonPress` when nothing can be resolved,
/// which `panel_diagnostics` then flags as unemittable rather than this guessing
/// silently.
fn element_default_kind(
    session: &EditorSession,
    node_id: u64,
    element: &str,
) -> xrds_scene_graph::XrdsTriggerKind {
    let doc = session.0.document();
    doc.node(XrdsSceneNodeId(node_id))
        .and_then(|n| match &n.payload {
            XrdsSceneNodePayload::Panel(i) => doc.panel_template(i.template_id),
            _ => None,
        })
        .and_then(|t| t.element(element))
        .map(default_trigger_kind_for)
        .unwrap_or(xrds_scene_graph::XrdsTriggerKind::ButtonPress)
}

/// The first trigger kind `element` can emit, for a newly added binding.
///
/// Picking a *reachable* default matters: seeding every element with
/// `ButtonPress` would make a slider's first binding silently inert, which is
/// exactly the confusion `emittable_triggers` exists to prevent. A non-emitting
/// kind (`Label`, `Image`) has no good answer, so it gets `ButtonPress` and the
/// diagnostic explains why it can never fire.
fn default_trigger_kind_for(element: &XrdsPanelElement) -> xrds_scene_graph::XrdsTriggerKind {
    use xrds_scene_graph::XrdsTriggerKind as K;
    for candidate in [K::ButtonPress, K::SliderChange, K::ToggleChange] {
        if element.can_emit(&candidate) {
            return candidate;
        }
    }
    K::ButtonPress
}

/// A sensible starting widget for a newly added element.
fn default_widget_for_kind(kind: &str) -> Option<XrdsSceneWorldWidget> {
    use xrds_scene_graph::{
        XrdsSceneWorldButton, XrdsSceneWorldImage, XrdsSceneWorldLabel, XrdsSceneWorldSlider,
        XrdsSceneWorldToggle,
    };
    Some(match kind {
        "Label" => XrdsSceneWorldWidget::Label(XrdsSceneWorldLabel::default()),
        "Button" => XrdsSceneWorldWidget::Button(XrdsSceneWorldButton::default()),
        "Image" => XrdsSceneWorldWidget::Image(XrdsSceneWorldImage::default()),
        "Slider" => XrdsSceneWorldWidget::Slider(XrdsSceneWorldSlider::default()),
        "Toggle" => XrdsSceneWorldWidget::Toggle(XrdsSceneWorldToggle::default()),
        _ => return None,
    })
}

#[cfg(test)]
#[path = "panel_library_tests.rs"]
mod tests;
