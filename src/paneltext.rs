//! The words of the panels and of the entries of the menus that the docking window adds to the ones the program already had, in the eight
//! languages of the suite: English, Spanish, Italian, German, French, Portuguese of Portugal, Chinese and Japanese (the order of the text of the
//! frame). A word that has a hole is written `{0}` and filled by [`fill`].

use eh3d_dock::texts::Text8;

/// Every word of this part of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pt {
    // the titles of the panels
    PanelProgram,
    PanelMachineState,
    PanelToolSetter,
    PanelProbe,
    PanelImport,
    PanelPlan,
    // the entries of the menus
    MenuImport,
    MenuLookAhead,
    MenuPost,
    MenuToolSetter,
    MenuResetGuard,
    MenuProbeZ,
    MenuProbeGrid,
    MenuUseMap,
    MenuDropMap,
    // the panel of planning and time
    PlanCalculate,
    PlanStale,
    PlanEmpty,
    PlanSaved,
    PlanSavedNothing,
    // the panel of import
    ImportFile,
    ImportRead,
    ImportUse,
    ImportPost,
    ImportNone,
    ImportSize,
    ImportNeedsFile,
    ImportCfgNote,
    ImportUsed,
    ImportLayer,
    ImportLayerAuto,
    ImportLayerIsolation,
    ImportLayerProfile,
    ImportLayerDrill,
    ImportMirror,
    ImportBoard,
    // the panel of the tool setter
    SetterTool,
    SetterMeasure,
    SetterConfirm,
    SetterLength,
    SetterWrite,
    SetterDrop,
    SetterWritten,
    SetterNoTool,
    SetterNoTable,
    SetterDisabled,
    // the table of tools
    ToolsEmpty,
    ToolsReload,
    // the guard
    GuardWatching,
    GuardOff,
    GuardStopped,
    GuardReset,
    // the panel of the probe and the height map
    ProbeMap,
    ProbeNoMap,
    ProbeCorrected,
}

/// All of them, for the tests and for anything that has to go through them.
pub const ALL: [Pt; 55] = [
    Pt::PanelProgram,
    Pt::PanelMachineState,
    Pt::PanelToolSetter,
    Pt::PanelProbe,
    Pt::PanelImport,
    Pt::PanelPlan,
    Pt::MenuImport,
    Pt::MenuLookAhead,
    Pt::MenuPost,
    Pt::MenuToolSetter,
    Pt::MenuResetGuard,
    Pt::MenuProbeZ,
    Pt::MenuProbeGrid,
    Pt::MenuUseMap,
    Pt::MenuDropMap,
    Pt::PlanCalculate,
    Pt::PlanStale,
    Pt::PlanEmpty,
    Pt::PlanSaved,
    Pt::PlanSavedNothing,
    Pt::ImportFile,
    Pt::ImportRead,
    Pt::ImportUse,
    Pt::ImportPost,
    Pt::ImportNone,
    Pt::ImportSize,
    Pt::ImportNeedsFile,
    Pt::ImportCfgNote,
    Pt::ImportUsed,
    Pt::ImportLayer,
    Pt::ImportLayerAuto,
    Pt::ImportLayerIsolation,
    Pt::ImportLayerProfile,
    Pt::ImportLayerDrill,
    Pt::ImportMirror,
    Pt::ImportBoard,
    Pt::SetterTool,
    Pt::SetterMeasure,
    Pt::SetterConfirm,
    Pt::SetterLength,
    Pt::SetterWrite,
    Pt::SetterDrop,
    Pt::SetterWritten,
    Pt::SetterNoTool,
    Pt::SetterNoTable,
    Pt::SetterDisabled,
    Pt::ToolsEmpty,
    Pt::ToolsReload,
    Pt::GuardWatching,
    Pt::GuardOff,
    Pt::GuardStopped,
    Pt::GuardReset,
    Pt::ProbeMap,
    Pt::ProbeNoMap,
    Pt::ProbeCorrected,
];

/// The word in the eight languages, in the order of the frame (en, es, it, de, fr, pt, zh, ja).
pub fn text8(w: Pt) -> Text8 {
    Text8(match w {
        Pt::PanelProgram => [
            "Program",
            "Programa",
            "Programma",
            "Programm",
            "Programme",
            "Programa",
            "程序",
            "プログラム",
        ],
        Pt::PanelMachineState => [
            "Machine state",
            "Estado de la máquina",
            "Stato della macchina",
            "Maschinenzustand",
            "État de la machine",
            "Estado da máquina",
            "机床状态",
            "機械の状態",
        ],
        Pt::PanelToolSetter => [
            "Tool setter",
            "Sensor de herramienta",
            "Sensore utensile",
            "Werkzeugsensor",
            "Capteur d'outil",
            "Sensor de ferramenta",
            "对刀仪",
            "ツールセッター",
        ],
        Pt::PanelProbe => [
            "Probe and height map",
            "Sonda y mapa de alturas",
            "Sonda e mappa delle altezze",
            "Taster und Höhenkarte",
            "Sonde et carte des hauteurs",
            "Sonda e mapa de alturas",
            "探针与高度图",
            "プローブと高さマップ",
        ],
        Pt::PanelImport => [
            "Import",
            "Importar",
            "Importa",
            "Importieren",
            "Importer",
            "Importar",
            "导入",
            "インポート",
        ],
        Pt::PanelPlan => [
            "Planning and time",
            "Planificación y tiempo",
            "Pianificazione e tempo",
            "Planung und Zeit",
            "Planification et temps",
            "Planeamento e tempo",
            "规划与时间",
            "計画と時間",
        ],
        Pt::MenuImport => [
            "Import a drawing (HPGL, DXF, SVG)...",
            "Importar un dibujo (HPGL, DXF, SVG)...",
            "Importa un disegno (HPGL, DXF, SVG)...",
            "Zeichnung importieren (HPGL, DXF, SVG)...",
            "Importer un dessin (HPGL, DXF, SVG)...",
            "Importar um desenho (HPGL, DXF, SVG)...",
            "导入图纸（HPGL、DXF、SVG）...",
            "図面をインポート（HPGL、DXF、SVG）...",
        ],
        Pt::MenuLookAhead => [
            "Time with and without look-ahead",
            "Tiempo con y sin anticipación",
            "Tempo con e senza anticipo",
            "Zeit mit und ohne Vorausschau",
            "Temps avec et sans anticipation",
            "Tempo com e sem antecipação",
            "有无前瞻的时间",
            "先読みあり・なしの時間",
        ],
        Pt::MenuPost => [
            "Post-processor...",
            "Postprocesador...",
            "Post-processore...",
            "Postprozessor...",
            "Post-processeur...",
            "Pós-processador...",
            "后处理器...",
            "ポストプロセッサ...",
        ],
        Pt::MenuToolSetter => [
            "Measure the tool on the setter...",
            "Medir la herramienta en el sensor...",
            "Misura l'utensile sul sensore...",
            "Werkzeug am Sensor messen...",
            "Mesurer l'outil sur le capteur...",
            "Medir a ferramenta no sensor...",
            "在对刀仪上测量刀具...",
            "ツールセッターで工具を測定...",
        ],
        Pt::MenuResetGuard => [
            "Give the all clear to the guard",
            "Dar el visto bueno a la guardia",
            "Dare il via libera alla guardia",
            "Wächter freigeben",
            "Donner le feu vert à la garde",
            "Dar luz verde à guarda",
            "解除保护停机",
            "ガードを解除",
        ],
        Pt::MenuProbeZ => [
            "Probe Z (touch plate)...",
            "Sondear Z (placa de contacto)...",
            "Tastare Z (piastra di contatto)...",
            "Z antasten (Tastplatte)...",
            "Palper Z (plaque de contact)...",
            "Sondar Z (placa de contacto)...",
            "探测 Z（对刀块）...",
            "Z プローブ（接触プレート）...",
        ],
        Pt::MenuProbeGrid => [
            "Probe a grid (height map)...",
            "Sondear una rejilla (mapa de alturas)...",
            "Tastare una griglia (mappa delle altezze)...",
            "Raster antasten (Höhenkarte)...",
            "Palper une grille (carte des hauteurs)...",
            "Sondar uma grelha (mapa de alturas)...",
            "探测网格（高度图）...",
            "グリッドをプローブ（高さマップ）...",
        ],
        Pt::MenuUseMap => [
            "Correct the program with the height map",
            "Corregir el programa con el mapa de alturas",
            "Correggere il programma con la mappa delle altezze",
            "Programm mit der Höhenkarte korrigieren",
            "Corriger le programme avec la carte des hauteurs",
            "Corrigir o programa com o mapa de alturas",
            "用高度图校正程序",
            "高さマップでプログラムを補正",
        ],
        Pt::MenuDropMap => [
            "Undo the height correction",
            "Deshacer la corrección de altura",
            "Annullare la correzione di altezza",
            "Höhenkorrektur rückgängig machen",
            "Annuler la correction de hauteur",
            "Anular a correção de altura",
            "撤销高度校正",
            "高さ補正を元に戻す",
        ],
        Pt::PlanCalculate => [
            "Calculate the time",
            "Calcular el tiempo",
            "Calcola il tempo",
            "Zeit berechnen",
            "Calculer le temps",
            "Calcular o tempo",
            "计算时间",
            "時間を計算",
        ],
        Pt::PlanStale => [
            "The program changed since this was calculated: calculate again.",
            "El programa cambió desde que se calculó: calcule de nuevo.",
            "Il programma è cambiato dopo il calcolo: calcola di nuovo.",
            "Das Programm hat sich seit der Berechnung geändert: neu berechnen.",
            "Le programme a changé depuis le calcul : calculez de nouveau.",
            "O programa mudou desde o cálculo: calcule de novo.",
            "程序在计算后已更改：请重新计算。",
            "計算後にプログラムが変わりました。もう一度計算してください。",
        ],
        Pt::PlanEmpty => [
            "Load a program first.",
            "Cargue primero un programa.",
            "Caricare prima un programma.",
            "Laden Sie zuerst ein Programm.",
            "Chargez d'abord un programme.",
            "Carregue primeiro um programa.",
            "请先载入程序。",
            "先にプログラムを読み込んでください。",
        ],
        Pt::PlanSaved => [
            "Saved by the look-ahead: {0} s ({1} %)",
            "Ahorrado por la anticipación: {0} s ({1} %)",
            "Risparmiato dall'anticipo: {0} s ({1} %)",
            "Durch die Vorausschau gespart: {0} s ({1} %)",
            "Gagné par l'anticipation : {0} s ({1} %)",
            "Poupado pela antecipação: {0} s ({1} %)",
            "前瞻节省：{0} 秒（{1} %）",
            "先読みによる短縮：{0} 秒（{1} %）",
        ],
        Pt::PlanSavedNothing => [
            "The look-ahead saves no time on this program.",
            "La anticipación no ahorra tiempo en este programa.",
            "L'anticipo non fa risparmiare tempo su questo programma.",
            "Die Vorausschau spart bei diesem Programm keine Zeit.",
            "L'anticipation ne fait gagner aucun temps sur ce programme.",
            "A antecipação não poupa tempo neste programa.",
            "前瞻对此程序没有节省时间。",
            "このプログラムでは先読みによる短縮はありません。",
        ],
        Pt::ImportFile => [
            "Drawing file (HPGL, DXF or SVG)",
            "Archivo de dibujo (HPGL, DXF o SVG)",
            "File di disegno (HPGL, DXF o SVG)",
            "Zeichnungsdatei (HPGL, DXF oder SVG)",
            "Fichier de dessin (HPGL, DXF ou SVG)",
            "Ficheiro de desenho (HPGL, DXF ou SVG)",
            "图纸文件（HPGL、DXF 或 SVG）",
            "図面ファイル（HPGL、DXF、SVG）",
        ],
        Pt::ImportRead => [
            "Read the drawing",
            "Leer el dibujo",
            "Leggi il disegno",
            "Zeichnung lesen",
            "Lire le dessin",
            "Ler o desenho",
            "读取图纸",
            "図面を読み込む",
        ],
        Pt::ImportUse => [
            "Use as the program",
            "Usar como programa",
            "Usa come programma",
            "Als Programm verwenden",
            "Utiliser comme programme",
            "Usar como programa",
            "用作程序",
            "プログラムとして使う",
        ],
        Pt::ImportPost => [
            "Post-processor (name or path, optional)",
            "Postprocesador (nombre o ruta, opcional)",
            "Post-processore (nome o percorso, facoltativo)",
            "Postprozessor (Name oder Pfad, optional)",
            "Post-processeur (nom ou chemin, facultatif)",
            "Pós-processador (nome ou caminho, opcional)",
            "后处理器（名称或路径，可选）",
            "ポストプロセッサ（名前またはパス、省略可）",
        ],
        Pt::ImportNone => [
            "No drawing has been read yet.",
            "Aún no se ha leído ningún dibujo.",
            "Nessun disegno è stato ancora letto.",
            "Es wurde noch keine Zeichnung gelesen.",
            "Aucun dessin n'a encore été lu.",
            "Ainda não foi lido nenhum desenho.",
            "尚未读取任何图纸。",
            "まだ図面を読み込んでいません。",
        ],
        Pt::ImportSize => [
            "Size of the drawing: {0} x {1} mm",
            "Tamaño del dibujo: {0} x {1} mm",
            "Dimensione del disegno: {0} x {1} mm",
            "Größe der Zeichnung: {0} x {1} mm",
            "Taille du dessin : {0} x {1} mm",
            "Tamanho do desenho: {0} x {1} mm",
            "图纸尺寸：{0} x {1} mm",
            "図面の大きさ：{0} x {1} mm",
        ],
        Pt::ImportNeedsFile => [
            "Write the path of a file first.",
            "Escriba primero la ruta de un archivo.",
            "Scrivere prima il percorso di un file.",
            "Geben Sie zuerst den Pfad einer Datei ein.",
            "Saisissez d'abord le chemin d'un fichier.",
            "Escreva primeiro o caminho de um ficheiro.",
            "请先输入文件路径。",
            "先にファイルのパスを入力してください。",
        ],
        Pt::ImportCfgNote => [
            "The depth, the passes, the feeds and the limits of size are in the motion file.",
            "La profundidad, las pasadas, los avances y los límites de tamaño están en el fichero de movimiento.",
            "La profondità, le passate, gli avanzamenti e i limiti di dimensione sono nel file di movimento.",
            "Tiefe, Durchgänge, Vorschübe und Größengrenzen stehen in der Bewegungsdatei.",
            "La profondeur, les passes, les avances et les limites de taille sont dans le fichier de mouvement.",
            "A profundidade, as passagens, os avanços e os limites de tamanho estão no ficheiro de movimento.",
            "深度、道次、进给和大小限制都在运动文件中。",
            "深さ、パス、送り、サイズの上限はモーションファイルにあります。",
        ],
        Pt::ImportUsed => [
            "The drawing is now the program: {0} lines.",
            "El dibujo es ahora el programa: {0} líneas.",
            "Il disegno è ora il programma: {0} righe.",
            "Die Zeichnung ist jetzt das Programm: {0} Zeilen.",
            "Le dessin est maintenant le programme : {0} lignes.",
            "O desenho é agora o programa: {0} linhas.",
            "图纸现在是程序：{0} 行。",
            "図面がプログラムになりました：{0} 行。",
        ],
        Pt::ImportLayer => [
            "Board layer",
            "Capa de la placa",
            "Strato della scheda",
            "Platinenlage",
            "Couche de la carte",
            "Camada da placa",
            "电路板图层",
            "基板のレイヤー",
        ],
        Pt::ImportLayerAuto => [
            "What the file says",
            "Lo que dice el archivo",
            "Quello che dice il file",
            "Was die Datei sagt",
            "Ce que dit le fichier",
            "O que diz o ficheiro",
            "按文件内容",
            "ファイルの記載どおり",
        ],
        Pt::ImportLayerIsolation => [
            "Isolation",
            "Aislamiento",
            "Isolamento",
            "Isolationsfräsen",
            "Isolation",
            "Isolamento",
            "隔离铣削",
            "アイソレーション",
        ],
        Pt::ImportLayerProfile => [
            "Profile",
            "Contorno",
            "Contorno del bordo",
            "Kontur",
            "Contour",
            "Contorno da placa",
            "外形",
            "外形加工",
        ],
        Pt::ImportLayerDrill => [
            "Drill",
            "Taladrado",
            "Foratura",
            "Bohren",
            "Perçage",
            "Furação",
            "钻孔",
            "穴あけ",
        ],
        Pt::ImportMirror => [
            "Mirror about the Y axis",
            "Reflejar respecto al eje Y",
            "Specchia rispetto all'asse Y",
            "An der Y-Achse spiegeln",
            "Inverser par rapport à l'axe Y",
            "Espelhar em relação ao eixo Y",
            "沿 Y 轴镜像",
            "Y 軸で反転",
        ],
        Pt::ImportBoard => [
            "Cut as {0}: {1} cuts or holes, {2} drill sizes.",
            "Se corta como {0}: {1} cortes o agujeros, {2} diámetros de broca.",
            "Taglio come {0}: {1} tagli o fori, {2} diametri di punta.",
            "Geschnitten als {0}: {1} Schnitte oder Löcher, {2} Bohrergrößen.",
            "Coupé en {0} : {1} coupes ou trous, {2} diamètres de foret.",
            "Cortado como {0}: {1} cortes ou furos, {2} diâmetros de broca.",
            "按{0}加工：{1} 条切削路径或孔，{2} 种钻头直径。",
            "{0}として加工：切削パスまたは穴 {1} 件、ドリル径 {2} 種類。",
        ],
        Pt::SetterTool => [
            "Tool number",
            "Número de herramienta",
            "Numero utensile",
            "Werkzeugnummer",
            "Numéro d'outil",
            "Número da ferramenta",
            "刀具号",
            "工具番号",
        ],
        Pt::SetterMeasure => [
            "Measure the tool",
            "Medir la herramienta",
            "Misura l'utensile",
            "Werkzeug messen",
            "Mesurer l'outil",
            "Medir a ferramenta",
            "测量刀具",
            "工具を測定",
        ],
        Pt::SetterConfirm => [
            "The setter is there and clear of the tool and the clamps: the machine may go to it.",
            "El sensor está en su sitio y libre de la herramienta y las sujeciones: la máquina puede ir hacia él.",
            "Il sensore è al suo posto e libero da utensile e staffe: la macchina può andarci.",
            "Der Sensor steht bereit, frei von Werkzeug und Spannmitteln: die Maschine darf hinfahren.",
            "Le capteur est en place et dégagé de l'outil et des brides : la machine peut s'y rendre.",
            "O sensor está no sítio e livre da ferramenta e das fixações: a máquina pode ir até ele.",
            "对刀仪已就位，且与刀具和夹具无干涉：机床可以移动过去。",
            "セッターは所定の位置にあり、工具やクランプに干渉しません。機械が移動してよい。",
        ],
        Pt::SetterLength => [
            "Length measured: {0} mm",
            "Longitud medida: {0} mm",
            "Lunghezza misurata: {0} mm",
            "Gemessene Länge: {0} mm",
            "Longueur mesurée : {0} mm",
            "Comprimento medido: {0} mm",
            "测得长度：{0} mm",
            "測定した長さ：{0} mm",
        ],
        Pt::SetterWrite => [
            "Write into the table of tools",
            "Escribir en la tabla de herramientas",
            "Scrivi nella tabella utensili",
            "In die Werkzeugtabelle schreiben",
            "Écrire dans la table d'outils",
            "Escrever na tabela de ferramentas",
            "写入刀具表",
            "工具テーブルに書き込む",
        ],
        Pt::SetterDrop => [
            "Drop the measure",
            "Descartar la medida",
            "Scarta la misura",
            "Messung verwerfen",
            "Abandonner la mesure",
            "Descartar a medida",
            "放弃测量",
            "測定を破棄",
        ],
        Pt::SetterWritten => [
            "Written into the table: tool {0}, length {1} mm.",
            "Escrito en la tabla: herramienta {0}, longitud {1} mm.",
            "Scritto nella tabella: utensile {0}, lunghezza {1} mm.",
            "In die Tabelle geschrieben: Werkzeug {0}, Länge {1} mm.",
            "Écrit dans la table : outil {0}, longueur {1} mm.",
            "Escrito na tabela: ferramenta {0}, comprimento {1} mm.",
            "已写入刀具表：刀具 {0}，长度 {1} mm。",
            "テーブルに書き込みました：工具 {0}、長さ {1} mm。",
        ],
        Pt::SetterNoTool => [
            "The table of tools has no tool with that number.",
            "La tabla de herramientas no tiene ninguna herramienta con ese número.",
            "La tabella utensili non ha nessun utensile con quel numero.",
            "Die Werkzeugtabelle enthält kein Werkzeug mit dieser Nummer.",
            "La table d'outils n'a aucun outil portant ce numéro.",
            "A tabela de ferramentas não tem nenhuma ferramenta com esse número.",
            "刀具表中没有该编号的刀具。",
            "その番号の工具はテーブルにありません。",
        ],
        Pt::SetterNoTable => [
            "The table of tools could not be written: {0}",
            "No se pudo escribir la tabla de herramientas: {0}",
            "Impossibile scrivere la tabella utensili: {0}",
            "Die Werkzeugtabelle konnte nicht geschrieben werden: {0}",
            "La table d'outils n'a pas pu être écrite : {0}",
            "Não foi possível escrever a tabela de ferramentas: {0}",
            "无法写入刀具表：{0}",
            "工具テーブルを書き込めませんでした：{0}",
        ],
        Pt::SetterDisabled => [
            "The numbers of the setter are in the motion file; the machine needs a probe input.",
            "Los valores del sensor están en el fichero de movimiento; la máquina necesita una entrada de sonda.",
            "I valori del sensore sono nel file di movimento; la macchina richiede un ingresso sonda.",
            "Die Werte des Sensors stehen in der Bewegungsdatei; die Maschine braucht einen Tastereingang.",
            "Les valeurs du capteur sont dans le fichier de mouvement ; la machine a besoin d'une entrée de sonde.",
            "Os valores do sensor estão no ficheiro de movimento; a máquina precisa de uma entrada de sonda.",
            "对刀仪的数值在运动文件中；机床需要探针输入。",
            "セッターの値はモーションファイルにあります。機械にはプローブ入力が必要です。",
        ],
        Pt::ToolsEmpty => [
            "The table of tools is empty: write the tools in {0}",
            "La tabla de herramientas está vacía: escriba las herramientas en {0}",
            "La tabella utensili è vuota: scrivere gli utensili in {0}",
            "Die Werkzeugtabelle ist leer: tragen Sie die Werkzeuge in {0} ein",
            "La table d'outils est vide : écrivez les outils dans {0}",
            "A tabela de ferramentas está vazia: escreva as ferramentas em {0}",
            "刀具表为空：请在 {0} 中写入刀具",
            "工具テーブルが空です。{0} に工具を書いてください",
        ],
        Pt::ToolsReload => [
            "Read the table again",
            "Leer la tabla de nuevo",
            "Rileggi la tabella",
            "Tabelle neu lesen",
            "Relire la table",
            "Ler a tabela de novo",
            "重新读取刀具表",
            "テーブルを読み直す",
        ],
        Pt::GuardWatching => [
            "Guard: watching the controller",
            "Guardia: vigilando el controlador",
            "Guardia: controlla il controller",
            "Wächter: überwacht die Steuerung",
            "Garde : surveille le contrôleur",
            "Guarda: a vigiar o controlador",
            "保护：正在监视控制器",
            "ガード：コントローラを監視中",
        ],
        Pt::GuardOff => [
            "Guard: off (guard = off in the motion file)",
            "Guardia: apagada (guard = off en el fichero de movimiento)",
            "Guardia: spenta (guard = off nel file di movimento)",
            "Wächter: aus (guard = off in der Bewegungsdatei)",
            "Garde : éteinte (guard = off dans le fichier de mouvement)",
            "Guarda: desligada (guard = off no ficheiro de movimento)",
            "保护：已关闭（运动文件中 guard = off）",
            "ガード：オフ（モーションファイルで guard = off）",
        ],
        Pt::GuardStopped => [
            "Guard: the machine is stopped",
            "Guardia: la máquina está parada",
            "Guardia: la macchina è ferma",
            "Wächter: die Maschine ist angehalten",
            "Garde : la machine est arrêtée",
            "Guarda: a máquina está parada",
            "保护：机床已停止",
            "ガード：機械は停止しています",
        ],
        Pt::GuardReset => [
            "Give the all clear",
            "Dar el visto bueno",
            "Dare il via libera",
            "Freigeben",
            "Donner le feu vert",
            "Dar luz verde",
            "解除",
            "解除する",
        ],
        Pt::ProbeMap => [
            "Height map: {0} - {1} mm",
            "Mapa de alturas: {0} - {1} mm",
            "Mappa delle altezze: {0} - {1} mm",
            "Höhenkarte: {0} - {1} mm",
            "Carte des hauteurs : {0} - {1} mm",
            "Mapa de alturas: {0} - {1} mm",
            "高度图：{0} - {1} mm",
            "高さマップ：{0} - {1} mm",
        ],
        Pt::ProbeNoMap => [
            "There is no height map: probe a grid first.",
            "No hay mapa de alturas: sonde primero una rejilla.",
            "Non c'è nessuna mappa delle altezze: tastare prima una griglia.",
            "Es gibt keine Höhenkarte: tasten Sie zuerst ein Raster ab.",
            "Il n'y a pas de carte des hauteurs : palpez d'abord une grille.",
            "Não há mapa de alturas: sonde primeiro uma grelha.",
            "没有高度图：请先探测网格。",
            "高さマップがありません。先にグリッドをプローブしてください。",
        ],
        Pt::ProbeCorrected => [
            "The program is corrected with the map.",
            "El programa está corregido con el mapa.",
            "Il programma è corretto con la mappa.",
            "Das Programm ist mit der Karte korrigiert.",
            "Le programme est corrigé avec la carte.",
            "O programa está corrigido com o mapa.",
            "程序已用高度图校正。",
            "プログラムは高さマップで補正されています。",
        ],
    })
}

/// The index of a language code in the order of the frame (English for a code that is not one of the eight).
pub fn index(code: &str) -> usize {
    match code {
        "es" => 1,
        "it" => 2,
        "de" => 3,
        "fr" => 4,
        "pt" => 5,
        "zh" => 6,
        "ja" => 7,
        _ => 0,
    }
}

/// The word in the language of the code.
pub fn text(code: &str, w: Pt) -> &'static str {
    text8(w).0[index(code)]
}

/// The word with its holes `{0}`, `{1}`, ... filled.
pub fn fill<S: AsRef<str>>(code: &str, w: Pt, args: &[S]) -> String {
    let mut s = text(code, w).to_string();
    for (i, a) in args.iter().enumerate() {
        s = s.replace(&format!("{{{i}}}"), a.as_ref());
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_is_in_the_eight_languages_and_none_is_left_empty_or_with_a_hole_open() {
        for w in ALL {
            let t = text8(w);
            for (i, s) in t.0.iter().enumerate() {
                assert!(!s.trim().is_empty(), "{w:?} {i}");
            }
            // the holes of every language are the holes of English
            let holes = |s: &str| (0..4).filter(|n| s.contains(&format!("{{{n}}}"))).count();
            for s in t.0 {
                assert_eq!(holes(s), holes(t.0[0]), "{w:?}: {s}");
            }
        }
    }

    #[test]
    fn the_list_has_every_word_once() {
        let mut seen = std::collections::HashSet::new();
        for w in ALL {
            assert!(seen.insert(w), "{w:?} twice");
        }
        // the number is the number of variants: a variant that is not in the list is not tested
        assert_eq!(ALL.len(), 55);
        assert_eq!(seen.len(), 55);
    }

    #[test]
    fn the_chinese_and_the_japanese_are_never_the_english() {
        for w in ALL {
            let t = text8(w).0;
            assert_ne!(t[6], t[0], "{w:?} zh");
            assert_ne!(t[7], t[0], "{w:?} ja");
        }
    }

    #[test]
    fn portuguese_is_the_one_of_portugal() {
        let pt = |w| text("pt", w);
        assert!(pt(Pt::ImportRead).contains("Ler o desenho"));
        assert!(pt(Pt::PanelPlan).contains("Planeamento"));
        for w in ALL {
            let s = pt(w);
            assert!(!s.contains("arquivo"), "{w:?}: Portugal says ficheiro");
            assert!(!s.contains("tela "), "{w:?}: Portugal says ecrã");
        }
    }

    #[test]
    fn the_holes_are_filled_and_a_language_outside_the_eight_gets_english() {
        assert_eq!(
            fill("es", Pt::SetterWritten, &["3", "21.5"]),
            "Escrito en la tabla: herramienta 3, longitud 21.5 mm."
        );
        assert_eq!(text("xx", Pt::PanelImport), "Import");
        assert_eq!(index("ja"), 7);
        assert_eq!(
            fill("en", Pt::PlanSaved, &["4.0", "12"]),
            "Saved by the look-ahead: 4.0 s (12 %)"
        );
    }
}
