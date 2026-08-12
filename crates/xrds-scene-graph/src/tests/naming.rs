//! Tests for the authored-name policy — see `naming.rs` and
//! `docs/done/xrds-widget-template-plan.md` §3b.
//!
//! The bug being guarded is the *invisible* kind: names that render
//! identically but hash differently, so a binding silently misses.

use super::*;

fn named_track(name: &str) -> XrdsNamedTrack {
    XrdsNamedTrack { name: name.to_string(), track: XrdsTrack::default() }
}

fn doc_with_track_names(names: &[&str]) -> XrdsSceneDocument {
    XrdsSceneDocument {
        tracks: names.iter().map(|n| named_track(n)).collect(),
        ..XrdsSceneDocument::default()
    }
}

fn titles(d: &XrdsSceneDocument) -> Vec<String> {
    d.track_diagnostics().into_iter().map(|x| x.title).collect()
}

// ---------------------------------------------------------------------------
// normalize_authored_name
// ---------------------------------------------------------------------------

#[test]
fn an_empty_or_whitespace_only_name_is_refused() {
    assert_eq!(normalize_authored_name(""), Err(XrdsNameError::Empty));
    assert_eq!(normalize_authored_name("   "), Err(XrdsNameError::Empty));
    assert_eq!(normalize_authored_name("\t\n"), Err(XrdsNameError::Empty));
}

#[test]
fn surrounding_whitespace_is_trimmed_so_it_cannot_hide_a_duplicate() {
    // The whole point: `"Door "` must *become* `"Door"`, so the next step
    // collides loudly instead of creating a second key that looks the same.
    assert_eq!(normalize_authored_name("Door "), Ok("Door".to_string()));
    assert_eq!(normalize_authored_name(" Door"), Ok("Door".to_string()));
    assert_eq!(normalize_authored_name("  Door  "), Ok("Door".to_string()));
}

#[test]
fn interior_whitespace_is_preserved_because_it_is_visible() {
    assert_eq!(normalize_authored_name("Front Door"), Ok("Front Door".to_string()));
}

#[test]
fn the_reserved_prefix_is_refused_because_pickers_use_it_as_a_sentinel() {
    // A Track named `__none__` would read as "nothing selected" in the Fires
    // picker — it would appear to unwire itself.
    assert_eq!(normalize_authored_name("__none__"), Err(XrdsNameError::ReservedPrefix));
    assert_eq!(normalize_authored_name("__anything"), Err(XrdsNameError::ReservedPrefix));
    // Trimmed first, so padding cannot smuggle it through.
    assert_eq!(normalize_authored_name("  __none__"), Err(XrdsNameError::ReservedPrefix));
}

#[test]
fn a_single_leading_underscore_is_fine() {
    // Only the doubled prefix is reserved; over-restricting would just annoy.
    assert_eq!(normalize_authored_name("_private"), Ok("_private".to_string()));
}

#[test]
fn english_digits_and_keyboard_punctuation_are_accepted() {
    for name in ["Door", "Door #2", "a-b_c.d", "100%", "why?", "a+b=c", "x&y", "(z)"] {
        assert_eq!(normalize_authored_name(name), Ok(name.to_string()), "{name:?}");
    }
}

#[test]
fn korean_is_accepted() {
    for name in ["버튼", "본문 여기", "Door 버튼 2"] {
        assert_eq!(normalize_authored_name(name), Ok(name.to_string()), "{name:?}");
    }
}

#[test]
fn standalone_keyboard_jamo_are_accepted() {
    // The compatibility jamo a Korean keyboard produces on their own.
    for name in ["ㄱ", "ㅏ", "ㄱㅏ"] {
        assert_eq!(normalize_authored_name(name), Ok(name.to_string()), "{name:?}");
    }
}

#[test]
fn characters_outside_english_and_korean_are_refused() {
    // Accented Latin, Han, emoji, and control characters. Restricting the set
    // is what keeps the normalization surface to just Hangul.
    for bad in ["Café", "門", "Door 🚪", "a	b", "a
b"] {
        assert!(
            matches!(
                normalize_authored_name(bad),
                Err(XrdsNameError::DisallowedCharacter { .. })
            ),
            "{bad:?} should be refused, got {:?}",
            normalize_authored_name(bad)
        );
    }
}

// ---------------------------------------------------------------------------
// Hangul composition — the Korean half of the invisible-duplicate bug
// ---------------------------------------------------------------------------

#[test]
fn decomposed_korean_composes_to_the_same_key_as_precomposed() {
    // 한 written as conjoining jamo (ᄒ + ᅡ + ᆫ) renders identically to the
    // precomposed syllable but is different bytes. Without composing, these are
    // two keys that look the same — the trailing-space bug in another costume.
    let decomposed = "한";
    let precomposed = "한";
    assert_ne!(decomposed, precomposed, "the fixture must actually differ in bytes");

    assert_eq!(compose_hangul(decomposed), precomposed);
    assert_eq!(
        normalize_authored_name(decomposed),
        normalize_authored_name(precomposed),
        "both spellings must canonicalize to one key"
    );
}

#[test]
fn a_precomposed_syllable_plus_a_trailing_jamo_also_composes() {
    // Regression for a hole in the hand-rolled composer this replaced: UAX #15
    // composes an already-precomposed LV syllable with a following conjoining
    // trailing consonant, not only sequences starting from conjoining jamo.
    // Missing it left 하 + ᆫ as two code points that render as 한 and were then
    // refused for containing a conjoining jamo.
    let lv_plus_trail = "\u{D558}\u{11AB}"; // 하 + ᆫ
    assert_eq!(compose_hangul(lv_plus_trail), "한");
    assert_eq!(
        normalize_authored_name(lv_plus_trail),
        normalize_authored_name("한"),
        "both spellings must canonicalize to one key"
    );
}

#[test]
fn composition_handles_syllables_with_no_trailing_consonant() {
    // 하 is lead + vowel only; the trailing consonant is optional and must not
    // swallow the next character.
    assert_eq!(compose_hangul("하"), "하");
    assert_eq!(compose_hangul("하Z"), "하Z");
}

#[test]
fn composition_leaves_non_korean_text_untouched() {
    for s in ["Door", "Door #2", "", "   "] {
        assert_eq!(compose_hangul(s), s, "{s:?}");
    }
}

#[test]
fn an_unpaired_conjoining_jamo_is_refused_rather_than_silently_kept() {
    // It cannot compose, so it stays a conjoining code point — which the
    // allowlist then rejects, pointing the author at the keyboard forms.
    let lone_lead = "ᄒ";
    assert!(matches!(
        normalize_authored_name(lone_lead),
        Err(XrdsNameError::DisallowedCharacter { .. })
    ));
}

#[test]
fn every_rejection_offers_a_usable_alternative() {
    // A refusal with no suggestion leaves the author guessing, which is the
    // failure mode this policy exists to remove.
    let empty = XrdsNameError::Empty;
    assert!(!empty.suggestion("").is_empty());
    assert!(normalize_authored_name(&empty.suggestion("")).is_ok());

    let reserved = XrdsNameError::ReservedPrefix;
    let s = reserved.suggestion("__none__");
    assert_eq!(s, "none");
    assert!(normalize_authored_name(&s).is_ok(), "the suggestion must itself be valid");

    // Degenerate input still yields something usable rather than another error.
    let s2 = reserved.suggestion("____");
    assert!(normalize_authored_name(&s2).is_ok(), "got {s2:?}");
}

// ---------------------------------------------------------------------------
// names_differing_only_by_case
// ---------------------------------------------------------------------------

#[test]
fn case_only_differences_are_reported_as_pairs() {
    let pairs = names_differing_only_by_case(["Door", "door"]);
    assert_eq!(pairs, vec![("Door".to_string(), "door".to_string())]);
}

#[test]
fn identical_names_are_not_reported_as_a_case_difference() {
    // An exact duplicate is a different problem with its own check; reporting
    // it here too would be noise.
    assert!(names_differing_only_by_case(["Door", "Door"]).is_empty());
}

#[test]
fn distinct_names_are_quiet() {
    assert!(names_differing_only_by_case(["Door", "Window", "Hatch"]).is_empty());
}

// ---------------------------------------------------------------------------
// Document diagnostics
// ---------------------------------------------------------------------------

#[test]
fn diagnostics_flag_a_track_name_with_surrounding_whitespace() {
    // The editor refuses this at input, but a document built in Rust or edited
    // by hand can still carry it — so the rules are reported, not assumed.
    let d = doc_with_track_names(&["Door "]);
    assert!(
        titles(&d).contains(&"Track name has surrounding whitespace".to_string()),
        "{:?}",
        titles(&d)
    );
}

#[test]
fn diagnostics_flag_an_unusable_track_name() {
    for bad in ["", "   ", "__none__"] {
        let d = doc_with_track_names(&[bad]);
        assert!(
            titles(&d).contains(&"Track name is not usable".to_string()),
            "{bad:?} should be flagged, got {:?}",
            titles(&d)
        );
    }
}

#[test]
fn diagnostics_warn_when_two_tracks_differ_only_by_case() {
    let d = doc_with_track_names(&["Door", "door"]);
    let t = titles(&d);
    assert!(t.contains(&"Two Tracks differ only by case".to_string()), "{t:?}");
    // A warning, not an error — it is legal, just invisible in review.
    let diag = d
        .track_diagnostics()
        .into_iter()
        .find(|x| x.title == "Two Tracks differ only by case")
        .expect("present");
    assert_eq!(diag.severity, XrdsSceneTriggerDiagnosticSeverity::Warning);
}

#[test]
fn well_named_tracks_produce_no_naming_diagnostics() {
    let d = doc_with_track_names(&["Door", "Window"]);
    let t = titles(&d);
    for unwanted in [
        "Track name has surrounding whitespace",
        "Track name is not usable",
        "Two Tracks differ only by case",
    ] {
        assert!(!t.contains(&unwanted.to_string()), "unexpected {unwanted:?} in {t:?}");
    }
}
