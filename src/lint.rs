//! What looks wrong in a G-code program even when every move is inside the travel: a rapid move that ends below the surface, a cut with the
//! spindle off, a spindle started with no speed, a tool change a simple controller does not take, and no end of program. They are warnings
//! to read before the start, never a refusal; the checks of the summary (travel, size, arcs) stay in `plan`.
//!
//! The words are written here in the eight languages of the interface (the others read English); `lang` is the code of the language
//! (`en`, `es`, `de`, `fr`, `it`, `ja`, `zh`, `pt`). It knows nothing of the window, so it is tested on its own.

use crate::gcode::words;

/// What the check looks for; both values come from the settings file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LintCfg {
    /// The checks are made (off: the program is only measured).
    pub enabled: bool,
    /// A rapid move (`G0`) that ends with Z below this value (work coordinates, mm) is said.
    pub rapid_floor_mm: f64,
    /// A feed rate above this (mm/min) or a spindle speed above this (rpm) is said; 0 turns the check off.
    pub max_feed_mm_min: f64,
    pub max_rpm: f64,
}

impl Default for LintCfg {
    fn default() -> Self {
        LintCfg {
            enabled: true,
            rapid_floor_mm: 0.0,
            max_feed_mm_min: 0.0,
            max_rpm: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A `G0` that ends below the floor: the tool would rush into the stock.
    RapidDown,
    /// A cutting move while no `M3` or `M4` is in force.
    CutWithoutSpindle,
    /// `M3` or `M4` with no `S` anywhere before it.
    SpindleWithoutSpeed,
    /// `M6`: a simple controller refuses it.
    ToolChange,
    /// An `F` above the most the machine feeds.
    FeedTooHigh,
    /// An `S` above the most the spindle turns.
    SpeedTooHigh,
    /// The program moves and has no `M2` or `M30`.
    NoEnd,
}

/// One thing that looks wrong: where it first happens (the line, from 1) and how many times.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Finding {
    pub kind: Kind,
    pub line: usize,
    pub count: usize,
    /// The limit that was passed (feed or speed), 0 for the other kinds.
    pub limit: f64,
}

fn template(kind: Kind, lang: &str) -> &'static str {
    let main = lang.split('-').next().unwrap_or("en");
    let t: [&'static str; 8] = match kind {
        Kind::RapidDown => [
            "{n} rapid moves end below the surface (first at line {line}): the tool would rush into the material",
            "{n} movimientos rápidos terminan por debajo de la superficie (el primero en la línea {line}): la herramienta se lanzaría contra el material",
            "{n} Eilgänge enden unter der Oberfläche (der erste in Zeile {line}): das Werkzeug würde ins Material rasen",
            "{n} déplacements rapides finissent sous la surface (le premier à la ligne {line}) : l'outil se précipiterait dans la matière",
            "{n} movimenti rapidi finiscono sotto la superficie (il primo alla riga {line}): l'utensile si lancerebbe nel materiale",
            "{n} 件の早送りが表面より下で終わります（最初は {line} 行目）：工具が材料に突っ込みます",
            "{n} 个快速移动终止于表面以下（第一个在第 {line} 行）：刀具会冲入材料",
            "{n} movimentos rápidos terminam abaixo da superfície (o primeiro na linha {line}): a ferramenta lançar-se-ia contra o material",
        ],
        Kind::CutWithoutSpindle => [
            "{n} cutting moves run with the spindle off (first at line {line})",
            "{n} movimientos de corte con el husillo parado (el primero en la línea {line})",
            "{n} Schnittbewegungen bei stehender Spindel (die erste in Zeile {line})",
            "{n} déplacements de coupe avec la broche à l'arrêt (le premier à la ligne {line})",
            "{n} movimenti di taglio con il mandrino fermo (il primo alla riga {line})",
            "{n} 件の切削移動がスピンドル停止のまま実行されます（最初は {line} 行目）",
            "{n} 个切削移动在主轴停止时执行（第一个在第 {line} 行）",
            "{n} movimentos de corte com o fuso parado (o primeiro na linha {line})",
        ],
        Kind::SpindleWithoutSpeed => [
            "The spindle is started at line {line} and no speed (S) was set before it",
            "El husillo se arranca en la línea {line} y no se fijó ninguna velocidad (S) antes",
            "Die Spindel wird in Zeile {line} gestartet, davor wurde keine Drehzahl (S) gesetzt",
            "La broche démarre à la ligne {line} et aucune vitesse (S) n'a été réglée avant",
            "Il mandrino parte alla riga {line} e prima non è stata impostata alcuna velocità (S)",
            "{line} 行目でスピンドルが始動しますが、それ以前に回転数（S）が設定されていません",
            "第 {line} 行启动主轴，但之前没有设置转速（S）",
            "O fuso arranca na linha {line} e nenhuma velocidade (S) foi definida antes",
        ],
        Kind::ToolChange => [
            "A tool change (M6) at line {line}: a simple controller refuses it",
            "Un cambio de herramienta (M6) en la línea {line}: un controlador sencillo lo rechaza",
            "Ein Werkzeugwechsel (M6) in Zeile {line}: eine einfache Steuerung lehnt ihn ab",
            "Un changement d'outil (M6) à la ligne {line} : un contrôleur simple le refuse",
            "Un cambio utensile (M6) alla riga {line}: un controllore semplice lo rifiuta",
            "{line} 行目の工具交換（M6）：簡易コントローラーは拒否します",
            "第 {line} 行有换刀指令（M6）：简单的控制器会拒绝",
            "Uma troca de ferramenta (M6) na linha {line}: um controlador simples recusa-a",
        ],
        Kind::FeedTooHigh => [
            "{n} feed rates are above the limit of {limit} mm/min (first at line {line})",
            "{n} avances superan el límite de {limit} mm/min (el primero en la línea {line})",
            "{n} Vorschübe liegen über dem Grenzwert von {limit} mm/min (der erste in Zeile {line})",
            "{n} avances dépassent la limite de {limit} mm/min (la première à la ligne {line})",
            "{n} avanzamenti superano il limite di {limit} mm/min (il primo alla riga {line})",
            "{n} 件の送り速度が上限 {limit} mm/min を超えています（最初は {line} 行目）",
            "{n} 个进给速度超过上限 {limit} mm/min（第一个在第 {line} 行）",
            "{n} avanços ultrapassam o limite de {limit} mm/min (o primeiro na linha {line})",
        ],
        Kind::SpeedTooHigh => [
            "{n} spindle speeds are above the limit of {limit} rpm (first at line {line})",
            "{n} velocidades del husillo superan el límite de {limit} rpm (la primera en la línea {line})",
            "{n} Spindeldrehzahlen liegen über dem Grenzwert von {limit} U/min (die erste in Zeile {line})",
            "{n} vitesses de broche dépassent la limite de {limit} tr/min (la première à la ligne {line})",
            "{n} velocità del mandrino superano il limite di {limit} giri/min (la prima alla riga {line})",
            "{n} 件の主軸回転数が上限 {limit} rpm を超えています（最初は {line} 行目）",
            "{n} 个主轴转速超过上限 {limit} rpm（第一个在第 {line} 行）",
            "{n} velocidades do fuso ultrapassam o limite de {limit} rpm (a primeira na linha {line})",
        ],
        Kind::NoEnd => [
            "The program has no end (M2 or M30): the spindle may stay on",
            "El programa no tiene final (M2 o M30): el husillo puede quedar encendido",
            "Das Programm hat kein Ende (M2 oder M30): die Spindel kann weiterlaufen",
            "Le programme n'a pas de fin (M2 ou M30) : la broche peut rester allumée",
            "Il programma non ha una fine (M2 o M30): il mandrino può restare acceso",
            "プログラムに終了（M2 または M30）がありません：スピンドルが回り続ける可能性があります",
            "程序没有结束指令（M2 或 M30）：主轴可能保持运转",
            "O programa não tem fim (M2 ou M30): o fuso pode ficar ligado",
        ],
    };
    let i = match main {
        "es" => 1,
        "de" => 2,
        "fr" => 3,
        "it" => 4,
        "ja" => 5,
        "zh" => 6,
        "pt" => 7,
        _ => 0,
    };
    t[i]
}

impl Finding {
    /// The sentence, in the language (English when it is not one of the eight).
    pub fn message(&self, lang: &str) -> String {
        template(self.kind, lang)
            .replace("{line}", &self.line.to_string())
            .replace("{n}", &self.count.to_string())
            .replace("{limit}", &crate::heightmap::num(self.limit))
    }
}

/// Reads the program and lists what looks wrong, in the order of the kinds above.
pub fn lint(lines: &[String], cfg: &LintCfg) -> Vec<Finding> {
    if !cfg.enabled {
        return Vec::new();
    }
    let (mut motion, mut absolute, mut z) = (0_u8, true, 0.0_f64);
    let (mut spindle_on, mut speed_set, mut moves, mut ended) = (false, false, false, false);
    let mut rapid: Option<Finding> = None;
    let mut dry: Option<Finding> = None;
    let mut nospeed: Option<Finding> = None;
    let mut change: Option<Finding> = None;
    let mut fast: Option<Finding> = None;
    let mut spinning: Option<Finding> = None;
    let mut scale = 1.0;
    let bump = |slot: &mut Option<Finding>, kind: Kind, line: usize| match slot {
        Some(f) => f.count += 1,
        None => {
            *slot = Some(Finding {
                kind,
                line,
                count: 1,
                limit: 0.0,
            })
        }
    };
    for (n, raw) in lines.iter().enumerate() {
        let ws = words(raw);
        let mut axis = false;
        let mut new_z = None;
        // the speed of the line itself counts for the start of the same line
        if ws.iter().any(|&(l, _)| l == 'S') {
            speed_set = true;
        }
        for &(l, v) in &ws {
            let tenths = (v * 10.0).round() as i32;
            match l {
                'G' => match tenths {
                    0 => motion = 0,
                    10 => motion = 1,
                    20 | 30 => motion = 2,
                    900 => absolute = true,
                    910 => absolute = false,
                    200 => scale = 25.4,
                    210 => scale = 1.0,
                    _ => {}
                },
                'F' if cfg.max_feed_mm_min > 0.0 && v * scale > cfg.max_feed_mm_min => {
                    bump(&mut fast, Kind::FeedTooHigh, n + 1)
                }
                'S' if cfg.max_rpm > 0.0 && v > cfg.max_rpm => {
                    bump(&mut spinning, Kind::SpeedTooHigh, n + 1)
                }
                'X' | 'Y' => axis = true,
                'Z' => {
                    axis = true;
                    new_z = Some(v);
                }
                'M' => match v as i32 {
                    3 | 4 => {
                        if !speed_set {
                            bump(&mut nospeed, Kind::SpindleWithoutSpeed, n + 1);
                        }
                        spindle_on = true;
                    }
                    5 => spindle_on = false,
                    2 | 30 => {
                        spindle_on = false;
                        ended = true;
                    }
                    6 => bump(&mut change, Kind::ToolChange, n + 1),
                    _ => {}
                },
                _ => {}
            }
        }
        if let Some(z_word) = new_z {
            z = if absolute { z_word } else { z + z_word };
        }
        if !axis {
            continue;
        }
        moves = true;
        if motion == 0 {
            if new_z.is_some() && z < cfg.rapid_floor_mm {
                bump(&mut rapid, Kind::RapidDown, n + 1);
            }
        } else if !spindle_on {
            bump(&mut dry, Kind::CutWithoutSpindle, n + 1);
        }
    }
    for (slot, limit) in [
        (&mut fast, cfg.max_feed_mm_min),
        (&mut spinning, cfg.max_rpm),
    ] {
        if let Some(f) = slot {
            f.limit = limit;
        }
    }
    let mut out: Vec<Finding> = [rapid, dry, nospeed, change, fast, spinning]
        .into_iter()
        .flatten()
        .collect();
    if moves && !ended {
        out.push(Finding {
            kind: Kind::NoEnd,
            line: lines.len(),
            count: 1,
            limit: 0.0,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(str::to_string).collect()
    }

    fn kinds(s: &str) -> Vec<Kind> {
        lint(&lines(s), &LintCfg::default())
            .iter()
            .map(|f| f.kind)
            .collect()
    }

    #[test]
    fn a_good_program_says_nothing() {
        assert!(
            kinds("G21 G90\nG0 Z5\nM3 S1000\nG0 X0 Y0\nG1 Z-1 F100\nG1 X10\nG0 Z5\nM5\nM30")
                .is_empty()
        );
        assert!(kinds("; nothing here").is_empty());
    }

    #[test]
    fn a_rapid_below_the_surface_is_counted() {
        let f = lint(
            &lines("M3 S100\nG0 Z-2\nG0 Z5\nG0 Z-1\nM30"),
            &LintCfg::default(),
        );
        assert_eq!(
            f,
            vec![Finding {
                kind: Kind::RapidDown,
                line: 2,
                count: 2,
                limit: 0.0
            }]
        );
        // the floor is a setting
        let low = LintCfg {
            rapid_floor_mm: -5.0,
            ..LintCfg::default()
        };
        assert!(lint(&lines("M3 S100\nG0 Z-2\nM30"), &low).is_empty());
    }

    #[test]
    fn a_rapid_in_relative_distances_is_followed() {
        // 5 down from 3 is -2
        assert_eq!(
            kinds("M3 S1\nG0 Z3\nG91\nG0 Z-5\nM30"),
            vec![Kind::RapidDown]
        );
    }

    #[test]
    fn a_cut_with_the_spindle_off_and_a_spindle_with_no_speed_are_found() {
        let f = lint(
            &lines("G1 X5 F100\nM3\nG1 X9\nM5\nG1 X1\nM30"),
            &LintCfg::default(),
        );
        let got: Vec<(Kind, usize, usize)> = f.iter().map(|f| (f.kind, f.line, f.count)).collect();
        assert_eq!(
            got,
            vec![
                (Kind::CutWithoutSpindle, 1, 2),
                (Kind::SpindleWithoutSpeed, 2, 1)
            ]
        );
    }

    #[test]
    fn a_speed_on_the_same_line_or_before_counts() {
        assert!(kinds("M3 S900\nG1 X1 F1\nM30").is_empty());
        assert!(kinds("S900\nM3\nG1 X1 F1\nM30").is_empty());
    }

    #[test]
    fn a_tool_change_and_a_missing_end_are_found() {
        let f = lint(&lines("M3 S1\nT1 M6\nG1 X1 F1"), &LintCfg::default());
        let got: Vec<(Kind, usize)> = f.iter().map(|f| (f.kind, f.line)).collect();
        assert_eq!(got, vec![(Kind::ToolChange, 2), (Kind::NoEnd, 3)]);
        // a program with no moves needs no end
        assert!(kinds("M3 S1").is_empty());
    }

    #[test]
    fn a_feed_or_a_speed_above_the_limit_is_found_and_the_limit_is_a_setting() {
        let cfg = LintCfg {
            max_feed_mm_min: 1000.0,
            max_rpm: 12_000.0,
            ..LintCfg::default()
        };
        let p = lines("M3 S24000\nG1 X1 F1500\nG1 X2 F900\nG1 X3 F2000\nM30");
        let got: Vec<(Kind, usize, usize, f64)> = lint(&p, &cfg)
            .iter()
            .map(|f| (f.kind, f.line, f.count, f.limit))
            .collect();
        assert_eq!(
            got,
            vec![
                (Kind::FeedTooHigh, 2, 2, 1000.0),
                (Kind::SpeedTooHigh, 1, 1, 12_000.0)
            ]
        );
        // 0 is off, which is the default
        assert!(lint(&p, &LintCfg::default()).is_empty());
        // inches are told: F40 in inches is 1016 mm/min
        let inch = lines("G20\nM3 S1000\nG1 X1 F40\nM30");
        assert_eq!(lint(&inch, &cfg).len(), 1);
        let metric = lines("G21\nM3 S1000\nG1 X1 F40\nM30");
        assert!(lint(&metric, &cfg).is_empty());
        let m = lint(&p, &cfg)[0].message("en");
        assert!(m.contains("1000 mm/min") && m.contains("line 2"), "{m}");
    }

    #[test]
    fn the_checks_can_be_turned_off() {
        let off = LintCfg {
            enabled: false,
            ..LintCfg::default()
        };
        assert!(lint(&lines("G1 X5 F100\nG0 Z-4"), &off).is_empty());
    }

    #[test]
    fn every_finding_is_said_in_the_eight_languages() {
        let all = [
            Kind::RapidDown,
            Kind::CutWithoutSpindle,
            Kind::SpindleWithoutSpeed,
            Kind::ToolChange,
            Kind::FeedTooHigh,
            Kind::SpeedTooHigh,
            Kind::NoEnd,
        ];
        for kind in all {
            let f = Finding {
                kind,
                line: 7,
                count: 3,
                limit: 1500.0,
            };
            let en = f.message("en");
            for code in ["es", "de", "fr", "it", "ja", "zh", "pt"] {
                let m = f.message(code);
                assert!(!m.contains('{') && !m.trim().is_empty(), "{kind:?} {code}");
                assert_ne!(m, en, "{kind:?} {code} says it in English");
            }
            assert!(!en.contains('{'));
            // a language outside the eight reads English, and a regional code reads its language
            assert_eq!(f.message("ru"), en);
            assert_eq!(f.message("pt-br"), f.message("pt"));
        }
        assert!(Finding {
            kind: Kind::RapidDown,
            line: 7,
            count: 3,
            limit: 0.0
        }
        .message("en")
        .starts_with("3 rapid"));
    }
}
