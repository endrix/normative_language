//! A clause's rule in one sentence, for its card in a diagram.
//!
//! A clause's title names it ("A trap is a halt"); its body says what it
//! obliges, in a sentence with the clause's modal word ("Entering
//! `cpu_state_trap` MUST stop the machine permanently."). A diagram's card
//! has room for that sentence and none for the body: the first sentence of
//! the body holding a modal word, as a reader of the card wants it.
//!
//! - Code blocks go, and so do references in brackets (a line number, a
//!   clause id): they are for the reader of the document.
//! - Links keep their text, `**` goes, a code span stays a code span.
//! - A sentence that ends in a colon (one that a list or a table follows)
//!   ends in a full stop instead.
//! - Past [`LONG`] characters a sentence stops at its first colon or
//!   semicolon after the modal word, outside code: what follows is the
//!   detail.

/// The length past which a sentence is cut at its first colon or semicolon.
pub const LONG: usize = 150;

/// The words that make a sentence a rule (RFC 2119, as NDF reads them).
const MODAL_WORDS: &[&str] = &["MUST", "SHALL", "SHOULD", "MAY", "REQUIRED"];

/// The body's first sentence with a modal word, or `None` when it has none.
pub fn rule_sentence(body: &str) -> Option<String> {
    let text = plain(body);
    let chars: Vec<char> = text.chars().collect();
    for sentence in sentences(&chars) {
        let Some(modal_end) = modal_word_end(&sentence) else {
            continue;
        };
        let mut sentence = sentence;
        if sentence.len() > LONG
            && let Some(cut) = first_outside_code(&sentence, modal_end, &[':', ';'])
        {
            sentence.truncate(cut);
            sentence.push('.');
        }
        if sentence.last() == Some(&':') {
            sentence.pop();
            sentence.push('.');
        }
        let sentence: String = sentence.into_iter().collect();
        let sentence = sentence.trim();
        if !sentence.is_empty() {
            return Some(sentence.to_owned());
        }
    }
    None
}

/// The body as one line of prose: no code blocks, links as their text, no
/// bold, no references in brackets, whitespace collapsed.
fn plain(body: &str) -> String {
    let mut prose = String::new();
    let mut fenced = false;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if !fenced {
            prose.push_str(line);
            prose.push(' ');
        }
    }
    let prose = without_links(&prose).replace("**", "").replace("__", "");
    let prose = without_brackets(&prose);
    prose.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `[text](target)` as `text`, `[[ID]]` as `ID`; a code span left alone.
fn without_links(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut code = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '`' {
            code = !code;
        } else if !code && c == '[' {
            if chars.get(i + 1) == Some(&'[')
                && let Some(end) = find(&chars, i + 2, &[']', ']'])
            {
                out.extend(&chars[i + 2..end]);
                i = end + 2;
                continue;
            }
            if let Some(close) = find(&chars, i + 1, &[']', '('])
                && let Some(end) = find(&chars, close + 2, &[')'])
            {
                out.extend(&chars[i + 1..close]);
                i = end + 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// The first index at or after `from` where `pattern` starts.
fn find(chars: &[char], from: usize, pattern: &[char]) -> Option<usize> {
    (from..chars.len().saturating_sub(pattern.len() - 1))
        .find(|&at| chars[at..at + pattern.len()] == *pattern)
}

/// The text without what is in round brackets, outside code, nested ones
/// with it, and the space before each.
fn without_brackets(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut code = false;
    let mut depth = 0usize;
    for c in text.chars() {
        match c {
            '`' if depth == 0 => {
                code = !code;
                out.push(c);
            }
            '(' if !code => {
                if depth == 0 {
                    while out.ends_with(' ') {
                        out.pop();
                    }
                }
                depth += 1;
            }
            ')' if !code && depth > 0 => depth -= 1,
            _ if depth > 0 => {}
            _ => out.push(c),
        }
    }
    out
}

/// The sentences of a line of prose: each ends at a `.`, `!`, `?` or `:`
/// outside code that a space and a capital, a code span or a `⟨` follow.
fn sentences(chars: &[char]) -> Vec<Vec<char>> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut code = false;
    for (i, &c) in chars.iter().enumerate() {
        if c == '`' {
            code = !code;
            continue;
        }
        if code || !matches!(c, '.' | '!' | '?' | ':') {
            continue;
        }
        if chars.get(i + 1) == Some(&' ')
            && chars
                .get(i + 2)
                .is_some_and(|next| next.is_uppercase() || matches!(next, '`' | '⟨'))
        {
            out.push(chars[start..=i].to_vec());
            start = i + 2;
        }
    }
    if start < chars.len() {
        out.push(chars[start..].to_vec());
    }
    out
}

/// Where the sentence's first modal word outside code ends, if it has one.
fn modal_word_end(sentence: &[char]) -> Option<usize> {
    let mut code = false;
    let mut i = 0;
    while i < sentence.len() {
        let c = sentence[i];
        if c == '`' {
            code = !code;
            i += 1;
            continue;
        }
        let starts_word = i == 0 || !sentence[i - 1].is_alphanumeric();
        if !code && starts_word && c.is_ascii_uppercase() {
            let end = (i..sentence.len())
                .find(|&at| !sentence[at].is_alphanumeric())
                .unwrap_or(sentence.len());
            let word: String = sentence[i..end].iter().collect();
            if MODAL_WORDS.contains(&word.as_str()) {
                return Some(end);
            }
            i = end;
            continue;
        }
        i += 1;
    }
    None
}

/// The first of `marks` at or after `from`, outside code.
fn first_outside_code(sentence: &[char], from: usize, marks: &[char]) -> Option<usize> {
    let mut code = false;
    for (i, &c) in sentence.iter().enumerate() {
        if c == '`' {
            code = !code;
        } else if i >= from && !code && marks.contains(&c) {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_sentence_with_a_modal_word() {
        let body = "Three conditions stop the machine.\n\nEntering `cpu_state_trap` MUST stop the machine\npermanently: the state is absorbing, and the only exit is reset.\n\nMore follows.";
        assert_eq!(
            rule_sentence(body).as_deref(),
            Some(
                "Entering `cpu_state_trap` MUST stop the machine permanently: the state is absorbing, and the only exit is reset."
            )
        );
    }

    #[test]
    fn a_long_sentence_stops_at_its_first_colon_after_the_modal_word_outside_code() {
        let body = "Entering `cpu_state_trap` MUST stop the machine permanently: the state is absorbing, `trap` is asserted and stays asserted, and the only exit is reset, which nothing else offers.";
        assert!(body.len() > LONG);
        assert_eq!(
            rule_sentence(body).as_deref(),
            Some("Entering `cpu_state_trap` MUST stop the machine permanently.")
        );
        // A colon inside a code span is no place to stop.
        let body = "The core MUST implement a state register `cpu_state` read at `picorv32.v:1374`, holding exactly these eight states, one-hot in eight bits, and nothing else at any time in any configuration whatsoever.";
        assert!(body.len() > LONG);
        assert_eq!(rule_sentence(body).as_deref(), Some(body));
    }

    #[test]
    fn references_in_brackets_links_and_bold_are_for_the_document() {
        let body = "The core MUST implement a second state register `mem_state`, a two-bit index (`picorv32.v:351`), advanced independently of `cpu_state` ([[PIPE-FSM-001]]); the **only** link is [the flags](fsm.md#L3).";
        assert_eq!(
            rule_sentence(body).as_deref(),
            Some(
                "The core MUST implement a second state register `mem_state`, a two-bit index, advanced independently of `cpu_state`; the only link is the flags."
            )
        );
    }

    #[test]
    fn a_sentence_a_list_follows_ends_in_a_full_stop() {
        let body = "With `CATCH_MISALIGN`, the core MUST check two alignments: Five obligations.\n\n1. The first.";
        assert_eq!(
            rule_sentence(body).as_deref(),
            Some("With `CATCH_MISALIGN`, the core MUST check two alignments.")
        );
    }

    #[test]
    fn a_code_block_is_no_sentence_and_a_body_without_a_modal_word_has_no_rule() {
        let body = "Cycles per instruction:\n\n```text ndf:normative\nload MUST take 5\n```\n\nThe table is informative.";
        assert_eq!(rule_sentence(body), None);
        assert_eq!(rule_sentence(""), None);
        // A modal word in a code span, or inside another word, is no rule.
        assert_eq!(
            rule_sentence("The flag `MUST_FLUSH` is set. MAYBE later."),
            None
        );
    }

    #[test]
    fn a_marker_starts_a_sentence() {
        let body = "The machines MUST be coupled. ⟨COMB-1⟩ applies.";
        assert_eq!(
            rule_sentence(body).as_deref(),
            Some("The machines MUST be coupled.")
        );
    }
}
