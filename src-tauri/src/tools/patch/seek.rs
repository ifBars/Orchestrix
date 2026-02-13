//! Sequence seeking with progressive fuzzy matching.
//!
//! Finds a pattern of lines within a file, trying increasingly lenient matching:
//! 1. Exact match
//! 2. Trailing whitespace ignored
//! 3. Leading + trailing whitespace ignored
//! 4. Unicode punctuation normalized to ASCII equivalents

/// Find `pattern` lines within `lines` starting at or after `start`.
/// Returns the index of the first match, or None.
///
/// When `eof` is true, starts searching from end of file for patterns
/// that should match near the bottom.
pub(crate) fn seek_sequence(
    lines: &[String],
    pattern: &[String],
    start: usize,
    eof: bool,
) -> Option<usize> {
    if pattern.is_empty() {
        return Some(start);
    }
    if pattern.len() > lines.len() {
        return None;
    }

    let search_start = if eof && lines.len() >= pattern.len() {
        lines.len() - pattern.len()
    } else {
        start
    };

    // Pass 1: exact match
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        if lines[i..i + pattern.len()] == *pattern {
            return Some(i);
        }
    }

    // Pass 2: trailing whitespace ignored
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        let ok = pattern
            .iter()
            .enumerate()
            .all(|(j, pat)| lines[i + j].trim_end() == pat.trim_end());
        if ok {
            return Some(i);
        }
    }

    // Pass 3: leading + trailing whitespace ignored
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        let ok = pattern
            .iter()
            .enumerate()
            .all(|(j, pat)| lines[i + j].trim() == pat.trim());
        if ok {
            return Some(i);
        }
    }

    // Pass 4: Unicode normalization (typographic -> ASCII)
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        let ok = pattern
            .iter()
            .enumerate()
            .all(|(j, pat)| normalise(&lines[i + j]) == normalise(pat));
        if ok {
            return Some(i);
        }
    }

    None
}

/// Normalize common Unicode punctuation to ASCII equivalents.
fn normalise(s: &str) -> String {
    s.trim()
        .chars()
        .map(|c| match c {
            // Various dashes -> ASCII hyphen
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}'
            | '\u{2212}' => '-',
            // Fancy single quotes -> apostrophe
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => '\'',
            // Fancy double quotes -> ASCII double quote
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => '"',
            // Non-breaking / fancy spaces -> regular space
            '\u{00A0}' | '\u{2002}' | '\u{2003}' | '\u{2004}' | '\u{2005}' | '\u{2006}'
            | '\u{2007}' | '\u{2008}' | '\u{2009}' | '\u{200A}' | '\u{202F}' | '\u{205F}'
            | '\u{3000}' => ' ',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::seek_sequence;

    fn to_vec(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn exact_match() {
        let lines = to_vec(&["foo", "bar", "baz"]);
        let pattern = to_vec(&["bar", "baz"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(1));
    }

    #[test]
    fn trailing_whitespace_match() {
        let lines = to_vec(&["foo   ", "bar\t"]);
        let pattern = to_vec(&["foo", "bar"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn trim_both_sides_match() {
        let lines = to_vec(&["  foo  ", "   bar\t"]);
        let pattern = to_vec(&["foo", "bar"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn unicode_normalisation() {
        // EN DASH in source, ASCII hyphen in pattern
        let lines = vec!["import asyncio  # local \u{2013} dep".to_string()];
        let pattern = vec!["import asyncio  # local - dep".to_string()];
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn pattern_longer_than_input() {
        let lines = to_vec(&["one"]);
        let pattern = to_vec(&["one", "two", "three"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), None);
    }

    #[test]
    fn empty_pattern() {
        let lines = to_vec(&["foo"]);
        let pattern: Vec<String> = Vec::new();
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn eof_flag_starts_from_end() {
        let lines = to_vec(&["a", "b", "c", "b", "c"]);
        let pattern = to_vec(&["b", "c"]);
        // With eof=true, should find the second occurrence
        assert_eq!(seek_sequence(&lines, &pattern, 0, true), Some(3));
    }

    #[test]
    fn respects_start_index() {
        let lines = to_vec(&["a", "b", "a", "b"]);
        let pattern = to_vec(&["a", "b"]);
        assert_eq!(seek_sequence(&lines, &pattern, 1, false), Some(2));
    }
}
