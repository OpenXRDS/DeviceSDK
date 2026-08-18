//! Reusable panel templates — the unified model behind both HUD panels and
//! world-space panels. See `docs/done/xrds-widget-template-plan.md`.
//!
//! **Why one model.** The two sides were complementary rather than similar: the
//! HUD had identity (`name`, addressable via `set_hud_item`), a template
//! registry and a library panel, but exactly one element kind — text. World
//! panels had five element kinds but no identity, no registry and no reuse. This
//! takes the world side's vocabulary and the HUD side's identity/reuse model, so
//! HUD gains buttons and sliders while world panels gain templates and
//! name-addressing.
//!
//! **Attachment is the only difference** between the two: a HUD template is
//! attached to the player's camera, a world panel sits in the scene. That is why
//! nothing in this file mentions either — placement belongs to whatever
//! instances a template, not to the template.
//!
//! This module started out additive, running alongside the old vocabulary until
//! the new one was validated. Both predecessors are now gone and this replaces
//! them outright: `XrdsHudTemplate` (§A4b-1) and `XrdsSceneWorldPanel` (§A4b-2).

use super::*;

/// Identity of a [`XrdsPanelTemplate`] within a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct XrdsPanelTemplateId(pub u64);

/// A panel's backdrop. Split out of the template so "what the panel looks like"
/// stays separable from "what is on it".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsPanelBackground {
    /// Background RGBA in 0–1 range.
    pub color: [f32; 4],
    /// Reserved for the rounded-corner shader; 0.0 = sharp corners.
    #[serde(default)]
    pub corner_radius: f32,
    /// Overall opacity multiplier (1.0 = fully opaque).
    #[serde(default = "one")]
    pub opacity: f32,
}

fn one() -> f32 {
    1.0
}

impl Default for XrdsPanelBackground {
    fn default() -> Self {
        Self { color: [0.1, 0.1, 0.12, 0.9], corner_radius: 0.0, opacity: 1.0 }
    }
}

/// One named thing on a panel.
///
/// `kind` deliberately **reuses** [`XrdsSceneWorldWidget`] rather than declaring
/// a parallel five-variant enum. The widget structs already carry everything an
/// element needs — `local_position`, per-kind sizing and colours — so a second
/// enum would be a duplicate that drifts, and it makes migrating existing
/// authored panels a matter of giving each widget a name.
///
/// What this adds over a bare widget is exactly what the world side lacked: an
/// **identity**.
///
/// **Deliberately carries no triggers.** They live on the *instance* —
/// [`XrdsScenePanelInstance::element_triggers`] — because a template is shared.
/// With bindings here, one template instanced on three floors gave every floor's
/// button the same Track and therefore the same door: the third-floor button
/// opened the ground-floor one, and there was no way to say "my door" because
/// `XrdsActionTarget::TriggerSource` resolves to the element that fired, not to
/// anything near it. That hazard used to need a diagnostic warning it could not
/// fix; moving bindings to the instance makes it unrepresentable instead.
///
/// The trade is ergonomic and accepted: twenty instances of a button wired the
/// same way now means twenty bindings rather than one. That is the price of each
/// instance being able to drive its own target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsPanelElement {
    /// Unique within its template. **The addressing key** — instance bindings,
    /// `set_hud_item` and Phase B's action targets all resolve by this name.
    ///
    /// A name rather than an index on purpose: reordering elements on the canvas
    /// must not silently re-point a binding. Validated by
    /// [`crate::normalize_authored_name`].
    pub name: String,
    pub kind: XrdsSceneWorldWidget,
}

impl XrdsPanelElement {
    /// A new element of `kind`, named `name`.
    pub fn new(name: impl Into<String>, kind: XrdsSceneWorldWidget) -> Self {
        Self { name: name.into(), kind }
    }

    /// Whether this element can ever emit `kind` at runtime.
    ///
    /// Only the interactive widgets emit anything, and each emits its own
    /// events. A `Label` or `Image` emits nothing at all, so a trigger authored
    /// on one is inert rather than wrong — diagnosed as a warning.
    ///
    /// `Custom` and `RunawayDetected` are **not** emittable by an element even
    /// though any node can carry them: both are dispatched to a document node's
    /// `XrdsId`, and an element has no id — it is addressed as
    /// `(panel, element name)`.
    pub fn can_emit(&self, kind: &XrdsTriggerKind) -> bool {
        matches!(
            (&self.kind, kind),
            (
                XrdsSceneWorldWidget::Button(_),
                XrdsTriggerKind::ButtonPress | XrdsTriggerKind::ButtonRelease
            ) | (XrdsSceneWorldWidget::Slider(_), XrdsTriggerKind::SliderChange)
                | (XrdsSceneWorldWidget::Toggle(_), XrdsTriggerKind::ToggleChange)
        )
    }

    /// Where this element sits on its panel's canvas, in metres (X right, Y up
    /// from centre).
    ///
    /// Lives on the widget rather than the element, so this reaches across the
    /// five variants for callers that need placement without matching on kind —
    /// notably the camera attachment, which turns a canvas position into a
    /// head-locked offset at its own depth.
    pub fn local_position(&self) -> [f32; 2] {
        match &self.kind {
            XrdsSceneWorldWidget::Label(l) => l.local_position,
            XrdsSceneWorldWidget::Button(b) => b.local_position,
            XrdsSceneWorldWidget::Image(i) => i.local_position,
            XrdsSceneWorldWidget::Slider(s) => s.local_position,
            XrdsSceneWorldWidget::Toggle(t) => t.local_position,
        }
    }

    /// Human name of the element's kind, for diagnostics.
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            XrdsSceneWorldWidget::Label(_) => "Label",
            XrdsSceneWorldWidget::Button(_) => "Button",
            XrdsSceneWorldWidget::Image(_) => "Image",
            XrdsSceneWorldWidget::Slider(_) => "Slider",
            XrdsSceneWorldWidget::Toggle(_) => "Toggle",
        }
    }

    /// Whether this element emits anything at all.
    pub fn is_interactive(&self) -> bool {
        matches!(
            self.kind,
            XrdsSceneWorldWidget::Button(_)
                | XrdsSceneWorldWidget::Slider(_)
                | XrdsSceneWorldWidget::Toggle(_)
        )
    }
}

/// A reusable panel: its canvas, its backdrop, and the named elements on it.
///
/// Holds **no placement**. A template instanced three times is three panels in
/// three places sharing one authored definition, which is the point — and is
/// why `depth` does not live here the way the retired `XrdsHudTemplate::depth`
/// did. Depth
/// is a property of a camera attachment, and keeping it on the template would
/// quietly prevent instancing one template at two depths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsPanelTemplate {
    pub id: XrdsPanelTemplateId,
    /// Display name, and the key an author picks the template by. Validated by
    /// [`crate::normalize_authored_name`].
    pub name: String,
    /// Canvas dimensions [width, height] in metres.
    pub size: [f32; 2],
    #[serde(default)]
    pub background: XrdsPanelBackground,
    /// Optional auto-layout applied to the elements.
    #[serde(default)]
    pub layout: XrdsSceneWorldLayout,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<XrdsPanelElement>,
}

impl Default for XrdsPanelTemplate {
    fn default() -> Self {
        Self {
            id: XrdsPanelTemplateId(1),
            name: "Panel".to_string(),
            size: [0.6, 0.4],
            background: XrdsPanelBackground::default(),
            layout: XrdsSceneWorldLayout::None,
            elements: Vec::new(),
        }
    }
}

impl XrdsPanelTemplate {
    pub fn element(&self, name: &str) -> Option<&XrdsPanelElement> {
        self.elements.iter().find(|e| e.name == name)
    }

    pub fn element_mut(&mut self, name: &str) -> Option<&mut XrdsPanelElement> {
        self.elements.iter_mut().find(|e| e.name == name)
    }

    /// Element names that appear more than once, each reported once.
    ///
    /// A duplicate is an **error**, not untidiness: names are how elements are
    /// addressed, so two elements sharing one makes `set_hud_item` and (Phase B)
    /// action targets ambiguous — they would silently hit whichever comes first.
    pub fn duplicate_element_names(&self) -> Vec<String> {
        let mut seen: Vec<&str> = Vec::new();
        let mut dupes: Vec<String> = Vec::new();
        for element in &self.elements {
            if seen.contains(&element.name.as_str())
                && !dupes.contains(&element.name)
            {
                dupes.push(element.name.clone());
            }
            seen.push(&element.name);
        }
        dupes
    }
}
