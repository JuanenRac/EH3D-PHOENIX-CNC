//! What is wrong with a line of a text file of the motion layer (the motion file, a post-processor), said in the eight languages of the suite
//! (English, Spanish, German, French, Italian, Japanese, Chinese and Portuguese of Portugal). A [`Problem`] keeps the number of the line, the
//! kind of fault and a detail that is the same in every language (a range, a list of words, a line number).

use crate::ctltext::LANGS;

fn column(lang: &str) -> usize {
    LANGS.iter().position(|l| *l == lang).unwrap_or(0)
}

/// The kinds of fault of a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    NotKeyValue,
    UnknownKey,
    NotANumber,
    /// The number is not in its range (the detail says the range).
    OutOfRange,
    OnOff,
    /// The key was already set (the detail says on which line).
    Repeated,
    TooBig,
    Unreadable,
    TooManyRules,
    ReplaceForm,
    DropForm,
    /// One of a few words is needed (the detail lists them).
    Choice,
    NeedsName,
    /// The address of Moonraker is not one the program may reach.
    HostRefused,
    /// The credential of Moonraker is not accepted (the text never says it).
    SecretRefused,
    /// A script of G-code would be cut off before the controller gives up on it.
    WaitShorter,
}

impl Why {
    pub const ALL: [Why; 16] = [
        Why::NotKeyValue,
        Why::UnknownKey,
        Why::NotANumber,
        Why::OutOfRange,
        Why::OnOff,
        Why::Repeated,
        Why::TooBig,
        Why::Unreadable,
        Why::TooManyRules,
        Why::ReplaceForm,
        Why::DropForm,
        Why::Choice,
        Why::NeedsName,
        Why::HostRefused,
        Why::SecretRefused,
        Why::WaitShorter,
    ];

    pub fn text(self, lang: &str) -> &'static str {
        let row: [&str; 8] = match self {
            Why::NotKeyValue => [
                "it is not key = value",
                "no es clave = valor",
                "es ist nicht Schlüssel = Wert",
                "ce n'est pas clé = valeur",
                "non è chiave = valore",
                "キー = 値の形式ではありません",
                "不是 键 = 值 的形式",
                "não é chave = valor",
            ],
            Why::UnknownKey => [
                "unknown key",
                "clave desconocida",
                "unbekannter Schlüssel",
                "clé inconnue",
                "chiave sconosciuta",
                "未知のキーです",
                "未知的键",
                "chave desconhecida",
            ],
            Why::NotANumber => [
                "not a number",
                "no es un número",
                "keine Zahl",
                "ce n'est pas un nombre",
                "non è un numero",
                "数値ではありません",
                "不是数字",
                "não é um número",
            ],
            Why::OutOfRange => [
                "out of range, it must be from",
                "fuera de rango, debe estar entre",
                "außerhalb des Bereichs, erlaubt ist",
                "hors plage, la valeur doit être de",
                "fuori intervallo, deve essere tra",
                "範囲外です。有効な範囲:",
                "超出范围，应为",
                "fora do intervalo, deve estar entre",
            ],
            Why::OnOff => [
                "it must be on or off",
                "debe ser on u off",
                "es muss on oder off sein",
                "ce doit être on ou off",
                "deve essere on oppure off",
                "on または off を指定してください",
                "必须为 on 或 off",
                "tem de ser on ou off",
            ],
            Why::Repeated => [
                "already set on line",
                "ya se fijó en la línea",
                "bereits gesetzt in Zeile",
                "déjà défini à la ligne",
                "già impostato alla riga",
                "すでに設定済みの行:",
                "已在以下行设置:",
                "já definido na linha",
            ],
            Why::TooBig => [
                "the file is too big",
                "el archivo es demasiado grande",
                "die Datei ist zu groß",
                "le fichier est trop grand",
                "il file è troppo grande",
                "ファイルが大きすぎます",
                "文件过大",
                "o ficheiro é demasiado grande",
            ],
            Why::Unreadable => [
                "the file cannot be read",
                "no se puede leer el archivo",
                "die Datei kann nicht gelesen werden",
                "le fichier ne peut pas être lu",
                "il file non può essere letto",
                "ファイルを読み込めません",
                "无法读取文件",
                "o ficheiro não pode ser lido",
            ],
            Why::TooManyRules => [
                "too many rules",
                "demasiadas reglas",
                "zu viele Regeln",
                "trop de règles",
                "troppe regole",
                "ルールが多すぎます",
                "规则过多",
                "demasiadas regras",
            ],
            Why::ReplaceForm => [
                "replace needs OLD -> NEW with a word to replace",
                "replace necesita ANTIGUO -> NUEVO con una palabra que reemplazar",
                "replace braucht ALT -> NEU mit einem zu ersetzenden Wort",
                "replace demande ANCIEN -> NOUVEAU avec un mot à remplacer",
                "replace richiede VECCHIO -> NUOVO con una parola da sostituire",
                "replace には置換する語を含む「旧 -> 新」が必要です",
                "replace 需要“旧 -> 新”，且旧值为一个字",
                "replace precisa de ANTIGO -> NOVO com uma palavra a substituir",
            ],
            Why::DropForm => [
                "drop needs a word such as M6",
                "drop necesita una palabra como M6",
                "drop braucht ein Wort wie M6",
                "drop demande un mot comme M6",
                "drop richiede una parola come M6",
                "drop には M6 のような語が必要です",
                "drop 需要像 M6 这样的字",
                "drop precisa de uma palavra como M6",
            ],
            Why::Choice => [
                "it must be one of",
                "debe ser uno de",
                "es muss einer von diesen sein",
                "ce doit être l'un de",
                "deve essere uno tra",
                "次のいずれかを指定してください:",
                "必须为以下之一",
                "tem de ser um de",
            ],
            Why::NeedsName => [
                "a file name is needed",
                "se necesita un nombre de archivo",
                "ein Dateiname wird benötigt",
                "un nom de fichier est nécessaire",
                "serve un nome di file",
                "ファイル名が必要です",
                "需要文件名",
                "é preciso um nome de ficheiro",
            ],
            Why::HostRefused => [
                "not an address this program may reach: the local network, or any address with klipper.allow_non_local on",
                "no es una dirección a la que este programa pueda llegar: la red local, o cualquiera con klipper.allow_non_local activado",
                "keine Adresse, die dieses Programm erreichen darf: das lokale Netz, oder jede mit eingeschaltetem klipper.allow_non_local",
                "ce n'est pas une adresse que ce programme peut joindre : le réseau local, ou toute adresse avec klipper.allow_non_local activé",
                "non è un indirizzo raggiungibile da questo programma: la rete locale, o qualsiasi indirizzo con klipper.allow_non_local attivo",
                "このプログラムが接続できるアドレスではありません。ローカルネットワークか、klipper.allow_non_local を有効にした場合の他のアドレスのみです",
                "不是本程序可以连接的地址：只能是本地网络，或在开启 klipper.allow_non_local 后的其他地址",
                "não é um endereço que este programa possa alcançar: a rede local, ou qualquer endereço com klipper.allow_non_local ativado",
            ],
            Why::SecretRefused => [
                "the credential is not accepted: 1 to 256 printable characters without spaces (it is not repeated here)",
                "la credencial no se acepta: de 1 a 256 caracteres imprimibles sin espacios (no se repite aquí)",
                "die Zugangsdaten werden nicht akzeptiert: 1 bis 256 druckbare Zeichen ohne Leerzeichen (sie werden hier nicht wiederholt)",
                "l'identifiant n'est pas accepté : de 1 à 256 caractères imprimables sans espace (il n'est pas répété ici)",
                "la credenziale non è accettata: da 1 a 256 caratteri stampabili senza spazi (non viene ripetuta qui)",
                "認証情報は受け付けられません。空白を含まない表示可能な文字で1〜256文字です（ここには繰り返しません）",
                "凭据不被接受：1 到 256 个不含空格的可打印字符（此处不会重复显示）",
                "a credencial não é aceite: de 1 a 256 carateres imprimíveis sem espaços (não é repetida aqui)",
            ],
            Why::WaitShorter => [
                "klipper.response_wait_ms is shorter than klipper.busy_limit_s: a move would be cut off",
                "klipper.response_wait_ms es más corto que klipper.busy_limit_s: se cortaría un movimiento",
                "klipper.response_wait_ms ist kürzer als klipper.busy_limit_s: eine Bewegung würde abgebrochen",
                "klipper.response_wait_ms est plus court que klipper.busy_limit_s : un mouvement serait coupé",
                "klipper.response_wait_ms è più breve di klipper.busy_limit_s: un movimento verrebbe interrotto",
                "klipper.response_wait_ms が klipper.busy_limit_s より短いため、移動が途中で打ち切られます",
                "klipper.response_wait_ms 短于 klipper.busy_limit_s：运动会被中途切断",
                "klipper.response_wait_ms é mais curto do que klipper.busy_limit_s: um movimento seria cortado",
            ],
        };
        row[column(lang)]
    }
}

/// A line of a text file of the motion layer that could not be used: its number (from 1; 0 for the whole file), what is wrong, and a detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub line: usize,
    pub why: Why,
    pub detail: String,
}

impl Problem {
    pub fn new(line: usize, why: Why, detail: impl Into<String>) -> Problem {
        Problem {
            line,
            why,
            detail: detail.into(),
        }
    }

    /// The fault said in the language: `3: it must be on or off`.
    pub fn text(&self, lang: &str) -> String {
        let mut t = format!("{}: {}", self.line, self.why.text(lang));
        if !self.detail.is_empty() {
            t.push(' ');
            t.push_str(&self.detail);
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fault_has_all_eight_languages_and_they_differ() {
        for w in Why::ALL {
            let mut all: Vec<&str> = LANGS.iter().map(|l| w.text(l)).collect();
            assert!(all.iter().all(|t| t.len() > 3), "{w:?}");
            all.sort_unstable();
            all.dedup();
            assert_eq!(all.len(), 8, "{w:?}");
            assert_eq!(w.text("xx"), w.text("en"));
        }
    }

    #[test]
    fn a_problem_is_said_with_its_line_and_its_detail_in_the_language() {
        let p = Problem::new(3, Why::OutOfRange, "0 - 10");
        assert_eq!(p.text("en"), "3: out of range, it must be from 0 - 10");
        assert_eq!(
            p.text("pt"),
            "3: fora do intervalo, deve estar entre 0 - 10"
        );
        assert_eq!(
            Problem::new(0, Why::TooBig, "").text("es"),
            "0: el archivo es demasiado grande"
        );
    }

    #[test]
    fn the_portuguese_is_that_of_portugal() {
        let t = [
            Why::TooBig.text("pt"),
            Why::NeedsName.text("pt"),
            Why::Unreadable.text("pt"),
        ]
        .join(" ");
        assert!(t.contains("ficheiro") && !t.contains("arquivo"));
    }
}
