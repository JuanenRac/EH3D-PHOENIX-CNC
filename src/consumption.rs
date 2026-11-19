//! How much of its life a program uses up in each tool, by the material that is cut. The life of a tool in the table of tools is taken
//! as its life in wood (a factor of 1); harder or more abrasive materials use it up faster, and softer ones slower. The factors are rule
//! of thumb numbers by the kind of material (named in the eight languages of the interface, matched in the name of the material in the
//! settings), not measures of a machine: the report says "about", and a material it does not know gets no estimate.
//!
//! The minutes a tool works are those of its moves in the statistics, the same as the wear notes use. Nothing here moves a machine.

use crate::stats::Stats;
use crate::tools::ToolTable;
use crate::words::{say, Msg};

/// The kinds of material, with the words that name them and how many times faster than wood they use a tool up. A word of up to three
/// letters has to be a word of the name on its own (`abs`, `pla`, `fr4`); a longer one may be part of a word (`alumin` in `aluminium`).
const CLASSES: &[(f64, &[&str])] = &[
    (
        0.5,
        &[
            "foam", "wax", "balsa", "espuma", "schaum", "mousse", "schiuma", "泡",
        ],
    ),
    (
        1.0,
        &[
            "wood", "mdf", "plywood", "pine", "oak", "birch", "beech", "madera", "holz", "bois",
            "legno", "madeira", "木",
        ],
    ),
    (
        1.5,
        &[
            "plastic",
            "acrylic",
            "pmma",
            "pla",
            "abs",
            "pvc",
            "nylon",
            "delrin",
            "plástico",
            "plastico",
            "kunststoff",
            "plastique",
            "plastica",
            "プラスチック",
            "塑料",
            "acrílico",
        ],
    ),
    (
        3.0,
        &["alumin", "alu", "アルミ", "铝", "aluminio", "alumínio"],
    ),
    (
        4.0,
        &[
            "brass", "copper", "bronze", "latón", "laton", "cobre", "messing", "kupfer", "laiton",
            "cuivre", "ottone", "rame", "latão", "真鍮", "黄铜", "铜", "bronce", "bronzo",
        ],
    ),
    (
        6.0,
        &[
            "fr4",
            "pcb",
            "g10",
            "fiberglass",
            "fibreglass",
            "carbon",
            "composite",
            "glass",
            "fibra",
            "composito",
            "玻纤",
            "カーボン",
            "碳纤维",
        ],
    ),
    (
        10.0,
        &[
            "steel",
            "stainless",
            "inox",
            "iron",
            "titanium",
            "acero",
            "stahl",
            "acier",
            "acciaio",
            "aço",
            "hierro",
            "eisen",
            "fer",
            "ferro",
            "titanio",
            "titane",
            "鋼",
            "钢",
            "鉄",
            "铁",
        ],
    ),
];

/// How many times faster than wood the material of this name uses a tool up, or nothing when the name is empty or names no kind that
/// is known. The first kind (from the softest) with a word in the name wins.
pub fn factor(material: &str) -> Option<f64> {
    let lower = material.trim().to_lowercase();
    if lower.is_empty() {
        return None;
    }
    let tokens: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    CLASSES
        .iter()
        .find(|(_, keys)| {
            keys.iter().any(|k| {
                if k.is_ascii() && k.len() <= 3 {
                    tokens.contains(k)
                } else {
                    lower.contains(k)
                }
            })
        })
        .map(|&(f, _)| f)
}

/// One tool of the program that the table follows by its life.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Use {
    pub tool: u32,
    /// The minutes the program works the tool.
    pub minutes: f64,
    pub factor: f64,
    /// The minutes of life that work uses up in this material (`minutes` times `factor`).
    pub life_min: f64,
    /// That as a share of the whole life of the tool, in percent.
    pub life_percent: f64,
    /// How many more runs of this program the tool has life for, counting the minutes it has used already (0 when it has none left).
    pub runs_left: u64,
}

/// What the program uses up of each tool whose life the table follows, in the order the program takes them. Nothing when the material
/// is not known or the program does not work a tool of the table that has a life.
pub fn uses(stats: &Stats, table: &ToolTable, material: &str) -> Vec<Use> {
    let Some(f) = factor(material) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for t in &stats.tools {
        let Some(tool) = t.tool else { continue };
        let Some(e) = table.find(tool).filter(|e| e.life_min > 0.0) else {
            continue;
        };
        let minutes = t.seconds / 60.0;
        let used = minutes * f;
        if !(used.is_finite() && used > 0.0) {
            continue;
        }
        let left = (e.life_min - e.used_min).max(0.0);
        out.push(Use {
            tool,
            minutes,
            factor: f,
            life_min: used,
            life_percent: used / e.life_min * 100.0,
            runs_left: (left / used).floor() as u64,
        });
    }
    out
}

/// The sentences of `uses` for the report, in a language, indented as the other lines of the tool table.
pub fn use_lines(stats: &Stats, table: &ToolTable, material: &str, lang: &str) -> Vec<String> {
    uses(stats, table, material)
        .into_iter()
        .map(|u| {
            let said = say(
                lang,
                Msg::ToolUse,
                &[
                    u.tool.to_string(),
                    format!("{:.1}", u.minutes),
                    material.trim().to_string(),
                    format!("{:.1}", u.life_min),
                    format!("{:.1}", u.life_percent),
                    u.runs_left.to_string(),
                ],
            );
            format!("  {said}")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::Rates;
    use crate::stats::measure;

    fn stats(program: &str) -> Stats {
        let lines: Vec<String> = program.lines().map(str::to_string).collect();
        measure(
            &lines,
            Rates {
                rapid: 6000.0,
                default_feed: 300.0,
            },
            0.0,
        )
    }

    // tool 1 cuts 600 mm at 600 mm/min: one minute of work
    const ONE_MINUTE: &str = "T1 M6\nG1 X600 F600";

    #[test]
    fn the_kind_of_material_is_found_in_its_name_in_the_languages_of_the_interface() {
        for (name, f) in [
            ("Oak", 1.0),
            ("MDF 18 mm", 1.0),
            ("Aluminium 6061", 3.0),
            ("aluminio", 3.0),
            ("Latón", 4.0),
            ("Messing", 4.0),
            ("FR4 board", 6.0),
            ("Stahl", 10.0),
            ("acciaio inox", 10.0),
            ("ABS", 1.5),
            ("Foam", 0.5),
            ("木材", 1.0),
            ("钢板", 10.0),
            ("アルミ板", 3.0),
        ] {
            assert_eq!(factor(name), Some(f), "{name}");
        }
    }

    #[test]
    fn a_short_word_must_be_a_word_of_the_name_and_a_name_that_is_not_known_has_no_factor() {
        // "pla" is not "plank" and "abs" is not "tabs"
        assert_eq!(factor("plank of pine"), Some(1.0));
        assert_eq!(factor("tabs"), None);
        assert_eq!(factor("PLA"), Some(1.5));
        assert_eq!(factor(""), None);
        assert_eq!(factor("   "), None);
        assert_eq!(factor("unobtainium"), None);
    }

    #[test]
    fn a_harder_material_uses_more_of_the_life_of_the_tool() {
        let t = ToolTable::parse("tool = 1 | diameter=3 | life=100 | used=40");
        let s = stats(ONE_MINUTE);
        let wood = uses(&s, &t, "oak");
        assert_eq!(wood.len(), 1);
        assert_eq!(wood[0].tool, 1);
        assert!((wood[0].minutes - 1.0).abs() < 0.05, "{}", wood[0].minutes);
        assert!((wood[0].life_min - wood[0].minutes).abs() < 1e-9);
        // 60 minutes left at one minute a run
        assert!(
            (59..=60).contains(&wood[0].runs_left),
            "{}",
            wood[0].runs_left
        );
        let steel = uses(&s, &t, "steel");
        assert!((steel[0].life_min - steel[0].minutes * 10.0).abs() < 1e-9);
        assert!((steel[0].life_percent - steel[0].life_min).abs() < 1e-9);
        assert!(
            (5..=6).contains(&steel[0].runs_left),
            "{}",
            steel[0].runs_left
        );
        assert!(steel[0].life_percent > wood[0].life_percent * 9.0);
    }

    #[test]
    fn a_tool_with_no_life_a_tool_not_in_the_table_and_an_unknown_material_give_nothing() {
        let s = stats(ONE_MINUTE);
        assert!(uses(&s, &ToolTable::parse("tool = 1 | diameter=3"), "oak").is_empty());
        assert!(uses(
            &s,
            &ToolTable::parse("tool = 2 | diameter=3 | life=50"),
            "oak"
        )
        .is_empty());
        assert!(uses(
            &s,
            &ToolTable::parse("tool = 1 | diameter=3 | life=50"),
            "unknown"
        )
        .is_empty());
        assert!(uses(&s, &ToolTable::parse("tool = 1 | diameter=3 | life=50"), "").is_empty());
        // moves before any tool is taken belong to no tool
        let none = stats("G1 X600 F600");
        assert!(uses(
            &none,
            &ToolTable::parse("tool = 1 | diameter=3 | life=50"),
            "oak"
        )
        .is_empty());
    }

    #[test]
    fn a_tool_that_has_no_life_left_has_no_runs_left() {
        let t = ToolTable::parse("tool = 1 | diameter=3 | life=10 | used=12");
        let u = uses(&stats(ONE_MINUTE), &t, "aluminium");
        assert_eq!(u[0].runs_left, 0);
    }

    #[test]
    fn the_sentence_names_the_tool_the_material_and_the_numbers_in_every_language() {
        let t = ToolTable::parse("tool = 1 | diameter=3 | life=100");
        let s = stats(ONE_MINUTE);
        let en = use_lines(&s, &t, "Steel", "en");
        assert_eq!(en.len(), 1);
        assert!(en[0].starts_with("  Tool 1: "), "{}", en[0]);
        assert!(
            en[0].contains("Steel") && en[0].contains("10.0 %"),
            "{}",
            en[0]
        );
        for lang in crate::words::LANGS {
            let l = use_lines(&s, &t, "Steel", lang);
            assert_eq!(l.len(), 1, "{lang}");
            assert!(l[0].contains("Steel"), "{lang}: {}", l[0]);
            assert!(!l[0].contains('{'), "{lang}: {}", l[0]);
        }
        assert!(use_lines(&s, &t, "", "en").is_empty());
    }
}
