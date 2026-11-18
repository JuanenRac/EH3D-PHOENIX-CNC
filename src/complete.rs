//! The completion of the command box: Tab finishes the code being typed (`G5` offers `G53` to `G59`, `M` offers the M codes, `$` the commands of
//! the controller itself). It only works out text; what is sent is still what the operator typed and confirmed with Enter.

/// The codes offered: the G and M codes of the usual programs and the `$` commands of GRBL that take no value.
pub const CODES: [&str; 69] = [
    "G0", "G1", "G2", "G3", "G4", "G10", "G17", "G18", "G19", "G20", "G21", "G28", "G28.1", "G30",
    "G30.1", "G38.2", "G38.3", "G38.4", "G38.5", "G40", "G43.1", "G49", "G53", "G54", "G55", "G56",
    "G57", "G58", "G59", "G61", "G80", "G90", "G90.1", "G91", "G91.1", "G92", "G92.1", "G93",
    "G94", "M0", "M1", "M2", "M3", "M4", "M5", "M6", "M7", "M8", "M9", "M30", "$H", "$X", "$G",
    "$$", "$#", "$I", "$N", "$C", "$SLP", "$RST=$", "$RST=#", "$RST=*", "$J=", "$10=", "$20=",
    "$21=", "$22=", "$100=", "$130=",
];

/// What Tab does with the text of the box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    /// The text the box holds afterwards (the same when nothing could be added).
    pub text: String,
    /// The codes that fit what was typed, when there are several.
    pub options: Vec<&'static str>,
}

/// The last word of the line (what is being typed) and the text before it; nothing when the line ends with a space or is empty.
fn last_word(line: &str) -> Option<(&str, &str)> {
    if line.is_empty() || line.ends_with(char::is_whitespace) {
        return None;
    }
    let start = line.rfind(char::is_whitespace).map_or(0, |i| {
        i + line[i..].chars().next().map_or(1, char::len_utf8)
    });
    Some((&line[..start], &line[start..]))
}

/// The codes that start with a word, without telling capitals from small letters, in the order of `CODES`.
pub fn matches(word: &str) -> Vec<&'static str> {
    if word.is_empty() {
        return Vec::new();
    }
    let w = word.to_ascii_uppercase();
    CODES
        .iter()
        .copied()
        .filter(|c| c.starts_with(&w))
        .collect()
}

/// The longest start that all the words share.
fn common_start(words: &[&str]) -> String {
    let Some(first) = words.first() else {
        return String::new();
    };
    let mut len = first.len();
    for w in words {
        len = len.min(
            first
                .bytes()
                .zip(w.bytes())
                .take_while(|(a, b)| a == b)
                .count(),
        );
    }
    first[..len].to_string()
}

/// Finishes the last word of the line as far as it is certain: with one code that fits it is written in full (and a space follows unless the
/// code takes a value), with several the start they share is written and they are listed, and with none the line stays as it is.
pub fn complete(line: &str) -> Completion {
    let unchanged = || Completion {
        text: line.to_string(),
        options: Vec::new(),
    };
    let Some((before, word)) = last_word(line) else {
        return unchanged();
    };
    let found = matches(word);
    match found.as_slice() {
        [] => unchanged(),
        [one] => Completion {
            text: if one.ends_with('=') || one.ends_with('$') {
                format!("{before}{one}")
            } else {
                format!("{before}{one} ")
            },
            options: Vec::new(),
        },
        many => {
            let start = common_start(many);
            // an exact word that is also the start of longer ones stays as it is: the list says what else there is
            let text = if start.len() > word.len() {
                format!("{before}{start}")
            } else {
                line.to_string()
            };
            Completion {
                text,
                options: many.to_vec(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_with_one_code_that_fits_is_finished_and_followed_by_a_space() {
        assert_eq!(complete("G38").text, "G38.");
        assert_eq!(complete("g28.").text, "G28.1 ");
        assert_eq!(complete("M30").text, "M30 ");
        assert_eq!(complete("$SL").text, "$SLP ");
        // a code that takes a value ends where the value begins
        assert_eq!(complete("$J").text, "$J=");
        assert_eq!(complete("$13").text, "$130=");
        assert!(complete("M30").options.is_empty());
    }

    #[test]
    fn several_codes_give_their_common_start_and_the_list() {
        let c = complete("G5");
        assert_eq!(c.text, "G5");
        assert_eq!(c.options, ["G53", "G54", "G55", "G56", "G57", "G58", "G59"]);
        let c = complete("G38");
        assert_eq!(c.options, ["G38.2", "G38.3", "G38.4", "G38.5"]);
        assert_eq!(c.text, "G38.");
        // a word that is a code and the start of others stays; the list says what else there is
        let c = complete("G1");
        assert_eq!(c.text, "G1");
        assert!(
            c.options.contains(&"G1") && c.options.contains(&"G10") && c.options.contains(&"G17")
        );
        assert_eq!(complete("M").options.len(), 11);
    }

    #[test]
    fn only_the_last_word_of_the_line_is_completed_and_the_rest_is_kept() {
        assert_eq!(complete("G0 X10 M5").text, "G0 X10 M5 ");
        assert_eq!(complete("G90 g5").options.len(), 7);
        assert_eq!(complete("G91 G92.").text, "G91 G92.1 ");
        // a tab or a space before the word
        assert_eq!(complete("G1\tM9").text, "G1\tM9 ");
    }

    #[test]
    fn nothing_is_added_to_an_empty_line_a_line_that_ends_with_a_space_or_a_word_that_is_not_a_code(
    ) {
        for line in ["", "G1 ", "G1\t", "X10", "Y", "F300", "hello", "G99", "M10"] {
            let c = complete(line);
            assert_eq!(c.text, line, "{line:?}");
            assert!(c.options.is_empty(), "{line:?}");
        }
        assert!(matches("").is_empty());
    }

    #[test]
    fn the_list_has_no_repeats_and_every_code_is_found_by_its_own_letters() {
        let mut seen = std::collections::HashSet::new();
        for c in CODES {
            assert!(seen.insert(c), "{c} twice");
            assert!(matches(c).contains(&c), "{c}");
            assert!(c.starts_with(['G', 'M', '$']), "{c}");
        }
    }

    #[test]
    fn capitals_and_small_letters_are_the_same_and_the_text_written_is_in_capitals() {
        assert_eq!(complete("m5").text, "M5 ");
        assert_eq!(complete("G0 x1 g21").text, "G0 x1 G21 ");
        assert_eq!(complete("é").text, "é");
    }
}
