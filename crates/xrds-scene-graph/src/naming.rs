//! Authored-name policy — one validator for every name that is used as a
//! **key** rather than as a label.
//!
//! See `docs/xrds-widget-template-plan.md` §3b. Track names, panel-template
//! names and panel-element names are all keys: bindings reference Tracks by
//! name, and elements will be addressed by name. That makes loose naming a
//! correctness problem, not a style one.
//!
//! The bug this exists to prevent is specifically the *invisible* kind: names
//! that render identically, hash differently, and give an author no way to see
//! why their binding "does nothing". Two ways that happens, both handled here:
//!
//! - `"Door "` versus `"Door"` — trailing whitespace.
//! - Korean written as conjoining jamo versus precomposed syllables — see
//!   [`compose_hangul`].
//!
//! Deliberately one module used by every caller rather than a check
//! per-command: three near-copies would drift, and the drift would be silent
//! for the same reason the original bug is.

/// Names starting with this are refused.
///
/// The editor's pickers use `__`-prefixed values as sentinels for "nothing
/// selected" and similar (`__none__`, `__any__`, `__add__`, `__clear__`,
/// `__add_asset__`, `__no_texture__`), because Radix `Select.Item` forbids an
/// empty string. A Track actually *named* `__none__` would render as "nothing
/// selected" in the Fires picker — it would appear to unwire itself.
///
/// Reserving the prefix is much cheaper than making six pickers
/// collision-proof, and no author wants this prefix anyway.
pub const RESERVED_NAME_PREFIX: &str = "__";

/// Why an authored name was refused.
///
/// Carries enough to *explain* the refusal, not just signal it. A silently
/// rejected rename would reproduce the exact class of bug this policy exists to
/// remove, so every variant can produce a human message and a suggested
/// replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsNameError {
    /// Empty, or nothing but whitespace.
    Empty,
    /// Starts with [`RESERVED_NAME_PREFIX`].
    ReservedPrefix,
    /// Contains something outside the allowed set — see [`is_allowed_char`].
    DisallowedCharacter { ch: char },
}

impl XrdsNameError {
    /// One sentence an editor can show verbatim.
    pub fn message(&self) -> String {
        match self {
            Self::Empty => "A name cannot be empty or only whitespace.".to_string(),
            Self::ReservedPrefix => "Names cannot start with \"__\" — that prefix is reserved for \
                 the editor's own internal values, and a name using it can read as \"nothing \
                 selected\"."
                .to_string(),
            Self::DisallowedCharacter { ch } => format!(
                "{ch:?} is not allowed in a name. Names may use English letters, digits, \
                 Korean (Hangul), spaces, and the punctuation on a standard keyboard."
            ),
        }
    }

    /// A concrete alternative to offer, so the author is not left guessing.
    /// `raw` is the name they tried.
    pub fn suggestion(&self, raw: &str) -> String {
        let cleaned = match self {
            Self::Empty => String::new(),
            // Both ends: `__none__` should suggest `none`, not `none__`. The
            // latter is *valid* but nobody wants it, and a suggestion an author
            // would immediately edit again is barely a suggestion.
            Self::ReservedPrefix => compose_hangul(raw.trim()).trim_matches('_').trim().to_string(),
            // Drop exactly the offending characters rather than mangling the
            // rest, so a mostly-fine name survives.
            Self::DisallowedCharacter { .. } => compose_hangul(raw.trim())
                .chars()
                .filter(|c| is_allowed_char(*c))
                .collect::<String>()
                .trim()
                .to_string(),
        };
        // The suggestion must itself pass, or the author is bounced twice.
        match normalize_authored_name(&cleaned) {
            Ok(ok) => ok,
            Err(_) => "Untitled".to_string(),
        }
    }
}

impl std::fmt::Display for XrdsNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

/// Whether `c` may appear in an authored name.
///
/// **All printable ASCII, plus Korean.** Deliberately narrow: the alternative
/// is accepting every script, which brings in the whole normalization and
/// confusable-character surface for names that are only ever typed and read by
/// the team authoring the scene.
///
/// Korean is admitted as precomposed Hangul syllables (가–힣) and the
/// standalone compatibility jamo that sit on a Korean keyboard (ㄱ–ㆎ).
/// Conjoining jamo are *composed* into syllables before this check rather than
/// rejected — see [`compose_hangul`].
///
/// Not admitted: accented Latin (é), Han/Kanji (門), emoji, control characters.
/// Han in particular is a judgement call — Hanja is occasionally used in Korean
/// text — and is easy to allow later by adding its range here.
pub fn is_allowed_char(c: char) -> bool {
    c == ' '
        || c.is_ascii_graphic()
        // Precomposed Hangul syllables: 가 (U+AC00) – 힣 (U+D7A3).
        || matches!(c, '\u{AC00}'..='\u{D7A3}')
        // Hangul compatibility jamo: ㄱ (U+3131) – ㆎ (U+318E).
        || matches!(c, '\u{3131}'..='\u{318E}')
}

/// NFC-normalizes a name, which for the allowed character set means composing
/// Hangul.
///
/// **This is the Korean half of the invisible-duplicate problem.** Hangul has
/// two encodings that render identically: precomposed syllables (한 = U+D55C)
/// and sequences of conjoining jamo (ᄒ + ᅡ + ᆫ). Without normalizing, those are
/// two different keys that look the same on screen — exactly the trailing-space
/// bug in another costume, and reachable in practice because some platforms and
/// paste paths carry the decomposed form.
///
/// Restricting the character set does *not* remove this: plain English has no
/// decomposed forms, but admitting Korean brings this in as the main case.
///
/// **Uses `unicode-normalization` rather than hand-rolled Hangul arithmetic.**
/// A hand-written version was written first and had a real hole: UAX #15 also
/// composes a *precomposed* LV syllable followed by a conjoining trailing
/// consonant (하 U+D558 + ᆫ U+11AB → 한 U+D55C), not only sequences that start
/// from conjoining jamo. Missing that case left such input as two code points,
/// rendering as 한 but then refused by [`is_allowed_char`] for containing a
/// conjoining jamo. The crate is small and this is not a hot path — names are
/// validated when a human types one.
///
/// Naming it after Hangul rather than NFC is deliberate: within the allowed set
/// Hangul is the only thing NFC changes, so that is what this means in practice.
/// An unpaired conjoining jamo still cannot compose and is left as-is, so
/// [`is_allowed_char`] rejects it with a message pointing at the keyboard
/// (compatibility) forms.
pub fn compose_hangul(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfc().collect()
}

/// Canonicalizes an authored name, or explains why it cannot be used.
///
/// Applies, in order:
///
/// 1. **Trim** surrounding whitespace. This is the rule that kills the
///    invisible-duplicate bug: `"Door "` becomes `"Door"` and then collides
///    loudly with the existing `"Door"` instead of silently shadowing it.
/// 2. **Compose Hangul** ([`compose_hangul`]) — the same hazard for Korean.
/// 3. **Reject empty** after trimming.
/// 4. **Reject the reserved prefix** ([`RESERVED_NAME_PREFIX`]).
/// 5. **Reject disallowed characters** ([`is_allowed_char`]).
///
/// Composition happens before the character check so decomposed Korean is
/// *accepted and canonicalized* rather than rejected for containing conjoining
/// jamo.
pub fn normalize_authored_name(raw: &str) -> Result<String, XrdsNameError> {
    let canonical = compose_hangul(raw.trim());
    if canonical.is_empty() {
        return Err(XrdsNameError::Empty);
    }
    if canonical.starts_with(RESERVED_NAME_PREFIX) {
        return Err(XrdsNameError::ReservedPrefix);
    }
    if let Some(ch) = canonical.chars().find(|c| !is_allowed_char(*c)) {
        return Err(XrdsNameError::DisallowedCharacter { ch });
    }
    Ok(canonical)
}

/// Case-folded form, for *diagnosing* names that differ only by case.
///
/// Uniqueness itself stays case-**sensitive**: case-insensitive keys drag in
/// locale edge cases (Turkish dotless i, where lowercasing `I` is not `i`) to
/// catch a trap a warning handles just as well. So this is only ever used to
/// group names for a diagnostic, never as the key itself.
pub fn name_case_fold(name: &str) -> String {
    name.to_lowercase()
}

/// Names in one scope that differ only by case, as `(first, second)` pairs in
/// input order.
///
/// Takes an iterator so callers can feed Track names, template names or element
/// names without building an intermediate collection each time.
pub fn names_differing_only_by_case<'a, I>(names: I) -> Vec<(String, String)>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut seen: Vec<(String, &'a str)> = Vec::new();
    let mut out = Vec::new();
    for name in names {
        let folded = name_case_fold(name);
        if let Some((_, first)) = seen.iter().find(|(f, other)| *f == folded && *other != name) {
            out.push(((*first).to_string(), name.to_string()));
        }
        seen.push((folded, name));
    }
    out
}
