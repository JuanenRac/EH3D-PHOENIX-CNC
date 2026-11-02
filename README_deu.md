<p align="center">
  <img src="images/EH3D_BANNER.svg" alt="EH3D-MOTIONCNC Banner" width="100%">
</p>

# 🧭 EH3D-MOTIONCNC

<p align="center"><a href="README.md">🇺🇸 English</a> | <a href="README_spa.md">🇪🇸 Español</a> | <a href="README_fra.md">🇫🇷 Français</a> | <a href="README_ita.md">🇮🇹 Italiano</a> | 🇩🇪 <b>Deutsch</b> | <a href="README_zho.md">🇨🇳 中文</a> | <a href="README_jpn.md">🇯🇵 日本語</a> | <a href="README_por.md">🇵🇹 Português</a></p>

### Die gemeinsame Basis der EH3D-Maschinensteuerprogramme: die Ansicht eines Auftrags, die Themen und die Texte der Oberfläche in acht Sprachen

<p align="center">
  <img src="https://img.shields.io/badge/License-GPL%203.0--or--later-blue.svg" alt="GPL 3.0 or later">
  <img src="https://img.shields.io/badge/Stack-Rust-DEA584.svg" alt="Rust">
  <img src="https://img.shields.io/badge/Languages-8-00E5FF.svg" alt="Languages">
  <img src="https://img.shields.io/badge/Version-0.0.8-0f6cbd.svg" alt="Version">
  <img src="https://img.shields.io/badge/Maturity-scaffolding-FFB020.svg" alt="Maturity">
  <img src="https://img.shields.io/badge/Tests-91-0f6cbd.svg" alt="Tests">
</p>

---

**Ehrlichkeitscheck - was heute tatsächlich läuft:** ein Rust-Programm mit einem Fenster (`motioncnc`), das eine **simulierte** Maschine steuert: es zeigt die Position der Maschine (Werkstück und Maschine), fährt sie von Hand mit einem Jog-Feld, setzt den Nullpunkt, fährt die Referenz an, nimmt die vom Bediener getippten Befehle an, führt ein G-Code-Programm Zeile für Zeile aus (mit Pause, Stopp und Not-Halt), zeichnet die Bahn des Programms auf den Tisch mit den erledigten Zeilen markiert und hält an, was die Steuerung ablehnt. Das Fenster spricht 37 Sprachen und trägt elf Themen. Die Steuerungsschnittstelle (`eh3d-controller`, in EH3D-COMMON) hat GRBL gegen eine geskriptete Leitung geschrieben und führt die übrigen Familien als noch nicht angesteuert. 91 Tests laufen auf einem Rechner. **Nichts davon lief auf einer echten Maschine**, eine serielle Schnittstelle wird nur für GRBL geöffnet und eine Netzverbindung nur, wenn ein Programm an den Server gesendet wird, und Bögen und die übrigen Codes werden vom Simulator weder gezeichnet noch ausgeführt.

---

> **Sicherheitsstatus:** dieses Programm steuert nur eine simulierte Maschine: es öffnet weder eine serielle Schnittstelle noch eine Netzverbindung. Wenn eine echte Steuerung angeschlossen wird, muss sie sich erst an einem Simulator und dann an einer echten Maschine bewähren, bevor sie als funktionierend gilt; die Endschalter und ein Not-Halt bleiben Aufgabe der eigenen Steuereinheit der Maschine, der Not-Halt des Fensters ist kein Sicherheitskreis.

## 📋 Auf einen Blick

| | |
|---|---|
| Version | `0.0.8` |
| Reifegrad | Gerüst |
| Tests | 91 |
| Technologie-Stack | Rust |
| Auf echter Hardware | Nein: es steuert nur eine simulierte Maschine |

**Springe zu:** [Überblick](#-überblick) · [Architektur](#-architektur) · [Schnellstart](#-schnellstart) · [Tests](#-tests) · [Struktur](#-struktur-des-repositorys) · [Verwandte](#-verwandte-projekte)

## 🚦 Projektstatus

Jede Zeile ist ein Teil dieses Repositorys, und die Ampel zeigt, wie weit er heute ist. Sie beruht auf dem, was das Repository zeigen kann (Tests, Builds, Läufe), nicht auf Plänen.

🟢 Fertig und überprüft · 🟡 Funktioniert, mit bekannten Grenzen · 🟠 Teilweise fertig oder noch nicht überprüft · 🔴 Nicht gemacht oder funktioniert nicht

| Bereich | Stand | Was das hier bedeutet |
|---|---|---|
| Basis-Bibliothek | 🟡 | Ein Rust-Crate, das `scene`, `texts` und `theme` aus `eh3d-view` unter dem Namen dieses Programms erneut exportiert; es braucht den Ordner von EH3D-COMMON daneben. |
| Szene und Ansichten | 🟡 | In EH3D-COMMON geschrieben und getestet; das Fenster dieses Programms zeichnet die Bahn eines Programms auf den Tisch und die Position des Werkzeugs (auf einem Rechner gesehen). |
| Themen und Texte | 🟡 | Elf Themen und die Wörter der Oberfläche in 37 Sprachen, im Fenster dieses Programms gesehen. |
| Tests | 🟡 | 91 Tests bestehen auf einem Rechner (`cargo test --features gui`): die Sitzung (Jog, Nullpunkt, Referenz, ein Programm, Pause, Alarm, Not-Halt), die Bahn eines Programms und die Befehlszeile; die simulierte Steuerung wird in EH3D-COMMON getestet. |
| Fenster und Jog | 🟡 | Läuft auf einem Rechner mit der simulierten Steuerung: Position, Jog-Feld, Schritte, Vorschub, Referenz, Nullpunkt, Entsperren, eine Befehlszeile und ein Protokoll; keine echte Maschine. |
| Maschinensteuerung | 🟠 | Nur der Simulator: ein Programm läuft Zeile für Zeile mit Pause, Stopp und Not-Halt; von hier ist noch keine echte Steuerung angeschlossen. |
| Dokumentation | 🟡 | README in acht Sprachen, die beiden Bilder, drei Beispielprogramme und das Changelog. |

## 🎯 Überblick

* Das **Fenster** (`src/gui.rs`, `motioncnc`): die Position der Maschine (Werkstück und Maschine), ein Jog-Feld mit Schritten und Vorschub, Referenz, Nullpunkt und Entsperren, der Not-Halt, eine Befehlszeile mit Protokoll und das Programm mit seiner Bahn auf dem Tisch.
* Die **Sitzung** (`src/session.rs`): was das Programm mit jeder Steuerung von `eh3d-controller` tut: Jog, Nullpunkt, Referenz, eine Zeile senden, ein Programm Zeile für Zeile ausführen, Pause, Stopp und die Alarme; sie wird an der simulierten Steuerung getestet.
* Die **Bahn eines Programms** (`src/gcode.rs`): die geraden Bewegungen (`G0`, `G1`, absolut oder relativ, in mm oder Zoll), vor dem Lauf gezeichnet; Bögen werden nicht gezeichnet.
* Die **simulierte Steuerung** (`eh3d_controller::sim`, in EH3D-COMMON): eine Maschine im Speicher mit Verfahrgrenzen, die einen Alarm auslösen; die Steuerungsfamilien des Marktes sind in `eh3d-controller` aufgeführt, GRBL ist die erste geschriebene und die anderen erscheinen als noch nicht angesteuert.
* Die **Themen und Wörter** von `eh3d-view`: Forge (Standard), Neon Arcade, Glass Pro, das klassische Windows-Grau und eines, das Windows folgt, und die Oberfläche auf Englisch, Spanisch, Italienisch, Deutsch, Französisch, Portugiesisch, Chinesisch und Japanisch. Was es werden soll: ein Programm zum Bohren, Fräsen und für 2D- und 3D-CNC auf den Engines von EH3D-DRILLPRO und EH3D-ROUTEPRO; dieser Teil existiert noch nicht.
* Die **Menüs, die Einstellungen und die Dateien**: die Menüs Datei, Ansicht, Programm, Maschine und Hilfe mit dem, was PlanetCNC darin hat (öffnen, zuletzt geöffnet, speichern, bearbeiten, die Nullpunkte, die gespeicherten Positionen, die Spindel und das Kühlmittel); ein Einstellungsfenster (Einheiten, Sprache, elf Themen, Verfahrgrenzen, Jog), das zwischen den Läufen gemerkt wird; und die Fenster Öffnen und Speichern der Suite (`eh3d-filedialog`: Orte, Sortierung, Filter, die Endung einer gespeicherten Datei und die Frage vor dem Ersetzen).
* **GRBL an einer seriellen Schnittstelle:** mit `--features serial` gebaut, steuert `motioncnc --grbl PORT[@BAUD]` einen GRBL-Controller (standardmäßig 115200 Baud) statt des Simulators, und `motioncnc --ports` listet die seriellen Ports; ein Build ohne das Feature oder ein Port, der sich nicht öffnet, sagt es und beendet sich. Nur gegen einen skriptgesteuerten Controller erprobt, nie gegen einen echten.
* **Ein Fenster, das dem Bildschirm folgt:** es öffnet sich maximiert, und von 720p über 1080p bis zu größeren und breiteren Bildschirmen wächst oder schrumpft alles mit.
* **Weitermachen nach einem Ausfall:** Während ein Programm läuft, wird die erreichte Zeile in einer kleinen Datei gespeichert; fällt das Fenster, der Rechner oder die Verbindung aus, fragt der nächste Start, ob ab dieser Zeile weitergemacht oder verworfen wird. Bis der Bediener antwortet, wird nichts an die Steuerung gesendet.
* **An den EH3D-Server gesendet:** Mit der Adresse des Servers, dem Token dieses Programms und der Kennung der Maschine in den Einstellungen (Abschnitt Server des Einstellungsfensters oder `server_address`, `server_token` und `server_machine` in der Einstellungsdatei) sendet **Programm > An den Server senden** den Text des Programms als G-Code-Auftrag; der Server führt ihn auf seiner Maschine aus (ein GRBL-Controller an seiner seriellen Schnittstelle oder sein Simulator) und das Fenster verfolgt ihn: Zustand, Prozent und erreichte Zeile in der Kopfleiste, im Panel und im Protokoll. Start, Pause und Stopp des Fensters erreichen den Auftrag auf dem Server, das Schließen des Fensters stoppt ihn nicht, und ein angehaltener Auftrag wird ab der Zeile fortgesetzt, die der Server erreicht hat (der nächste Start des Fensters bietet es an, wie bei einem Programm des Controllers): der Server gibt die geltenden Codes zurück und wartet vor der ersten Bewegung auf den Bediener. Spindel und Kühlmittel werden nur mit `server_restore_outputs = yes` wieder gestartet. Gegen einen Stellvertreter-Server geprüft; die eigenen Tests des Servers senden dieselben Anfragen an seinen Simulator. Nie an einer Maschine.
* **Fortsetzen nach einem Fehler:** Ein halb gelaufenes Programm wird zuerst in seinen Zustand zurückgesetzt: Einheiten und absolute Maße, ein sicheres Z (`resume_safe_z` in der Einstellungsdatei, 5 mm), die Spindel mit der Zeit bis zur Drehzahl (`resume_spindle_wait`, 3 s), das Kühlmittel, der erreichte Punkt und der Modalzustand; die Wiederherstellungsdatei enthält den SHA-256 des Programms und setzt nicht fort, wenn sich das Programm geändert hat.
* **Die Hilfen des Fensters:** eine **Zusammenfassung vor dem Start eines Programms** (die Bewegungen, der Schnittweg, die Zeit bei den Geschwindigkeiten der Einstellungen, die Reichweite der Achsen und eine Prüfung gegen den Verfahrweg der Maschine, mit den Warnungen, die sich die Suite teilt: ein winziges oder weit von der Ecke entferntes Programm, größer als der Tisch, Bögen, die die Schätzung auslässt, und Schnitte vor jedem Vorschub; **Trotzdem starten**, oder eine Ablehnung, wenn das Programm den Verfahrweg verlässt, die `reject_out_of_travel = no` zur Warnung macht, und `confirm_start = no` schaltet die Zusammenfassung ab); **Tasten der Tastatur** (Strg+O öffnen, Strg+S speichern, Strg+E bearbeiten, F5 Start, Leertaste Pause, Esc Stopp (bei offenem Fenster schließt Esc es, und mit der Tastatur in einem Textfeld bleiben Leertaste und Esc dem Feld), F1 listet sie auf; `shortcuts = no` schaltet sie ab); eine **Werkzeugleiste** mit gezeichneten Bildern und ihren Namen; Zahlen, die mit Minus und Plus gestuft oder **eingetippt** werden; die Bahn **gezogen und gezoomt von oben oder gedreht und gezoomt in 3D** mit dem Werkzeug; eine **Vorschau der G-Code-Datei** im Öffnen-Fenster; die **Aufzeichnung eines Absturzes** (`eh3d-crash.log`); und ein Abschnitt Sicherheit in den Einstellungen (`rapid`, `summary_tiny`, `summary_far` und `summary_margin` in der Einstellungsdatei).
* **Eine Warteschlange von Programmen und der Verfahrweg der Steuerung:** mehrere im Öffnen-Fenster mit Strg+Klick gewählte, zusammen abgelegte oder in der Befehlszeile genannte Dateien (`motioncnc a.nc b.nc`) laufen nacheinander, jede nach ihrer eigenen Zusammenfassung; das Fenster listet sie mit ihrem Zustand auf, erlaubt das Entfernen, Überspringen und Umordnen der wartenden, und die Warteschlange hält an, wenn ein Programm nicht fertig wird. Bei einer GRBL-Steuerung liest das Programm beim Verbinden `$130`, `$131` und `$132` (den größten Verfahrweg von X, Y und Z) und nimmt für Zusammenfassung und Ablehnung den kleineren Wert aus diesem und `travel_x`, `travel_y` und `travel_z`; antwortet die Steuerung nicht oder liefert sie die Werte nicht, gelten die Einstellungen und das Protokoll sagt es (`read_controller_travel = no` schaltet das Lesen ab).

## 🧩 Rolle in EH3D

EH3D-MOTIONCNC ist das Maschinensteuerungsprogramm von EH3D, heute auf einer simulierten Maschine. Es hängt per Pfad von EH3D-COMMON ab (der Ordner `EH3D-COMMON` muss neben diesem liegen: die Ansicht, die Themen, die Wörter und die Steuerungen) und noch von keinem anderen EH3D-Repository.

## 🏛️ Architektur

<p align="center">
  <img src="images/architecture.svg" alt="Architektur von EH3D-MOTIONCNC: wer es nutzen wird, was es enthält und worauf es sich stützt" width="100%">
</p>

Wie die Teile zusammenpassen: wer es nutzen wird, was dieses Repository enthält und worauf es sich stützt. Das Bild wird aus `docs/architecture.json` gezeichnet, gegen den Code geprüft und kennzeichnet Künftiges als künftig.

## 🖼️ Screenshots

<p align="center">
  <img src="images/screenshots/program-running.png" alt="a program running" width="100%">
</p>

<p align="center"><em>Ein Programm läuft auf der simulierten Maschine: die erledigten Zeilen sind grün, die aktuelle Zeile hell, die Bahn ist auf den Tisch gezeichnet und das Werkzeug steht, wo die Maschine ist.</em></p>

<p align="center">
  <img src="images/screenshots/spiral-neon.png" alt="Midnight theme" width="100%">
</p>

<p align="center"><em>Das Thema Mitternacht mit einem langen, beendeten Programm.</em></p>

<p align="center">
  <img src="images/screenshots/square-german.png" alt="German interface, light theme" width="100%">
</p>

<p align="center"><em>Die Oberfläche auf Deutsch mit dem hellen Thema Papier: jedes Wort des Fensters folgt der Sprache.</em></p>

<p align="center">
  <img src="images/screenshots/settings-window.png" alt="settings window" width="100%">
</p>

<p align="center"><em>Das Einstellungsfenster, mit den Abschnitten links wie der Baum von PlanetCNC: die Sprache, die elf Themen und was gezeigt wird.</em></p>

<p align="center">
  <img src="images/screenshots/open-window.png" alt="Open window" width="100%">
</p>

<p align="center"><em>Das Fenster Öffnen der Suite: Orte, zuletzt geöffnet, die Liste des Ordners mit Datum und Größe, der Name und der Typ.</em></p>

## 🔒 Sicherheit und Schutz

Dieses Programm öffnet eine serielle Schnittstelle nur, wenn es für GRBL gestartet wird (`--grbl`, ein Build mit `--features serial`), und eine Netzverbindung nur zum EH3D-Server, dessen Adresse in den Einstellungen steht, wenn der Bediener ihm ein Programm sendet; das Token liegt als Text in der Einstellungsdatei. Es öffnet keine Dateien anderer Personen außer der G-Code-Datei, die der Bediener wählt (als Text gelesen, nie von selbst ausgeführt), und ist kein Sicherheitssystem. Siehe [SECURITY.md](SECURITY.md).

## 🚀 Schnellstart

Du brauchst eine stabile Rust-Toolchain und den Ordner `EH3D-COMMON` neben diesem.

```bash
cargo build --release --features gui
cargo test --features gui
cargo clippy --all-targets --features gui -- -D warnings
cargo fmt --all --check
python -I tools/ci_validate.py
cargo run --release --features gui -- examples/02_frame_and_pocket.nc
```

`build.bat` / `build.sh` bauen das Programm, `test.bat` / `test.sh` führen die Tests aus und `run.bat` / `run.sh` öffnen das Fenster mit einem Beispielprogramm. Die Skripte warten am Ende auf eine Taste, außer `NOPAUSE` ist gesetzt.

## 🧪 Tests

`cargo test --features gui` führt 91 Tests aus: die Sitzung (Jog und Nullpunkt, ein Programm Zeile für Zeile, Pause und Weiter, eine abgelehnte Zeile, der Not-Halt und der Alarm, das Protokoll der Befehle, die Referenz), die Bahn eines Programms und die Befehlszeile; die Ansicht und die simulierte Steuerung werden in EH3D-COMMON getestet. Die CI führt außerdem `cargo fmt --check` und `cargo clippy` mit Warnungen als Fehlern aus, und `python -I tools/ci_validate.py` prüft Manifest, Versionen, Changelog und die acht READMEs.

## 🏗️ Build und Veröffentlichung

Die Version steht in `Cargo.toml` und in `eh3d.project.json`, und jede Version hat ihren Eintrag in [CHANGELOG.md](CHANGELOG.md). Es gibt noch kein Programm zu verpacken, also keine fertigen Binärdateien.

## 📂 Struktur des Repositorys

<p align="center">
  <img src="images/repository-structure.svg" alt="Repository-Struktur von EH3D-MOTIONCNC" width="100%">
</p>

<details>
<summary>Textversion des Baums</summary>

```text
EH3D-MOTIONCNC/
├── src/
│   ├── lib.rs                                   die Module und die Ansicht von eh3d-view unter dem Namen dieses Programms
│   ├── gui.rs                                   das Fenster: Position, Jog, Programm, Bahn, Befehle und Protokoll
│   ├── session.rs                               was mit einer Steuerung getan wird: Jog, Nullpunkt, Referenz, ein Programm Zeile für Zeile, Pause, Stopp, Alarme
│   ├── queue.rs                                 die Programm-Warteschlange: Reihenfolge, Zustand, überspringen, entfernen, Halt bei Fehler
│   ├── gcode.rs                                 die geraden Bewegungen eines G-Code-Programms, um seine Bahn zu zeichnen
│   ├── view.rs                                  der Plan, der gezogen und gezoomt wird, und die gedrehte 3D-Ansicht, als Zahlen
│   ├── plan.rs                                  die Prüfung vor dem Start: Zeit, Reichweite und Verfahrweg der Bewegungen
│   ├── cli.rs                                   die Befehlszeile: die Datei, die Sprache, das Thema und die Bildoptionen
│   ├── bin/motioncnc.rs                         das Programm, das das Fenster öffnet
│   ├── server.rs                                die Verbindung zum EH3D-Server: senden, verfolgen, anhalten, stoppen
│   ├── settings.rs                              was zwischen Läufen gemerkt wird
│   ├── gui/dialogs.rs                           Menüs, Einstellungen, Öffnen, Speichern
│   ├── gui/remote.rs                            das Programm auf dem Server: Start, Pause, Stopp und Fortsetzen
│   ├── gui/views.rs                             die Fenstermitte: die Bahn von oben oder in 3D, mit dem Werkzeug
│   ├── gui/summary.rs                           das Zusammenfassungsfenster vor dem Start und die Tastenliste
│   ├── gui/numfield.rs                          eine Zahl, die mit den Tasten Minus und Plus oder durch Eintippen gesetzt wird
│   ├── gui/keys.rs                              die Tasten der Tastatur und die Tabelle, die Menüs und Tastenliste lesen
│   ├── gui/queue.rs                             die Warteschlange im Fenster: Dateien nacheinander, jede nach ihrer Zusammenfassung
│   └── gui/icons.rs                             die Bilder der Schaltflächen, mit Linien gezeichnet, und die Schaltfläche der Werkzeugleiste
├── examples/                                    drei Beispiel-G-Code-Programme, vom Quadrat bis zur langen Spirale
├── tools/                                       CI-Prüfung, README-Prüfung und die Bild-Werkzeuge
├── docs/                                        die Quellen der Bilder von Architektur und Repository-Struktur
├── .github/                                     CI-Workflow, Vorlagen für Issues und Pull Requests
├── images/                                      Banner, die beiden Bilder und die Screenshots
├── build.sh build.bat                           bauen die Bibliothek
├── run.sh run.bat                               melden, dass es noch kein Programm gibt, und führen die Tests aus
├── eh3d.project.json                            Manifest: Version, Rolle, Reifegrad und Build-Befehl
├── Cargo.toml Cargo.lock                        das Rust-Paket und seine festgelegten Abhängigkeiten
├── CHANGELOG.md                                 was sich in jeder Version geändert hat
├── SECURITY.md SUPPORT.md                       Schwachstellen melden und Fragen stellen
└── CONTRIBUTING.md CODE_OF_CONDUCT.md LICENSE   Mitwirken, Verhaltenskodex und Lizenz
```

</details>

## 🗺️ Roadmap und bekannte Grenzen

**Bekannte Grenzen heute**

* Nichts davon lief auf einer echten Maschine: die Steuerung ist ein Simulator (oder GRBL an einer seriellen Schnittstelle, nie ausprobiert), und der Server wird nur von den Tests erreicht.
* Der Simulator nimmt nur einfachen G-Code (`G0`, `G1`, `G90`, `G91`, `G20`, `G21`, `G92`, `M3` bis `M9`); Bögen, Festzyklen und Tasten werden abgelehnt, und die Bahn zeichnet keine Bögen.
* Es braucht den Ordner `EH3D-COMMON` daneben, weil es per Pfad davon abhängt.

**Als Nächstes**

* Eine echte Steuerung: zuerst GRBL über eine serielle Schnittstelle, dann die anderen Familien, dann die Steuerung von HYDRA-UMC.
* Bögen und der Rest des G-Codes im Simulator und in der Bahn.
* Eine 3D-Ansicht der Bahn mit der Höhe der Schnitte.
* Tasten, Werkzeugwechsler und die Materialien der Maschine.
* Die Engines von EH3D-DRILLPRO und EH3D-ROUTEPRO verbinden, damit ein Programm bohrt und fräst.

## 🌍 Sprachen

README und Texte gibt es auf Englisch, Spanisch, Französisch, Italienisch, Deutsch, Chinesisch, Japanisch und europäischem Portugiesisch.

## 🔗 Verwandte Projekte

* [EH3D-COMMON](https://github.com/JuanenRac/EH3D-COMMON): dort liegen die Ansicht (`eh3d-view`) und das ZIP der Aufträge (`eh3d-archive`)
* [EH3D-DRILLPRO](https://github.com/JuanenRac/EH3D-DRILLPRO): das Bohrprogramm, dessen Motor hier verbunden wird
* [EH3D-ROUTEPRO](https://github.com/JuanenRac/EH3D-ROUTEPRO): das Fräsprogramm, dessen Motor hier verbunden wird

## 📚 Dokumentation

* [Änderungsprotokoll](CHANGELOG.md)
* [Sicherheitsrichtlinie](SECURITY.md) und [Support](SUPPORT.md)
* Die Ansicht eines Auftrags ist in EH3D-COMMON dokumentiert, wo sie liegt.

## 🤝 Mitwirken und Community

* Vor einem Pull Request `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` und `python -I tools/ci_validate.py` ausführen: alles muss bestehen. Siehe [CONTRIBUTING.md](CONTRIBUTING.md) und den [Verhaltenskodex](CODE_OF_CONDUCT.md).
* Das README gibt es in acht Sprachen: eine Textänderung wird in allen acht Dateien gemacht, und die beiden Bilder werden aus `docs/architecture.json` und `docs/structure.json` mit `tools/make_architecture.py` und `tools/make_structure.py` gezeichnet.
* Beschreibe nichts als funktionierend, was nicht auf einer Maschine gelaufen ist.
* Schwachstellen bitte privat melden: [SECURITY.md](SECURITY.md). Fragen, Ideen und Berichte: electrohobby3d@gmail.com ([SUPPORT.md](SUPPORT.md)).

## 👤 AUTOR

**JuanenRac (Electro Hobby 3D)** · electrohobby3d@gmail.com

## 📜 LIZENZ

Software: GPL-3.0-or-later. Siehe [LICENSE](LICENSE). Hardware-Entwürfe der Suite verwenden CERN OHL-S.
