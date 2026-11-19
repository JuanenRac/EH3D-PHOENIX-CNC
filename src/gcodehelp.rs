//! The help of the words of G-code, one word at a time: what a code does, how it is written and what to watch for, in the eight languages
//! of the interface. A word of the line being read (`G83`, `M6`, `F300`) or a word typed by the operator finds its entry; a line gives the
//! entries of all its words. The command line prints it with `--gcode`. It is a short reminder, not the manual of a controller: what a
//! controller does with a code can differ, and the entries say so where it matters.

use crate::gcode::words;

/// The language codes, in the order of the texts of an entry.
pub use crate::words::LANGS;

/// One entry of the help: the words it answers for, how it is written and what it does.
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    /// The words that find it (`G2` and `G3` share one), in their short form.
    pub names: &'static [&'static str],
    /// How it is written; the same in every language.
    pub syntax: &'static str,
    /// What it does, in the order of `LANGS`.
    pub text: [&'static str; 8],
}

impl Entry {
    /// What the entry says in a language (a code of `LANGS`; anything else gives English).
    pub fn say(&self, lang: &str) -> &'static str {
        let i = LANGS
            .iter()
            .position(|l| l.eq_ignore_ascii_case(lang.trim().split('-').next().unwrap_or("")))
            .unwrap_or(0);
        self.text[i]
    }
}

/// Every entry of the help.
pub const ENTRIES: &[Entry] = &[
    Entry {
        names: &["G0"],
        syntax: "G0 X Y Z",
        text: [
            "Rapid move: goes to the point as fast as the machine can, without cutting.",
            "Movimiento rápido: va al punto lo más deprisa que puede la máquina, sin cortar.",
            "Eilgang: fährt so schnell wie die Maschine kann zum Punkt, ohne zu schneiden.",
            "Déplacement rapide : va au point aussi vite que la machine le peut, sans couper.",
            "Spostamento rapido: va al punto alla massima velocità della macchina, senza tagliare.",
            "早送り：切削せずに、機械の最高速度で指定の点へ移動します。",
            "快速移动：不切削，以机床最快速度移动到指定点。",
            "Movimento rápido: vai ao ponto o mais depressa que a máquina consegue, sem cortar.",
        ],
    },
    Entry {
        names: &["G1"],
        syntax: "G1 X Y Z F",
        text: [
            "Straight cutting move to the point at the feed rate F.",
            "Movimiento recto de corte hasta el punto a la velocidad de avance F.",
            "Gerade Schnittbewegung zum Punkt mit dem Vorschub F.",
            "Déplacement de coupe en ligne droite jusqu'au point, à l'avance F.",
            "Spostamento di taglio in linea retta fino al punto, con l'avanzamento F.",
            "送り速度 F で指定の点まで直線切削します。",
            "以进给速度 F 直线切削到指定点。",
            "Movimento de corte em linha reta até ao ponto, com o avanço F.",
        ],
    },
    Entry {
        names: &["G2", "G3"],
        syntax: "G2 X Y I J  |  G3 X Y R",
        text: [
            "Arc move, clockwise (G2) or counter-clockwise (G3), to the end point; the centre is given by I, J, K (from the start) or the radius by R.",
            "Arco en sentido horario (G2) o antihorario (G3) hasta el punto final; el centro se da con I, J, K (desde el inicio) o el radio con R.",
            "Kreisbogen im Uhrzeigersinn (G2) oder gegen den Uhrzeigersinn (G3) zum Endpunkt; der Mittelpunkt wird mit I, J, K (vom Start aus) oder der Radius mit R angegeben.",
            "Arc dans le sens horaire (G2) ou antihoraire (G3) jusqu'au point final ; le centre est donné par I, J, K (depuis le départ) ou le rayon par R.",
            "Arco in senso orario (G2) o antiorario (G3) fino al punto finale; il centro si dà con I, J, K (dal punto di partenza) o il raggio con R.",
            "終点まで時計回り（G2）または反時計回り（G3）の円弧を描きます。中心は I、J、K（始点からの距離）、または半径 R で指定します。",
            "顺时针（G2）或逆时针（G3）圆弧移动到终点；圆心用 I、J、K（相对起点）给出，或用 R 给出半径。",
            "Arco no sentido horário (G2) ou anti-horário (G3) até ao ponto final; o centro dá-se com I, J, K (desde o início) ou o raio com R.",
        ],
    },
    Entry {
        names: &["G4"],
        syntax: "G4 P",
        text: [
            "Dwell: waits P seconds (some controllers read P in milliseconds) before the next line.",
            "Pausa: espera P segundos (algunos controladores leen P en milisegundos) antes de la línea siguiente.",
            "Verweilzeit: wartet P Sekunden (manche Steuerungen lesen P in Millisekunden) vor der nächsten Zeile.",
            "Temporisation : attend P secondes (certains contrôleurs lisent P en millisecondes) avant la ligne suivante.",
            "Pausa: attende P secondi (alcuni controller leggono P in millisecondi) prima della riga successiva.",
            "ドウェル：次の行の前に P 秒待ちます（P をミリ秒で読むコントローラもあります）。",
            "暂停：在下一行之前等待 P 秒（部分控制器把 P 当作毫秒）。",
            "Pausa: espera P segundos (alguns controladores leem P em milissegundos) antes da linha seguinte.",
        ],
    },
    Entry {
        names: &["G10"],
        syntax: "G10 L2 P X Y Z  |  G10 L20 P X Y Z",
        text: [
            "Writes values into the tables of the machine: G10 L2 Pn sets the offsets of a work system, G10 L20 Pn makes the current position the given one. It changes where the work is.",
            "Escribe valores en las tablas de la máquina: G10 L2 Pn fija los desplazamientos de un sistema de trabajo, G10 L20 Pn hace que la posición actual sea la indicada. Cambia dónde está la pieza.",
            "Schreibt Werte in die Tabellen der Maschine: G10 L2 Pn setzt die Verschiebungen eines Arbeitskoordinatensystems, G10 L20 Pn macht die aktuelle Position zur angegebenen. Es ändert, wo das Werkstück liegt.",
            "Écrit des valeurs dans les tables de la machine : G10 L2 Pn fixe les décalages d'un système de travail, G10 L20 Pn fait de la position actuelle celle indiquée. Cela change l'emplacement de la pièce.",
            "Scrive valori nelle tabelle della macchina: G10 L2 Pn imposta gli offset di un sistema di lavoro, G10 L20 Pn rende la posizione attuale quella indicata. Cambia dove si trova il pezzo.",
            "機械のテーブルに値を書き込みます。G10 L2 Pn はワーク座標系のオフセットを設定し、G10 L20 Pn は現在位置を指定の値にします。ワークの位置が変わります。",
            "向机床表格写入数值：G10 L2 Pn 设置工件坐标系的偏移，G10 L20 Pn 把当前位置设为给定值。它会改变工件的位置。",
            "Escreve valores nas tabelas da máquina: G10 L2 Pn define os desvios de um sistema de trabalho, G10 L20 Pn faz da posição atual a indicada. Muda o sítio onde está a peça.",
        ],
    },
    Entry {
        names: &["G17", "G18", "G19"],
        syntax: "G17  |  G18  |  G19",
        text: [
            "Chooses the plane of the arcs: G17 is XY, G18 is XZ, G19 is YZ.",
            "Elige el plano de los arcos: G17 es XY, G18 es XZ, G19 es YZ.",
            "Wählt die Ebene der Kreisbögen: G17 ist XY, G18 ist XZ, G19 ist YZ.",
            "Choisit le plan des arcs : G17 est XY, G18 est XZ, G19 est YZ.",
            "Sceglie il piano degli archi: G17 è XY, G18 è XZ, G19 è YZ.",
            "円弧の平面を選びます：G17 は XY、G18 は XZ、G19 は YZ です。",
            "选择圆弧所在平面：G17 为 XY，G18 为 XZ，G19 为 YZ。",
            "Escolhe o plano dos arcos: G17 é XY, G18 é XZ, G19 é YZ.",
        ],
    },
    Entry {
        names: &["G20", "G21"],
        syntax: "G20  |  G21",
        text: [
            "Units: G20 is inches, G21 is millimetres. Put one before the first move.",
            "Unidades: G20 son pulgadas, G21 son milímetros. Pon una antes del primer movimiento.",
            "Einheiten: G20 ist Zoll, G21 ist Millimeter. Setzen Sie eine vor die erste Bewegung.",
            "Unités : G20 est le pouce, G21 est le millimètre. Mettez-en une avant le premier mouvement.",
            "Unità: G20 sono i pollici, G21 i millimetri. Mettine una prima del primo movimento.",
            "単位：G20 はインチ、G21 はミリメートルです。最初の移動の前に指定してください。",
            "单位：G20 为英寸，G21 为毫米。请在第一次移动之前设置其中之一。",
            "Unidades: G20 são polegadas, G21 são milímetros. Ponha uma antes do primeiro movimento.",
        ],
    },
    Entry {
        names: &["G28", "G28.1", "G30", "G30.1"],
        syntax: "G28 X Y Z  |  G28.1  |  G30  |  G30.1",
        text: [
            "Goes to a position stored in the controller (G28, G30; G28.1 and G30.1 store the current position), by way of the point given. The stored place may not be where you expect.",
            "Va a una posición guardada en el controlador (G28, G30; G28.1 y G30.1 guardan la posición actual), pasando por el punto indicado. El lugar guardado puede no ser el que esperas.",
            "Fährt eine in der Steuerung gespeicherte Position an (G28, G30; G28.1 und G30.1 speichern die aktuelle Position), über den angegebenen Punkt. Der gespeicherte Ort ist vielleicht nicht der erwartete.",
            "Va à une position mémorisée dans le contrôleur (G28, G30 ; G28.1 et G30.1 mémorisent la position actuelle), en passant par le point donné. L'endroit mémorisé n'est peut-être pas celui attendu.",
            "Va a una posizione memorizzata nel controller (G28, G30; G28.1 e G30.1 memorizzano la posizione attuale), passando per il punto indicato. Il luogo memorizzato può non essere quello atteso.",
            "コントローラに保存された位置へ、指定の点を経由して移動します（G28、G30。G28.1 と G30.1 は現在位置を保存します）。保存された場所が想定と違うことがあります。",
            "经指定点移动到控制器中保存的位置（G28、G30；G28.1 和 G30.1 保存当前位置）。保存的位置可能与预期不同。",
            "Vai a uma posição guardada no controlador (G28, G30; G28.1 e G30.1 guardam a posição atual), passando pelo ponto indicado. O sítio guardado pode não ser o que espera.",
        ],
    },
    Entry {
        names: &["G38.2", "G38.3", "G38.4", "G38.5"],
        syntax: "G38.2 Z F",
        text: [
            "Probe: moves toward the point until the probe touches (G38.2 and G38.3) or lets go (G38.4 and G38.5); G38.2 and G38.4 raise an error when nothing happens.",
            "Palpado: avanza hacia el punto hasta que la sonda toca (G38.2 y G38.3) o se suelta (G38.4 y G38.5); G38.2 y G38.4 dan error si no ocurre nada.",
            "Antasten: fährt zum Punkt, bis der Taster berührt (G38.2 und G38.3) oder loslässt (G38.4 und G38.5); G38.2 und G38.4 melden einen Fehler, wenn nichts geschieht.",
            "Palpage : avance vers le point jusqu'à ce que la sonde touche (G38.2 et G38.3) ou lâche (G38.4 et G38.5) ; G38.2 et G38.4 donnent une erreur si rien ne se produit.",
            "Tastatura: avanza verso il punto finché la sonda tocca (G38.2 e G38.3) o si stacca (G38.4 e G38.5); G38.2 e G38.4 danno errore se non succede nulla.",
            "プロービング：プローブが接触する（G38.2、G38.3）か離れる（G38.4、G38.5）まで指定の点へ向かいます。何も起きなければ G38.2 と G38.4 はエラーになります。",
            "探测：向指定点移动，直到探针接触（G38.2、G38.3）或离开（G38.4、G38.5）；若未发生，G38.2 和 G38.4 会报错。",
            "Apalpação: avança para o ponto até a sonda tocar (G38.2 e G38.3) ou largar (G38.4 e G38.5); G38.2 e G38.4 dão erro se nada acontecer.",
        ],
    },
    Entry {
        names: &["G40"],
        syntax: "G40",
        text: [
            "Cutter compensation off: the centre of the tool follows the programmed path.",
            "Compensación de herramienta desactivada: el centro de la herramienta sigue el camino programado.",
            "Fräserradiuskorrektur aus: die Werkzeugmitte folgt der programmierten Bahn.",
            "Compensation d'outil désactivée : le centre de l'outil suit la trajectoire programmée.",
            "Compensazione utensile disattivata: il centro dell'utensile segue il percorso programmato.",
            "工具径補正オフ：工具の中心がプログラムした経路をたどります。",
            "取消刀具半径补偿：刀具中心沿编程路径移动。",
            "Compensação da ferramenta desligada: o centro da ferramenta segue o caminho programado.",
        ],
    },
    Entry {
        names: &["G41", "G42"],
        syntax: "G41 D  |  G42 D",
        text: [
            "Cutter compensation on: G41 keeps the tool to the left of the path, G42 to the right, by its radius (D names the tool). Turn it on and off with a straight move.",
            "Compensación de herramienta activada: G41 mantiene la herramienta a la izquierda del camino, G42 a la derecha, según su radio (D nombra la herramienta). Actívala y desactívala con un movimiento recto.",
            "Fräserradiuskorrektur ein: G41 hält das Werkzeug links der Bahn, G42 rechts, um seinen Radius (D nennt das Werkzeug). Ein- und ausschalten mit einer Geraden.",
            "Compensation d'outil activée : G41 garde l'outil à gauche de la trajectoire, G42 à droite, de la valeur de son rayon (D nomme l'outil). Activez-la et désactivez-la par un déplacement droit.",
            "Compensazione utensile attiva: G41 tiene l'utensile a sinistra del percorso, G42 a destra, del suo raggio (D indica l'utensile). Attivala e disattivala con uno spostamento rettilineo.",
            "工具径補正オン：G41 は経路の左側、G42 は右側に工具の半径分ずらします（D で工具を指定）。直線移動でオン・オフしてください。",
            "启用刀具半径补偿：G41 使刀具位于路径左侧，G42 位于右侧，偏移量为其半径（D 指定刀具）。请用直线移动来开启和关闭。",
            "Compensação da ferramenta ligada: G41 mantém a ferramenta à esquerda do caminho, G42 à direita, segundo o seu raio (D indica a ferramenta). Ligue-a e desligue-a com um movimento reto.",
        ],
    },
    Entry {
        names: &["G43.1", "G49"],
        syntax: "G43.1 Z  |  G49",
        text: [
            "Tool length offset: G43.1 Zn takes the length n for the tool in use, G49 cancels it.",
            "Corrección de longitud de herramienta: G43.1 Zn toma la longitud n para la herramienta en uso, G49 la cancela.",
            "Werkzeuglängenkorrektur: G43.1 Zn übernimmt die Länge n für das aktive Werkzeug, G49 hebt sie auf.",
            "Correction de longueur d'outil : G43.1 Zn prend la longueur n pour l'outil en cours, G49 l'annule.",
            "Correzione della lunghezza utensile: G43.1 Zn assume la lunghezza n per l'utensile in uso, G49 la annulla.",
            "工具長オフセット：G43.1 Zn は使用中の工具の長さを n にし、G49 で取り消します。",
            "刀具长度补偿：G43.1 Zn 为当前刀具取长度 n，G49 取消。",
            "Correção do comprimento da ferramenta: G43.1 Zn assume o comprimento n para a ferramenta em uso, G49 cancela-a.",
        ],
    },
    Entry {
        names: &["G53"],
        syntax: "G53 G0 X Y Z",
        text: [
            "Moves in machine coordinates for this line only, ignoring the work zero and the offsets.",
            "Se mueve en coordenadas de máquina solo en esta línea, sin tener en cuenta el cero de trabajo ni los desplazamientos.",
            "Bewegt in Maschinenkoordinaten, nur in dieser Zeile, ohne Werkstücknullpunkt und Verschiebungen.",
            "Se déplace en coordonnées machine pour cette ligne seulement, sans tenir compte du zéro pièce ni des décalages.",
            "Si muove in coordinate macchina solo in questa riga, ignorando lo zero pezzo e gli offset.",
            "この行だけ機械座標で移動し、ワークの原点とオフセットを無視します。",
            "仅此行使用机床坐标移动，忽略工件零点和偏移。",
            "Move-se em coordenadas da máquina só nesta linha, sem ter em conta o zero da peça nem os desvios.",
        ],
    },
    Entry {
        names: &["G54", "G55", "G56", "G57", "G58", "G59"],
        syntax: "G54  ...  G59",
        text: [
            "Chooses the work coordinate system (G54 to G59); each one keeps its own zero. Change it with the tool clear of the work.",
            "Elige el sistema de coordenadas de trabajo (G54 a G59); cada uno guarda su propio cero. Cámbialo con la herramienta lejos de la pieza.",
            "Wählt das Arbeitskoordinatensystem (G54 bis G59); jedes hat seinen eigenen Nullpunkt. Wechseln Sie, wenn das Werkzeug frei vom Werkstück ist.",
            "Choisit le système de coordonnées de travail (G54 à G59) ; chacun garde son propre zéro. Changez-le quand l'outil est dégagé de la pièce.",
            "Sceglie il sistema di coordinate di lavoro (da G54 a G59); ciascuno ha il proprio zero. Cambialo con l'utensile lontano dal pezzo.",
            "ワーク座標系（G54〜G59）を選びます。それぞれが独自の原点を持ちます。工具がワークから離れているときに切り替えてください。",
            "选择工件坐标系（G54 至 G59）；每个坐标系有各自的零点。请在刀具离开工件时切换。",
            "Escolhe o sistema de coordenadas de trabalho (G54 a G59); cada um guarda o seu próprio zero. Mude-o com a ferramenta afastada da peça.",
        ],
    },
    Entry {
        names: &["G61"],
        syntax: "G61",
        text: [
            "Exact path mode: the machine slows to the exact corner at every change of direction.",
            "Modo de trayectoria exacta: la máquina frena hasta la esquina exacta en cada cambio de dirección.",
            "Genauhaltmodus: die Maschine bremst bei jedem Richtungswechsel bis zur genauen Ecke ab.",
            "Mode trajectoire exacte : la machine ralentit jusqu'au coin exact à chaque changement de direction.",
            "Modo percorso esatto: la macchina rallenta fino allo spigolo esatto a ogni cambio di direzione.",
            "正確経路モード：方向が変わるたびに、機械は正確な角まで減速します。",
            "精确路径模式：每次改变方向时，机床都减速到准确的拐角。",
            "Modo de trajetória exata: a máquina abranda até ao canto exato em cada mudança de direção.",
        ],
    },
    Entry {
        names: &["G73"],
        syntax: "G73 X Y Z R Q F",
        text: [
            "Drilling cycle with chip breaking: cuts down in steps of Q and backs off a little each time; Z is the depth and R the plane to return to.",
            "Ciclo de taladrado con rotura de viruta: corta en pasos de Q y retrocede un poco cada vez; Z es la profundidad y R el plano de retorno.",
            "Bohrzyklus mit Spanbruch: bohrt in Schritten von Q und fährt jeweils ein Stück zurück; Z ist die Tiefe und R die Rückzugsebene.",
            "Cycle de perçage avec brise-copeaux : perce par pas de Q et recule un peu à chaque fois ; Z est la profondeur et R le plan de retour.",
            "Ciclo di foratura con rottura del truciolo: fora a passi di Q e arretra un poco ogni volta; Z è la profondità e R il piano di ritorno.",
            "切りくず分断の穴あけサイクル：Q ずつ切り込み、そのたびに少し戻ります。Z は深さ、R は戻り面です。",
            "断屑钻孔循环：每次按 Q 的步距钻入并稍微退回；Z 为深度，R 为返回平面。",
            "Ciclo de furação com quebra de apara: fura em passos de Q e recua um pouco de cada vez; Z é a profundidade e R o plano de retorno.",
        ],
    },
    Entry {
        names: &["G80"],
        syntax: "G80",
        text: [
            "Cancels the canned cycle that is on (G73, G81 to G89). Put it after the last hole.",
            "Cancela el ciclo fijo activo (G73, G81 a G89). Ponlo después del último agujero.",
            "Hebt den aktiven Bearbeitungszyklus auf (G73, G81 bis G89). Setzen Sie es nach dem letzten Loch.",
            "Annule le cycle fixe actif (G73, G81 à G89). Mettez-le après le dernier trou.",
            "Annulla il ciclo fisso attivo (G73, G81–G89). Mettilo dopo l'ultimo foro.",
            "有効な固定サイクル（G73、G81〜G89）を取り消します。最後の穴の後に置いてください。",
            "取消当前的固定循环（G73、G81 至 G89）。请放在最后一个孔之后。",
            "Cancela o ciclo fixo ativo (G73, G81 a G89). Ponha-o depois do último furo.",
        ],
    },
    Entry {
        names: &["G81"],
        syntax: "G81 X Y Z R F",
        text: [
            "Drilling cycle: goes to the hole, down to the R plane, cuts to the depth Z at the feed F and goes back up.",
            "Ciclo de taladrado: va al agujero, baja al plano R, corta hasta la profundidad Z con el avance F y vuelve a subir.",
            "Bohrzyklus: fährt zum Loch, auf die Ebene R, schneidet mit dem Vorschub F bis zur Tiefe Z und fährt wieder hoch.",
            "Cycle de perçage : va au trou, descend au plan R, perce jusqu'à la profondeur Z à l'avance F puis remonte.",
            "Ciclo di foratura: va al foro, scende al piano R, taglia fino alla profondità Z con l'avanzamento F e risale.",
            "穴あけサイクル：穴の位置へ移動し、R 面まで下がり、送り F で深さ Z まで切り込んで戻ります。",
            "钻孔循环：移到孔位，下到 R 平面，以进给 F 钻到深度 Z，然后退回。",
            "Ciclo de furação: vai ao furo, desce ao plano R, fura até à profundidade Z com o avanço F e volta a subir.",
        ],
    },
    Entry {
        names: &["G82"],
        syntax: "G82 X Y Z R P F",
        text: [
            "Drilling cycle with a dwell: like G81, and waits P seconds at the bottom of the hole.",
            "Ciclo de taladrado con pausa: como G81, y espera P segundos en el fondo del agujero.",
            "Bohrzyklus mit Verweilzeit: wie G81 und wartet P Sekunden am Lochgrund.",
            "Cycle de perçage avec temporisation : comme G81, et attend P secondes au fond du trou.",
            "Ciclo di foratura con pausa: come G81, e attende P secondi sul fondo del foro.",
            "ドウェル付き穴あけサイクル：G81 と同じで、穴の底で P 秒待ちます。",
            "带暂停的钻孔循环：与 G81 相同，并在孔底等待 P 秒。",
            "Ciclo de furação com pausa: como o G81, e espera P segundos no fundo do furo.",
        ],
    },
    Entry {
        names: &["G83"],
        syntax: "G83 X Y Z R Q F",
        text: [
            "Deep-hole peck drilling: cuts down in steps of Q, goes back out to the R plane after each step and comes back down to just above the last depth.",
            "Taladrado profundo por picoteo: corta en pasos de Q, sale hasta el plano R tras cada paso y vuelve a bajar hasta justo encima de la última profundidad.",
            "Tieflochbohren mit Spänebrechen: bohrt in Schritten von Q, fährt nach jedem Schritt bis zur Ebene R zurück und wieder bis knapp über die letzte Tiefe.",
            "Perçage profond par débourrage : perce par pas de Q, ressort jusqu'au plan R après chaque pas et redescend juste au-dessus de la dernière profondeur.",
            "Foratura profonda a step: fora a passi di Q, esce fino al piano R dopo ogni passo e riscende appena sopra l'ultima profondità.",
            "深穴ステップ加工：Q ずつ切り込み、各ステップの後で R 面まで戻り、前回の深さのすぐ上まで下がります。",
            "深孔啄钻：按 Q 的步距钻入，每步之后退回 R 平面，再下降到上次深度的上方。",
            "Furação profunda por passos: fura em passos de Q, sai até ao plano R depois de cada passo e volta a descer até mesmo acima da última profundidade.",
        ],
    },
    Entry {
        names: &["G84"],
        syntax: "G84 X Y Z R F",
        text: [
            "Tapping cycle: feeds down to the depth Z and back out at the same feed (the spindle direction is for the controller to handle).",
            "Ciclo de roscado: baja hasta la profundidad Z y sale con el mismo avance (el sentido del husillo lo gestiona el controlador).",
            "Gewindebohrzyklus: fährt mit Vorschub bis zur Tiefe Z und mit demselben Vorschub zurück (die Drehrichtung der Spindel übernimmt die Steuerung).",
            "Cycle de taraudage : descend à la profondeur Z et ressort à la même avance (le sens de la broche est géré par le contrôleur).",
            "Ciclo di maschiatura: scende alla profondità Z e risale con lo stesso avanzamento (il senso del mandrino lo gestisce il controller).",
            "タッピングサイクル：深さ Z まで送りで下がり、同じ送りで戻ります（主軸の回転方向はコントローラが扱います）。",
            "攻丝循环：以进给下到深度 Z，再以相同进给退出（主轴转向由控制器处理）。",
            "Ciclo de roscagem: desce até à profundidade Z e sai com o mesmo avanço (o sentido do fuso fica a cargo do controlador).",
        ],
    },
    Entry {
        names: &["G85", "G89"],
        syntax: "G85 X Y Z R F  |  G89 X Y Z R P F",
        text: [
            "Boring cycle: feeds down to the depth Z and feeds back out to R; G89 waits P seconds at the bottom first.",
            "Ciclo de mandrinado: baja con avance hasta la profundidad Z y sube con avance hasta R; G89 espera antes P segundos en el fondo.",
            "Ausbohrzyklus: fährt mit Vorschub bis zur Tiefe Z und mit Vorschub zurück bis R; G89 wartet zuvor P Sekunden am Grund.",
            "Cycle d'alésage : descend à l'avance jusqu'à la profondeur Z et remonte à l'avance jusqu'à R ; G89 attend d'abord P secondes au fond.",
            "Ciclo di alesatura: scende in avanzamento fino alla profondità Z e risale in avanzamento fino a R; G89 prima attende P secondi sul fondo.",
            "ボーリングサイクル：送りで深さ Z まで下がり、送りで R まで戻ります。G89 は先に底で P 秒待ちます。",
            "镗孔循环：以进给下到深度 Z，再以进给退回到 R；G89 先在孔底等待 P 秒。",
            "Ciclo de mandrilagem: desce com avanço até à profundidade Z e sobe com avanço até R; o G89 espera primeiro P segundos no fundo.",
        ],
    },
    Entry {
        names: &["G86"],
        syntax: "G86 X Y Z R F",
        text: [
            "Boring cycle with a spindle stop: feeds down to the depth Z, stops the spindle, rises fast and starts the spindle again.",
            "Ciclo de mandrinado con parada del husillo: baja con avance hasta Z, para el husillo, sube rápido y vuelve a arrancarlo.",
            "Ausbohrzyklus mit Spindelstopp: fährt mit Vorschub bis Z, stoppt die Spindel, fährt im Eilgang hoch und startet die Spindel wieder.",
            "Cycle d'alésage avec arrêt de broche : descend à l'avance jusqu'à Z, arrête la broche, remonte en rapide et redémarre la broche.",
            "Ciclo di alesatura con arresto del mandrino: scende in avanzamento fino a Z, ferma il mandrino, risale in rapido e lo riavvia.",
            "主軸停止付きボーリングサイクル：送りで Z まで下がり、主軸を止め、早送りで戻って主軸を再始動します。",
            "带主轴停止的镗孔循环：以进给下到 Z，停止主轴，快速上升，再重新启动主轴。",
            "Ciclo de mandrilagem com paragem do fuso: desce com avanço até Z, para o fuso, sobe em rápido e volta a ligá-lo.",
        ],
    },
    Entry {
        names: &["G90", "G91"],
        syntax: "G90  |  G91",
        text: [
            "Distances: G90 reads the coordinates as positions on the work, G91 as steps from where the tool is.",
            "Distancias: G90 lee las coordenadas como posiciones sobre la pieza, G91 como pasos desde donde está la herramienta.",
            "Maßangaben: G90 liest die Koordinaten als Positionen am Werkstück, G91 als Schritte von der aktuellen Werkzeugposition.",
            "Distances : G90 lit les coordonnées comme des positions sur la pièce, G91 comme des pas depuis la position de l'outil.",
            "Distanze: G90 legge le coordinate come posizioni sul pezzo, G91 come passi a partire da dove si trova l'utensile.",
            "距離の指定：G90 は座標をワーク上の位置として、G91 は工具の現在位置からの移動量として読みます。",
            "距离模式：G90 把坐标当作工件上的位置，G91 把坐标当作相对刀具当前位置的增量。",
            "Distâncias: o G90 lê as coordenadas como posições na peça, o G91 como passos a partir de onde está a ferramenta.",
        ],
    },
    Entry {
        names: &["G90.1", "G91.1"],
        syntax: "G90.1  |  G91.1",
        text: [
            "Arc centres: G90.1 reads I, J, K as positions, G91.1 as distances from the start of the arc (the usual one).",
            "Centros de arco: G90.1 lee I, J, K como posiciones, G91.1 como distancias desde el inicio del arco (lo habitual).",
            "Bogenmittelpunkte: G90.1 liest I, J, K als Positionen, G91.1 als Abstände vom Bogenanfang (üblich).",
            "Centres d'arc : G90.1 lit I, J, K comme des positions, G91.1 comme des distances depuis le début de l'arc (le cas habituel).",
            "Centri degli archi: G90.1 legge I, J, K come posizioni, G91.1 come distanze dall'inizio dell'arco (il caso usuale).",
            "円弧の中心：G90.1 は I、J、K を位置として、G91.1 は円弧の始点からの距離として読みます（通常はこちら）。",
            "圆弧圆心：G90.1 把 I、J、K 当作位置，G91.1 当作相对圆弧起点的距离（常用）。",
            "Centros dos arcos: o G90.1 lê I, J, K como posições, o G91.1 como distâncias desde o início do arco (o habitual).",
        ],
    },
    Entry {
        names: &["G92", "G92.1"],
        syntax: "G92 X Y Z  |  G92.1",
        text: [
            "Shifts the coordinates so the current position of the tool takes the values given (G92.1 clears the shift). It is not saved like the work systems.",
            "Desplaza las coordenadas para que la posición actual de la herramienta tome los valores indicados (G92.1 borra el desplazamiento). No se guarda como los sistemas de trabajo.",
            "Verschiebt die Koordinaten, sodass die aktuelle Werkzeugposition die angegebenen Werte erhält (G92.1 hebt die Verschiebung auf). Sie wird nicht wie die Arbeitskoordinatensysteme gespeichert.",
            "Décale les coordonnées pour que la position actuelle de l'outil prenne les valeurs données (G92.1 efface le décalage). Il n'est pas mémorisé comme les systèmes de travail.",
            "Sposta le coordinate in modo che la posizione attuale dell'utensile assuma i valori indicati (G92.1 cancella lo spostamento). Non viene salvato come i sistemi di lavoro.",
            "工具の現在位置が指定の値になるよう座標をずらします（G92.1 でずれを解除）。ワーク座標系のようには保存されません。",
            "移动坐标，使刀具当前位置取给定值（G92.1 清除该偏移）。它不像工件坐标系那样被保存。",
            "Desloca as coordenadas para que a posição atual da ferramenta tome os valores indicados (o G92.1 apaga o deslocamento). Não fica guardado como os sistemas de trabalho.",
        ],
    },
    Entry {
        names: &["G93", "G94"],
        syntax: "G93  |  G94",
        text: [
            "Feed mode: G94 gives F in units per minute, G93 as the inverse of the time of the move.",
            "Modo de avance: G94 da F en unidades por minuto, G93 como inverso del tiempo del movimiento.",
            "Vorschubmodus: G94 gibt F in Einheiten pro Minute an, G93 als Kehrwert der Zeit der Bewegung.",
            "Mode d'avance : G94 donne F en unités par minute, G93 comme l'inverse de la durée du mouvement.",
            "Modo di avanzamento: G94 dà F in unità al minuto, G93 come inverso del tempo del movimento.",
            "送りモード：G94 は F を毎分の距離で、G93 は移動時間の逆数で指定します。",
            "进给模式：G94 以每分钟距离给出 F，G93 以移动时间的倒数给出。",
            "Modo de avanço: o G94 dá F em unidades por minuto, o G93 como o inverso do tempo do movimento.",
        ],
    },
    Entry {
        names: &["G98", "G99"],
        syntax: "G98  |  G99",
        text: [
            "Where a canned cycle goes back to: G98 to the height it started from, G99 to the R plane.",
            "Adonde vuelve un ciclo fijo: G98 a la altura de la que partió, G99 al plano R.",
            "Wohin ein Bearbeitungszyklus zurückfährt: G98 auf die Ausgangshöhe, G99 auf die Ebene R.",
            "Où revient un cycle fixe : G98 à la hauteur de départ, G99 au plan R.",
            "Dove torna un ciclo fisso: G98 all'altezza di partenza, G99 al piano R.",
            "固定サイクルの戻り位置：G98 は開始時の高さへ、G99 は R 面へ戻ります。",
            "固定循环的返回位置：G98 回到起始高度，G99 回到 R 平面。",
            "Para onde volta um ciclo fixo: o G98 à altura de onde partiu, o G99 ao plano R.",
        ],
    },
    Entry {
        names: &["M0", "M1"],
        syntax: "M0  |  M1",
        text: [
            "Stops the program until the operator continues (M1 only when the optional stop is on).",
            "Detiene el programa hasta que el operador continúe (M1 solo si la parada opcional está activa).",
            "Hält das Programm an, bis der Bediener fortfährt (M1 nur, wenn der optionale Halt aktiv ist).",
            "Arrête le programme jusqu'à ce que l'opérateur continue (M1 seulement si l'arrêt optionnel est actif).",
            "Ferma il programma finché l'operatore non prosegue (M1 solo se l'arresto opzionale è attivo).",
            "オペレータが続行するまでプログラムを止めます（M1 はオプショナルストップが有効なときだけ）。",
            "暂停程序，直到操作员继续（M1 仅在可选停止开启时生效）。",
            "Para o programa até o operador continuar (o M1 só se a paragem opcional estiver ativa).",
        ],
    },
    Entry {
        names: &["M2", "M30"],
        syntax: "M2  |  M30",
        text: [
            "Ends the program (M30 also goes back to its start).",
            "Termina el programa (M30 además vuelve a su inicio).",
            "Beendet das Programm (M30 springt zusätzlich zum Anfang zurück).",
            "Termine le programme (M30 revient en plus à son début).",
            "Termina il programma (M30 torna anche al suo inizio).",
            "プログラムを終了します（M30 は先頭にも戻ります）。",
            "结束程序（M30 还会回到程序开头）。",
            "Termina o programa (o M30 volta também ao seu início).",
        ],
    },
    Entry {
        names: &["M3", "M4"],
        syntax: "M3 S  |  M4 S",
        text: [
            "Starts the spindle: M3 clockwise, M4 counter-clockwise, at the speed S. Stop it before reversing.",
            "Arranca el husillo: M3 en sentido horario, M4 antihorario, a la velocidad S. Páralo antes de invertir el giro.",
            "Startet die Spindel: M3 im Uhrzeigersinn, M4 dagegen, mit der Drehzahl S. Vor dem Umkehren anhalten.",
            "Démarre la broche : M3 dans le sens horaire, M4 antihoraire, à la vitesse S. Arrêtez-la avant d'inverser.",
            "Avvia il mandrino: M3 in senso orario, M4 antiorario, alla velocità S. Fermalo prima di invertire.",
            "主軸を回転させます：M3 は時計回り、M4 は反時計回りで、速度は S です。反転する前に停止してください。",
            "启动主轴：M3 顺时针，M4 逆时针，转速为 S。反转前请先停止。",
            "Liga o fuso: M3 no sentido horário, M4 no anti-horário, à velocidade S. Pare-o antes de inverter.",
        ],
    },
    Entry {
        names: &["M5"],
        syntax: "M5",
        text: [
            "Stops the spindle.",
            "Para el husillo.",
            "Stoppt die Spindel.",
            "Arrête la broche.",
            "Ferma il mandrino.",
            "主軸を停止します。",
            "停止主轴。",
            "Para o fuso.",
        ],
    },
    Entry {
        names: &["M6"],
        syntax: "T M6",
        text: [
            "Tool change: takes the tool chosen with T (a simple controller does not do it; the operator changes it by hand).",
            "Cambio de herramienta: toma la herramienta elegida con T (un controlador sencillo no lo hace; lo cambia el operador a mano).",
            "Werkzeugwechsel: übernimmt das mit T gewählte Werkzeug (eine einfache Steuerung führt ihn nicht aus; der Bediener wechselt von Hand).",
            "Changement d'outil : prend l'outil choisi avec T (un contrôleur simple ne le fait pas ; l'opérateur le change à la main).",
            "Cambio utensile: prende l'utensile scelto con T (un controller semplice non lo esegue; lo cambia l'operatore a mano).",
            "工具交換：T で選んだ工具に交換します（簡易コントローラは行いません。オペレータが手で交換します）。",
            "换刀：换上用 T 选定的刀具（简单的控制器不执行此命令，由操作员手动换刀）。",
            "Troca de ferramenta: passa para a ferramenta escolhida com T (um controlador simples não a faz; o operador troca-a à mão).",
        ],
    },
    Entry {
        names: &["M7", "M8"],
        syntax: "M7  |  M8",
        text: [
            "Coolant on: M7 mist, M8 flood.",
            "Refrigerante activado: M7 niebla, M8 inundación.",
            "Kühlmittel ein: M7 Nebel, M8 Flut.",
            "Arrosage activé : M7 brouillard, M8 abondant.",
            "Refrigerante acceso: M7 nebulizzato, M8 a getto.",
            "クーラントオン：M7 はミスト、M8 は大量供給です。",
            "开启冷却液：M7 为雾状，M8 为浇注。",
            "Refrigerante ligado: M7 névoa, M8 jorro.",
        ],
    },
    Entry {
        names: &["M9"],
        syntax: "M9",
        text: [
            "Coolant off.",
            "Refrigerante desactivado.",
            "Kühlmittel aus.",
            "Arrosage coupé.",
            "Refrigerante spento.",
            "クーラントオフ。",
            "关闭冷却液。",
            "Refrigerante desligado.",
        ],
    },
    Entry {
        names: &["F"],
        syntax: "F",
        text: [
            "Feed rate: how fast the tool cuts, in units per minute; it stays until another F.",
            "Velocidad de avance: lo deprisa que corta la herramienta, en unidades por minuto; se mantiene hasta otra F.",
            "Vorschub: wie schnell das Werkzeug schneidet, in Einheiten pro Minute; gilt bis zum nächsten F.",
            "Vitesse d'avance : la vitesse de coupe de l'outil, en unités par minute ; elle reste jusqu'au prochain F.",
            "Avanzamento: la velocità di taglio dell'utensile, in unità al minuto; resta fino al prossimo F.",
            "送り速度：工具が切削する速さ（毎分の距離）。次の F まで有効です。",
            "进给速度：刀具切削的快慢，单位为每分钟的距离；保持到下一个 F。",
            "Velocidade de avanço: a rapidez com que a ferramenta corta, em unidades por minuto; mantém-se até outro F.",
        ],
    },
    Entry {
        names: &["S"],
        syntax: "S",
        text: [
            "Spindle speed in revolutions per minute; it takes effect with M3 or M4.",
            "Velocidad del husillo en revoluciones por minuto; entra en vigor con M3 o M4.",
            "Spindeldrehzahl in Umdrehungen pro Minute; wirkt mit M3 oder M4.",
            "Vitesse de la broche en tours par minute ; elle prend effet avec M3 ou M4.",
            "Velocità del mandrino in giri al minuto; ha effetto con M3 o M4.",
            "主軸の回転速度（毎分の回転数）。M3 または M4 で有効になります。",
            "主轴转速，单位为转每分钟；随 M3 或 M4 生效。",
            "Velocidade do fuso em rotações por minuto; entra em vigor com M3 ou M4.",
        ],
    },
    Entry {
        names: &["T"],
        syntax: "T",
        text: [
            "Chooses the tool by its number; the change itself is made by M6.",
            "Elige la herramienta por su número; el cambio lo hace M6.",
            "Wählt das Werkzeug nach seiner Nummer; der Wechsel selbst geschieht mit M6.",
            "Choisit l'outil par son numéro ; le changement lui-même est fait par M6.",
            "Sceglie l'utensile dal suo numero; il cambio vero e proprio lo fa M6.",
            "工具を番号で選びます。交換そのものは M6 が行います。",
            "按编号选择刀具；换刀动作由 M6 完成。",
            "Escolhe a ferramenta pelo número; a troca em si é feita pelo M6.",
        ],
    },
    Entry {
        names: &["D"],
        syntax: "D",
        text: [
            "Names the tool whose radius the cutter compensation (G41, G42) uses.",
            "Nombra la herramienta cuyo radio usa la compensación (G41, G42).",
            "Nennt das Werkzeug, dessen Radius die Fräserradiuskorrektur (G41, G42) verwendet.",
            "Nomme l'outil dont la compensation (G41, G42) utilise le rayon.",
            "Indica l'utensile il cui raggio usa la compensazione (G41, G42).",
            "工具径補正（G41、G42）が半径を使う工具を指定します。",
            "指定刀具半径补偿（G41、G42）所用半径的刀具。",
            "Indica a ferramenta cujo raio a compensação (G41, G42) usa.",
        ],
    },
];

/// The name a word is looked up by: `G01` and `G1.0` are `G1`, `G38.2` stays, and the words F, S, T and D are their letter whatever
/// their number. A word that is not a letter and a number (or one of those four letters alone) has none.
pub fn name_of(word: &str) -> Option<String> {
    let w = word.trim().to_ascii_uppercase();
    let mut chars = w.chars();
    let letter = chars.next().filter(char::is_ascii_alphabetic)?;
    let rest: &str = chars.as_str();
    if matches!(letter, 'F' | 'S' | 'T' | 'D') {
        if rest.is_empty() || rest.parse::<f64>().is_ok() {
            return Some(letter.to_string());
        }
        return None;
    }
    let v = rest
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)?;
    // codes have at most one decimal (G38.2)
    let tenths = (v * 10.0).round() as i64;
    if (v * 10.0 - tenths as f64).abs() > 1e-6 {
        return None;
    }
    Some(if tenths % 10 == 0 {
        format!("{letter}{}", tenths / 10)
    } else {
        format!("{letter}{}.{}", tenths / 10, tenths % 10)
    })
}

/// The entry that answers for a word.
pub fn lookup(word: &str) -> Option<&'static Entry> {
    let name = name_of(word)?;
    ENTRIES.iter().find(|e| e.names.contains(&name.as_str()))
}

/// The help of one word, as a text in a language: the word, how it is written and what it does. Nothing for a word with no entry.
pub fn describe(word: &str, lang: &str) -> Option<String> {
    let e = lookup(word)?;
    let name = name_of(word)?;
    Some(format!("{name}: {}\n  {}", e.syntax, e.say(lang)))
}

/// The entries of the words of a line of G-code, each once and in the order of the line; a word with no entry (an axis, a number the
/// program gives) is left out.
pub fn for_line(line: &str, lang: &str) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for (c, v) in words(line) {
        let word = if matches!(c, 'F' | 'S' | 'T' | 'D') {
            c.to_string()
        } else {
            format!("{c}{v}")
        };
        let Some(name) = name_of(&word) else { continue };
        if seen.contains(&name) {
            continue;
        }
        if let Some(text) = describe(&word, lang) {
            seen.push(name);
            out.push(text);
        }
    }
    out
}

/// What `--gcode` prints for its argument: the help of the word, or of every word of a line when it has several. Nothing when no word
/// has an entry.
pub fn answer(arg: &str, lang: &str) -> Option<String> {
    let arg = arg.trim();
    let parts: Vec<String> = if arg.split_whitespace().count() > 1 {
        for_line(arg, lang)
    } else {
        describe(arg, lang).into_iter().collect()
    };
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n") + "\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_has_its_text_in_the_eight_languages() {
        for e in ENTRIES {
            assert!(!e.syntax.is_empty(), "{:?}", e.names);
            for (i, t) in e.text.iter().enumerate() {
                assert!(
                    !t.trim().is_empty(),
                    "{:?} is empty in {}",
                    e.names,
                    LANGS[i]
                );
                assert_eq!(t.trim(), *t, "{:?} in {}", e.names, LANGS[i]);
            }
            // a translation is not the English text copied
            let same = e.text.iter().skip(1).filter(|t| **t == e.text[0]).count();
            assert_eq!(same, 0, "{:?} is not translated", e.names);
        }
    }

    #[test]
    fn the_help_is_in_the_portuguese_of_portugal() {
        for e in ENTRIES {
            let pt = e.text[7];
            for brazilian in ["arquivo", "tela", "celular", "usuário", "salvo "] {
                assert!(!pt.contains(brazilian), "{:?}: {pt}", e.names);
            }
        }
    }

    #[test]
    fn no_name_belongs_to_two_entries_and_every_name_finds_its_entry() {
        let mut seen = std::collections::HashSet::new();
        for e in ENTRIES {
            for n in e.names {
                assert!(seen.insert(*n), "{n} twice");
                let found = lookup(n).expect(n);
                assert_eq!(found.names, e.names, "{n}");
            }
        }
    }

    #[test]
    fn every_code_the_completion_offers_has_its_help() {
        for code in crate::complete::CODES {
            if code.starts_with('$') {
                continue;
            }
            assert!(lookup(code).is_some(), "{code} has no help");
        }
        // and the codes that the checks and the cycles name
        for code in [
            "G73", "G81", "G82", "G83", "G84", "G85", "G86", "G89", "G98", "G99", "G41", "G42",
        ] {
            assert!(lookup(code).is_some(), "{code}");
        }
    }

    #[test]
    fn a_word_is_found_whatever_its_form() {
        assert_eq!(lookup("g01").unwrap().names, ["G1"]);
        assert_eq!(lookup(" G1.0 ").unwrap().names, ["G1"]);
        assert_eq!(lookup("G038.2").unwrap().names[0], "G38.2");
        assert_eq!(lookup("G3").unwrap().names, ["G2", "G3"]);
        assert_eq!(lookup("F300").unwrap().names, ["F"]);
        assert_eq!(lookup("s").unwrap().names, ["S"]);
        assert_eq!(lookup("T12").unwrap().names, ["T"]);
        assert_eq!(lookup("M08").unwrap().names, ["M7", "M8"]);
        // what is no word, no code or no entry
        for bad in [
            "", "G", "X10", "G999", "G-1", "G1.25", "Fxyz", "1G", "hello", "M99",
        ] {
            assert!(lookup(bad).is_none(), "{bad}");
        }
        assert_eq!(name_of("G38.2").as_deref(), Some("G38.2"));
        assert_eq!(name_of("D3").as_deref(), Some("D"));
    }

    #[test]
    fn the_help_of_a_word_gives_its_syntax_and_its_text_in_the_language() {
        let en = describe("g83", "en").unwrap();
        assert!(
            en.starts_with("G83: G83 X Y Z R Q F\n  Deep-hole peck drilling"),
            "{en}"
        );
        let es = describe("G83", "es").unwrap();
        assert!(es.contains("Taladrado profundo"), "{es}");
        let pt = describe("G4", "pt-PT").unwrap();
        assert!(pt.contains("Pausa"), "{pt}");
        // an unknown language reads English
        assert_eq!(describe("G4", "xx"), describe("G4", "en"));
        assert!(describe("X5", "en").is_none());
    }

    #[test]
    fn a_line_gives_one_entry_for_each_word_it_has_help_for() {
        let l = for_line("G1 X10 Y5 F300 (move)", "en");
        assert_eq!(l.len(), 2, "{l:?}");
        assert!(l[0].starts_with("G1:") && l[1].starts_with("F:"));
        // a word twice is said once, and the second G of a pair names its own code
        let two = for_line("G2 X1 Y1 I1 J0 F5 G3", "en");
        assert_eq!(two.iter().filter(|t| t.starts_with("G2")).count(), 1);
        assert_eq!(two.iter().filter(|t| t.starts_with("G3")).count(), 1);
        assert!(for_line("X1 Y2", "en").is_empty());
        assert!(for_line("; G1 X1", "en").is_empty());
        assert_eq!(for_line("T3 M6", "en").len(), 2);
    }

    #[test]
    fn the_answer_of_the_command_line_takes_a_word_or_a_line() {
        let one = answer("G81", "en").unwrap();
        assert!(one.starts_with("G81: ") && one.ends_with('\n'));
        let many = answer("M3 S9000", "en").unwrap();
        assert!(many.contains("M3:") && many.contains("S:"));
        assert!(answer("nothing", "en").is_none());
        assert!(answer("X1 Y2", "en").is_none());
        assert!(answer("  ", "en").is_none());
    }
}
