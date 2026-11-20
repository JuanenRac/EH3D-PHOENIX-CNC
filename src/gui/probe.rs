//! The probe in the window: the entries of the Machine menu and the window that asks before the machine moves (the Z cycle on a touch plate, a
//! grid for the height map) and the two entries that correct the program with the map and put it back. The cycle itself is `probe`, the
//! map is `heightmap`; here there is only the question and the words, which are written in the eight languages of the interface (the others
//! read English).

use super::*;
use crate::heightmap::num;

/// What the operator is asked to confirm.
pub(super) enum Ask {
    /// The Z cycle on a touch plate.
    Z,
    /// A grid of touches over the area (`[x0, y0, x1, y1]` of the cuts of the program, work coordinates).
    Grid {
        area: [f64; 4],
        nx: usize,
        ny: usize,
    },
}

/// The words of this part of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum W {
    MenuZ,
    MenuGrid,
    MenuUseMap,
    MenuDropMap,
    TitleZ,
    TitleGrid,
    BodyZ,
    BodyGrid,
    Go,
    NoCuts,
}

#[cfg(test)]
impl W {
    pub(super) const ALL: [W; 10] = [
        W::MenuZ,
        W::MenuGrid,
        W::MenuUseMap,
        W::MenuDropMap,
        W::TitleZ,
        W::TitleGrid,
        W::BodyZ,
        W::BodyGrid,
        W::Go,
        W::NoCuts,
    ];
}

/// The word in the language (English for a language outside the eight). The order: en, es, de, fr, it, ja, zh, pt.
pub(super) fn word(lang: Lang, w: W) -> &'static str {
    let t: [&'static str; 8] = match w {
        W::MenuZ => return crate::paneltext::text(lang.code(), crate::paneltext::Pt::MenuProbeZ),
        W::MenuGrid => return crate::paneltext::text(lang.code(), crate::paneltext::Pt::MenuProbeGrid),
        W::MenuUseMap => return crate::paneltext::text(lang.code(), crate::paneltext::Pt::MenuUseMap),
        W::MenuDropMap => return crate::paneltext::text(lang.code(), crate::paneltext::Pt::MenuDropMap),
        W::TitleZ => [
            "Probe Z",
            "Sondear Z",
            "Z antasten",
            "Palper Z",
            "Tastare Z",
            "Z プローブ",
            "探测 Z",
            "Sondar Z",
        ],
        W::TitleGrid => [
            "Probe a grid",
            "Sondear una rejilla",
            "Raster antasten",
            "Palper une grille",
            "Tastare una griglia",
            "グリッドをプローブ",
            "探测网格",
            "Sondar uma grelha",
        ],
        W::BodyZ => [
            "The machine will move: the head goes down up to {depth} mm at {feed} mm/min until the probe closes, the Z of the work is set to {plate} mm and the head goes up {retract} mm.",
            "La máquina se moverá: el cabezal baja hasta {depth} mm a {feed} mm/min hasta que la sonda cierre, la Z de la pieza se fija en {plate} mm y el cabezal sube {retract} mm.",
            "Die Maschine bewegt sich: der Kopf fährt bis zu {depth} mm mit {feed} mm/min nach unten, bis der Taster schließt, das Z des Werkstücks wird auf {plate} mm gesetzt und der Kopf fährt {retract} mm hoch.",
            "La machine va bouger : la tête descend jusqu'à {depth} mm à {feed} mm/min jusqu'à ce que la sonde se ferme, le Z de la pièce est réglé à {plate} mm et la tête remonte de {retract} mm.",
            "La macchina si muoverà: la testa scende fino a {depth} mm a {feed} mm/min finché la sonda si chiude, lo Z del pezzo viene impostato a {plate} mm e la testa sale di {retract} mm.",
            "マシンが動きます：プローブが閉じるまで、ヘッドは {feed} mm/min で最大 {depth} mm 下がり、ワークの Z を {plate} mm に設定して {retract} mm 上がります。",
            "机床将会移动：刀头以 {feed} mm/min 的速度最多下降 {depth} mm，直到探针闭合，工件的 Z 设为 {plate} mm，然后刀头上升 {retract} mm。",
            "A máquina vai mover-se: a cabeça desce até {depth} mm a {feed} mm/min até a sonda fechar, o Z da peça é definido em {plate} mm e a cabeça sobe {retract} mm.",
        ],
        W::BodyGrid => [
            "The machine will move: the head probes {points} points ({nx} x {ny}) over {w} x {h} mm, rising to {clear} mm between them. The map is kept and can then correct the program.",
            "La máquina se moverá: el cabezal sondea {points} puntos ({nx} x {ny}) sobre {w} x {h} mm, subiendo a {clear} mm entre ellos. El mapa se guarda y después puede corregir el programa.",
            "Die Maschine bewegt sich: der Kopf tastet {points} Punkte ({nx} x {ny}) auf {w} x {h} mm ab und fährt dazwischen auf {clear} mm hoch. Die Karte wird gespeichert und kann danach das Programm korrigieren.",
            "La machine va bouger : la tête palpe {points} points ({nx} x {ny}) sur {w} x {h} mm, en remontant à {clear} mm entre eux. La carte est conservée et peut ensuite corriger le programme.",
            "La macchina si muoverà: la testa tasta {points} punti ({nx} x {ny}) su {w} x {h} mm, salendo a {clear} mm tra l'uno e l'altro. La mappa viene conservata e poi può correggere il programma.",
            "マシンが動きます：ヘッドは {w} x {h} mm の範囲で {points} 点（{nx} x {ny}）をプローブし、点の間では {clear} mm まで上がります。マップは保存され、その後プログラムを補正できます。",
            "机床将会移动：刀头在 {w} x {h} mm 范围内探测 {points} 个点（{nx} x {ny}），点与点之间升到 {clear} mm。高度图会被保存，之后可用来校正程序。",
            "A máquina vai mover-se: a cabeça sonda {points} pontos ({nx} x {ny}) sobre {w} x {h} mm, subindo a {clear} mm entre eles. O mapa fica guardado e depois pode corrigir o programa.",
        ],
        W::Go => [
            "Probe",
            "Sondear",
            "Antasten",
            "Palper",
            "Tastare",
            "プローブ開始",
            "开始探测",
            "Sondar",
        ],
        W::NoCuts => [
            "Load a program with cutting moves first: the grid covers their area.",
            "Cargue primero un programa con movimientos de corte: la rejilla cubre su área.",
            "Laden Sie zuerst ein Programm mit Schnittbewegungen: das Raster deckt deren Bereich ab.",
            "Chargez d'abord un programme avec des déplacements de coupe : la grille couvre leur zone.",
            "Caricare prima un programma con movimenti di taglio: la griglia copre la loro area.",
            "先に切削移動を含むプログラムを読み込んでください。グリッドはその範囲を覆います。",
            "请先载入含有切削移动的程序：网格覆盖其范围。",
            "Carregue primeiro um programa com movimentos de corte: a grelha cobre a sua área.",
        ],
    };
    let i = match lang.code() {
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

impl App {
    /// Corrects the program with the map and draws the corrected path.
    pub(super) fn use_height_map(&mut self) {
        if self
            .session
            .apply_height_map(self.settings.heightmap_segment_mm)
        {
            self.refresh_path();
        }
    }

    /// The grid over the area of the cuts of the program, with the distance between points of the settings; the log says so when there is no area.
    pub(super) fn grid_ask(&mut self) -> Option<Ask> {
        let mut area: Option<[f64; 4]> = None;
        for s in gcode::toolpath(&self.session.program)
            .iter()
            .filter(|s| !s.rapid)
        {
            for p in [s.from, s.to] {
                area = Some(match area {
                    None => [p.0, p.1, p.0, p.1],
                    Some([x0, y0, x1, y1]) => [x0.min(p.0), y0.min(p.1), x1.max(p.0), y1.max(p.1)],
                });
            }
        }
        let spacing = self.settings.probe_grid_mm;
        match area {
            Some(a) if a[2] > a[0] && a[3] > a[1] => {
                let count = |len: f64| ((len / spacing).ceil() as usize + 1).clamp(2, 100);
                Some(Ask::Grid {
                    area: a,
                    nx: count(a[2] - a[0]),
                    ny: count(a[3] - a[1]),
                })
            }
            _ => {
                self.session
                    .log
                    .push(word(self.lang, W::NoCuts).to_string());
                None
            }
        }
    }

    /// The window that asks before the machine moves.
    pub(super) fn probe_window(&mut self, ctx: &egui::Context, ask: Ask) -> Option<Win> {
        let cfg = self.settings.probe;
        let (title, body) = match &ask {
            Ask::Z => (
                word(self.lang, W::TitleZ),
                word(self.lang, W::BodyZ)
                    .replace("{depth}", &num(cfg.depth_mm))
                    .replace("{feed}", &num(cfg.feed_mm_min))
                    .replace("{plate}", &num(cfg.plate_mm))
                    .replace("{retract}", &num(cfg.retract_mm)),
            ),
            Ask::Grid { area, nx, ny } => (
                word(self.lang, W::TitleGrid),
                word(self.lang, W::BodyGrid)
                    .replace("{points}", &(nx * ny).to_string())
                    .replace("{nx}", &nx.to_string())
                    .replace("{ny}", &ny.to_string())
                    .replace("{w}", &num(area[2] - area[0]))
                    .replace("{h}", &num(area[3] - area[1]))
                    .replace("{clear}", &num(cfg.clearance_mm)),
            ),
        };
        let mut go = false;
        let mut cancel = false;
        self.window(title).default_width(480.0).show(ctx, |ui| {
            ui.colored_label(col(self.theme.warn), &body);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(word(self.lang, W::Go)).clicked() {
                    go = true;
                }
                if ui.button(self.t(Text::McCancel)).clicked() {
                    cancel = true;
                }
            });
        });
        if go {
            match ask {
                Ask::Z => {
                    self.session.probe_z(&cfg, true);
                }
                Ask::Grid { area, nx, ny } => {
                    self.session.probe_grid(area, nx, ny, &cfg, true);
                }
            }
            return None;
        }
        if cancel {
            return None;
        }
        Some(Win::Probe(ask))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::new(&Options::default())
    }

    #[test]
    fn every_word_is_said_in_the_eight_languages() {
        for w in W::ALL {
            let en = word(Lang::En, w);
            assert!(!en.is_empty());
            for l in [
                Lang::Es,
                Lang::De,
                Lang::Fr,
                Lang::It,
                Lang::Ja,
                Lang::Zh,
                Lang::Pt,
            ] {
                let t = word(l, w);
                assert!(!t.trim().is_empty() && t != en, "{w:?} {l:?}");
                // the places for the numbers are the same in every language
                for mark in [
                    "{depth}",
                    "{feed}",
                    "{plate}",
                    "{retract}",
                    "{points}",
                    "{nx}",
                ] {
                    assert_eq!(en.contains(mark), t.contains(mark), "{w:?} {l:?} {mark}");
                }
            }
            // a language outside the eight reads English
            assert_eq!(word(Lang::Ru, w), en);
        }
    }

    #[test]
    fn the_grid_covers_the_cuts_of_the_program_and_not_the_rapids() {
        let mut a = app();
        a.session
            .load("G0 X-50 Y-50\nG0 X10 Y10\nG1 Z-1 F100\nG1 X30 Y20\nG0 Z5");
        a.settings.probe_grid_mm = 10.0;
        let Some(Ask::Grid { area, nx, ny }) = a.grid_ask() else {
            panic!("no grid")
        };
        assert_eq!(area, [10.0, 10.0, 30.0, 20.0]);
        assert_eq!((nx, ny), (3, 2));
    }

    #[test]
    fn a_program_with_no_cuts_has_no_grid_and_the_log_says_why() {
        let mut a = app();
        a.session.load("G0 X10 Y10");
        assert!(a.grid_ask().is_none());
        assert!(a.session.log.iter().any(|l| l.contains("cutting moves")));
    }

    #[test]
    fn without_a_map_the_program_is_not_corrected() {
        let mut a = app();
        a.session.load("G1 X10 F100");
        // no map: nothing changes
        a.use_height_map();
        assert!(!a.session.corrected());
    }
}
