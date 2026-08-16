//! Replicates npm `slugify`'s default-options behavior (`slugify(item)`,
//! no options object — the only way this crate ever needs it called). Not
//! a general-purpose slug function: `lower` defaults to `false` upstream,
//! so casing is preserved, and the allowed character set is wider than the
//! `[a-z0-9-]` most "slug" crates assume.

use std::collections::HashMap;
use std::sync::LazyLock;

use unicode_normalization::UnicodeNormalization;

/// The same char->replacement table npm `slugify` embeds (locale overrides
/// omitted: adc never passes a `locale` option, so they'd never apply).
static CHAR_MAP: LazyLock<HashMap<char, String>> = LazyLock::new(|| {
    let raw: HashMap<String, String> =
        serde_json::from_str(include_str!("slugify_charmap.json")).expect("bundled charmap is valid JSON");
    raw.into_iter().map(|(k, v)| (k.chars().next().expect("charmap keys are single characters"), v)).collect()
});

/// Characters the default `remove` regex (`/[^\w\s$*_+~.()'"!\-:@]+/g`) lets
/// through beyond `\w` (ASCII alphanumeric + `_`) and `\s`.
const EXTRA_ALLOWED: &str = "$*+~.()'\"!-:@";

fn is_allowed(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c.is_whitespace() || EXTRA_ALLOWED.contains(c)
}

pub fn slugify(input: &str) -> String {
    let mut mapped = String::new();
    for c in input.nfc() {
        let owned;
        let chunk: &str = match CHAR_MAP.get(&c) {
            Some(replacement) => replacement.as_str(),
            None => {
                owned = c.to_string();
                &owned
            }
        };
        // A mapped (or literal) chunk equal to the replacement char is
        // folded into whitespace instead of inserted directly, so it goes
        // through the same trim+collapse pass below rather than surviving
        // as a literal char at the very start/end of the result.
        let chunk = if chunk == "-" { " " } else { chunk };
        mapped.extend(chunk.chars().filter(|&rc| is_allowed(rc)));
    }

    let trimmed = mapped.trim();
    let mut result = String::with_capacity(trimmed.len());
    let mut prev_was_space = false;
    for c in trimmed.chars() {
        if c.is_whitespace() {
            if !prev_was_space {
                result.push('-');
            }
            prev_was_space = true;
        } else {
            result.push(c);
            prev_was_space = false;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spaces_become_a_single_hyphen() {
        assert_eq!(slugify("some name"), "some-name");
    }

    #[test]
    fn casing_is_preserved() {
        assert_eq!(slugify("Some Name"), "Some-Name");
    }

    #[test]
    fn diacritics_are_transliterated() {
        assert_eq!(slugify("café"), "cafe");
    }

    #[test]
    fn multiple_consecutive_spaces_collapse_to_one_hyphen() {
        assert_eq!(slugify("a   b"), "a-b");
    }

    #[test]
    fn a_leading_en_dash_is_trimmed_away_like_whitespace() {
        assert_eq!(slugify("\u{2013}leading"), "leading");
    }

    #[test]
    fn allowed_punctuation_survives() {
        assert_eq!(slugify("get_user(v1)!"), "get_user(v1)!");
    }

    #[test]
    fn disallowed_punctuation_is_stripped() {
        assert_eq!(slugify("a/b?c=d"), "abcd");
    }
}
