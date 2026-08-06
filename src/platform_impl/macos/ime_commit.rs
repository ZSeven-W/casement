//! Decide whether an `insertText:` callback is an IME commit.
//!
//! `interpretKeyEvents` routes BOTH ordinary typing and IME output
//! through `insertText:replacementRange:`, so the view has to tell them
//! apart. Keying that off `hasMarkedText()` alone is not enough: CJK
//! punctuation (《 》 【 】 —— ……) is resolved by the IME the instant the
//! key is pressed, so no composition is ever opened, `hasMarkedText()`
//! is false, and the committed string used to be dropped — leaving the
//! raw key event to deliver the untransformed ASCII (`<` for 《).
//!
//! The extra signal is the key's own `characters`: ordinary typing
//! inserts exactly what the key produces, while an IME that transformed
//! the keystroke inserts something else.

/// Whether the string handed to `insertText:` should be reported as
/// [`Ime::Commit`](crate::event::Ime::Commit).
///
/// `key_characters` is the `characters` of the `NSEvent` currently being
/// interpreted, or `None` when `insertText:` did not arrive from a key
/// event (a candidate picked with the mouse, say) — such a call always
/// carries marked text, so the transformation check is not needed there.
pub(super) fn is_ime_commit(
    ime_enabled: bool,
    has_marked_text: bool,
    inserted: &str,
    key_characters: Option<&str>,
) -> bool {
    if !ime_enabled {
        return false;
    }
    // Control characters are the key's own doing (Enter, Tab, Ctrl+C);
    // `doCommandBySelector:` forwards them to the application instead.
    if inserted.chars().next().is_some_and(|c| c.is_control()) {
        return false;
    }
    if has_marked_text {
        return true;
    }
    // No composition was ever opened, so this is either ordinary typing
    // or an IME that resolved the key outright. Only the latter — where
    // the inserted string differs from what the key itself produces — is
    // a commit; treating ordinary typing as one would suppress the key
    // event that already carries the same character.
    key_characters.is_some_and(|raw| raw != inserted)
}

#[cfg(test)]
mod tests {
    use super::is_ime_commit;

    /// The reported bug: 《 on Shift+comma under a Chinese IME. No
    /// composition opens, so the commit used to be dropped and the raw
    /// `<` from the key event landed instead.
    #[test]
    fn cjk_punctuation_without_composition_is_a_commit() {
        assert!(is_ime_commit(true, false, "《", Some("<")));
    }

    #[test]
    fn every_transformed_punctuation_without_composition_is_a_commit() {
        for (inserted, raw) in [
            ("《", "<"),
            ("》", ">"),
            ("【", "["),
            ("】", "]"),
            ("——", "-"),
            ("……", "^"),
            ("，", ","),
            ("。", "."),
            ("、", "\\"),
            ("？", "?"),
        ] {
            assert!(
                is_ime_commit(true, false, inserted, Some(raw)),
                "{inserted:?} (raw {raw:?}) must be reported as a commit"
            );
        }
    }

    /// Ordinary typing also reaches `insertText:` while IME is enabled.
    /// The key event already carries the character, so reporting a
    /// commit here would either double it or suppress the key event.
    #[test]
    fn untransformed_typing_is_not_a_commit() {
        assert!(!is_ime_commit(true, false, "a", Some("a")));
        assert!(!is_ime_commit(true, false, "A", Some("A")));
        assert!(!is_ime_commit(true, false, " ", Some(" ")));
    }

    /// A Chinese IME switched to half-width punctuation inserts exactly
    /// what the key produces — still ordinary typing.
    #[test]
    fn english_punctuation_mode_is_not_a_commit() {
        assert!(!is_ime_commit(true, false, ",", Some(",")));
        assert!(!is_ime_commit(true, false, "<", Some("<")));
    }

    /// A dead-key accent (é) composes first, so marked text exists and
    /// the transformation check is bypassed — the pre-existing path.
    #[test]
    fn composed_text_is_a_commit_regardless_of_the_key() {
        assert!(is_ime_commit(true, true, "é", Some("e")));
        assert!(is_ime_commit(true, true, "你好", Some(" ")));
        // Committing a candidate whose text happens to equal the key.
        assert!(is_ime_commit(true, true, "a", Some("a")));
    }

    /// `insertText:` outside a key event (mouse-picked candidate) has no
    /// characters to compare against; such calls carry marked text.
    #[test]
    fn absent_key_characters_fall_back_to_marked_text() {
        assert!(is_ime_commit(true, true, "你好", None));
        assert!(!is_ime_commit(true, false, "你好", None));
    }

    /// An empty `characters` string is not a match for any inserted
    /// text, so a transformation is still detected.
    #[test]
    fn empty_key_characters_still_detect_a_transformation() {
        assert!(is_ime_commit(true, false, "《", Some("")));
        assert!(!is_ime_commit(true, false, "", Some("")));
    }

    #[test]
    fn control_characters_are_never_a_commit() {
        assert!(!is_ime_commit(true, true, "\r", Some("\r")));
        assert!(!is_ime_commit(true, true, "\t", Some("\t")));
        assert!(!is_ime_commit(true, false, "\u{3}", Some("c")));
    }

    #[test]
    fn a_disabled_ime_never_commits() {
        assert!(!is_ime_commit(false, true, "你好", Some(" ")));
        assert!(!is_ime_commit(false, false, "《", Some("<")));
    }
}
