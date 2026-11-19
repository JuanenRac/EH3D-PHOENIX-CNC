//! The table of tools of the machine: for each tool number the name, the diameter, the length offset and the number of flutes of the tool
//! that is meant to be in the spindle for it. The values are not in the code: they are in a text file, `phoenix-cnc-tools.txt` next to the
//! settings file (`examples/tools.txt` is a model); with no file there is no table and nothing changes. The program only reads it: the
//! report and the statistics list the tools a program takes with what the table says of each, and a tool that the table does not have is
//! named as a warning (a program may still use it). Nothing here moves anything.
//!
//! A line of the file: `tool = NUMBER | name=NAME | diameter=MM | length=MM | flutes=N`; the diameter is needed, the rest may be left out.
//! Empty lines and lines that start with `#` or `;` are skipped; every other line that cannot be used is left out and named, with its
//! number and the reason, in the report.

use std::path::{Path, PathBuf};

use crate::atc::changes_of;
use crate::heightmap::num;
use crate::profiles::clean_name;
use crate::words::{say, Msg};

/// The most tools a table holds, the highest tool number, the biggest file that is read and the limits of the numbers.
pub const MAX_TOOLS: usize = 64;
pub const MAX_TOOL: u32 = 255;
pub const MAX_FILE_BYTES: u64 = 16 * 1024;
pub const MAX_DIAMETER_MM: f64 = 1000.0;
pub const MAX_LENGTH_MM: f64 = 10_000.0;
pub const MAX_FLUTES: u32 = 64;

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
            _ => {}
        }
    }
    Ok(Tool {
        number,
        name,
        diameter_mm: diameter.ok_or(ToolError::Diameter)?,
        length_mm: length,
        flutes,
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
                flutes: 2
            }
        );
        assert_eq!(
            t.tools[1],
            Tool {
                number: 2,
                name: String::new(),
                diameter_mm: 0.8,
                length_mm: 0.0,
                flutes: 0
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
}
