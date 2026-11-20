//! The table of tools of the machine: for each tool number the name, the diameter, the length offset and the number of flutes of the tool
//! that is meant to be in the spindle for it. The values are not in the code: they are in a text file, `phoenix-cnc-tools.txt` next to the
//! settings file (`examples/tools.txt` is a model); with no file there is no table and nothing changes. The program only reads it: the
//! report and the statistics list the tools a program takes with what the table says of each, and a tool that the table does not have is
//! named as a warning (a program may still use it). Nothing here moves anything.
//!
//! A line of the file: `tool = NUMBER | name=NAME | diameter=MM | length=MM | flutes=N | reach=MM | life=MIN | used=MIN`; the diameter is needed, the
//! rest may be left out. `reach` is how far the tool stands out below its holder: the deepest it may go into the material (`holder`). `life` is the minutes of work the tool is good for and `used` the minutes it has worked already: with a life, the
//! report says a tool that is worn out, one that the program would wear out and one that has little life left. Empty lines and lines that
//! start with `#` or `;` are skipped; every other line that cannot be used is left out and named, with its number and the reason, in the
//! report.

use std::path::{Path, PathBuf};

use crate::atc::changes_of;
use crate::heightmap::num;
use crate::profiles::clean_name;
use crate::stats::Stats;
use crate::words::{say, Msg};

/// The most tools a table holds, the highest tool number, the biggest file that is read and the limits of the numbers.
pub const MAX_TOOLS: usize = 64;
pub const MAX_TOOL: u32 = 255;
pub const MAX_FILE_BYTES: u64 = 16 * 1024;
pub const MAX_DIAMETER_MM: f64 = 1000.0;
pub const MAX_LENGTH_MM: f64 = 10_000.0;
pub const MAX_FLUTES: u32 = 64;
/// The biggest life, or minutes used, a line may say.
pub const MAX_LIFE_MIN: f64 = 100_000.0;
/// A tool with this share of its life left or less (in percent) is said to have little left.
pub const LOW_LIFE_PERCENT: f64 = 20.0;

/// The most lines of a kind the report lists one by one.
const LISTED: usize = 20;

/// A tool of the table.
#[derive(Debug, Clone, PartialEq)]
pub struct Tool {
    pub number: u32,
    /// What the operator calls it; may be empty.
    pub name: String,
    pub diameter_mm: f64,
    /// The length offset in mm (0 when the line has none).
    pub length_mm: f64,
    /// The number of flutes (0 when the line has none).
    pub flutes: u32,
    /// How far the tool stands out below its holder, in mm (0 when the line has none: the depth it can cut to is not checked).
    pub reach_mm: f64,
    /// The minutes of work the tool is good for (0 when the line has none: its wear is not followed) and the minutes it has worked.
    pub life_min: f64,
    pub used_min: f64,
}

/// What is wrong with a line of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolError {
    /// The line is not `tool = NUMBER | key=value ...`.
    Syntax,
    /// The tool is not a whole number from 1 to `MAX_TOOL`.
    Number,
    /// The name has nothing in it once the signs of the file are taken out.
    Name,
    /// The diameter is missing or is not a number above 0 up to `MAX_DIAMETER_MM`.
    Diameter,
    /// The length or the number of flutes is not a number inside its limit.
    Other,
    /// The life or the minutes used are not numbers from 0 to `MAX_LIFE_MIN`.
    Life,
    /// The tool is in the table already (the first line stays).
    Repeated,
    /// The table holds `MAX_TOOLS` already.
    Full,
    /// The file is bigger than `MAX_FILE_BYTES` and was not read (line 0).
    TooBig,
    /// The file could not be read (line 0).
    Unreadable,
}

impl ToolError {
    /// The words that name the reason.
    pub fn msg(self) -> Msg {
        match self {
            ToolError::Syntax => Msg::ToolsErrSyntax,
            ToolError::Number => Msg::ToolsErrNumber,
            ToolError::Name => Msg::ToolsErrName,
            ToolError::Diameter => Msg::ToolsErrDiameter,
            ToolError::Other => Msg::ToolsErrOther,
            ToolError::Life => Msg::ToolsErrLife,
            ToolError::Repeated => Msg::ToolsErrRepeated,
            ToolError::Full => Msg::ToolsErrFull,
            ToolError::TooBig => Msg::AtcErrTooBig,
            ToolError::Unreadable => Msg::AtcErrUnreadable,
        }
    }
}

/// A line of the table that was left out, with the reason. `line` counts from 1; 0 is the whole file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Issue {
    pub line: usize,
    pub error: ToolError,
}

/// The tools of the file of the operator and the lines of it that were left out.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolTable {
    pub tools: Vec<Tool>,
    pub issues: Vec<Issue>,
}

fn parse_tool(line: &str) -> Result<Tool, ToolError> {
    let (k, v) = line.split_once('=').ok_or(ToolError::Syntax)?;
    if !k.trim().eq_ignore_ascii_case("tool") {
        return Err(ToolError::Syntax);
    }
    let mut parts = v.split('|');
    let number = parts
        .next()
        .unwrap_or_default()
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|n| (1..=MAX_TOOL).contains(n))
        .ok_or(ToolError::Number)?;
    let (mut name, mut diameter, mut length, mut flutes) = (String::new(), None, 0.0, 0);
    let mut reach = 0.0;
    let (mut life, mut used) = (0.0, 0.0);
    let minutes = |v: &str| {
        v.trim()
            .parse::<f64>()
            .ok()
            .filter(|m| m.is_finite() && (0.0..=MAX_LIFE_MIN).contains(m))
            .ok_or(ToolError::Life)
    };
    for p in parts {
        let (k, v) = p.split_once('=').ok_or(ToolError::Syntax)?;
        match k.trim().to_ascii_lowercase().as_str() {
            "name" => name = clean_name(v).ok_or(ToolError::Name)?,
            "diameter" => {
                diameter = Some(
                    v.trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|d| d.is_finite() && *d > 0.0 && *d <= MAX_DIAMETER_MM)
                        .ok_or(ToolError::Diameter)?,
                )
            }
            "length" => {
                length = v
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|l| l.is_finite() && l.abs() <= MAX_LENGTH_MM)
                    .ok_or(ToolError::Other)?
            }
            "flutes" => {
                flutes = v
                    .trim()
                    .parse::<u32>()
                    .ok()
                    .filter(|f| *f <= MAX_FLUTES)
                    .ok_or(ToolError::Other)?
            }
            "reach" => {
                reach = v
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|r| r.is_finite() && (0.0..=MAX_LENGTH_MM).contains(r))
                    .ok_or(ToolError::Other)?
            }
            "life" => life = minutes(v)?,
            "used" => used = minutes(v)?,
            _ => {}
        }
    }
    Ok(Tool {
        number,
        name,
        diameter_mm: diameter.ok_or(ToolError::Diameter)?,
        length_mm: length,
        flutes,
        reach_mm: reach,
        life_min: life,
        used_min: used,
    })
}

impl ToolTable {
    /// Reads the text of the file (see the top of the module).
    pub fn parse(text: &str) -> ToolTable {
        let mut t = ToolTable::default();
        for (i, raw) in text.trim_start_matches('\u{feff}').lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            let found = parse_tool(line).and_then(|tool| {
                if t.tools.iter().any(|o| o.number == tool.number) {
                    Err(ToolError::Repeated)
                } else if t.tools.len() >= MAX_TOOLS {
                    Err(ToolError::Full)
                } else {
                    Ok(tool)
                }
            });
            match found {
                Ok(tool) => t.tools.push(tool),
                Err(error) => t.issues.push(Issue { line: i + 1, error }),
            }
        }
        t
    }

    /// The tool with this number, when the table has it.
    pub fn find(&self, number: u32) -> Option<&Tool> {
        self.tools.iter().find(|t| t.number == number)
    }

    /// Nothing in the table: no tools and nothing left out.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty() && self.issues.is_empty()
    }
}

/// Where the table of the operator is: next to the settings, with the name of the settings file in front (`phoenix-cnc-tools.txt`).
pub fn table_path(settings: &Path) -> PathBuf {
    let stem = settings.file_stem().map_or_else(
        || "phoenix-cnc".to_string(),
        |s| s.to_string_lossy().to_string(),
    );
    settings.with_file_name(format!("{stem}-tools.txt"))
}

/// The table of the operator: empty when there is no file, with one issue on line 0 when the file is too big or cannot be read.
pub fn load(settings: &Path) -> ToolTable {
    let path = table_path(settings);
    let whole = |error| ToolTable {
        tools: Vec::new(),
        issues: vec![Issue { line: 0, error }],
    };
    match std::fs::metadata(&path) {
        Err(_) => ToolTable::default(),
        Ok(m) if m.len() > MAX_FILE_BYTES => whole(ToolError::TooBig),
        Ok(_) => match std::fs::read(&path) {
            Ok(b) => ToolTable::parse(&String::from_utf8_lossy(&b)),
            Err(_) => whole(ToolError::Unreadable),
        },
    }
}

/// The tools a program takes (`T` then `M6`): each one once, with the line (from 0) where it is first taken, in order.
pub fn taken(lines: &[String]) -> Vec<(usize, u32)> {
    let mut out: Vec<(usize, u32)> = Vec::new();
    for (line, _, tool) in changes_of(lines) {
        if !out.iter().any(|&(_, t)| t == tool) {
            out.push((line, tool));
        }
    }
    out
}

/// The tools the program takes that the table does not have, with the line each is first taken at. Nothing while the table has no tools:
/// a machine with no table has nothing to be missing from.
pub fn missing(lines: &[String], table: &ToolTable) -> Vec<(usize, u32)> {
    if table.tools.is_empty() {
        return Vec::new();
    }
    taken(lines)
        .into_iter()
        .filter(|&(_, tool)| table.find(tool).is_none())
        .collect()
}

/// The lines for the report and the statistics: the size of the table, the lines of the file that were left out, and each tool the program
/// takes with what the table says of it (or that the table does not have it). Nothing when there is no table.
pub fn report_lines(lines: &[String], table: &ToolTable, lang: &str) -> Vec<String> {
    if table.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if !table.tools.is_empty() {
        out.push(say(lang, Msg::ToolsTable, &[table.tools.len().to_string()]));
    }
    for i in table.issues.iter().take(LISTED) {
        let why = say(lang, i.error.msg(), &[] as &[&str]);
        let said = if i.line == 0 {
            say(lang, Msg::ToolsIssueFile, &[why])
        } else {
            say(lang, Msg::ToolsIssue, &[i.line.to_string(), why])
        };
        out.push(format!("  {said}"));
    }
    if table.tools.is_empty() {
        return out;
    }
    for (_, tool) in taken(lines).into_iter().take(LISTED) {
        let said = match table.find(tool) {
            Some(t) => {
                let name = if t.name.is_empty() { "-" } else { &t.name };
                let mut args = vec![tool.to_string(), name.to_string(), num(t.diameter_mm)];
                if t.flutes > 0 {
                    args.push(t.flutes.to_string());
                    say(lang, Msg::ToolsUsedFlutes, &args)
                } else {
                    say(lang, Msg::ToolsUsed, &args)
                }
            }
            None => say(lang, Msg::ToolsMissing, &[tool.to_string()]),
        };
        out.push(format!("  {said}"));
    }
    out
}

/// How a tool stands against its life once the program has run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wear {
    /// The life is used up already.
    WornOut,
    /// The program works the tool longer than the life it has left.
    WillWear,
    /// The tool has `LOW_LIFE_PERCENT` of its life left or less.
    Low,
}

/// One tool of the program that the table follows by its life.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WearNote {
    pub tool: u32,
    pub wear: Wear,
    pub life_min: f64,
    pub used_min: f64,
    /// The minutes the program works the tool and the minutes of life it has left before the program (never below 0).
    pub program_min: f64,
    pub left_min: f64,
}

/// The tools of the program whose life the table follows (a `life` above 0) that are worn out, that the program would wear out or that
/// have little life left, in the order the program takes them. `stats` holds the time each tool works.
pub fn wear_notes(stats: &Stats, table: &ToolTable) -> Vec<WearNote> {
    let mut out = Vec::new();
    for t in &stats.tools {
        let Some(tool) = t.tool else { continue };
        let Some(entry) = table.find(tool).filter(|e| e.life_min > 0.0) else {
            continue;
        };
        let left = (entry.life_min - entry.used_min).max(0.0);
        let program = t.seconds / 60.0;
        let wear = if left <= 0.0 {
            Wear::WornOut
        } else if program > left {
            Wear::WillWear
        } else if left / entry.life_min * 100.0 <= LOW_LIFE_PERCENT {
            Wear::Low
        } else {
            continue;
        };
        out.push(WearNote {
            tool,
            wear,
            life_min: entry.life_min,
            used_min: entry.used_min,
            program_min: program,
            left_min: left,
        });
    }
    out
}

/// The sentences of `wear_notes`, in a language, indented as the other lines of the tool table in the report. Nothing when no tool
/// has a problem.
pub fn wear_lines(stats: &Stats, table: &ToolTable, lang: &str) -> Vec<String> {
    wear_notes(stats, table)
        .into_iter()
        .take(LISTED)
        .map(|n| {
            let said = match n.wear {
                Wear::WornOut => say(
                    lang,
                    Msg::ToolsWornOut,
                    &[n.tool.to_string(), num(n.used_min), num(n.life_min)],
                ),
                Wear::WillWear => say(
                    lang,
                    Msg::ToolsWillWear,
                    &[
                        n.tool.to_string(),
                        format!("{:.1}", n.program_min),
                        format!("{:.1}", n.left_min),
                    ],
                ),
                Wear::Low => say(
                    lang,
                    Msg::ToolsLow,
                    &[
                        n.tool.to_string(),
                        format!("{:.1}", n.left_min),
                        format!("{:.0}", n.left_min / n.life_min * 100.0),
                    ],
                ),
            };
            format!("  {said}")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(str::to_string).collect()
    }

    #[test]
    fn a_line_has_a_tool_a_diameter_and_may_have_a_name_a_length_and_flutes() {
        let t = ToolTable::parse(
            "\u{feff}# tools\n; more\n\ntool = 1 | name=End mill 3 mm | diameter=3 | length=-2.5 | flutes=2\nTOOL = 2 | DIAMETER=0.8\ntool = 3 | diameter=6.35 | name = V bit \n",
        );
        assert!(t.issues.is_empty(), "{:?}", t.issues);
        assert_eq!(
            t.tools[0],
            Tool {
                number: 1,
                name: "End mill 3 mm".to_string(),
                diameter_mm: 3.0,
                length_mm: -2.5,
                flutes: 2,
                reach_mm: 0.0,
                life_min: 0.0,
                used_min: 0.0
            }
        );
        assert_eq!(
            t.tools[1],
            Tool {
                number: 2,
                name: String::new(),
                diameter_mm: 0.8,
                length_mm: 0.0,
                flutes: 0,
                reach_mm: 0.0,
                life_min: 0.0,
                used_min: 0.0
            }
        );
        assert_eq!(t.tools[2].name, "V bit");
        assert_eq!(t.find(3).map(|x| x.diameter_mm), Some(6.35));
        assert!(t.find(4).is_none());
        assert!(!t.is_empty() && ToolTable::default().is_empty());
    }

    #[test]
    fn a_line_that_cannot_be_used_is_left_out_and_named_with_its_number() {
        let text = "tool = 1 | diameter=3\n\
                    tool = 1 | diameter=4\n\
                    tool = 0 | diameter=3\n\
                    tool = 256 | diameter=3\n\
                    tool = x | diameter=3\n\
                    tool = 2\n\
                    tool = 3 | diameter=0\n\
                    tool = 4 | diameter=-1\n\
                    tool = 5 | diameter=1001\n\
                    tool = 6 | diameter=NaN\n\
                    tool = 7 | diameter=3 | length=1e9\n\
                    tool = 8 | diameter=3 | flutes=65\n\
                    tool = 9 | diameter=3 | flutes=-1\n\
                    tool = 10 | diameter=3 | name=|\n\
                    tool = 11 | diameter=3 | name==\n\
                    nonsense\n\
                    material = A | diameter=3\n\
                    tool = 12 | diameter=3 | colour=red\n\
                    tool = 13 | diameter\n";
        let t = ToolTable::parse(text);
        assert_eq!(
            t.tools.iter().map(|x| x.number).collect::<Vec<_>>(),
            [1, 12]
        );
        let got: Vec<(usize, ToolError)> = t.issues.iter().map(|i| (i.line, i.error)).collect();
        assert_eq!(
            got,
            vec![
                (2, ToolError::Repeated),
                (3, ToolError::Number),
                (4, ToolError::Number),
                (5, ToolError::Number),
                (6, ToolError::Diameter),
                (7, ToolError::Diameter),
                (8, ToolError::Diameter),
                (9, ToolError::Diameter),
                (10, ToolError::Diameter),
                (11, ToolError::Other),
                (12, ToolError::Other),
                (13, ToolError::Other),
                (14, ToolError::Name),
                (15, ToolError::Name),
                (16, ToolError::Syntax),
                (17, ToolError::Syntax),
                (19, ToolError::Syntax),
            ]
        );
    }

    #[test]
    fn a_table_holds_64_tools() {
        let text: String = (1..=66)
            .map(|n| format!("tool = {n} | diameter=3\n"))
            .collect();
        let t = ToolTable::parse(&text);
        assert_eq!(t.tools.len(), MAX_TOOLS);
        assert_eq!(
            t.issues,
            vec![
                Issue {
                    line: 65,
                    error: ToolError::Full
                },
                Issue {
                    line: 66,
                    error: ToolError::Full
                }
            ]
        );
    }

    #[test]
    fn the_table_is_read_from_its_file_next_to_the_settings() {
        let dir = std::env::temp_dir().join(format!("eh3d-tools-table-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let settings = dir.join("phoenix-cnc.cfg");
        let file = table_path(&settings);
        assert_eq!(file.file_name().unwrap(), "phoenix-cnc-tools.txt");
        assert!(load(&settings).is_empty());
        std::fs::write(&file, "tool = 1 | diameter=3\ntool = 2\n").unwrap();
        let t = load(&settings);
        assert_eq!((t.tools.len(), t.issues.len()), (1, 1));
        std::fs::write(&file, "#".repeat(MAX_FILE_BYTES as usize + 1)).unwrap();
        assert_eq!(
            load(&settings).issues,
            vec![Issue {
                line: 0,
                error: ToolError::TooBig
            }]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_example_table_is_valid() {
        let t = ToolTable::parse(include_str!("../examples/tools.txt"));
        assert!(t.issues.is_empty(), "{:?}", t.issues);
        assert!(t.tools.len() >= 4);
    }

    #[test]
    fn the_tools_a_program_takes_and_the_ones_the_table_lacks_are_found_once_each() {
        let p = lines("T1 M6\nG1 X1 F10\nT2 M6\nT1 M6\nT7 M6\nT2 M6\nT7 M6\nM30");
        assert_eq!(taken(&p), vec![(0, 1), (2, 2), (4, 7)]);
        let table = ToolTable::parse("tool = 1 | diameter=3\ntool = 2 | diameter=6\n");
        assert_eq!(missing(&p, &table), vec![(4, 7)]);
        // a machine with no table has nothing to be missing from
        assert!(missing(&p, &ToolTable::default()).is_empty());
        assert!(taken(&lines("G1 X1 F10")).is_empty());
    }

    #[test]
    fn the_report_lists_the_table_the_lines_left_out_and_each_tool_taken() {
        let table = ToolTable::parse(
            "tool = 1 | name=End mill | diameter=3 | flutes=2\ntool = 2 | diameter=6\ntool = 3\n",
        );
        let p = lines("T1 M6\nT2 M6\nT9 M6\nM30");
        let r = report_lines(&p, &table, "en");
        assert_eq!(r.len(), 1 + 1 + 3, "{r:?}");
        assert!(r[0].contains('2') && r[0].contains("table"), "{r:?}");
        assert!(r[1].contains("line 3"), "{r:?}");
        assert!(
            r[2].contains("T1")
                && r[2].contains("End mill")
                && r[2].contains("3 mm")
                && r[2].contains("2 flutes"),
            "{r:?}"
        );
        assert!(
            r[3].contains("T2") && r[3].contains("6 mm") && !r[3].contains("flutes"),
            "{r:?}"
        );
        assert!(
            r[4].contains("T9") && r[4].contains("not in the tool table"),
            "{r:?}"
        );
        // in the other languages too
        for lang in ["es", "de", "fr", "it", "ja", "zh", "pt"] {
            let o = report_lines(&p, &table, lang);
            assert_eq!(o.len(), r.len(), "{lang}");
            assert_ne!(o[0], r[0], "{lang}");
            assert_ne!(o[4], r[4], "{lang}");
        }
        // no table, nothing; a file that was not read is told as a whole, with nothing about the program
        assert!(report_lines(&p, &ToolTable::default(), "en").is_empty());
        let big = ToolTable {
            tools: Vec::new(),
            issues: vec![Issue {
                line: 0,
                error: ToolError::TooBig,
            }],
        };
        let said = report_lines(&p, &big, "en");
        assert_eq!(said.len(), 1);
        assert!(
            said[0].contains("16 KiB") && !said[0].contains("T1"),
            "{said:?}"
        );
    }

    const RATES: crate::sim::Rates = crate::sim::Rates {
        rapid: 6000.0,
        default_feed: 300.0,
    };

    /// Four tools that each work 2 minutes (120 mm at 60 mm/min).
    const FOUR: &str = "T1 M6\nG1 X120 F60\nT2 M6\nG1 X240\nT3 M6\nG1 X360\nT4 M6\nG1 X480\nM30";

    fn worn(table: &str, program: &str) -> Vec<WearNote> {
        let stats = crate::stats::measure(&lines(program), RATES, 0.0);
        wear_notes(&stats, &ToolTable::parse(table))
    }

    #[test]
    fn the_life_and_the_minutes_used_of_a_tool_are_read_and_checked() {
        let t = ToolTable::parse(
            "tool = 1 | diameter=3 | life=120 | used=30.5\ntool = 2 | diameter=3 | life=60\ntool = 3 | diameter=3 | used=5\ntool = 4 | diameter=3\n",
        );
        assert!(t.issues.is_empty(), "{:?}", t.issues);
        let got: Vec<(f64, f64)> = t.tools.iter().map(|x| (x.life_min, x.used_min)).collect();
        assert_eq!(
            got,
            vec![(120.0, 30.5), (60.0, 0.0), (0.0, 5.0), (0.0, 0.0)]
        );
        // a life or minutes that are not numbers inside 0 to 100000 leave the line out
        let bad = ToolTable::parse(
            "tool = 1 | diameter=3 | life=-1\ntool = 2 | diameter=3 | used=x\ntool = 3 | diameter=3 | life=100001\ntool = 4 | diameter=3 | life=NaN\ntool = 5 | diameter=3 | used=inf\ntool = 6 | diameter=3 | life=100000 | used=0\n",
        );
        assert_eq!(bad.tools.len(), 1);
        assert_eq!(
            bad.issues.iter().map(|i| i.error).collect::<Vec<_>>(),
            vec![ToolError::Life; 5]
        );
    }

    #[test]
    fn a_tool_that_is_worn_out_one_the_program_would_wear_out_and_one_with_little_left_are_found() {
        let table = "tool = 1 | diameter=3 | life=100 | used=100\n\
                     tool = 2 | diameter=3 | life=10 | used=9\n\
                     tool = 3 | diameter=3 | life=100 | used=85\n\
                     tool = 4 | diameter=3 | life=100 | used=10\n";
        let got = worn(table, FOUR);
        let kinds: Vec<(u32, Wear)> = got.iter().map(|n| (n.tool, n.wear)).collect();
        assert_eq!(
            kinds,
            vec![(1, Wear::WornOut), (2, Wear::WillWear), (3, Wear::Low)]
        );
        // the second one works 2 minutes and has 1 left
        assert!((got[1].program_min - 2.0).abs() < 1e-9 && (got[1].left_min - 1.0).abs() < 1e-9);
        assert!((got[2].left_min - 15.0).abs() < 1e-9);
        // 20 % left is little, a bit more is not
        assert_eq!(
            worn("tool = 1 | diameter=3 | life=100 | used=80", FOUR)[0].wear,
            Wear::Low
        );
        assert!(worn("tool = 1 | diameter=3 | life=100 | used=79", FOUR).is_empty());
        // more used than the life is worn out and has nothing left, and a program that just fits the life does not wear it out
        let over = worn("tool = 1 | diameter=3 | life=10 | used=50", FOUR);
        assert_eq!((over[0].wear, over[0].left_min), (Wear::WornOut, 0.0));
        assert!(worn("tool = 1 | diameter=3 | life=300 | used=200", FOUR)
            .iter()
            .all(|n| n.wear != Wear::WillWear));
    }

    #[test]
    fn a_tool_with_no_life_a_tool_not_in_the_table_and_moves_with_no_tool_are_not_followed() {
        // no life, so no wear; tool 9 is not in the table; the move before any tool has none
        let table = "tool = 1 | diameter=3 | used=500\ntool = 2 | diameter=3 | life=1000\n";
        let p = "G1 X10 F60\nT1 M6\nG1 X130\nT9 M6\nG1 X250\nT2 M6\nG1 X370\nM30";
        assert!(worn(table, p).is_empty());
        assert!(worn("", p).is_empty());
        assert!(wear_lines(
            &crate::stats::measure(&lines(p), RATES, 0.0),
            &ToolTable::default(),
            "en"
        )
        .is_empty());
    }

    #[test]
    fn the_wear_lines_follow_the_language_and_carry_the_minutes() {
        let table = ToolTable::parse(
            "tool = 1 | diameter=3 | life=100 | used=100\ntool = 2 | diameter=3 | life=10 | used=9\ntool = 3 | diameter=3 | life=100 | used=85\n",
        );
        let stats = crate::stats::measure(&lines(FOUR), RATES, 0.0);
        let en = wear_lines(&stats, &table, "en");
        assert_eq!(en.len(), 3);
        assert_eq!(
            en[0],
            "  Tool 1 is worn out: 100 min used of 100 min of life"
        );
        assert!(
            en[1].contains("Tool 2 works 2.0 min") && en[1].contains("1.0 min of life left"),
            "{en:?}"
        );
        assert_eq!(en[2], "  Tool 3 has 15.0 min of life left (15 % of it)");
        for lang in ["es", "de", "fr", "it", "ja", "zh", "pt"] {
            let o = wear_lines(&stats, &table, lang);
            assert_eq!(o.len(), 3, "{lang}");
            for (a, b) in o.iter().zip(&en) {
                assert_ne!(a, b, "{lang}");
                assert!(!a.contains('{'), "{lang}: {a}");
            }
        }
        // the list is cut at the limit of the report
        let many: String = (1..=30)
            .map(|n| format!("tool = {n} | diameter=3 | life=1 | used=1\n"))
            .collect();
        let program: String = (1..=30)
            .map(|n| format!("T{n} M6\nG1 X{n} F60\n"))
            .collect();
        let stats = crate::stats::measure(&lines(&program), RATES, 0.0);
        assert_eq!(
            wear_lines(&stats, &ToolTable::parse(&many), "en").len(),
            LISTED
        );
    }
}
