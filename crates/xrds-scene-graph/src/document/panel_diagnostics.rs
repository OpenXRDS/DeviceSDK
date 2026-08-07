//! Authoring checks for [`XrdsPanelTemplate`]s — see
//! `docs/xrds-widget-template-plan.md` §A1.
//!
//! Reports [`XrdsSceneTriggerDiagnostic`] rather than a panel-specific type: the
//! editor already renders that shape, and most of what can go wrong on a panel
//! *is* a trigger problem (a binding that can never fire, a Track that does not
//! exist). Naming problems ride along because they break addressing, which is
//! the same class of silent failure.

use super::*;

impl XrdsSceneDocument {
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

                for (i, binding) in element.triggers.iter().enumerate() {
                    // Inert rather than wrong: a Label emits nothing, so a
                    // trigger on one can never fire. Warning, not error — it is
                    // most likely a leftover after changing an element's kind.
                    if !element.can_emit(&binding.trigger) {
                        out.push(XrdsSceneTriggerDiagnostic {
                            node_id: None,
                            severity: Severity::Warning,
                            title: "Element cannot emit this trigger".to_string(),
                            detail: format!(
                                "{el} is a {} and never emits {:?}, so binding #{i} can never \
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
                            node_id: None,
                            severity: Severity::Warning,
                            title: "Element binding runs nothing".to_string(),
                            detail: format!("{el} binding #{i} names no Track."),
                        }),
                        Some(name) if !track_names.contains(name.as_str()) => {
                            out.push(XrdsSceneTriggerDiagnostic {
                                node_id: None,
                                severity: Severity::Error,
                                title: "Element binding names a missing Track".to_string(),
                                detail: format!(
                                    "{el} binding #{i} fires {name:?}, which is not in this \
                                     document."
                                ),
                            });
                        }
                        Some(_) => {}
                    }
                }
            }
        }

        out
    }
}
