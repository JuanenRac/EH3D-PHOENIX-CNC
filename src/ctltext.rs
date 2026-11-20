//! The words of the controller layer in the eight languages of the suite (English, Spanish, German, French, Italian, Japanese, Chinese
//! and Portuguese of Portugal): the alarms and the errors of GRBL and grblHAL by number, and the messages of the guard that keeps the
//! machine safe when a controller falls silent. A number that is not in the table gives `None`, and the program shows the text of the
//! controller as it came.

/// The language codes, in the order of the columns of the tables.
pub const LANGS: [&str; 8] = ["en", "es", "de", "fr", "it", "ja", "zh", "pt"];

fn column(lang: &str) -> usize {
    LANGS.iter().position(|l| *l == lang).unwrap_or(0)
}

/// The text of `ALARM:N` in the language (English for a language that is not one of the eight); `None` for a number not in the table.
pub fn alarm(code: u32, lang: &str) -> Option<&'static str> {
    let row: [&str; 8] = match code {
        1 => [
            "A hard limit was hit. The position is probably lost: home the machine again.",
            "Se activó un límite duro. La posición probablemente se perdió: vuelva a hacer el home.",
            "Ein Hardware-Endschalter wurde ausgelöst. Die Position ist wahrscheinlich verloren: Referenzfahrt wiederholen.",
            "Une limite matérielle a été atteinte. La position est probablement perdue : refaites la prise d'origine.",
            "Si è attivato un limite fisico. La posizione è probabilmente persa: rifare l'homing.",
            "ハードリミットが作動しました。位置は失われた可能性があります。原点復帰をやり直してください。",
            "触发了硬限位。位置可能已丢失：请重新回零。",
            "Um limite físico foi acionado. A posição provavelmente perdeu-se: volte a fazer o homing.",
        ],
        2 => [
            "The move goes past the travel of the machine. The position is kept; the alarm can be cleared.",
            "El movimiento sale del recorrido de la máquina. La posición se conserva; se puede quitar la alarma.",
            "Die Bewegung überschreitet den Verfahrweg der Maschine. Die Position bleibt erhalten; der Alarm kann gelöscht werden.",
            "Le mouvement dépasse la course de la machine. La position est conservée ; l'alarme peut être effacée.",
            "Il movimento supera la corsa della macchina. La posizione è conservata; l'allarme si può cancellare.",
            "移動が機械の可動範囲を超えています。位置は保持されており、アラームは解除できます。",
            "运动超出机床行程。位置已保留，可以清除报警。",
            "O movimento ultrapassa o curso da máquina. A posição mantém-se; o alarme pode ser limpo.",
        ],
        3 => [
            "Reset while moving. The position cannot be trusted: home the machine again.",
            "Reinicio en pleno movimiento. No se puede fiar de la posición: vuelva a hacer el home.",
            "Zurücksetzen während der Bewegung. Der Position ist nicht zu trauen: Referenzfahrt wiederholen.",
            "Réinitialisation en plein mouvement. La position n'est plus fiable : refaites la prise d'origine.",
            "Reset durante il movimento. La posizione non è affidabile: rifare l'homing.",
            "移動中にリセットされました。位置は信頼できません。原点復帰をやり直してください。",
            "运动中复位。位置不可信：请重新回零。",
            "Reinício durante o movimento. Não se pode confiar na posição: volte a fazer o homing.",
        ],
        4 => [
            "Probe failed: the probe was not in its expected state before the cycle.",
            "Fallo de sonda: la sonda no estaba en su estado esperado antes del ciclo.",
            "Tastfehler: Der Taster war vor dem Zyklus nicht im erwarteten Zustand.",
            "Échec de la sonde : elle n'était pas dans l'état attendu avant le cycle.",
            "Sonda in errore: non era nello stato previsto prima del ciclo.",
            "プローブ失敗：サイクル開始前のプローブの状態が想定と異なります。",
            "探针失败：循环开始前探针不在预期状态。",
            "Falha da sonda: a sonda não estava no estado esperado antes do ciclo.",
        ],
        5 => [
            "Probe failed: it touched nothing within the travel asked for.",
            "Fallo de sonda: no tocó nada en el recorrido pedido.",
            "Tastfehler: Innerhalb des geforderten Wegs wurde nichts berührt.",
            "Échec de la sonde : elle n'a rien touché sur la course demandée.",
            "Sonda in errore: non ha toccato nulla nella corsa richiesta.",
            "プローブ失敗：指定した移動量の間に何も接触しませんでした。",
            "探针失败：在要求的行程内没有接触到任何东西。",
            "Falha da sonda: não tocou em nada no curso pedido.",
        ],
        6 => [
            "Homing failed: reset during the cycle.",
            "Fallo de homing: reinicio durante el ciclo.",
            "Referenzfahrt fehlgeschlagen: Zurücksetzen während des Zyklus.",
            "Échec de la prise d'origine : réinitialisation pendant le cycle.",
            "Homing fallito: reset durante il ciclo.",
            "原点復帰失敗：サイクル中にリセットされました。",
            "回零失败：循环中被复位。",
            "Falha do homing: reinício durante o ciclo.",
        ],
        7 => [
            "Homing failed: the door was opened during the cycle.",
            "Fallo de homing: se abrió la puerta durante el ciclo.",
            "Referenzfahrt fehlgeschlagen: Die Tür wurde während des Zyklus geöffnet.",
            "Échec de la prise d'origine : la porte a été ouverte pendant le cycle.",
            "Homing fallito: lo sportello è stato aperto durante il ciclo.",
            "原点復帰失敗：サイクル中にドアが開きました。",
            "回零失败：循环中门被打开。",
            "Falha do homing: a porta foi aberta durante o ciclo.",
        ],
        8 => [
            "Homing failed: the limit switch was not cleared when pulling off. Raise the pull-off or check the wiring.",
            "Fallo de homing: no se liberó el final de carrera al retirarse. Aumente el retroceso o revise el cableado.",
            "Referenzfahrt fehlgeschlagen: Der Endschalter wurde beim Freifahren nicht freigegeben. Freifahrweg erhöhen oder Verkabelung prüfen.",
            "Échec de la prise d'origine : le fin de course n'a pas été libéré au dégagement. Augmentez le dégagement ou vérifiez le câblage.",
            "Homing fallito: il finecorsa non è stato liberato nel ritiro. Aumentare il ritiro o controllare il cablaggio.",
            "原点復帰失敗：引き戻し時にリミットスイッチが解除されませんでした。引き戻し量を増やすか配線を確認してください。",
            "回零失败：退让时未能释放限位开关。请增大退让距离或检查接线。",
            "Falha do homing: o fim de curso não foi libertado ao recuar. Aumente o recuo ou verifique a cablagem.",
        ],
        9 => [
            "Homing failed: no limit switch was found within the search distance.",
            "Fallo de homing: no se encontró el final de carrera en la distancia de búsqueda.",
            "Referenzfahrt fehlgeschlagen: Innerhalb des Suchwegs wurde kein Endschalter gefunden.",
            "Échec de la prise d'origine : aucun fin de course trouvé sur la distance de recherche.",
            "Homing fallito: nessun finecorsa trovato nella distanza di ricerca.",
            "原点復帰失敗：探索距離内にリミットスイッチが見つかりませんでした。",
            "回零失败：在搜索距离内未找到限位开关。",
            "Falha do homing: não se encontrou o fim de curso na distância de procura.",
        ],
        _ => return None,
    };
    Some(row[column(lang)])
}

/// The text of `error:N` in the language; `None` for a number not in the table.
pub fn error(code: u32, lang: &str) -> Option<&'static str> {
    let row: [&str; 8] = match code {
        1 => [
            "A G-code word has no letter.",
            "Una palabra de G-code no tiene letra.",
            "Einem G-Code-Wort fehlt der Buchstabe.",
            "Un mot de G-code n'a pas de lettre.",
            "Una parola di G-code non ha la lettera.",
            "Gコードのワードに文字がありません。",
            "G 代码字缺少字母。",
            "Uma palavra de G-code não tem letra.",
        ],
        2 => [
            "A number is not valid or is missing.",
            "Un número no es válido o falta.",
            "Eine Zahl ist ungültig oder fehlt.",
            "Un nombre n'est pas valide ou manque.",
            "Un numero non è valido o manca.",
            "数値が無効または不足しています。",
            "数字无效或缺失。",
            "Um número não é válido ou falta.",
        ],
        3 => [
            "The '$' command is not known or not supported.",
            "El comando '$' no se conoce o no se admite.",
            "Der '$'-Befehl ist unbekannt oder wird nicht unterstützt.",
            "La commande '$' est inconnue ou non prise en charge.",
            "Il comando '$' non è riconosciuto o non è supportato.",
            "'$' コマンドが未知か未対応です。",
            "'$' 命令未知或不受支持。",
            "O comando '$' não é conhecido ou não é suportado.",
        ],
        5 => [
            "Homing is not enabled in the settings.",
            "El homing no está activado en los ajustes.",
            "Die Referenzfahrt ist in den Einstellungen nicht aktiviert.",
            "La prise d'origine n'est pas activée dans les réglages.",
            "L'homing non è abilitato nelle impostazioni.",
            "設定で原点復帰が有効になっていません。",
            "设置中未启用回零。",
            "O homing não está ativado nas definições.",
        ],
        8 => [
            "The '$' command only works while the controller is idle.",
            "El comando '$' solo funciona con el controlador en reposo.",
            "Der '$'-Befehl funktioniert nur im Leerlauf der Steuerung.",
            "La commande '$' ne marche que lorsque le contrôleur est au repos.",
            "Il comando '$' funziona solo a controllore fermo.",
            "'$' コマンドはコントローラーが待機中のときだけ使えます。",
            "'$' 命令仅在控制器空闲时可用。",
            "O comando '$' só funciona com o controlador em repouso.",
        ],
        9 => [
            "G-code is locked while the controller is in an alarm or a jog.",
            "El G-code está bloqueado con el controlador en alarma o en jog.",
            "G-Code ist gesperrt, solange die Steuerung im Alarm oder im Tippbetrieb ist.",
            "Le G-code est verrouillé tant que le contrôleur est en alarme ou en jog.",
            "Il G-code è bloccato mentre il controllore è in allarme o in jog.",
            "アラーム中またはジョグ中はGコードが使えません。",
            "控制器报警或点动期间 G 代码被锁定。",
            "O G-code está bloqueado enquanto o controlador está em alarme ou em jog.",
        ],
        10 => [
            "Soft limits cannot be on without homing.",
            "Los límites blandos no pueden estar activos sin homing.",
            "Software-Endschalter sind ohne Referenzfahrt nicht möglich.",
            "Les limites logicielles ne peuvent pas être actives sans prise d'origine.",
            "I limiti software non possono essere attivi senza homing.",
            "原点復帰なしではソフトリミットを有効にできません。",
            "没有回零时不能启用软限位。",
            "Os limites por software não podem estar ativos sem homing.",
        ],
        11 => [
            "The line is too long and was not run.",
            "La línea es demasiado larga y no se ejecutó.",
            "Die Zeile ist zu lang und wurde nicht ausgeführt.",
            "La ligne est trop longue et n'a pas été exécutée.",
            "La riga è troppo lunga e non è stata eseguita.",
            "行が長すぎるため実行されませんでした。",
            "该行过长，未执行。",
            "A linha é demasiado longa e não foi executada.",
        ],
        15 => [
            "The jog goes past the travel of the machine and was ignored.",
            "El jog sale del recorrido de la máquina y se ignoró.",
            "Der Tippbetrieb überschreitet den Verfahrweg und wurde ignoriert.",
            "Le jog dépasse la course de la machine et a été ignoré.",
            "Il jog supera la corsa della macchina ed è stato ignorato.",
            "ジョグが機械の可動範囲を超えるため無視されました。",
            "点动超出机床行程，已忽略。",
            "O jog ultrapassa o curso da máquina e foi ignorado.",
        ],
        20 => [
            "The line has a G-code command that is not supported or not valid.",
            "La línea tiene un comando G-code no admitido o no válido.",
            "Die Zeile enthält einen nicht unterstützten oder ungültigen G-Code-Befehl.",
            "La ligne contient une commande G-code non prise en charge ou non valide.",
            "La riga contiene un comando G-code non supportato o non valido.",
            "この行には未対応または無効なGコードがあります。",
            "该行含有不支持或无效的 G 代码命令。",
            "A linha tem um comando G-code não suportado ou inválido.",
        ],
        21 => [
            "The line has two commands of the same modal group.",
            "La línea tiene dos comandos del mismo grupo modal.",
            "Die Zeile enthält zwei Befehle derselben Modalgruppe.",
            "La ligne contient deux commandes du même groupe modal.",
            "La riga contiene due comandi dello stesso gruppo modale.",
            "この行には同じモーダルグループのコマンドが2つあります。",
            "该行含有同一模态组的两个命令。",
            "A linha tem dois comandos do mesmo grupo modal.",
        ],
        22 => [
            "No feed has been set yet.",
            "Todavía no se ha fijado ningún avance.",
            "Es wurde noch kein Vorschub gesetzt.",
            "Aucune avance n'a encore été définie.",
            "Non è ancora stato impostato alcun avanzamento.",
            "送り速度がまだ設定されていません。",
            "尚未设定进给速度。",
            "Ainda não foi definido nenhum avanço.",
        ],
        23 => [
            "A command needs a whole number.",
            "Un comando necesita un número entero.",
            "Ein Befehl benötigt eine ganze Zahl.",
            "Une commande demande un nombre entier.",
            "Un comando richiede un numero intero.",
            "コマンドには整数が必要です。",
            "命令需要整数。",
            "Um comando precisa de um número inteiro.",
        ],
        24 => [
            "Two commands in the line both need the axis words.",
            "Dos comandos de la línea necesitan ambos las palabras de eje.",
            "Zwei Befehle in der Zeile benötigen beide die Achsenwörter.",
            "Deux commandes de la ligne demandent toutes deux les mots d'axe.",
            "Due comandi nella riga richiedono entrambi le parole degli assi.",
            "この行の2つのコマンドがどちらも軸ワードを必要としています。",
            "该行中有两个命令都需要轴字。",
            "Dois comandos da linha precisam ambos das palavras de eixo.",
        ],
        33 => [
            "The target of the move is not valid for this command.",
            "El destino del movimiento no es válido para este comando.",
            "Das Bewegungsziel ist für diesen Befehl ungültig.",
            "La cible du mouvement n'est pas valide pour cette commande.",
            "La destinazione del movimento non è valida per questo comando.",
            "このコマンドでは移動先が無効です。",
            "该命令的运动目标无效。",
            "O destino do movimento não é válido para este comando.",
        ],
        _ => return None,
    };
    Some(row[column(lang)])
}

/// A message of the guard that keeps the machine safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Guard {
    /// A controller that moves did not answer: the machine was stopped and the order was not sent again.
    Silent,
    /// Orders that move are refused until the operator gives the all clear.
    Tripped,
    /// More than one move order was active.
    TwoMoves,
    /// The target of the jog was not renewed in time.
    StaleJog,
    /// The controller refused a line of the program; the line number and the reason follow.
    LineRefused,
    /// The controller said something that cannot be believed (a position that jumped).
    Implausible,
}

impl Guard {
    pub const ALL: [Guard; 6] = [
        Guard::Silent,
        Guard::Tripped,
        Guard::TwoMoves,
        Guard::StaleJog,
        Guard::LineRefused,
        Guard::Implausible,
    ];

    /// The words of the message in the language.
    pub fn text(self, lang: &str) -> &'static str {
        let row: [&str; 8] = match self {
            Guard::Silent => [
                "The controller did not answer: the machine was stopped and the order was not sent again.",
                "El controlador no respondió: se paró la máquina y la orden no se reenvió.",
                "Die Steuerung hat nicht geantwortet: Die Maschine wurde angehalten und der Befehl nicht erneut gesendet.",
                "Le contrôleur n'a pas répondu : la machine a été arrêtée et l'ordre n'a pas été renvoyé.",
                "Il controllore non ha risposto: la macchina è stata fermata e l'ordine non è stato rinviato.",
                "コントローラーが応答しませんでした。機械を停止し、命令は再送していません。",
                "控制器没有应答：机床已停止，命令没有重发。",
                "O controlador não respondeu: a máquina foi parada e a ordem não foi reenviada.",
            ],
            Guard::Tripped => [
                "Stopped for safety: orders that move are refused until the operator resets.",
                "Parada de seguridad: las órdenes de movimiento se rechazan hasta que el operador reinicie.",
                "Sicherheitsstopp: Bewegungsbefehle werden abgelehnt, bis der Bediener zurücksetzt.",
                "Arrêt de sécurité : les ordres de mouvement sont refusés tant que l'opérateur n'a pas réarmé.",
                "Arresto di sicurezza: gli ordini di movimento sono rifiutati finché l'operatore non ripristina.",
                "安全停止：オペレーターがリセットするまで移動命令は拒否されます。",
                "安全停止：在操作员复位之前，拒绝一切运动命令。",
                "Paragem de segurança: as ordens de movimento são recusadas até o operador repor.",
            ],
            Guard::TwoMoves => [
                "More than one move order is active: stopped.",
                "Hay más de una orden de movimiento activa: parada.",
                "Mehr als ein Bewegungsbefehl ist aktiv: angehalten.",
                "Plus d'un ordre de mouvement est actif : arrêt.",
                "Più di un ordine di movimento è attivo: fermata.",
                "移動命令が複数有効です。停止しました。",
                "有多个运动命令同时生效：已停止。",
                "Há mais do que uma ordem de movimento ativa: paragem.",
            ],
            Guard::StaleJog => [
                "The jog target was not renewed in time: the machine stands still.",
                "El objetivo del jog no se renovó a tiempo: la máquina se queda quieta.",
                "Das Tippziel wurde nicht rechtzeitig erneuert: Die Maschine steht still.",
                "La cible du jog n'a pas été renouvelée à temps : la machine reste immobile.",
                "L'obiettivo del jog non è stato rinnovato in tempo: la macchina resta ferma.",
                "ジョグの目標が時間内に更新されませんでした。機械は停止しています。",
                "点动目标未及时更新：机床保持静止。",
                "O alvo do jog não foi renovado a tempo: a máquina fica parada.",
            ],
            Guard::LineRefused => [
                "The controller refused a line of the program.",
                "El controlador rechazó una línea del programa.",
                "Die Steuerung hat eine Programmzeile abgelehnt.",
                "Le contrôleur a refusé une ligne du programme.",
                "Il controllore ha rifiutato una riga del programma.",
                "コントローラーがプログラムの1行を拒否しました。",
                "控制器拒绝了程序的一行。",
                "O controlador recusou uma linha do programa.",
            ],
            Guard::Implausible => [
                "The position the controller gave cannot be believed: the machine was stopped.",
                "La posición que dio el controlador no es creíble: se paró la máquina.",
                "Die gemeldete Position ist nicht glaubwürdig: Die Maschine wurde angehalten.",
                "La position donnée par le contrôleur n'est pas crédible : la machine a été arrêtée.",
                "La posizione data dal controllore non è credibile: la macchina è stata fermata.",
                "コントローラーが示した位置は信用できません。機械を停止しました。",
                "控制器给出的位置不可信：机床已停止。",
                "A posição dada pelo controlador não é credível: a máquina foi parada.",
            ],
        };
        row[column(lang)]
    }
}

/// The text of a refusal or an alarm of a controller in the language: the table of the numbers when it has the number, and the text the
/// controller gave when it has not (`kind` is `alarm` or `error`).
pub fn explain(kind: &str, code: Option<u32>, given: &str, lang: &str) -> String {
    let known = code.and_then(|c| match kind {
        "alarm" => alarm(c, lang),
        _ => error(c, lang),
    });
    match known {
        Some(t) => t.to_string(),
        None => given.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_alarm_and_every_error_of_the_table_has_all_eight_languages_and_they_differ() {
        for (what, codes) in [
            ("alarm", (1..=9).collect::<Vec<u32>>()),
            (
                "error",
                vec![1, 2, 3, 5, 8, 9, 10, 11, 15, 20, 21, 22, 23, 24, 33],
            ),
        ] {
            for c in codes {
                let mut seen = Vec::new();
                for l in LANGS {
                    let t = if what == "alarm" {
                        alarm(c, l)
                    } else {
                        error(c, l)
                    };
                    let t = t.unwrap_or_else(|| panic!("{what} {c} has no {l}"));
                    assert!(t.len() > 8, "{what} {c} {l}");
                    seen.push(t);
                }
                let mut un = seen.clone();
                un.sort_unstable();
                un.dedup();
                assert_eq!(un.len(), 8, "{what} {c}: two languages say the same");
            }
        }
        assert!(alarm(0, "en").is_none() && error(4, "en").is_none());
    }

    #[test]
    fn the_guard_messages_exist_in_every_language_and_the_unknown_language_is_english() {
        for g in Guard::ALL {
            let mut all: Vec<&str> = LANGS.iter().map(|l| g.text(l)).collect();
            assert!(all.iter().all(|t| t.len() > 8));
            all.sort_unstable();
            all.dedup();
            assert_eq!(all.len(), 8, "{g:?}");
            assert_eq!(g.text("xx"), g.text("en"));
        }
    }

    #[test]
    fn the_portuguese_is_that_of_portugal() {
        let pt = [alarm(1, "pt"), alarm(8, "pt"), error(5, "pt")];
        let text = pt.iter().flatten().copied().collect::<Vec<_>>().join(" ");
        assert!(text.contains("cablagem") && text.contains("definições"));
        assert!(!text.contains("fiação") && !text.contains("configurações"));
    }

    #[test]
    fn a_number_that_is_not_in_the_table_keeps_the_words_of_the_controller() {
        assert_eq!(
            explain("error", Some(999), "error:999 odd", "es"),
            "error:999 odd"
        );
        assert_eq!(explain("alarm", None, "ALARM", "de"), "ALARM");
        assert_eq!(
            explain("error", Some(22), "x", "fr"),
            "Aucune avance n'a encore été définie."
        );
    }
}
