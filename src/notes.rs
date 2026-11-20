//! The words of the importers and of the planner tool in the eight languages of the suite (English, Spanish, German, French, Italian,
//! Japanese, Chinese and Portuguese of Portugal): what a reader did not understand, why a file could not be imported, and the labels of the
//! lines the tools print. A language that is not one of the eight gets English.

use crate::ctltext::LANGS;

fn column(lang: &str) -> usize {
    LANGS.iter().position(|l| *l == lang).unwrap_or(0)
}

/// Something the importers did not understand or had to assume, said once for a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Note {
    /// A command, an entity or an element that is not read.
    UnsupportedCommand,
    /// The file does not say its unit and millimetres were taken.
    UnitsAssumedMm,
    /// The file does not say its size and the pixels of a screen (96 to the inch) were taken.
    UnitsAssumedPixels,
    /// A plotter file: its units are 0.025 mm.
    UnitsPlotter,
    /// A transformation of the drawing was not applied.
    TransformIgnored,
}

impl Note {
    pub const ALL: [Note; 5] = [
        Note::UnsupportedCommand,
        Note::UnitsAssumedMm,
        Note::UnitsAssumedPixels,
        Note::UnitsPlotter,
        Note::TransformIgnored,
    ];

    pub fn text(self, lang: &str) -> &'static str {
        let row: [&str; 8] = match self {
            Note::UnsupportedCommand => [
                "Some commands or elements of the file are not read.",
                "Algunos comandos o elementos del archivo no se leen.",
                "Einige Befehle oder Elemente der Datei werden nicht gelesen.",
                "Certaines commandes ou certains éléments du fichier ne sont pas lus.",
                "Alcuni comandi o elementi del file non vengono letti.",
                "ファイルの一部のコマンドや要素は読み込まれません。",
                "文件中的某些命令或元素不会被读取。",
                "Alguns comandos ou elementos do ficheiro não são lidos.",
            ],
            Note::UnitsAssumedMm => [
                "The file does not say its unit: millimetres were taken.",
                "El archivo no indica su unidad: se tomaron milímetros.",
                "Die Datei nennt keine Einheit: Es wurden Millimeter angenommen.",
                "Le fichier n'indique pas son unité : les millimètres ont été pris.",
                "Il file non indica l'unità: sono stati presi i millimetri.",
                "ファイルに単位の記載がないため、ミリメートルとみなしました。",
                "文件未说明单位：按毫米处理。",
                "O ficheiro não indica a unidade: foram tomados milímetros.",
            ],
            Note::UnitsAssumedPixels => [
                "The file does not say its size: pixels of 96 to the inch were taken.",
                "El archivo no indica su tamaño: se tomaron píxeles de 96 por pulgada.",
                "Die Datei nennt keine Größe: Es wurden Pixel mit 96 pro Zoll angenommen.",
                "Le fichier n'indique pas sa taille : des pixels de 96 par pouce ont été pris.",
                "Il file non indica la dimensione: sono stati presi pixel da 96 per pollice.",
                "ファイルにサイズの記載がないため、1インチ96ピクセルとみなしました。",
                "文件未说明尺寸：按每英寸 96 像素处理。",
                "O ficheiro não indica o tamanho: foram tomados píxeis de 96 por polegada.",
            ],
            Note::UnitsPlotter => [
                "A plotter file: its units are 0.025 mm.",
                "Un archivo de plóter: sus unidades son de 0,025 mm.",
                "Eine Plotterdatei: Ihre Einheiten sind 0,025 mm.",
                "Un fichier de traceur : ses unités sont de 0,025 mm.",
                "Un file per plotter: le sue unità sono di 0,025 mm.",
                "プロッターファイルです。単位は 0.025 mm です。",
                "绘图仪文件：其单位为 0.025 毫米。",
                "Um ficheiro de plotter: as suas unidades são de 0,025 mm.",
            ],
            Note::TransformIgnored => [
                "A transformation of the drawing was not applied.",
                "No se aplicó una transformación del dibujo.",
                "Eine Transformation der Zeichnung wurde nicht angewendet.",
                "Une transformation du dessin n'a pas été appliquée.",
                "Una trasformazione del disegno non è stata applicata.",
                "図面の変換は適用されませんでした。",
                "图形的某个变换没有被应用。",
                "Uma transformação do desenho não foi aplicada.",
            ],
        };
        row[column(lang)]
    }
}

/// Why a file could not be imported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fail {
    /// The file is bigger than the limit of the motion file.
    TooBig,
    /// The drawing has more points than the limit.
    TooManyPoints,
    /// The file is not well formed.
    Syntax,
    /// Nothing in it can be cut.
    Empty,
    /// The format is not known.
    NotRecognised,
    /// The file could not be opened.
    Unreadable,
}

impl Fail {
    pub const ALL: [Fail; 6] = [
        Fail::TooBig,
        Fail::TooManyPoints,
        Fail::Syntax,
        Fail::Empty,
        Fail::NotRecognised,
        Fail::Unreadable,
    ];

    pub fn text(self, lang: &str) -> &'static str {
        let row: [&str; 8] = match self {
            Fail::TooBig => [
                "The file is bigger than the limit.",
                "El archivo supera el límite.",
                "Die Datei ist größer als das Limit.",
                "Le fichier dépasse la limite.",
                "Il file supera il limite.",
                "ファイルが上限を超えています。",
                "文件超过了上限。",
                "O ficheiro excede o limite.",
            ],
            Fail::TooManyPoints => [
                "The drawing has more points than the limit.",
                "El dibujo tiene más puntos que el límite.",
                "Die Zeichnung hat mehr Punkte als das Limit.",
                "Le dessin a plus de points que la limite.",
                "Il disegno ha più punti del limite.",
                "図面の点の数が上限を超えています。",
                "图形的点数超过了上限。",
                "O desenho tem mais pontos do que o limite.",
            ],
            Fail::Syntax => [
                "The file is not well formed.",
                "El archivo no está bien formado.",
                "Die Datei ist nicht wohlgeformt.",
                "Le fichier n'est pas bien formé.",
                "Il file non è ben formato.",
                "ファイルの形式が正しくありません。",
                "文件格式不正确。",
                "O ficheiro não está bem formado.",
            ],
            Fail::Empty => [
                "There is nothing in the file to cut.",
                "No hay nada que cortar en el archivo.",
                "Die Datei enthält nichts zum Schneiden.",
                "Il n'y a rien à couper dans le fichier.",
                "Nel file non c'è nulla da tagliare.",
                "ファイルに加工できるものがありません。",
                "文件中没有可切割的内容。",
                "Não há nada para cortar no ficheiro.",
            ],
            Fail::NotRecognised => [
                "The format of the file is not known.",
                "No se conoce el formato del archivo.",
                "Das Format der Datei ist nicht bekannt.",
                "Le format du fichier n'est pas connu.",
                "Il formato del file non è riconosciuto.",
                "ファイルの形式が不明です。",
                "无法识别文件格式。",
                "O formato do ficheiro não é conhecido.",
            ],
            Fail::Unreadable => [
                "The file could not be opened.",
                "No se pudo abrir el archivo.",
                "Die Datei konnte nicht geöffnet werden.",
                "Le fichier n'a pas pu être ouvert.",
                "Non è stato possibile aprire il file.",
                "ファイルを開けませんでした。",
                "无法打开文件。",
                "Não foi possível abrir o ficheiro.",
            ],
        };
        row[column(lang)]
    }
}

/// The labels of the lines the tools of the motion layer print.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    /// The time with the look-ahead of the planner.
    Time,
    /// The time if every move started and stopped.
    TimeStopToStop,
    Blocks,
    Stops,
    Length,
    ArcsRebuilt,
    /// Paths read from a drawing.
    Paths,
    /// Lines of G-code written.
    Lines,
    /// A post-processor of that name does not exist.
    NoPost,
    /// The post-processor is off or a script is asked for while scripts are off.
    ScriptsOff,
    /// Scripts are on but this build has no script engine yet.
    NoEngine,
}

impl Label {
    pub const ALL: [Label; 11] = [
        Label::Time,
        Label::TimeStopToStop,
        Label::Blocks,
        Label::Stops,
        Label::Length,
        Label::ArcsRebuilt,
        Label::Paths,
        Label::Lines,
        Label::NoPost,
        Label::ScriptsOff,
        Label::NoEngine,
    ];

    pub fn text(self, lang: &str) -> &'static str {
        let row: [&str; 8] = match self {
            Label::Time => [
                "Time with look-ahead",
                "Tiempo con anticipación",
                "Zeit mit Vorausschau",
                "Durée avec anticipation",
                "Tempo con anticipazione",
                "先読みありの時間",
                "含前瞻的时间",
                "Tempo com antecipação",
            ],
            Label::TimeStopToStop => [
                "Time stopping at every move",
                "Tiempo parando en cada movimiento",
                "Zeit mit Halt bei jeder Bewegung",
                "Durée avec arrêt à chaque mouvement",
                "Tempo fermandosi a ogni movimento",
                "移動ごとに停止する場合の時間",
                "每次运动都停止的时间",
                "Tempo parando em cada movimento",
            ],
            Label::Blocks => [
                "Moves",
                "Movimientos",
                "Bewegungen",
                "Mouvements",
                "Movimenti",
                "移動数",
                "运动段数",
                "Movimentos",
            ],
            Label::Stops => [
                "Full stops",
                "Paradas completas",
                "Vollständige Halts",
                "Arrêts complets",
                "Arresti completi",
                "完全停止の回数",
                "完全停止次数",
                "Paragens completas",
            ],
            Label::Length => [
                "Length (mm)",
                "Longitud (mm)",
                "Länge (mm)",
                "Longueur (mm)",
                "Lunghezza (mm)",
                "長さ (mm)",
                "长度 (毫米)",
                "Comprimento (mm)",
            ],
            Label::ArcsRebuilt => [
                "Arcs rebuilt as straight pieces",
                "Arcos reconstruidos como tramos rectos",
                "Als Geraden nachgebildete Bögen",
                "Arcs reconstruits en tronçons droits",
                "Archi ricostruiti come tratti rettilinei",
                "直線に分けて再構成した円弧",
                "重建为直线段的圆弧",
                "Arcos reconstruídos como troços retos",
            ],
            Label::Paths => [
                "Paths read",
                "Trayectorias leídas",
                "Gelesene Pfade",
                "Trajectoires lues",
                "Percorsi letti",
                "読み込んだパス",
                "已读取的路径",
                "Trajetórias lidas",
            ],
            Label::Lines => [
                "Lines of G-code",
                "Líneas de G-code",
                "G-Code-Zeilen",
                "Lignes de G-code",
                "Righe di G-code",
                "Gコードの行数",
                "G 代码行数",
                "Linhas de G-code",
            ],
            Label::NoPost => [
                "There is no post-processor with that name.",
                "No hay ningún postprocesador con ese nombre.",
                "Es gibt keinen Postprozessor mit diesem Namen.",
                "Il n'existe pas de post-processeur de ce nom.",
                "Non esiste alcun post-processore con quel nome.",
                "その名前のポストプロセッサーはありません。",
                "没有该名称的后处理器。",
                "Não existe nenhum pós-processador com esse nome.",
            ],
            Label::ScriptsOff => [
                "Scripts are switched off: the post-processor is not run.",
                "Los scripts están desactivados: el postprocesador no se ejecuta.",
                "Skripte sind ausgeschaltet: Der Postprozessor wird nicht ausgeführt.",
                "Les scripts sont désactivés : le post-processeur n'est pas exécuté.",
                "Gli script sono disattivati: il post-processore non viene eseguito.",
                "スクリプトは無効です。ポストプロセッサーは実行されません。",
                "脚本已关闭：不运行后处理器。",
                "Os scripts estão desativados: o pós-processador não é executado.",
            ],
            Label::NoEngine => [
                "This build has no script engine yet: the post-processor is not run.",
                "Esta versión aún no tiene motor de scripts: el postprocesador no se ejecuta.",
                "Diese Version hat noch keine Skript-Engine: Der Postprozessor wird nicht ausgeführt.",
                "Cette version n'a pas encore de moteur de scripts : le post-processeur n'est pas exécuté.",
                "Questa versione non ha ancora un motore di script: il post-processore non viene eseguito.",
                "このビルドにはまだスクリプトエンジンがないため、ポストプロセッサーは実行されません。",
                "此版本尚无脚本引擎：不运行后处理器。",
                "Esta versão ainda não tem motor de scripts: o pós-processador não é executado.",
            ],
        };
        row[column(lang)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn by(f: &dyn Fn(&str) -> &'static str) -> [&'static str; 8] {
        let mut a = [""; 8];
        for (i, l) in LANGS.iter().enumerate() {
            a[i] = f(l);
        }
        a
    }

    fn check(rows: Vec<[&'static str; 8]>) {
        for row in rows {
            assert!(row.iter().all(|t| t.len() > 3), "{row:?}");
            let mut r = row.to_vec();
            r.sort_unstable();
            r.dedup();
            assert_eq!(r.len(), 8, "{row:?}");
        }
    }

    #[test]
    fn the_notes_the_failures_and_the_labels_have_all_eight_languages_and_they_differ() {
        check(Note::ALL.iter().map(|n| by(&|l| n.text(l))).collect());
        check(Fail::ALL.iter().map(|n| by(&|l| n.text(l))).collect());
        check(Label::ALL.iter().map(|n| by(&|l| n.text(l))).collect());
        assert_eq!(Fail::Empty.text("xx"), Fail::Empty.text("en"));
    }

    #[test]
    fn the_portuguese_uses_the_words_of_portugal() {
        let text = [
            Note::UnitsAssumedPixels.text("pt"),
            Fail::Unreadable.text("pt"),
            Note::UnsupportedCommand.text("pt"),
            Label::ScriptsOff.text("pt"),
        ]
        .join(" ");
        assert!(
            text.contains("píxeis") && text.contains("ficheiro") && text.contains("desativados")
        );
        assert!(!text.contains("arquivo"));
    }
}
