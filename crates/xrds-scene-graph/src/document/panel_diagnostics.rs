//! Authoring checks for [`XrdsPanelTemplate`]s — see
//! `docs/done/xrds-widget-template-plan.md` §A1.
//!
//! Reports [`XrdsSceneTriggerDiagnostic`] rather than a panel-specific type: the
//! editor already renders that shape, and most of what can go wrong on a panel
//! *is* a trigger problem (a binding that can never fire, a Track that does not
//! exist). Naming problems ride along because they break addressing, which is
//! the same class of silent failure.

use super::*;

impl XrdsSceneDocument {
    /// Every trigger binding in the document, paired with the node it belongs to.
    ///
    /// Two sources exist and both matter: a node's own `triggers`, and each Panel
    /// node's per-element `element_triggers`. Anything that reasons about bindings
    /// document-wide — like "does anything actually fire this Track?" — has to see
    /// both, and a caller that walked only `node.triggers` would quietly ignore
    /// every panel button.
    ///
    /// One iterator rather than each caller rolling its own, so adding a third
    /// binding source later is a change in one place instead of a hunt.
    pub fn all_trigger_bindings(
        &self,
    ) -> impl Iterator<Item = (Option<XrdsSceneNodeId>, &XrdsTriggerBinding)> {
        self.nodes.iter().flat_map(|node| {
            let own = node.triggers.iter().map(move |b| (Some(node.id), b));
            let element = match &node.payload {
                XrdsSceneNodePayload::Panel(i) => Some(
                    i.element_triggers
                        .values()
                        .flatten()
                        .map(move |b| (Some(node.id), b)),
                ),
                _ => None,
            };
            own.chain(element.into_iter().flatten())
        })
    }

    /// How many scene `Panel` nodes instance `template_id`.
    pub fn panel_instance_count(&self, template_id: XrdsPanelTemplateId) -> usize {
        self.nodes
            .iter()
            .filter(|n| matches!(&n.payload,
                XrdsSceneNodePayload::Panel(i) if i.template_id == template_id))
            .count()
    }

    /// Everything wrong with this document's panel templates.
    ///
    /// Separate from [`XrdsSceneDocument::track_diagnostics`] so the editor can
    /// show panel problems in the panel workspace rather than mixing them into
    /// the Sequencer's list.
    pub fn panel_diagnostics(&self) -> Vec<XrdsSceneTriggerDiagnostic> {
        use XrdsSceneTriggerDiagnosticSeverity as Severity;

        let mut out = Vec::new();
        let track_names: std::collections::HashSet<&str> =
            self.tracks.iter().map(|t| t.name.as_str()).collect();

        // -- Template names ---------------------------------------------------
        for template in &self.panels {
            match crate::normalize_authored_name(&template.name) {
                Ok(canonical) if canonical != template.name => {
                    out.push(XrdsSceneTriggerDiagnostic {
                        node_id: None,
                        severity: Severity::Error,
                        title: "Panel name is not canonical".to_string(),
                        detail: format!(
                            "Panel {:?} is not the same key as {canonical:?} but renders \
                             identically.",
                            template.name
                        ),
                    });
                }
                Err(e) => out.push(XrdsSceneTriggerDiagnostic {
                    node_id: None,
                    severity: Severity::Error,
                    title: "Panel name is not usable".to_string(),
                    detail: format!("Panel {:?}: {}", template.name, e.message()),
                }),
                Ok(_) => {}
            }
        }

        for (first, second) in
            crate::names_differing_only_by_case(self.panels.iter().map(|t| t.name.as_str()))
        {
            out.push(XrdsSceneTriggerDiagnostic {
                node_id: None,
                severity: Severity::Warning,
                title: "Two panels differ only by case".to_string(),
                detail: format!("{first:?} and {second:?} are separate panel templates."),
            });
        }

        // -- Per-template ------------------------------------------------------
        for template in &self.panels {
            let where_ = format!("panel {:?}", template.name);

            // A duplicate name makes `(panel, element)` addressing ambiguous —
            // it would silently resolve to whichever comes first.
            for dupe in template.duplicate_element_names() {
                out.push(XrdsSceneTriggerDiagnostic {
                    node_id: None,
                    severity: Severity::Error,
                    title: "Duplicate element name".to_string(),
                    detail: format!(
                        "{where_} has more than one element named {dupe:?}. Elements are \
                         addressed by name, so this is ambiguous."
                    ),
                });
            }

            for (first, second) in crate::names_differing_only_by_case(
                template.elements.iter().map(|e| e.name.as_str()),
            ) {
                out.push(XrdsSceneTriggerDiagnostic {
                    node_id: None,
                    severity: Severity::Warning,
                    title: "Two elements differ only by case".to_string(),
                    detail: format!("{where_} has both {first:?} and {second:?}."),
                });
            }

            for element in &template.elements {
                let el = format!("{where_}, element {:?}", element.name);

                match crate::normalize_authored_name(&element.name) {
                    Ok(canonical) if canonical != element.name => {
                        out.push(XrdsSceneTriggerDiagnostic {
                            node_id: None,
                            severity: Severity::Error,
                            title: "Element name is not canonical".to_string(),
                            detail: format!(
                                "{el} is not the same key as {canonical:?} but renders \
                                 identically."
                            ),
                        });
                    }
                    Err(e) => out.push(XrdsSceneTriggerDiagnostic {
                        node_id: None,
                        severity: Severity::Error,
                        title: "Element name is not usable".to_string(),
                        detail: format!("{el}: {}", e.message()),
                    }),
                    Ok(_) => {}
                }

                // Element trigger checks moved to the per-instance pass below.
                // A template carries no bindings any more, so there is nothing
                // here to validate beyond the element's own name and kind.
                let _ = &el;
            }
        }

        // -- Per-instance element bindings ------------------------------------
        //
        // Bindings live on the instance, so this is where "can this fire?" and
        // "does this Track exist?" are answered. Reported against `node_id` so
        // the scene Inspector can show each Panel node its own problems.
        for node in &self.nodes {
            let XrdsSceneNodePayload::Panel(instance) = &node.payload else { continue };
            let Some(template) = self.panel_template(instance.template_id) else {
                continue; // dangling template — reported in the attachment pass
            };

            for (element_name, bindings) in &instance.element_triggers {
                let where_ = format!("panel node {:?}, element {element_name:?}", node.name);

                // A key naming an element the template no longer has. Deleting an
                // element after instances were wired is the way this happens, and
                // the binding is kept rather than dropped so the authored work is
                // recoverable — hence a diagnostic instead of silent cleanup.
                let Some(element) = template.element(element_name) else {
                    out.push(XrdsSceneTriggerDiagnostic {
                        node_id: Some(node.id),
                        severity: Severity::Error,
                        title: "Binding names an element the template does not have".to_string(),
                        detail: format!(
                            "{where_} is wired, but template {:?} has no such element — it was \
                             probably deleted or renamed outside the editor. The binding is kept \
                             so it can be repointed; it will never fire.",
                            template.name
                        ),
                    });
                    continue;
                };

                for (i, binding) in bindings.iter().enumerate() {
                    // Inert rather than wrong: a Label emits nothing, so a
                    // trigger on one can never fire. Warning, not error — most
                    // likely a leftover after changing the element's kind in the
                    // template, which the instance cannot see.
                    if !element.can_emit(&binding.trigger) {
                        out.push(XrdsSceneTriggerDiagnostic {
                            node_id: Some(node.id),
                            severity: Severity::Warning,
                            title: "Element cannot emit this trigger".to_string(),
                            detail: format!(
                                "{where_} is a {} and never emits {:?}, so binding #{i} can never \
                                 fire. A Button emits ButtonPress/ButtonRelease, a Slider \
                                 SliderChange, a Toggle ToggleChange; Labels and Images emit \
                                 nothing.",
                                element.kind_name(),
                                binding.trigger
                            ),
                        });
                    }

                    match &binding.track {
                        None => out.push(XrdsSceneTriggerDiagnostic {
                            node_id: Some(node.id),
                            severity: Severity::Warning,
                            title: "Element binding runs nothing".to_string(),
                            detail: format!("{where_} binding #{i} names no Track."),
                        }),
                        Some(name) if !track_names.contains(name.as_str()) => {
                            out.push(XrdsSceneTriggerDiagnostic {
                                node_id: Some(node.id),
                                severity: Severity::Error,
                                title: "Element binding names a missing Track".to_string(),
                                detail: format!(
                                    "{where_} binding #{i} fires {name:?}, which is not in this \
                                     document."
                                ),
                            });
                        }
                        Some(_) => {}
                    }
                }
            }
        }

        // §A4's "Template fires a Track that drives fixed nodes" warning stood
        // here. It is **deleted, not disabled**: it existed because a shared
        // template meant every instance's button fired the same Track at the same
        // fixed node, and it could only state the consequence — there was no fix
        // to suggest, since `XrdsActionTarget::TriggerSource` resolves to the
        // element that fired rather than to anything near it.
        //
        // With bindings on the instance, each floor's panel wires its own door.
        // The hazard is not merely diagnosed, it is unrepresentable, and a warning
        // for a condition that cannot occur is noise. `panel_instance_count`
        // survives because the editor still wants to say "used by N nodes".

        // -- Attachments -------------------------------------------------------
        // Both halves of "attachment is the only difference" reference a
        // template by id, and a dangling reference means the panel silently does
        // not appear — the runtime logs and spawns nothing rather than failing
        // the load, so this is the only author-time signal.
        for node in &self.nodes {
            match &node.payload {
                XrdsSceneNodePayload::Panel(instance) => {
                    if self.panel_template(instance.template_id).is_none() {
                        out.push(XrdsSceneTriggerDiagnostic {
                            node_id: Some(node.id),
                            severity: Severity::Error,
                            title: "Panel instance names a missing template".to_string(),
                            detail: format!(
                                "Node {:?} instances panel template {:?}, which is not in this \
                                 document. Nothing will be spawned.",
                                node.name, instance.template_id
                            ),
                        });
                    }
                }
                _ => {}
            }
        }

        out
    }
}
