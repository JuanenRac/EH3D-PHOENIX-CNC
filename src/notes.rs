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
    /// Something of the layer of the board was not drawn (a shape the reader cannot draw), so the cut leaves it out.
    PcbNotDrawn,
    /// The file does not say which layer it is and copper was taken.
    PcbAssumedCopper,
    /// The outline is not closed: the open runs are cut along the line.
    PcbOutlineOpen,
    /// The path of the outline could not be moved by the radius of the tool: it follows the line.
    PcbNoCompensation,
    /// The layer was mirrored about the Y axis.
    PcbMirrored,
    /// A hole whose tool is not in the file, or that has no width, was left out.
    PcbHoleSkipped,
    /// The drill file did not say its format, zeros or units and what was supposed was used (the setting allows it).
    PcbAssumedDrill,
}

impl Note {
    pub const ALL: [Note; 11] = [
        Note::UnsupportedCommand,
        Note::UnitsAssumedMm,
        Note::UnitsAssumedPixels,
        Note::UnitsPlotter,
        Note::PcbNotDrawn,
        Note::PcbAssumedCopper,
        Note::PcbOutlineOpen,
        Note::PcbNoCompensation,
        Note::PcbMirrored,
        Note::PcbHoleSkipped,
        Note::PcbAssumedDrill,
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
            Note::PcbNotDrawn => [
                "Some shapes of the layer cannot be drawn and are left out of the cut.",
                "Algunas formas de la capa no se pueden dibujar y quedan fuera del corte.",
                "Einige Formen der Lage lassen sich nicht zeichnen und fehlen im Schnitt.",
                "Certaines formes de la couche ne peuvent pas être dessinées et sont absentes de la coupe.",
                "Alcune forme dello strato non si possono disegnare e restano fuori dal taglio.",
                "レイヤーの一部の図形は描画できないため、加工から除外されます。",
                "图层中的某些形状无法绘制，已从切削中排除。",
                "Algumas formas da camada não podem ser desenhadas e ficam fora do corte.",
            ],
            Note::PcbAssumedCopper => [
                "The file does not say which layer it is: copper was taken.",
                "El archivo no dice qué capa es: se tomó cobre.",
                "Die Datei nennt ihre Lage nicht: Es wurde Kupfer angenommen.",
                "Le fichier n'indique pas de quelle couche il s'agit : le cuivre a été pris.",
                "Il file non indica di quale strato si tratta: è stato preso il rame.",
                "ファイルにレイヤーの種類の記載がないため、銅箔層とみなしました。",
                "文件未说明是哪一层：按铜层处理。",
                "O ficheiro não diz que camada é: foi tomado o cobre.",
            ],
            Note::PcbOutlineOpen => [
                "The outline is not closed: the open runs are cut along the line.",
                "El contorno no está cerrado: los tramos abiertos se cortan sobre la línea.",
                "Die Kontur ist nicht geschlossen: Die offenen Stücke werden auf der Linie geschnitten.",
                "Le contour n'est pas fermé : les tronçons ouverts sont coupés sur la ligne.",
                "Il contorno non è chiuso: i tratti aperti vengono tagliati sulla linea.",
                "外形が閉じていません。開いた部分は線上で加工されます。",
                "轮廓未闭合：开放的部分沿线切割。",
                "O contorno não está fechado: os troços abertos são cortados sobre a linha.",
            ],
            Note::PcbNoCompensation => [
                "The outline could not be moved by the radius of the tool: it is cut along the line.",
                "El contorno no se pudo desplazar el radio de la herramienta: se corta sobre la línea.",
                "Die Kontur ließ sich nicht um den Werkzeugradius versetzen: Sie wird auf der Linie geschnitten.",
                "Le contour n'a pas pu être décalé du rayon de l'outil : il est coupé sur la ligne.",
                "Il contorno non si è potuto spostare del raggio dell'utensile: viene tagliato sulla linea.",
                "外形を工具半径分オフセットできませんでした。線上で加工されます。",
                "无法将轮廓偏移一个刀具半径：将沿线切割。",
                "O contorno não pôde ser deslocado pelo raio da ferramenta: é cortado sobre a linha.",
            ],
            Note::PcbMirrored => [
                "The layer was mirrored about the Y axis.",
                "La capa se reflejó respecto al eje Y.",
                "Die Lage wurde an der Y-Achse gespiegelt.",
                "La couche a été inversée par rapport à l'axe Y.",
                "Lo strato è stato specchiato rispetto all'asse Y.",
                "レイヤーを Y 軸で反転しました。",
                "该图层已沿 Y 轴镜像。",
                "A camada foi espelhada em relação ao eixo Y.",
            ],
            Note::PcbHoleSkipped => [
                "A hole whose tool is not in the file, or that has no width, was left out.",
                "Se omitió un agujero cuya herramienta no está en el archivo o que no tiene diámetro.",
                "Ein Loch, dessen Werkzeug nicht in der Datei steht oder das keinen Durchmesser hat, wurde ausgelassen.",
                "Un trou dont l'outil n'est pas dans le fichier, ou qui n'a pas de diamètre, a été omis.",
                "Un foro il cui utensile non è nel file, o che non ha diametro, è stato omesso.",
                "工具がファイルにない、または径のない穴は除外されました。",
                "刀具不在文件中或没有直径的孔已被略去。",
                "Um furo cuja ferramenta não está no ficheiro, ou que não tem diâmetro, foi omitido.",
            ],
            Note::PcbAssumedDrill => [
                "The drill file does not say its format, zeros or units: they were supposed. Check the positions before drilling.",
                "El archivo de taladrado no dice su formato, ceros o unidades: se han supuesto. Comprueba las posiciones antes de taladrar.",
                "Die Bohrdatei nennt Format, Nullen oder Einheiten nicht: sie wurden angenommen. Vor dem Bohren die Positionen prüfen.",
                "Le fichier de perçage ne dit pas son format, ses zéros ou ses unités : ils ont été supposés. Vérifiez les positions avant de percer.",
                "Il file di foratura non dice formato, zeri o unità: sono stati supposti. Controlla le posizioni prima di forare.",
                "ドリルファイルに書式、ゼロ省略、単位の指定がないため推定しました。穴あけ前に位置を確認してください。",
                "钻孔文件未说明格式、零省略方式或单位，现为推定值。钻孔前请检查位置。",
                "O ficheiro de furação não diz o formato, os zeros ou as unidades: foram supostos. Verifique as posições antes de furar.",
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
    /// The layer of the board needs a grid bigger than the setting allows (or the CAM could not make it).
    GridTooBig,
    /// The file is a layer of a board that is not cut (the mask, the legend, the paste), or not of the kind asked for.
    NotACutLayer,
    /// A `pcb_` setting of the motion file is out of what the CAM can use.
    BadPcbSetting,
    /// The drill file does not say its format, zeros or units, so a hole could be in another place.
    DrillAssumed,
    /// The drawing uses a unit, a scaling or a plane that is not read: cut as it stands it could come out at another size or place.
    Unmeasured,
}

impl Fail {
    pub const ALL: [Fail; 11] = [
        Fail::TooBig,
        Fail::TooManyPoints,
        Fail::Syntax,
        Fail::Empty,
        Fail::NotRecognised,
        Fail::Unreadable,
        Fail::GridTooBig,
        Fail::NotACutLayer,
        Fail::BadPcbSetting,
        Fail::DrillAssumed,
        Fail::Unmeasured,
    ];

    pub fn text(self, lang: &str) -> &'static str {
        let row: [&str; 8] = match self {
            Fail::Unmeasured => [
                "The drawing uses a unit, a scaling, a command or a plane that is not read: it could be cut at another size or place, so nothing is imported.",
                "El dibujo usa una unidad, una escala, un comando o un plano que no se lee: podría cortarse con otro tamaño o en otro sitio, así que no se importa nada.",
                "Die Zeichnung nutzt eine Einheit, Skalierung, einen Befehl oder eine Ebene, die nicht gelesen werden: sie könnte in anderer Größe oder Lage geschnitten werden, daher wird nichts importiert.",
                "Le dessin utilise une unité, une échelle, une commande ou un plan non lus : il pourrait être coupé à une autre taille ou ailleurs, rien n'est donc importé.",
                "Il disegno usa un'unità, una scala, un comando o un piano non letti: potrebbe essere tagliato con un'altra misura o altrove, quindi non si importa nulla.",
                "図面に読み取れない単位、拡大縮小、コマンド、平面があります。違う大きさや位置で加工される恐れがあるため、何も読み込みません。",
                "图形使用了无法读取的单位、缩放、命令或平面：可能以错误的尺寸或位置切割，因此不导入任何内容。",
                "O desenho usa uma unidade, uma escala, um comando ou um plano que não são lidos: poderia ser cortado com outro tamanho ou noutro sítio, por isso nada é importado.",
            ],
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
            Fail::GridTooBig => [
                "The layer needs a grid bigger than the setting allows: raise pcb_res or pcb_max_cells.",
                "La capa necesita una malla mayor de la que permite el ajuste: suba pcb_res o pcb_max_cells.",
                "Die Lage braucht ein größeres Raster als die Einstellung erlaubt: pcb_res oder pcb_max_cells erhöhen.",
                "La couche demande une grille plus grande que le réglage ne le permet : augmentez pcb_res ou pcb_max_cells.",
                "Lo strato richiede una griglia più grande di quella consentita: aumentare pcb_res o pcb_max_cells.",
                "このレイヤーには設定が許す以上のグリッドが必要です。pcb_res か pcb_max_cells を上げてください。",
                "该图层所需的网格超出设置允许的大小：请提高 pcb_res 或 pcb_max_cells。",
                "A camada precisa de uma grelha maior do que a definição permite: aumente pcb_res ou pcb_max_cells.",
            ],
            Fail::NotACutLayer => [
                "This file is not a layer that is cut (or not of the kind asked for): choose the layer by hand.",
                "Este archivo no es una capa que se corte (o no es del tipo pedido): elija la capa a mano.",
                "Diese Datei ist keine Lage, die geschnitten wird (oder nicht von der gewünschten Art): Lage von Hand wählen.",
                "Ce fichier n'est pas une couche qui se coupe (ou pas du type demandé) : choisissez la couche à la main.",
                "Questo file non è uno strato che si taglia (o non è del tipo richiesto): scegliere lo strato a mano.",
                "このファイルは加工するレイヤーではありません（または指定の種類ではありません）。手動でレイヤーを選んでください。",
                "此文件不是需要切削的图层（或不是所要求的类型）：请手动选择图层。",
                "Este ficheiro não é uma camada que se corte (ou não é do tipo pedido): escolha a camada à mão.",
            ],
            Fail::BadPcbSetting => [
                "A pcb_ setting of the motion file is out of what the CAM can use (the cell must be at most half of the tool).",
                "Un ajuste pcb_ del archivo de movimiento está fuera de lo que el CAM puede usar (la celda debe ser como mucho la mitad de la herramienta).",
                "Eine pcb_-Einstellung der Bewegungsdatei liegt außerhalb dessen, was das CAM nutzen kann (die Zelle darf höchstens die Hälfte des Werkzeugs sein).",
                "Un réglage pcb_ du fichier de mouvement est hors de ce que la FAO peut utiliser (la cellule doit valoir au plus la moitié de l'outil).",
                "Un'impostazione pcb_ del file di movimento è fuori da ciò che il CAM può usare (la cella deve essere al massimo metà dell'utensile).",
                "動作ファイルの pcb_ 設定が CAM の使える範囲外です（セルは工具の半分以下にしてください）。",
                "运动文件中的某个 pcb_ 设置超出 CAM 可用的范围（单元格最多为刀具的一半）。",
                "Uma definição pcb_ do ficheiro de movimento está fora do que o CAM pode usar (a célula deve ser no máximo metade da ferramenta).",
            ],
            Fail::DrillAssumed => [
                "The drill file does not say its format, zeros or units, so a hole could be in another place. Export it with them, or set pcb_allow_assumed_drill in the motion file.",
                "El archivo de taladrado no dice su formato, ceros o unidades, así que un agujero podría quedar en otro sitio. Expórtalo con ellos o activa pcb_allow_assumed_drill en el archivo de movimiento.",
                "Die Bohrdatei nennt Format, Nullen oder Einheiten nicht, ein Loch könnte woanders liegen. Mit diesen Angaben exportieren oder pcb_allow_assumed_drill in der Bewegungsdatei setzen.",
                "Le fichier de perçage ne dit pas son format, ses zéros ou ses unités : un trou pourrait être ailleurs. Exportez-le avec eux ou activez pcb_allow_assumed_drill dans le fichier de mouvement.",
                "Il file di foratura non dice formato, zeri o unità, quindi un foro potrebbe finire altrove. Esportalo con questi dati o attiva pcb_allow_assumed_drill nel file di movimento.",
                "ドリルファイルに書式、ゼロ省略、単位の指定がないため、穴の位置がずれるおそれがあります。指定を付けて出力するか、動作ファイルで pcb_allow_assumed_drill を設定してください。",
                "钻孔文件未说明格式、零省略方式或单位，孔可能位于别处。请带上这些信息重新导出，或在运动文件中设置 pcb_allow_assumed_drill。",
                "O ficheiro de furação não diz o formato, os zeros ou as unidades, pelo que um furo poderia ficar noutro sítio. Exporte-o com eles ou ative pcb_allow_assumed_drill no ficheiro de movimento.",
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
    /// The check of a dialect passed, or did not.
    CheckPassed,
    CheckNotPassed,
    /// The layer of a board that was cut, the holes and slots drilled and the sizes of drill.
    Layer,
    Holes,
    DrillSizes,
}

impl Label {
    pub const ALL: [Label; 16] = [
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
        Label::CheckPassed,
        Label::CheckNotPassed,
        Label::Layer,
        Label::Holes,
        Label::DrillSizes,
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
            Label::CheckPassed => [
                "Dialect check passed",
                "Prueba del dialecto superada",
                "Dialektprüfung bestanden",
                "Vérification du dialecte réussie",
                "Verifica del dialetto superata",
                "ダイアレクトの検査に合格",
                "协议方言检查通过",
                "Verificação do dialeto aprovada",
            ],
            Label::CheckNotPassed => [
                "Dialect check not passed",
                "Prueba del dialecto no superada",
                "Dialektprüfung nicht bestanden",
                "Vérification du dialecte non réussie",
                "Verifica del dialetto non superata",
                "ダイアレクトの検査に不合格",
                "协议方言检查未通过",
                "Verificação do dialeto não aprovada",
            ],
            Label::Layer => [
                "Layer",
                "Capa",
                "Lage",
                "Couche",
                "Strato",
                "レイヤー",
                "图层",
                "Camada",
            ],
            Label::Holes => [
                "Holes and slots",
                "Agujeros y ranuras",
                "Löcher und Langlöcher",
                "Trous et rainures",
                "Fori e asole",
                "穴とスロット",
                "孔和槽",
                "Furos e ranhuras",
            ],
            Label::DrillSizes => [
                "Drill sizes",
                "Diámetros de broca",
                "Bohrergrößen",
                "Diamètres de foret",
                "Diametri delle punte",
                "ドリル径",
                "钻头直径",
                "Diâmetros de broca",
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
