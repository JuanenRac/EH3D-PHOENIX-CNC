//! The words of the program-side tools (the checker of a program, the keep-out boxes, the queue rules, the macros, the session log and the
//! statistics) in the eight languages of the suite: English, Spanish, German, French, Italian, Japanese, Chinese and Portuguese of Portugal.
//! A message has numbered holes (`{0}`, `{1}`, ...) that `say` fills; the window shows the result.

/// The language codes, in the order of the columns of the table.
pub const LANGS: [&str; 8] = ["en", "es", "de", "fr", "it", "ja", "zh", "pt"];

/// Every message of these tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Msg {
    NoUnits,
    UnitSwitch,
    Stored,
    Shift,
    Offsets,
    MachineCoords,
    Wcs,
    Plane,
    Probe,
    Stop,
    Emergency,
    Relative,
    ToolSpindle,
    NoMoves,
    BoxHit,
    BoxArc,
    TravelOut,
    NextJob,
    WaitJob,
    QueueHalted,
    MacroAsk,
    MacroName,
    MacroSystem,
    MacroLong,
    MacroOutside,
    MacroTwice,
    LogExported,
    LogExists,
    LogTitle,
    StatsTitle,
    StatsTool,
    StatsNoTool,
    StatsChanges,
    StatsLength,
    StatsTime,
    ReportClean,
    ReportMoves,
}

/// All the messages, so a test can walk through them.
pub const ALL: [Msg; 37] = [
    Msg::NoUnits,
    Msg::UnitSwitch,
    Msg::Stored,
    Msg::Shift,
    Msg::Offsets,
    Msg::MachineCoords,
    Msg::Wcs,
    Msg::Plane,
    Msg::Probe,
    Msg::Stop,
    Msg::Emergency,
    Msg::Relative,
    Msg::ToolSpindle,
    Msg::NoMoves,
    Msg::BoxHit,
    Msg::BoxArc,
    Msg::TravelOut,
    Msg::NextJob,
    Msg::WaitJob,
    Msg::QueueHalted,
    Msg::MacroAsk,
    Msg::MacroName,
    Msg::MacroSystem,
    Msg::MacroLong,
    Msg::MacroOutside,
    Msg::MacroTwice,
    Msg::LogExported,
    Msg::LogExists,
    Msg::LogTitle,
    Msg::StatsTitle,
    Msg::StatsTool,
    Msg::StatsNoTool,
    Msg::StatsChanges,
    Msg::StatsLength,
    Msg::StatsTime,
    Msg::ReportClean,
    Msg::ReportMoves,
];

/// The eight texts of a message, in the order of `LANGS`.
fn table(m: Msg) -> [&'static str; 8] {
    match m {
        Msg::NoUnits => [
            "No G20 or G21 comes before the first move: the units depend on the controller",
            "No hay G20 ni G21 antes del primer movimiento: las unidades dependen del controlador",
            "Vor der ersten Bewegung steht kein G20 oder G21: die Einheiten hängen von der Steuerung ab",
            "Aucun G20 ni G21 avant le premier mouvement : les unités dépendent du contrôleur",
            "Nessun G20 o G21 prima del primo movimento: le unità dipendono dal controller",
            "最初の移動の前に G20 または G21 がありません。単位はコントローラ次第です",
            "第一次移动之前没有 G20 或 G21：单位取决于控制器",
            "Não há G20 nem G21 antes do primeiro movimento: as unidades dependem do controlador",
        ],
        Msg::UnitSwitch => [
            "Line {0}: the units change to {1} in the middle of the program",
            "Línea {0}: las unidades cambian a {1} en mitad del programa",
            "Zeile {0}: die Einheiten wechseln mitten im Programm zu {1}",
            "Ligne {0} : les unités passent en {1} au milieu du programme",
            "Riga {0}: le unità passano a {1} a metà programma",
            "{0} 行目：プログラムの途中で単位が {1} に変わります",
            "第 {0} 行：程序中途单位改为 {1}",
            "Linha {0}: as unidades mudam para {1} a meio do programa",
        ],
        Msg::Stored => [
            "Line {0}: {1} goes to a stored position that may not be the one you expect",
            "Línea {0}: {1} va a una posición guardada que puede no ser la que esperas",
            "Zeile {0}: {1} fährt eine gespeicherte Position an, die vielleicht nicht die erwartete ist",
            "Ligne {0} : {1} va à une position mémorisée qui n'est peut-être pas celle attendue",
            "Riga {0}: {1} va a una posizione memorizzata che potrebbe non essere quella attesa",
            "{0} 行目：{1} は保存された位置へ移動します。期待した位置とは限りません",
            "第 {0} 行：{1} 会移动到已保存的位置，可能不是你期望的位置",
            "Linha {0}: {1} vai para uma posição guardada que pode não ser a que espera",
        ],
        Msg::Shift => [
            "Line {0}: G92 shifts the work zero until G92.1 or a reset",
            "Línea {0}: G92 desplaza el cero de la pieza hasta G92.1 o un reinicio",
            "Zeile {0}: G92 verschiebt den Werkstücknullpunkt bis G92.1 oder einen Reset",
            "Ligne {0} : G92 décale le zéro pièce jusqu'à G92.1 ou une réinitialisation",
            "Riga {0}: G92 sposta lo zero pezzo fino a G92.1 o a un reset",
            "{0} 行目：G92 は G92.1 またはリセットまでワークのゼロ点をずらします",
            "第 {0} 行：G92 会平移工件零点，直到 G92.1 或复位",
            "Linha {0}: G92 desloca o zero da peça até G92.1 ou um reinício",
        ],
        Msg::Offsets => [
            "Line {0}: G10 writes offsets into the controller",
            "Línea {0}: G10 escribe desplazamientos en el controlador",
            "Zeile {0}: G10 schreibt Versätze in die Steuerung",
            "Ligne {0} : G10 écrit des décalages dans le contrôleur",
            "Riga {0}: G10 scrive offset nel controller",
            "{0} 行目：G10 はコントローラにオフセットを書き込みます",
            "第 {0} 行：G10 会向控制器写入偏置",
            "Linha {0}: G10 escreve desvios no controlador",
        ],
        Msg::MachineCoords => [
            "Line {0}: G53 moves in machine coordinates and ignores the work zero",
            "Línea {0}: G53 se mueve en coordenadas de máquina e ignora el cero de la pieza",
            "Zeile {0}: G53 fährt in Maschinenkoordinaten und ignoriert den Werkstücknullpunkt",
            "Ligne {0} : G53 se déplace en coordonnées machine et ignore le zéro pièce",
            "Riga {0}: G53 si muove in coordinate macchina e ignora lo zero pezzo",
            "{0} 行目：G53 は機械座標で動き、ワークのゼロ点を無視します",
            "第 {0} 行：G53 按机床坐标移动，忽略工件零点",
            "Linha {0}: G53 move-se em coordenadas da máquina e ignora o zero da peça",
        ],
        Msg::Wcs => [
            "Line {0}: the work coordinate system changes to G{1}",
            "Línea {0}: el sistema de coordenadas de trabajo cambia a G{1}",
            "Zeile {0}: das Werkstückkoordinatensystem wechselt zu G{1}",
            "Ligne {0} : le système de coordonnées de travail passe à G{1}",
            "Riga {0}: il sistema di coordinate di lavoro passa a G{1}",
            "{0} 行目：ワーク座標系が G{1} に変わります",
            "第 {0} 行：工件坐标系改为 G{1}",
            "Linha {0}: o sistema de coordenadas de trabalho muda para G{1}",
        ],
        Msg::Plane => [
            "Line {0}: the plane changes to G{1}; arcs are cut in that plane",
            "Línea {0}: el plano cambia a G{1}; los arcos se cortan en ese plano",
            "Zeile {0}: die Ebene wechselt zu G{1}; Kreisbögen werden in dieser Ebene gefahren",
            "Ligne {0} : le plan passe à G{1} ; les arcs sont coupés dans ce plan",
            "Riga {0}: il piano passa a G{1}; gli archi vengono tagliati in quel piano",
            "{0} 行目：平面が G{1} に変わります。円弧はその平面で切削されます",
            "第 {0} 行：平面改为 G{1}；圆弧将在该平面内切削",
            "Linha {0}: o plano muda para G{1}; os arcos são cortados nesse plano",
        ],
        Msg::Probe => [
            "Line {0}: {1} is a probing move that stops on contact",
            "Línea {0}: {1} es un movimiento de palpado que se detiene al tocar",
            "Zeile {0}: {1} ist eine Tastbewegung, die bei Kontakt anhält",
            "Ligne {0} : {1} est un mouvement de palpage qui s'arrête au contact",
            "Riga {0}: {1} è un movimento di tastatura che si ferma al contatto",
            "{0} 行目：{1} は接触で停止するプローブ動作です",
            "第 {0} 行：{1} 是接触即停的探测移动",
            "Linha {0}: {1} é um movimento de apalpação que pára ao tocar",
        ],
        Msg::Stop => [
            "Line {0}: {1} stops the program",
            "Línea {0}: {1} detiene el programa",
            "Zeile {0}: {1} hält das Programm an",
            "Ligne {0} : {1} arrête le programme",
            "Riga {0}: {1} ferma il programma",
            "{0} 行目：{1} はプログラムを停止します",
            "第 {0} 行：{1} 会停止程序",
            "Linha {0}: {1} pára o programa",
        ],
        Msg::Emergency => [
            "Line {0}: M112 is an emergency stop",
            "Línea {0}: M112 es una parada de emergencia",
            "Zeile {0}: M112 ist ein Nothalt",
            "Ligne {0} : M112 est un arrêt d'urgence",
            "Riga {0}: M112 è un arresto di emergenza",
            "{0} 行目：M112 は非常停止です",
            "第 {0} 行：M112 是紧急停止",
            "Linha {0}: M112 é uma paragem de emergência",
        ],
        Msg::Relative => [
            "The program ends in relative mode (G91)",
            "El programa termina en modo relativo (G91)",
            "Das Programm endet im Relativmodus (G91)",
            "Le programme se termine en mode relatif (G91)",
            "Il programma termina in modo relativo (G91)",
            "プログラムは相対モード (G91) で終わります",
            "程序以相对模式 (G91) 结束",
            "O programa termina em modo relativo (G91)",
        ],
        Msg::ToolSpindle => [
            "Line {0}: tool change while the spindle is running",
            "Línea {0}: cambio de herramienta con el husillo en marcha",
            "Zeile {0}: Werkzeugwechsel bei laufender Spindel",
            "Ligne {0} : changement d'outil avec la broche en marche",
            "Riga {0}: cambio utensile con il mandrino in rotazione",
            "{0} 行目：スピンドルが回ったまま工具を交換します",
            "第 {0} 行：主轴转动时换刀",
            "Linha {0}: troca de ferramenta com o fuso em marcha",
        ],
        Msg::NoMoves => [
            "The program has no moves",
            "El programa no tiene movimientos",
            "Das Programm enthält keine Bewegungen",
            "Le programme ne contient aucun mouvement",
            "Il programma non contiene movimenti",
            "プログラムに移動がありません",
            "程序中没有移动",
            "O programa não tem movimentos",
        ],
        Msg::BoxHit => [
            "Line {0}: the move enters the keep-out box {1}",
            "Línea {0}: el movimiento entra en la caja prohibida {1}",
            "Zeile {0}: die Bewegung dringt in die Sperrbox {1} ein",
            "Ligne {0} : le mouvement entre dans la boîte interdite {1}",
            "Riga {0}: il movimento entra nel riquadro vietato {1}",
            "{0} 行目：移動が進入禁止ボックス {1} に入ります",
            "第 {0} 行：移动进入禁区盒 {1}",
            "Linha {0}: o movimento entra na caixa proibida {1}",
        ],
        Msg::BoxArc => [
            "{0} arc moves are checked by their chord only",
            "{0} movimientos en arco se comprueban solo por su cuerda",
            "{0} Kreisbögen werden nur anhand ihrer Sehne geprüft",
            "{0} mouvements en arc ne sont vérifiés que par leur corde",
            "{0} movimenti ad arco sono controllati solo con la loro corda",
            "{0} 個の円弧移動は弦のみで確認されます",
            "{0} 个圆弧移动仅按弦检查",
            "{0} movimentos em arco são verificados apenas pela sua corda",
        ],
        Msg::TravelOut => [
            "Line {0}: the move leaves the travel of axis {1}",
            "Línea {0}: el movimiento sale del recorrido del eje {1}",
            "Zeile {0}: die Bewegung verlässt den Verfahrweg der Achse {1}",
            "Ligne {0} : le mouvement sort de la course de l'axe {1}",
            "Riga {0}: il movimento esce dalla corsa dell'asse {1}",
            "{0} 行目：移動が {1} 軸のストロークを超えます",
            "第 {0} 行：移动超出 {1} 轴行程",
            "Linha {0}: o movimento sai do curso do eixo {1}",
        ],
        Msg::NextJob => [
            "Next: {0}. Place the work and confirm to go on",
            "Siguiente: {0}. Coloca la pieza y confirma para seguir",
            "Weiter mit: {0}. Werkstück einlegen und bestätigen",
            "Suivant : {0}. Placez la pièce et confirmez pour continuer",
            "Prossimo: {0}. Posiziona il pezzo e conferma per proseguire",
            "次：{0}。ワークをセットして確認すると続行します",
            "下一个：{0}。放好工件并确认后继续",
            "Seguinte: {0}. Coloque a peça e confirme para continuar",
        ],
        Msg::WaitJob => [
            "Waiting {0} s before {1}",
            "Esperando {0} s antes de {1}",
            "Warte {0} s vor {1}",
            "Attente de {0} s avant {1}",
            "Attesa di {0} s prima di {1}",
            "{1} の前に {0} 秒待機します",
            "在 {1} 之前等待 {0} 秒",
            "A aguardar {0} s antes de {1}",
        ],
        Msg::QueueHalted => [
            "The queue stopped because a program did not finish",
            "La cola se detuvo porque un programa no terminó",
            "Die Warteschlange wurde angehalten, weil ein Programm nicht fertig wurde",
            "La file s'est arrêtée parce qu'un programme ne s'est pas terminé",
            "La coda si è fermata perché un programma non è terminato",
            "プログラムが完了しなかったためキューが停止しました",
            "由于有程序未完成，队列已停止",
            "A fila parou porque um programa não terminou",
        ],
        Msg::MacroAsk => [
            "Run the macro {0}? It has {1} lines and can move the machine",
            "¿Ejecutar la macro {0}? Tiene {1} líneas y puede mover la máquina",
            "Makro {0} ausführen? Es hat {1} Zeilen und kann die Maschine bewegen",
            "Exécuter la macro {0} ? Elle a {1} lignes et peut déplacer la machine",
            "Eseguire la macro {0}? Ha {1} righe e può muovere la macchina",
            "マクロ {0} を実行しますか？{1} 行あり、機械が動く可能性があります",
            "运行宏 {0}？它有 {1} 行，可能会移动机器",
            "Executar a macro {0}? Tem {1} linhas e pode mover a máquina",
        ],
        Msg::MacroName => [
            "Line {0}: the name of the macro is not valid (letters, digits, - and _, up to 32)",
            "Línea {0}: el nombre de la macro no es válido (letras, cifras, - y _, hasta 32)",
            "Zeile {0}: der Makroname ist ungültig (Buchstaben, Ziffern, - und _, bis 32)",
            "Ligne {0} : le nom de la macro n'est pas valide (lettres, chiffres, - et _, jusqu'à 32)",
            "Riga {0}: il nome della macro non è valido (lettere, cifre, - e _, fino a 32)",
            "{0} 行目：マクロ名が無効です（英数字、- と _、32 文字まで）",
            "第 {0} 行：宏名称无效（字母、数字、- 和 _，最多 32 个）",
            "Linha {0}: o nome da macro não é válido (letras, algarismos, - e _, até 32)",
        ],
        Msg::MacroSystem => [
            "Line {0}: a line that starts with $ is not allowed in a macro",
            "Línea {0}: una línea que empieza por $ no se permite en una macro",
            "Zeile {0}: eine Zeile, die mit $ beginnt, ist in einem Makro nicht erlaubt",
            "Ligne {0} : une ligne commençant par $ n'est pas permise dans une macro",
            "Riga {0}: una riga che inizia con $ non è permessa in una macro",
            "{0} 行目：$ で始まる行はマクロでは使えません",
            "第 {0} 行：宏中不允许以 $ 开头的行",
            "Linha {0}: uma linha que começa por $ não é permitida numa macro",
        ],
        Msg::MacroLong => [
            "The macro {0} has more than {1} lines and was left out",
            "La macro {0} tiene más de {1} líneas y se ha omitido",
            "Das Makro {0} hat mehr als {1} Zeilen und wurde ausgelassen",
            "La macro {0} a plus de {1} lignes et a été ignorée",
            "La macro {0} ha più di {1} righe ed è stata omessa",
            "マクロ {0} は {1} 行を超えているため除外されました",
            "宏 {0} 超过 {1} 行，已忽略",
            "A macro {0} tem mais de {1} linhas e foi omitida",
        ],
        Msg::MacroOutside => [
            "Line {0}: this line is outside any macro",
            "Línea {0}: esta línea está fuera de cualquier macro",
            "Zeile {0}: diese Zeile steht außerhalb eines Makros",
            "Ligne {0} : cette ligne est en dehors de toute macro",
            "Riga {0}: questa riga è fuori da qualsiasi macro",
            "{0} 行目：この行はどのマクロにも属していません",
            "第 {0} 行：此行不属于任何宏",
            "Linha {0}: esta linha está fora de qualquer macro",
        ],
        Msg::MacroTwice => [
            "Line {0}: the macro {1} is already defined",
            "Línea {0}: la macro {1} ya está definida",
            "Zeile {0}: das Makro {1} ist schon definiert",
            "Ligne {0} : la macro {1} est déjà définie",
            "Riga {0}: la macro {1} è già definita",
            "{0} 行目：マクロ {1} は既に定義されています",
            "第 {0} 行：宏 {1} 已定义",
            "Linha {0}: a macro {1} já está definida",
        ],
        Msg::LogExported => [
            "Session log saved to {0}",
            "Registro de la sesión guardado en {0}",
            "Sitzungsprotokoll gespeichert in {0}",
            "Journal de session enregistré dans {0}",
            "Registro della sessione salvato in {0}",
            "セッションログを {0} に保存しました",
            "会话日志已保存到 {0}",
            "Registo da sessão guardado em {0}",
        ],
        Msg::LogExists => [
            "{0} already exists; the log was not written",
            "{0} ya existe; no se ha escrito el registro",
            "{0} existiert bereits; das Protokoll wurde nicht geschrieben",
            "{0} existe déjà ; le journal n'a pas été écrit",
            "{0} esiste già; il registro non è stato scritto",
            "{0} は既に存在するため、ログは書き込まれませんでした",
            "{0} 已存在；未写入日志",
            "{0} já existe; o registo não foi escrito",
        ],
        Msg::LogTitle => [
            "Session log",
            "Registro de la sesión",
            "Sitzungsprotokoll",
            "Journal de session",
            "Registro della sessione",
            "セッションログ",
            "会话日志",
            "Registo da sessão",
        ],
        Msg::StatsTitle => [
            "Program statistics",
            "Estadísticas del programa",
            "Programmstatistik",
            "Statistiques du programme",
            "Statistiche del programma",
            "プログラム統計",
            "程序统计",
            "Estatísticas do programa",
        ],
        Msg::StatsTool => [
            "Tool {0}",
            "Herramienta {0}",
            "Werkzeug {0}",
            "Outil {0}",
            "Utensile {0}",
            "工具 {0}",
            "刀具 {0}",
            "Ferramenta {0}",
        ],
        Msg::StatsNoTool => [
            "No tool chosen",
            "Sin herramienta elegida",
            "Kein Werkzeug gewählt",
            "Aucun outil choisi",
            "Nessun utensile scelto",
            "工具未選択",
            "未选择刀具",
            "Sem ferramenta escolhida",
        ],
        Msg::StatsChanges => [
            "{0} tool changes",
            "{0} cambios de herramienta",
            "{0} Werkzeugwechsel",
            "{0} changements d'outil",
            "{0} cambi utensile",
            "工具交換 {0} 回",
            "换刀 {0} 次",
            "{0} trocas de ferramenta",
        ],
        Msg::StatsLength => [
            "Cutting {0} mm, rapid {1} mm",
            "Corte {0} mm, rápido {1} mm",
            "Schnitt {0} mm, Eilgang {1} mm",
            "Coupe {0} mm, rapide {1} mm",
            "Taglio {0} mm, rapido {1} mm",
            "切削 {0} mm、早送り {1} mm",
            "切削 {0} mm，快速 {1} mm",
            "Corte {0} mm, rápido {1} mm",
        ],
        Msg::StatsTime => [
            "Estimated time {0}",
            "Tiempo estimado {0}",
            "Geschätzte Zeit {0}",
            "Temps estimé {0}",
            "Tempo stimato {0}",
            "推定時間 {0}",
            "预计时间 {0}",
            "Tempo estimado {0}",
        ],
        Msg::ReportClean => [
            "Nothing looks wrong",
            "No se ve nada raro",
            "Nichts Auffälliges",
            "Rien d'anormal",
            "Non si vede nulla di strano",
            "問題は見つかりませんでした",
            "未发现问题",
            "Nada de anormal",
        ],
        Msg::ReportMoves => [
            "{0} moves in the path, estimated time {1}",
            "{0} movimientos en la trayectoria, tiempo estimado {1}",
            "{0} Bewegungen im Pfad, geschätzte Zeit {1}",
            "{0} mouvements dans la trajectoire, temps estimé {1}",
            "{0} movimenti nel percorso, tempo stimato {1}",
            "経路に {0} 回の移動、推定時間 {1}",
            "路径中有 {0} 次移动，预计时间 {1}",
            "{0} movimentos no trajeto, tempo estimado {1}",
        ],
    }
}

/// The text of a message in a language (a code of `LANGS`; anything else gives English), with its holes still in it.
pub fn text(lang: &str, m: Msg) -> &'static str {
    let i = LANGS
        .iter()
        .position(|l| l.eq_ignore_ascii_case(lang.trim().split('-').next().unwrap_or("")))
        .unwrap_or(0);
    table(m)[i]
}

/// The message with `{0}`, `{1}`, ... replaced by the arguments (a hole with no argument is left as it is).
pub fn say<S: AsRef<str>>(lang: &str, m: Msg, args: &[S]) -> String {
    let mut out = text(lang, m).to_string();
    // from the highest hole down, so that `{1}` is not touched while `{10}` would still be pending
    for (i, a) in args.iter().enumerate().rev() {
        out = out.replace(&format!("{{{i}}}"), a.as_ref());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn holes(s: &str) -> Vec<usize> {
        let mut v = Vec::new();
        let mut rest = s;
        while let Some(i) = rest.find('{') {
            let Some(j) = rest[i..].find('}') else { break };
            if let Ok(n) = rest[i + 1..i + j].parse::<usize>() {
                v.push(n);
            }
            rest = &rest[i + j + 1..];
        }
        v.sort_unstable();
        v
    }

    #[test]
    fn every_message_is_written_in_the_eight_languages() {
        for m in ALL {
            let t = table(m);
            for (i, s) in t.iter().enumerate() {
                assert!(!s.trim().is_empty(), "{m:?} is empty in {}", LANGS[i]);
            }
            // the same holes in every language
            for (i, s) in t.iter().enumerate() {
                assert_eq!(holes(s), holes(t[0]), "{m:?} holes differ in {}", LANGS[i]);
            }
            // a translation is not the English text copied (apart from the names that are the same in a language)
            let same = t.iter().skip(1).filter(|s| **s == t[0]).count();
            assert!(same < 7, "{m:?} is not translated");
        }
    }

    #[test]
    fn the_list_of_messages_has_no_repeats() {
        let mut seen = std::collections::HashSet::new();
        for m in ALL {
            assert!(seen.insert(m), "{m:?} twice");
        }
        assert_eq!(seen.len(), 37);
    }

    #[test]
    fn say_fills_the_holes() {
        assert_eq!(
            say("es", Msg::Wcs, &["12", "55"]),
            "Línea 12: el sistema de coordenadas de trabajo cambia a G55"
        );
        assert_eq!(
            say("fr", Msg::Stop, &[String::from("3"), "M0".to_string()]),
            "Ligne 3 : M0 arrête le programme"
        );
    }

    #[test]
    fn an_unknown_language_gives_english_and_case_does_not_matter() {
        assert_eq!(text("xx", Msg::LogTitle), "Session log");
        assert_eq!(text("PT", Msg::LogTitle), "Registo da sessão");
        assert_eq!(text(" de ", Msg::LogTitle), "Sitzungsprotokoll");
        // the variants of a language (pt-br, fr-ca) read as their main language
        assert_eq!(text("fr-ca", Msg::LogTitle), "Journal de session");
    }

    #[test]
    fn a_missing_argument_leaves_the_hole() {
        assert_eq!(
            say::<&str>("en", Msg::Stop, &["4"]),
            "Line 4: {1} stops the program"
        );
    }

    #[test]
    fn portuguese_is_the_one_of_portugal() {
        for m in ALL {
            let pt = table(m)[7];
            for brazilian in ["arquivo", "tela", "celular", "usuário", "salvo "] {
                assert!(!pt.contains(brazilian), "{m:?}: {pt}");
            }
        }
    }
}
