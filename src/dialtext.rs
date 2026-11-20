//! The words of the errors and the alarms that belong to one dialect only, in the eight languages of the suite (English, Spanish, German,
//! French, Italian, Japanese, Chinese and Portuguese of Portugal): the numbers that grblHAL adds to the ones of GRBL, and the kinds of
//! error of Marlin (which says them in words, and which the program numbers from 101). The numbers GRBL shares with grblHAL are in
//! `ctltext`; [`explain`] looks there first. A number that is in no table gives the text of the controller as it came.

use eh3d_controller::Family;

use crate::ctltext::LANGS;

fn column(lang: &str) -> usize {
    LANGS.iter().position(|l| *l == lang).unwrap_or(0)
}

/// The errors (`error:N`) that grblHAL has and GRBL 1.1 does not.
pub const HAL_ERRORS: [u32; 10] = [39, 40, 41, 44, 45, 46, 50, 51, 52, 53];

/// The alarms (`ALARM:N`) that grblHAL has and GRBL 1.1 does not.
pub const HAL_ALARMS: [u32; 6] = [10, 11, 12, 13, 14, 17];

/// The kinds of error of Marlin, with the number the program gives each.
pub const MARLIN_KINDS: [u32; 8] = [101, 102, 103, 104, 105, 106, 107, 108];

/// The text of `error:N` of grblHAL in the language; `None` for a number that is not one of the ones it adds.
pub fn hal_error(code: u32, lang: &str) -> Option<&'static str> {
    let row: [&str; 8] = match code {
        39 => [
            "A value of the line is out of the range the controller takes.",
            "Un valor de la línea está fuera del rango que admite el controlador.",
            "Ein Wert der Zeile liegt außerhalb des Bereichs, den die Steuerung annimmt.",
            "Une valeur de la ligne est hors de la plage acceptée par le contrôleur.",
            "Un valore della riga è fuori dall'intervallo accettato dal controllore.",
            "行内の値がコントローラーの受け付ける範囲外です。",
            "该行的某个值超出了控制器可接受的范围。",
            "Um valor da linha está fora do intervalo que o controlador aceita.",
        ],
        40 => [
            "A tool change is waiting: finish it before the next move.",
            "Hay un cambio de herramienta pendiente: termínelo antes del siguiente movimiento.",
            "Ein Werkzeugwechsel steht aus: Schließen Sie ihn vor der nächsten Bewegung ab.",
            "Un changement d'outil est en attente : terminez-le avant le mouvement suivant.",
            "Un cambio utensile è in attesa: terminarlo prima del movimento successivo.",
            "工具交換が保留中です。次の移動の前に完了してください。",
            "有换刀正在等待：请在下一次运动前完成。",
            "Há uma troca de ferramenta pendente: conclua-a antes do movimento seguinte.",
        ],
        41 => [
            "The spindle is not running.",
            "El husillo no está en marcha.",
            "Die Spindel läuft nicht.",
            "La broche ne tourne pas.",
            "Il mandrino non è in rotazione.",
            "スピンドルが回転していません。",
            "主轴没有运转。",
            "O fuso não está a rodar.",
        ],
        44 => [
            "The spindle speed is out of the range of the machine.",
            "La velocidad del husillo está fuera del rango de la máquina.",
            "Die Spindeldrehzahl liegt außerhalb des Bereichs der Maschine.",
            "La vitesse de la broche est hors de la plage de la machine.",
            "La velocità del mandrino è fuori dall'intervallo della macchina.",
            "スピンドル回転数が機械の範囲外です。",
            "主轴转速超出了机床的范围。",
            "A velocidade do fuso está fora do intervalo da máquina.",
        ],
        45 => [
            "A limit switch is pressed: the move is refused.",
            "Un final de carrera está pulsado: se rechaza el movimiento.",
            "Ein Endschalter ist betätigt: Die Bewegung wird abgelehnt.",
            "Une fin de course est enfoncée : le mouvement est refusé.",
            "Un finecorsa è premuto: il movimento è rifiutato.",
            "リミットスイッチが押されています。移動は拒否されました。",
            "限位开关被触发：运动被拒绝。",
            "Um fim de curso está premido: o movimento é recusado.",
        ],
        46 => [
            "The machine must be homed before it moves.",
            "Hay que hacer el home de la máquina antes de moverla.",
            "Die Maschine muss vor der Bewegung referenziert werden.",
            "La machine doit faire sa prise d'origine avant de bouger.",
            "La macchina deve fare l'homing prima di muoversi.",
            "移動する前に機械の原点復帰が必要です。",
            "机床移动前必须先回零。",
            "A máquina tem de fazer o homing antes de se mover.",
        ],
        50 => [
            "The emergency stop is active.",
            "La parada de emergencia está activa.",
            "Der Not-Halt ist aktiv.",
            "L'arrêt d'urgence est actif.",
            "L'arresto di emergenza è attivo.",
            "非常停止が作動中です。",
            "急停处于激活状态。",
            "A paragem de emergência está ativa.",
        ],
        51 => [
            "A motor driver reports a fault.",
            "Un driver de motor informa de un fallo.",
            "Ein Motortreiber meldet einen Fehler.",
            "Un driver de moteur signale un défaut.",
            "Un driver di motore segnala un guasto.",
            "モータードライバーが異常を報告しています。",
            "某个电机驱动器报告故障。",
            "Um driver de motor comunica uma falha.",
        ],
        52 => [
            "The value of the setting is out of range.",
            "El valor del ajuste está fuera de rango.",
            "Der Wert der Einstellung liegt außerhalb des Bereichs.",
            "La valeur du réglage est hors plage.",
            "Il valore dell'impostazione è fuori intervallo.",
            "設定値が範囲外です。",
            "设置值超出范围。",
            "O valor da definição está fora do intervalo.",
        ],
        53 => [
            "The setting is disabled in this build.",
            "El ajuste está desactivado en esta compilación.",
            "Die Einstellung ist in diesem Build deaktiviert.",
            "Le réglage est désactivé dans cette version.",
            "L'impostazione è disattivata in questa versione.",
            "この版では設定が無効になっています。",
            "该设置在此版本中被禁用。",
            "A definição está desativada nesta versão.",
        ],
        _ => return None,
    };
    Some(row[column(lang)])
}

/// The text of `ALARM:N` of grblHAL in the language; `None` for a number that is not one of the ones it adds.
pub fn hal_alarm(code: u32, lang: &str) -> Option<&'static str> {
    let row: [&str; 8] = match code {
        10 => [
            "The emergency stop was pressed. Release it and unlock the machine.",
            "Se pulsó la parada de emergencia. Suéltela y desbloquee la máquina.",
            "Der Not-Halt wurde gedrückt. Lösen Sie ihn und entsperren Sie die Maschine.",
            "L'arrêt d'urgence a été enfoncé. Relâchez-le et déverrouillez la machine.",
            "È stato premuto l'arresto di emergenza. Rilasciarlo e sbloccare la macchina.",
            "非常停止が押されました。解除して機械のロックを外してください。",
            "急停已被按下。请释放并解锁机床。",
            "A paragem de emergência foi premida. Solte-a e desbloqueie a máquina.",
        ],
        11 => [
            "The machine must be homed before it moves.",
            "Hay que hacer el home de la máquina antes de moverla.",
            "Die Maschine muss vor der Bewegung referenziert werden.",
            "La machine doit faire sa prise d'origine avant de bouger.",
            "La macchina deve fare l'homing prima di muoversi.",
            "移動する前に機械の原点復帰が必要です。",
            "机床移动前必须先回零。",
            "A máquina tem de fazer o homing antes de se mover.",
        ],
        12 => [
            "A limit switch is pressed at start or after a reset.",
            "Un final de carrera está pulsado al arrancar o tras un reinicio.",
            "Ein Endschalter ist beim Start oder nach einem Reset betätigt.",
            "Une fin de course est enfoncée au démarrage ou après une réinitialisation.",
            "Un finecorsa è premuto all'avvio o dopo un reset.",
            "起動時またはリセット後にリミットスイッチが押されています。",
            "启动或复位后限位开关处于触发状态。",
            "Um fim de curso está premido ao arrancar ou após um reinício.",
        ],
        13 => [
            "The probe is already triggered before the cycle: the probe protection stopped the machine.",
            "La sonda ya está activada antes del ciclo: la protección de la sonda detuvo la máquina.",
            "Der Taster ist schon vor dem Zyklus ausgelöst: Der Tasterschutz hat die Maschine angehalten.",
            "La sonde est déjà déclenchée avant le cycle : la protection de la sonde a arrêté la machine.",
            "La sonda è già attivata prima del ciclo: la protezione della sonda ha fermato la macchina.",
            "サイクル前にプローブが既に作動しています。プローブ保護により機械を停止しました。",
            "循环开始前探针已被触发：探针保护使机床停止。",
            "A sonda já está acionada antes do ciclo: a proteção da sonda parou a máquina.",
        ],
        14 => [
            "The spindle did not reach its speed in time.",
            "El husillo no alcanzó su velocidad a tiempo.",
            "Die Spindel hat ihre Drehzahl nicht rechtzeitig erreicht.",
            "La broche n'a pas atteint sa vitesse à temps.",
            "Il mandrino non ha raggiunto la velocità in tempo.",
            "スピンドルが時間内に回転数に達しませんでした。",
            "主轴未能及时达到转速。",
            "O fuso não atingiu a sua velocidade a tempo.",
        ],
        17 => [
            "A motor driver reports a fault.",
            "Un driver de motor informa de un fallo.",
            "Ein Motortreiber meldet einen Fehler.",
            "Un driver de moteur signale un défaut.",
            "Un driver di motore segnala un guasto.",
            "モータードライバーが異常を報告しています。",
            "某个电机驱动器报告故障。",
            "Um driver de motor comunica uma falha.",
        ],
        _ => return None,
    };
    Some(row[column(lang)])
}

/// The text of a kind of error of Marlin (101 and up) in the language; `None` for another number.
pub fn marlin(code: u32, lang: &str) -> Option<&'static str> {
    let row: [&str; 8] = match code {
        101 => [
            "Marlin does not know this command.",
            "Marlin no conoce este comando.",
            "Marlin kennt diesen Befehl nicht.",
            "Marlin ne connaît pas cette commande.",
            "Marlin non conosce questo comando.",
            "Marlin はこのコマンドを認識しません。",
            "Marlin 不认识此命令。",
            "O Marlin não conhece este comando.",
        ],
        102 => [
            "The number of the line is not the one that follows the last: the line is asked for again.",
            "El número de línea no es el siguiente al último: se pide la línea de nuevo.",
            "Die Zeilennummer folgt nicht auf die letzte: Die Zeile wird erneut angefordert.",
            "Le numéro de ligne ne suit pas le dernier : la ligne est redemandée.",
            "Il numero di riga non segue l'ultimo: la riga viene richiesta di nuovo.",
            "行番号が直前の番号に続いていません。行を再要求します。",
            "行号不是上一行号的下一个：将重新请求该行。",
            "O número da linha não é o que segue o último: a linha é pedida de novo.",
        ],
        103 => [
            "The checksum of the line does not match: the line was spoiled on the way.",
            "La suma de comprobación de la línea no coincide: la línea se estropeó por el camino.",
            "Die Prüfsumme der Zeile stimmt nicht: Die Zeile wurde unterwegs verfälscht.",
            "La somme de contrôle de la ligne ne correspond pas : la ligne a été altérée en chemin.",
            "Il checksum della riga non corrisponde: la riga si è rovinata lungo il percorso.",
            "行のチェックサムが一致しません。回線上で行が壊れました。",
            "该行的校验和不匹配：这一行在传输途中被损坏。",
            "A soma de verificação da linha não coincide: a linha estragou-se pelo caminho.",
        ],
        104 => [
            "Homing failed: an end stop was not found.",
            "Fallo de homing: no se encontró un final de carrera.",
            "Referenzfahrt fehlgeschlagen: Ein Endschalter wurde nicht gefunden.",
            "Échec de la prise d'origine : une fin de course n'a pas été trouvée.",
            "Homing fallito: un finecorsa non è stato trovato.",
            "原点復帰失敗：エンドストップが見つかりませんでした。",
            "回零失败：未找到限位开关。",
            "Falha do homing: não foi encontrado um fim de curso.",
        ],
        105 => [
            "The probe failed: it touched nothing, or it was already triggered.",
            "Fallo de sonda: no tocó nada, o ya estaba activada.",
            "Tastfehler: Der Taster hat nichts berührt oder war schon ausgelöst.",
            "Échec de la sonde : elle n'a rien touché, ou elle était déjà déclenchée.",
            "Sonda in errore: non ha toccato nulla, oppure era già attivata.",
            "プローブ失敗：何にも接触しなかったか、既に作動していました。",
            "探针失败：没有接触到任何东西，或一开始就已触发。",
            "Falha da sonda: não tocou em nada, ou já estava acionada.",
        ],
        106 => [
            "Marlin is halted. Nothing moves until it is unlocked (and some halts need a reset of the board).",
            "Marlin está detenido. Nada se mueve hasta desbloquearlo (y algunas paradas piden reiniciar la placa).",
            "Marlin ist angehalten. Nichts bewegt sich, bis entsperrt wird (und manche Halts verlangen einen Reset der Platine).",
            "Marlin est à l'arrêt. Rien ne bouge avant le déverrouillage (et certains arrêts demandent de réinitialiser la carte).",
            "Marlin è fermo. Nulla si muove finché non viene sbloccato (e alcuni arresti richiedono il reset della scheda).",
            "Marlin は停止しています。ロック解除まで何も動きません（基板のリセットが必要な停止もあります）。",
            "Marlin 已停机。解锁之前什么都不会动（有些停机还需要复位主板）。",
            "O Marlin está parado. Nada se move até ser desbloqueado (e algumas paragens pedem o reinício da placa).",
        ],
        107 => [
            "A temperature error stopped Marlin. Look at the heater and the sensor before going on.",
            "Un error de temperatura detuvo Marlin. Revise el calentador y el sensor antes de seguir.",
            "Ein Temperaturfehler hat Marlin angehalten. Prüfen Sie Heizung und Sensor, bevor Sie weitermachen.",
            "Une erreur de température a arrêté Marlin. Vérifiez le chauffage et la sonde avant de continuer.",
            "Un errore di temperatura ha fermato Marlin. Controllare il riscaldatore e il sensore prima di proseguire.",
            "温度エラーで Marlin が停止しました。続行する前にヒーターとセンサーを確認してください。",
            "温度错误使 Marlin 停机。继续之前请检查加热器和传感器。",
            "Um erro de temperatura parou o Marlin. Verifique o aquecedor e o sensor antes de continuar.",
        ],
        108 => [
            "Marlin reported an error that this program does not know: its own text follows.",
            "Marlin informó de un error que este programa no conoce: sigue su propio texto.",
            "Marlin hat einen Fehler gemeldet, den dieses Programm nicht kennt: Sein eigener Text folgt.",
            "Marlin a signalé une erreur que ce programme ne connaît pas : son propre texte suit.",
            "Marlin ha segnalato un errore che questo programma non conosce: segue il suo testo.",
            "Marlin がこのプログラムの知らないエラーを報告しました。Marlin 自身のテキストを表示します。",
            "Marlin 报告了本程序不认识的错误：下面是它自己的文字。",
            "O Marlin comunicou um erro que este programa não conhece: segue-se o seu próprio texto.",
        ],
        _ => return None,
    };
    Some(row[column(lang)])
}

/// The text of a refusal or an alarm of a controller of this family in the language: the table of GRBL for the families that speak GRBL,
/// the numbers that only grblHAL has, the kinds of Marlin, and the text the controller gave when no table has the number (`kind` is
/// `alarm` or `error`).
pub fn explain(family: Family, kind: &str, code: Option<u32>, given: &str, lang: &str) -> String {
    let own = code.and_then(|c| match (family, kind) {
        (Family::Marlin, _) => marlin(c, lang),
        (Family::GrblHal, "alarm") => hal_alarm(c, lang),
        (Family::GrblHal, _) => hal_error(c, lang),
        _ => None,
    });
    match own {
        Some(t) => t.to_string(),
        None => match family {
            Family::Marlin => given.to_string(),
            _ => crate::ctltext::explain(kind, code, given, lang),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_eight(texts: Vec<&'static str>, what: &str) {
        assert_eq!(texts.len(), 8, "{what}");
        assert!(texts.iter().all(|t| t.len() > 8), "{what}");
        let mut un = texts.clone();
        un.sort_unstable();
        un.dedup();
        assert_eq!(un.len(), 8, "{what}: two languages say the same");
    }

    #[test]
    fn every_number_of_the_dialects_has_all_eight_languages_and_they_differ() {
        for c in HAL_ERRORS {
            all_eight(
                LANGS.iter().map(|l| hal_error(c, l).unwrap()).collect(),
                &format!("hal error {c}"),
            );
        }
        for c in HAL_ALARMS {
            all_eight(
                LANGS.iter().map(|l| hal_alarm(c, l).unwrap()).collect(),
                &format!("hal alarm {c}"),
            );
        }
        for c in MARLIN_KINDS {
            all_eight(
                LANGS.iter().map(|l| marlin(c, l).unwrap()).collect(),
                &format!("marlin {c}"),
            );
        }
    }

    #[test]
    fn the_numbers_of_grbl_are_not_redefined_and_the_numbers_of_marlin_are_not_those_of_grbl() {
        for c in HAL_ERRORS {
            assert!(
                crate::ctltext::error(c, "en").is_none(),
                "{c} is already in the table of GRBL"
            );
        }
        for c in HAL_ALARMS {
            assert!(crate::ctltext::alarm(c, "en").is_none(), "{c}");
        }
        for c in MARLIN_KINDS {
            assert!(c > 100 && crate::ctltext::error(c, "en").is_none());
        }
        assert!(
            hal_error(20, "en").is_none()
                && hal_alarm(1, "en").is_none()
                && marlin(9, "en").is_none()
        );
    }

    #[test]
    fn explain_picks_the_table_of_the_family_and_keeps_the_text_of_the_controller_otherwise() {
        // GRBL numbers go through the table of GRBL for the families that speak it
        assert_eq!(
            explain(Family::GrblHal, "error", Some(22), "x", "fr"),
            "Aucune avance n'a encore été définie."
        );
        // a number that only grblHAL has
        assert!(explain(Family::GrblHal, "error", Some(46), "x", "es").contains("home"));
        assert!(explain(Family::GrblHal, "alarm", Some(10), "x", "de").contains("Not-Halt"));
        // the same number means nothing for plain GRBL
        assert_eq!(
            explain(Family::Grbl, "error", Some(46), "error:46", "es"),
            "error:46"
        );
        // Marlin
        assert!(explain(Family::Marlin, "error", Some(104), "x", "en").starts_with("Homing failed"));
        assert_eq!(
            explain(Family::Marlin, "error", Some(22), "raw text", "en"),
            "raw text"
        );
        assert_eq!(explain(Family::Marlin, "alarm", None, "raw", "ja"), "raw");
    }

    #[test]
    fn the_portuguese_of_the_dialects_is_that_of_portugal() {
        let mut text = String::new();
        for c in HAL_ERRORS {
            text.push_str(hal_error(c, "pt").unwrap());
        }
        for c in MARLIN_KINDS {
            text.push_str(marlin(c, "pt").unwrap());
        }
        assert!(text.contains("definição") || text.contains("ficheiro") || text.contains("fuso"));
        for brazilian in [
            "configuração",
            "arquivo",
            "fuso horário",
            "tela",
            "celular",
            "usuário",
        ] {
            assert!(!text.contains(brazilian), "{brazilian}");
        }
    }
}
