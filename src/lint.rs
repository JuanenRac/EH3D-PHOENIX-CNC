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
    /// A feed rate above this (mm/min) or a spindle speed above this (rpm) is said (5000 and 24000 by default); 0 turns the check off.
    pub max_feed_mm_min: f64,
    pub max_rpm: f64,
    /// The range of cutting feed (mm/min) and of spindle speed (rpm) of the material in use, from the library of materials; a cutting
    /// move or a spindle speed outside it is said. `None` checks nothing.
    pub material_feed: Option<(f64, f64)>,
    pub material_rpm: Option<(f64, f64)>,
    /// The arcs a controller refuses with a radius error are looked for (`lint_arcs`), and the gap in mm that an arc given by its centre
    /// may end off its circle before it is said (`lint_arc_gap`).
    pub arcs: bool,
    pub arc_gap_mm: f64,
    /// A spindle told to turn the other way with no stop between is said (`lint_reversal`).
    pub reversal: bool,
    /// A feed of zero is said (`lint_zero_feed`).
    pub zero_feed: bool,
}

impl Default for LintCfg {
    fn default() -> Self {
        LintCfg {
            enabled: true,
            rapid_floor_mm: 0.0,
            max_feed_mm_min: 5000.0,
            max_rpm: 24_000.0,
            material_feed: None,
            material_rpm: None,
            arcs: true,
            arc_gap_mm: crate::gcode::ARC_GAP_MM,
            reversal: true,
            zero_feed: true,
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
    /// A cutting move in X or Y below / above the range of feed of the material in use.
    FeedBelowMaterial,
    FeedAboveMaterial,
    /// An `S` below / above the range of spindle speed of the material in use.
    SpeedBelowMaterial,
    SpeedAboveMaterial,
    /// An arc given by its centre that ends at another distance from the centre than it starts (the limit is the biggest gap, in mm).
    ArcEndRadius,
    /// An arc given by its radius (`R`) smaller than half the distance between its ends (the limit is the biggest shortfall, in mm).
    ArcRadiusSmall,
    /// `M3` while `M4` turns, or the other way round, with no `M5` between: the spindle is told to reverse at speed.
    SpindleReversal,
    /// An `F` of zero: a controller refuses a cut with no feed.
    FeedZero,
    /// The program moves and has no `M2` or `M30`.
    NoEnd,
    /// A part of a line is not a letter with a number: it is left out of every check.
    Unreadable,
    /// `M0` or `M1` while the spindle turns.
    PauseSpindleOn,
    /// `G93` or `G95`: `F` is not in millimetres a minute, so the feeds cannot be judged.
    FeedMode,
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
        Kind::FeedBelowMaterial => [
            "{n} cutting moves run below the {limit} mm/min suggested for the material in use (first at line {line})",
            "{n} movimientos de corte van por debajo de los {limit} mm/min sugeridos para el material en uso (el primero en la línea {line})",
            "{n} Schnittbewegungen laufen unter den für das gewählte Material empfohlenen {limit} mm/min (die erste in Zeile {line})",
            "{n} déplacements de coupe vont sous les {limit} mm/min conseillés pour le matériau choisi (le premier à la ligne {line})",
            "{n} movimenti di taglio vanno sotto i {limit} mm/min suggeriti per il materiale in uso (il primo alla riga {line})",
            "{n} 件の切削移動が、使用中の材料の推奨値 {limit} mm/min を下回っています（最初は {line} 行目）",
            "{n} 个切削移动低于所选材料建议的 {limit} mm/min（第一个在第 {line} 行）",
            "{n} movimentos de corte ficam abaixo dos {limit} mm/min sugeridos para o material em uso (o primeiro na linha {line})",
        ],
        Kind::FeedAboveMaterial => [
            "{n} cutting moves run above the {limit} mm/min suggested for the material in use (first at line {line})",
            "{n} movimientos de corte van por encima de los {limit} mm/min sugeridos para el material en uso (el primero en la línea {line})",
            "{n} Schnittbewegungen laufen über den für das gewählte Material empfohlenen {limit} mm/min (die erste in Zeile {line})",
            "{n} déplacements de coupe vont au-dessus des {limit} mm/min conseillés pour le matériau choisi (le premier à la ligne {line})",
            "{n} movimenti di taglio vanno sopra i {limit} mm/min suggeriti per il materiale in uso (il primo alla riga {line})",
            "{n} 件の切削移動が、使用中の材料の推奨値 {limit} mm/min を上回っています（最初は {line} 行目）",
            "{n} 个切削移动高于所选材料建议的 {limit} mm/min（第一个在第 {line} 行）",
            "{n} movimentos de corte ficam acima dos {limit} mm/min sugeridos para o material em uso (o primeiro na linha {line})",
        ],
        Kind::SpeedBelowMaterial => [
            "{n} spindle speeds are below the {limit} rpm suggested for the material in use (first at line {line})",
            "{n} velocidades del husillo están por debajo de las {limit} rpm sugeridas para el material en uso (la primera en la línea {line})",
            "{n} Spindeldrehzahlen liegen unter den für das gewählte Material empfohlenen {limit} U/min (die erste in Zeile {line})",
            "{n} vitesses de broche sont sous les {limit} tr/min conseillés pour le matériau choisi (la première à la ligne {line})",
            "{n} velocità del mandrino sono sotto i {limit} giri/min suggeriti per il materiale in uso (la prima alla riga {line})",
            "{n} 件の主軸回転数が、使用中の材料の推奨値 {limit} rpm を下回っています（最初は {line} 行目）",
            "{n} 个主轴转速低于所选材料建议的 {limit} rpm（第一个在第 {line} 行）",
            "{n} velocidades do fuso ficam abaixo das {limit} rpm sugeridas para o material em uso (a primeira na linha {line})",
        ],
        Kind::SpeedAboveMaterial => [
            "{n} spindle speeds are above the {limit} rpm suggested for the material in use (first at line {line})",
            "{n} velocidades del husillo están por encima de las {limit} rpm sugeridas para el material en uso (la primera en la línea {line})",
            "{n} Spindeldrehzahlen liegen über den für das gewählte Material empfohlenen {limit} U/min (die erste in Zeile {line})",
            "{n} vitesses de broche sont au-dessus des {limit} tr/min conseillés pour le matériau choisi (la première à la ligne {line})",
            "{n} velocità del mandrino sono sopra i {limit} giri/min suggeriti per il materiale in uso (la prima alla riga {line})",
            "{n} 件の主軸回転数が、使用中の材料の推奨値 {limit} rpm を上回っています（最初は {line} 行目）",
            "{n} 个主轴转速高于所选材料建议的 {limit} rpm（第一个在第 {line} 行）",
            "{n} velocidades do fuso ficam acima das {limit} rpm sugeridas para o material em uso (a primeira na linha {line})",
        ],
        Kind::ArcEndRadius => [
            "{n} arcs end at a different distance from their centre than they start, by up to {limit} mm (first at line {line}): a controller refuses them with a radius error",
            "{n} arcos terminan a una distancia de su centro distinta de la de su comienzo, hasta {limit} mm (el primero en la línea {line}): un controlador los rechaza con un error de radio",
            "{n} Kreisbögen enden in einem anderen Abstand vom Mittelpunkt, als sie beginnen, um bis zu {limit} mm (der erste in Zeile {line}): eine Steuerung lehnt sie mit einem Radiusfehler ab",
            "{n} arcs finissent à une distance de leur centre différente de celle du départ, jusqu'à {limit} mm (le premier à la ligne {line}) : un contrôleur les refuse avec une erreur de rayon",
            "{n} archi finiscono a una distanza dal centro diversa da quella di partenza, fino a {limit} mm (il primo alla riga {line}): un controllore li rifiuta con un errore di raggio",
            "{n} 件の円弧の終点が、始点とは異なる中心からの距離にあり、最大 {limit} mm ずれています（最初は {line} 行目）：コントローラーは半径エラーで拒否します",
            "{n} 个圆弧的终点到圆心的距离与起点不同，最大相差 {limit} mm（第一个在第 {line} 行）：控制器会以半径错误拒绝",
            "{n} arcos terminam a uma distância do centro diferente da do início, até {limit} mm (o primeiro na linha {line}): um controlador recusa-os com um erro de raio",
        ],
        Kind::ArcRadiusSmall => [
            "{n} arcs have a radius (R) smaller than half the distance between their ends, by up to {limit} mm (first at line {line}): a controller refuses them",
            "{n} arcos tienen un radio (R) menor que la mitad de la distancia entre sus extremos, hasta {limit} mm (el primero en la línea {line}): un controlador los rechaza",
            "{n} Kreisbögen haben einen Radius (R), der um bis zu {limit} mm kleiner ist als der halbe Abstand ihrer Enden (der erste in Zeile {line}): eine Steuerung lehnt sie ab",
            "{n} arcs ont un rayon (R) inférieur à la moitié de la distance entre leurs extrémités, jusqu'à {limit} mm (le premier à la ligne {line}) : un contrôleur les refuse",
            "{n} archi hanno un raggio (R) minore della metà della distanza tra i loro estremi, fino a {limit} mm (il primo alla riga {line}): un controllore li rifiuta",
            "{n} 件の円弧で、半径（R）が両端の距離の半分より最大 {limit} mm 小さくなっています（最初は {line} 行目）：コントローラーは拒否します",
            "{n} 个圆弧的半径（R）比两端距离的一半小，最多小 {limit} mm（第一个在第 {line} 行）：控制器会拒绝",
            "{n} arcos têm um raio (R) menor do que metade da distância entre as extremidades, até {limit} mm (o primeiro na linha {line}): um controlador recusa-os",
        ],
        Kind::SpindleReversal => [
            "{n} times the spindle is told to turn the other way without stopping first (first at line {line}): stop it with M5 and wait",
            "{n} veces se manda al husillo girar al otro lado sin pararlo antes (la primera en la línea {line}): páralo con M5 y espera",
            "{n}-mal soll die Spindel ohne vorheriges Anhalten die Drehrichtung wechseln (das erste Mal in Zeile {line}): mit M5 anhalten und warten",
            "{n} fois la broche reçoit l'ordre de tourner dans l'autre sens sans s'arrêter avant (la première à la ligne {line}) : arrêtez-la avec M5 et attendez",
            "{n} volte il mandrino riceve l'ordine di girare nell'altro verso senza fermarsi prima (la prima alla riga {line}): fermalo con M5 e attendi",
            "{n} 回、停止せずにスピンドルの回転方向が逆転されます（最初は {line} 行目）：M5 で止めて待ってください",
            "{n} 次未先停止就让主轴反向旋转（第一次在第 {line} 行）：请先用 M5 停转并等待",
            "{n} vezes o fuso recebe ordem para rodar no sentido contrário sem parar antes (a primeira na linha {line}): pare-o com M5 e aguarde",
        ],
        Kind::FeedZero => [
            "{n} feed rates are set to zero (first at line {line}): a controller refuses a cut with no feed",
            "{n} avances se fijan en cero (el primero en la línea {line}): un controlador rechaza un corte sin avance",
            "{n} Vorschübe sind auf null gesetzt (der erste in Zeile {line}): eine Steuerung lehnt einen Schnitt ohne Vorschub ab",
            "{n} avances sont réglés à zéro (le premier à la ligne {line}) : un contrôleur refuse une coupe sans avance",
            "{n} avanzamenti sono impostati a zero (il primo alla riga {line}): un controllore rifiuta un taglio senza avanzamento",
            "{n} 件の送り速度が 0 に設定されています（最初は {line} 行目）：送り速度のない切削はコントローラーが拒否します",
            "{n} 个进给速度被设为零（第一个在第 {line} 行）：控制器会拒绝没有进给的切削",
            "{n} avanços estão definidos como zero (o primeiro na linha {line}): um controlador recusa um corte sem avanço",
        ],
        Kind::Unreadable => [
            "{n} lines hold something that cannot be read (first at line {line}): it is left out of every check",
            "{n} líneas contienen algo que no se puede leer (la primera es la línea {line}): queda fuera de todas las comprobaciones",
            "{n} Zeilen enthalten etwas Unlesbares (die erste ist Zeile {line}): es fehlt in jeder Prüfung",
            "{n} lignes contiennent quelque chose d'illisible (la première est la ligne {line}) : cela échappe à toutes les vérifications",
            "{n} righe contengono qualcosa di illeggibile (la prima è la riga {line}): resta fuori da ogni controllo",
            "{n} 行に読み取れない部分があります（最初は {line} 行目）：どの確認にも含まれません",
            "{n} 行含有无法读取的内容（第一个在第 {line} 行）：任何检查都不包含它",
            "{n} linhas têm algo que não se consegue ler (a primeira é a linha {line}): fica fora de todas as verificações",
        ],
        Kind::PauseSpindleOn => [
            "{n} pauses (M0 or M1) with the spindle turning (first at line {line}): stop it with M5 before the pause",
            "{n} pausas (M0 o M1) con el husillo girando (la primera en la línea {line}): páralo con M5 antes de la pausa",
            "{n} Pausen (M0 oder M1) bei drehender Spindel (die erste in Zeile {line}): vor der Pause mit M5 anhalten",
            "{n} pauses (M0 ou M1) broche en rotation (la première à la ligne {line}) : arrêtez-la avec M5 avant la pause",
            "{n} pause (M0 o M1) con il mandrino in rotazione (la prima alla riga {line}): fermalo con M5 prima della pausa",
            "{n} 件の一時停止（M0 または M1）で主軸が回っています（最初は {line} 行目）：停止の前に M5 で止めてください",
            "{n} 次暂停（M0 或 M1）时主轴仍在旋转（第一个在第 {line} 行）：请在暂停前用 M5 停转",
            "{n} pausas (M0 ou M1) com o fuso a rodar (a primeira na linha {line}): pare-o com M5 antes da pausa",
        ],
        Kind::FeedMode => [
            "G93 or G95 at line {line} changes what F means: the feeds of this program cannot be judged",
            "G93 o G95 en la línea {line} cambia lo que significa F: los avances de este programa no se pueden juzgar",
            "G93 oder G95 in Zeile {line} ändert die Bedeutung von F: die Vorschübe dieses Programms können nicht beurteilt werden",
            "G93 ou G95 à la ligne {line} change le sens de F : les avances de ce programme ne peuvent pas être jugées",
            "G93 o G95 alla riga {line} cambia il significato di F: gli avanzamenti di questo programma non si possono giudicare",
            "{line} 行目の G93 または G95 は F の意味を変えます：このプログラムの送り速度は判断できません",
            "第 {line} 行的 G93 或 G95 改变了 F 的含义：无法判断此程序的进给速度",
            "G93 ou G95 na linha {line} muda o significado de F: os avanços deste programa não se podem julgar",
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
    let mut reversal: Option<Finding> = None;
    let mut zero_feed: Option<Finding> = None;
    // the way the spindle turns now (3 or 4), when it does
    let mut turning: Option<i32> = None;
    let (mut feed_low, mut feed_high): (Option<Finding>, Option<Finding>) = (None, None);
    let (mut speed_low, mut speed_high): (Option<Finding>, Option<Finding>) = (None, None);
    // the feed in force, in mm/min
    let mut feed_now: Option<f64> = None;
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
    let (mut unread, mut paused, mut feed_mode): (
        Option<Finding>,
        Option<Finding>,
        Option<Finding>,
    ) = (None, None, None);
    for (n, raw) in lines.iter().enumerate() {
        if crate::gcode::unreadable(raw) {
            bump(&mut unread, Kind::Unreadable, n + 1);
        }
        let ws = words(raw);
        let mut axis = false;
        let mut xy = false;
        let mut new_z = None;
        // the speed of the line itself counts for the start of the same line
        if ws.iter().any(|&(l, _)| l == 'S') {
            speed_set = true;
        }
        for &(l, v) in &ws {
            let tenths = (v * 10.0).round() as i32;
            // the material in use: the feed in force is kept for the moves, and a spindle speed outside the range is said at once
            if l == 'F' {
                feed_now = Some(v * scale);
                if v <= 0.0 {
                    bump(&mut zero_feed, Kind::FeedZero, n + 1);
                }
            }
            if let ('S', Some((lo, hi))) = (l, cfg.material_rpm) {
                if v > 0.0 && v < lo {
                    bump(&mut speed_low, Kind::SpeedBelowMaterial, n + 1);
                } else if v > hi {
                    bump(&mut speed_high, Kind::SpeedAboveMaterial, n + 1);
                }
            }
            match l {
                'G' => match tenths {
                    0 => motion = 0,
                    10 => motion = 1,
                    20 | 30 => motion = 2,
                    900 => absolute = true,
                    910 => absolute = false,
                    200 => scale = 25.4,
                    210 => scale = 1.0,
                    930 | 950 => bump(&mut feed_mode, Kind::FeedMode, n + 1),
                    _ => {}
                },
                'F' if cfg.max_feed_mm_min > 0.0 && v * scale > cfg.max_feed_mm_min => {
                    bump(&mut fast, Kind::FeedTooHigh, n + 1)
                }
                'S' if cfg.max_rpm > 0.0 && v > cfg.max_rpm => {
                    bump(&mut spinning, Kind::SpeedTooHigh, n + 1)
                }
                'X' | 'Y' => {
                    axis = true;
                    xy = true;
                }
                'Z' => {
                    axis = true;
                    new_z = Some(v);
                }
                'M' => match v as i32 {
                    3 | 4 => {
                        if !speed_set {
                            bump(&mut nospeed, Kind::SpindleWithoutSpeed, n + 1);
                        }
                        let way = v as i32;
                        if turning.is_some_and(|t| t != way) {
                            bump(&mut reversal, Kind::SpindleReversal, n + 1);
                        }
                        turning = Some(way);
                        spindle_on = true;
                    }
                    5 => {
                        spindle_on = false;
                        turning = None;
                    }
                    2 | 30 => {
                        spindle_on = false;
                        turning = None;
                        ended = true;
                    }
                    6 => bump(&mut change, Kind::ToolChange, n + 1),
                    0 | 1 if spindle_on => bump(&mut paused, Kind::PauseSpindleOn, n + 1),
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
        } else {
            if !spindle_on {
                bump(&mut dry, Kind::CutWithoutSpindle, n + 1);
            }
            // a plunge (Z alone) has a feed of its own: only the moves in X or Y are held to the range of the material
            if let (true, Some((lo, hi)), Some(f)) = (xy, cfg.material_feed, feed_now) {
                if f < lo {
                    bump(&mut feed_low, Kind::FeedBelowMaterial, n + 1);
                } else if f > hi {
                    bump(&mut feed_high, Kind::FeedAboveMaterial, n + 1);
                }
            }
        }
    }
    for (slot, limit) in [
        (&mut fast, cfg.max_feed_mm_min),
        (&mut spinning, cfg.max_rpm),
        (&mut feed_low, cfg.material_feed.map_or(0.0, |r| r.0)),
        (&mut feed_high, cfg.material_feed.map_or(0.0, |r| r.1)),
        (&mut speed_low, cfg.material_rpm.map_or(0.0, |r| r.0)),
        (&mut speed_high, cfg.material_rpm.map_or(0.0, |r| r.1)),
    ] {
        if let Some(f) = slot {
            f.limit = limit;
        }
    }
    // the arcs a controller refuses
    let (mut end_radius, mut too_small): (Option<Finding>, Option<Finding>) = (None, None);
    let faults = if cfg.arcs {
        crate::gcode::arc_faults_within(lines, cfg.arc_gap_mm)
    } else {
        Vec::new()
    };
    for (line, fault, gap) in faults {
        let (slot, kind) = match fault {
            crate::gcode::ArcFault::EndRadius => (&mut end_radius, Kind::ArcEndRadius),
            crate::gcode::ArcFault::RadiusTooSmall => (&mut too_small, Kind::ArcRadiusSmall),
        };
        bump(slot, kind, line + 1);
        if let Some(f) = slot {
            f.limit = f.limit.max(gap);
        }
    }
    let mut out: Vec<Finding> = [
        rapid,
        dry,
        nospeed,
        change,
        fast,
        spinning,
        feed_low,
        feed_high,
        speed_low,
        speed_high,
        end_radius,
        too_small,
        reversal.filter(|_| cfg.reversal),
        zero_feed.filter(|_| cfg.zero_feed),
        unread,
        paused,
        feed_mode,
    ]
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
    fn the_feed_and_the_speed_are_held_to_the_range_of_the_material_in_use() {
        let cfg = LintCfg {
            material_feed: Some((100.0, 600.0)),
            material_rpm: Some((10_000.0, 24_000.0)),
            max_feed_mm_min: 0.0,
            max_rpm: 0.0,
            ..LintCfg::default()
        };
        let p = lines(
            "M3 S5000\nG1 X5 F50\nG1 X9 F700\nG1 Z-1 F20\nG1 X1 F300\nG2 X3 Y0 I1 J0 F650\nS30000\nM30",
        );
        let got: Vec<(Kind, usize, usize, f64)> = lint(&p, &cfg)
            .iter()
            .map(|f| (f.kind, f.line, f.count, f.limit))
            .collect();
        assert_eq!(
            got,
            vec![
                (Kind::FeedBelowMaterial, 2, 1, 100.0),
                (Kind::FeedAboveMaterial, 3, 2, 600.0),
                (Kind::SpeedBelowMaterial, 1, 1, 10_000.0),
                (Kind::SpeedAboveMaterial, 7, 1, 24_000.0),
            ]
        );
        // a plunge in Z alone and a rapid are never held to the feed, and S0 is not a speed below the range
        let q = lines("M3 S12000\nG0 X1 F10\nG1 Z-2 F20\nS0\nM30");
        assert!(lint(&q, &cfg).is_empty());
        // without a material nothing is said, and a material with only one range checks only that one
        assert!(lint(&p, &LintCfg::default()).iter().all(|f| !matches!(
            f.kind,
            Kind::FeedBelowMaterial
                | Kind::FeedAboveMaterial
                | Kind::SpeedBelowMaterial
                | Kind::SpeedAboveMaterial
        )));
        let only = LintCfg {
            material_rpm: Some((10_000.0, 24_000.0)),
            ..LintCfg::default()
        };
        assert!(lint(&p, &only)
            .iter()
            .all(|f| !matches!(f.kind, Kind::FeedBelowMaterial | Kind::FeedAboveMaterial)));
        // inches: F2 is 50.8 mm/min, under the range
        let inch = lines("G20\nM3 S12000\nG1 X1 F2\nM30");
        assert_eq!(lint(&inch, &cfg)[0].kind, Kind::FeedBelowMaterial);
        // the sentence has the number in it, in each language
        let f = lint(&p, &cfg)[0];
        assert!(f.message("en").contains("100 mm/min"));
        assert!(f.message("de").contains("100 mm/min"));
        assert!(f.message("zh").contains("100"));
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
    fn the_default_limits_are_24000_rpm_and_5000_mm_per_min() {
        let d = LintCfg::default();
        assert_eq!((d.max_feed_mm_min, d.max_rpm), (5000.0, 24_000.0));
        let got: Vec<Kind> = lint(&lines("M3 S30000\nG1 X1 F6000\nM30"), &d)
            .iter()
            .map(|f| f.kind)
            .collect();
        assert_eq!(got, vec![Kind::FeedTooHigh, Kind::SpeedTooHigh]);
        // at the limit is not above it
        assert!(lint(&lines("M3 S24000\nG1 X1 F5000\nM30"), &d).is_empty());
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
        // the defaults are 5000 mm/min and 24000 rpm: this program is under both, and 0 turns the check off
        assert!(lint(&p, &LintCfg::default()).is_empty());
        let off = LintCfg {
            max_feed_mm_min: 0.0,
            max_rpm: 0.0,
            ..LintCfg::default()
        };
        assert!(lint(&lines("M3 S90000\nG1 X1 F90000\nM30"), &off).is_empty());
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
            Kind::ArcEndRadius,
            Kind::ArcRadiusSmall,
            Kind::SpindleReversal,
            Kind::FeedZero,
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

    #[test]
    fn an_arc_that_ends_off_its_circle_or_is_too_small_is_said_with_the_biggest_gap() {
        // line 3 starts at X1 and goes round the centre (2, 0): its end at X4 is 2 mm from the centre where it started 1 mm away
        // (a gap of 1 mm), line 4 ends at X3.5 (a gap of 0.5 mm), line 5 asks for a radius of 3 over 20 mm and line 6 one of 4 over 10 mm
        let p = lines("M3 S1000\nG1 X1 F100\nG2 X4 Y0 I1 J0\nG2 X3.5 Y0 I1 J0\nG3 X20 Y0 R3\nG3 X30 Y0 R4\nM30");
        let got: Vec<(Kind, usize, usize, f64)> = lint(&p, &LintCfg::default())
            .iter()
            .map(|f| (f.kind, f.line, f.count, f.limit))
            .collect();
        assert_eq!(got.len(), 2, "{got:?}");
        assert_eq!((got[0].0, got[0].1, got[0].2), (Kind::ArcEndRadius, 3, 2));
        assert!((got[0].3 - 1.0).abs() < 1e-9, "{got:?}");
        assert_eq!((got[1].0, got[1].1), (Kind::ArcRadiusSmall, 5));
        assert!((got[1].3 - 5.25).abs() < 1e-9, "{got:?}");
        // a sound arc, by its centre and by its radius, says nothing
        assert!(kinds("M3 S1000\nG1 X1 F100\nG2 X3 Y0 I1 J0\nG3 X1 Y0 R1\nM30").is_empty());
        assert!(lint(&p, &LintCfg::default())[0]
            .message("en")
            .contains("by up to"));
        // with the checks off nothing is said
        let off = LintCfg {
            enabled: false,
            ..LintCfg::default()
        };
        assert!(lint(&p, &off).is_empty());
    }

    #[test]
    fn a_spindle_told_to_reverse_without_stopping_and_a_feed_of_zero_are_found() {
        let p = lines("M3 S1000\nG1 X1 F100\nM4\nG1 X2\nM5\nM4\nM3\nM30");
        let got: Vec<(Kind, usize, usize)> = lint(&p, &LintCfg::default())
            .iter()
            .map(|f| (f.kind, f.line, f.count))
            .collect();
        // M4 on line 3 and M3 on line 7 reverse; M4 on line 6 starts after a stop
        assert_eq!(got, vec![(Kind::SpindleReversal, 3, 2)]);
        // the same direction again, a stop and a new start are not reversals
        assert!(kinds("M3 S1000\nM3\nG1 X1 F5\nM5\nM4\nM30").is_empty());
        // F0 is said once per line, in inches too, and a feed after it is fine
        let z = lint(
            &lines("M3 S1000\nG1 X1 F0\nG1 X2 F100\nG20\nG1 X3 F0.000\nM30"),
            &LintCfg::default(),
        );
        assert_eq!(z.len(), 1);
        assert_eq!((z[0].kind, z[0].line, z[0].count), (Kind::FeedZero, 2, 2));
        assert!(z[0]
            .message("en")
            .starts_with("2 feed rates are set to zero"));
        assert!(kinds("M3 S1000\nG1 X1 F0.1\nM30").is_empty());
    }

    #[test]
    fn each_of_the_new_checks_has_its_own_switch_and_the_others_keep_working() {
        let p = lines("M3 S1000\nG1 X1 F0\nM4\nG2 X4 Y0 I1 J0\nM30");
        let all: Vec<Kind> = lint(&p, &LintCfg::default())
            .iter()
            .map(|f| f.kind)
            .collect();
        assert_eq!(
            all,
            vec![Kind::ArcEndRadius, Kind::SpindleReversal, Kind::FeedZero]
        );
        let kinds_of =
            |cfg: LintCfg| -> Vec<Kind> { lint(&p, &cfg).iter().map(|f| f.kind).collect() };
        assert_eq!(
            kinds_of(LintCfg {
                arcs: false,
                ..LintCfg::default()
            }),
            vec![Kind::SpindleReversal, Kind::FeedZero]
        );
        assert_eq!(
            kinds_of(LintCfg {
                reversal: false,
                ..LintCfg::default()
            }),
            vec![Kind::ArcEndRadius, Kind::FeedZero]
        );
        assert_eq!(
            kinds_of(LintCfg {
                zero_feed: false,
                ..LintCfg::default()
            }),
            vec![Kind::ArcEndRadius, Kind::SpindleReversal]
        );
        let none = LintCfg {
            arcs: false,
            reversal: false,
            zero_feed: false,
            ..LintCfg::default()
        };
        assert!(kinds_of(none).is_empty());
        // the switches only silence these three: the other checks still speak
        assert_eq!(
            lint(&lines("G1 X5 F100\nG0 Z-4"), &none)
                .iter()
                .map(|f| f.kind)
                .collect::<Vec<_>>(),
            vec![Kind::RapidDown, Kind::CutWithoutSpindle, Kind::NoEnd]
        );
    }

    #[test]
    fn the_gap_of_the_arcs_comes_from_the_settings() {
        // 0.02 mm off on a circle of radius 5
        let p = lines("M3 S1000\nG1 X5 F100\nG2 X10.02 Y0 I2.5 J0\nM30");
        assert_eq!(lint(&p, &LintCfg::default()).len(), 1);
        let loose = LintCfg {
            arc_gap_mm: 0.05,
            ..LintCfg::default()
        };
        assert!(lint(&p, &loose).is_empty());
        assert_eq!(LintCfg::default().arc_gap_mm, crate::gcode::ARC_GAP_MM);
    }
}
