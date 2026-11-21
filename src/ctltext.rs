//! The words of the controller layer in the eight languages of the suite: the messages of the guard that keeps the machine safe when a
//! controller falls silent are here, and the alarms and the errors of the controllers by number are those of `eh3d-controller`
//! (EH3D-COMMON), re-exported under the names this program has always used. A number that is not in the table gives `None`, and the
//! program shows the text of the controller as it came.

pub use eh3d_controller::dialtext::{alarm, error, explain_grbl as explain, LANGS};

fn column(lang: &str) -> usize {
    LANGS.iter().position(|l| *l == lang).unwrap_or(0)
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
    fn the_portuguese_of_the_controllers_is_that_of_portugal() {
        use eh3d_controller::dialtext::{
            HAL_ALARMS, HAL_ERRORS, KLIPPER_KINDS, KLIPPER_LINK_KINDS, MARLIN_KINDS,
        };
        use eh3d_controller::Family;
        let mut text = String::new();
        for (family, kind, codes) in [
            (Family::GrblHal, "error", HAL_ERRORS.to_vec()),
            (Family::GrblHal, "alarm", HAL_ALARMS.to_vec()),
            (Family::Marlin, "error", MARLIN_KINDS.to_vec()),
            (Family::Klipper, "error", KLIPPER_KINDS.to_vec()),
            (Family::Klipper, "error", KLIPPER_LINK_KINDS.to_vec()),
        ] {
            for c in codes {
                let said = eh3d_controller::dialtext::explain(family, kind, Some(c), "", "pt");
                assert!(!said.is_empty(), "{family:?} {c} has no words in pt");
                text.push_str(&said);
                text.push(' ');
            }
        }
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
