<p align="center">
  <img src="images/EH3D_PHOENIX_CNC_BANNER.svg" alt="EH3D-PHOENIX-CNC Banner" width="100%">
</p>

# <img src="images/brand/icon-64.png" alt="" width="40" height="40" align="absmiddle"> EH3D-PHOENIX-CNC
<p align="center"><a href="README.md">🇺🇸 English</a> | <a href="README_spa.md">🇪🇸 Español</a> | <a href="README_fra.md">🇫🇷 Français</a> | <a href="README_ita.md">🇮🇹 Italiano</a> | 🇩🇪 <b>Deutsch</b> | <a href="README_zho.md">🇨🇳 中文</a> | <a href="README_jpn.md">🇯🇵 日本語</a> | <a href="README_por.md">🇵🇹 Português</a></p>

### Die gemeinsame Basis der EH3D-Maschinensteuerprogramme: die Ansicht eines Auftrags, die Themen und die Texte der Oberfläche in acht Sprachen

<p align="center">
  <img src="https://img.shields.io/badge/License-GPL%203.0--or--later-blue.svg" alt="GPL 3.0 or later">
  <img src="https://img.shields.io/badge/Stack-Rust-DEA584.svg" alt="Rust">
  <img src="https://img.shields.io/badge/Languages-8-00E5FF.svg" alt="Languages">
  <img src="https://img.shields.io/badge/Version-0.1.6-0f6cbd.svg" alt="Version">
  <img src="https://img.shields.io/badge/Maturity-functional-4CAF50.svg" alt="Maturity">
  <img src="https://img.shields.io/badge/Tests-493-0f6cbd.svg" alt="Tests">
</p>

---

**Ehrlichkeitscheck - was heute tatsächlich läuft:** ein Rust-Programm mit einem Fenster (`phoenix-cnc`), das eine **simulierte** Maschine steuert: es zeigt die Position der Maschine (Werkstück und Maschine), fährt sie von Hand mit einem Jog-Feld, setzt den Nullpunkt, fährt die Referenz an, nimmt die vom Bediener getippten Befehle an, führt ein G-Code-Programm Zeile für Zeile aus (mit Pause, Stopp und Not-Halt), zeichnet die Bahn des Programms auf den Tisch mit den erledigten Zeilen markiert und hält an, was die Steuerung ablehnt. Das Fenster spricht 37 Sprachen und trägt elf Themen. Die Steuerungsschnittstelle (`eh3d-controller`, in EH3D-COMMON) hat GRBL gegen eine geskriptete Leitung geschrieben und führt die übrigen Familien als noch nicht angesteuert. 493 Tests laufen auf einem Rechner. **Nichts davon lief auf einer echten Maschine**, eine serielle Schnittstelle wird nur für GRBL geöffnet und eine Netzverbindung nur, wenn ein Programm an den Server gesendet wird, und Bögen und die übrigen Codes werden vom Simulator weder gezeichnet noch ausgeführt.

---

> **Sicherheitsstatus:** dieses Programm steuert standardmäßig eine simulierte Maschine. Mit `--grbl` (ein Build mit `--features serial`) öffnet es eine serielle Schnittstelle und steuert eine echte GRBL-Steuerung, nie an einer echten Maschine erprobt; mit einer Serveradresse in den Einstellungen öffnet es eine Netzverbindung zu diesem Server und sendet den Token im Klartext über HTTP. Wenn eine echte Steuerung angeschlossen wird, muss sie sich erst an einem Simulator und dann an einer echten Maschine bewähren, bevor sie als funktionierend gilt; die Endschalter und ein Not-Halt bleiben Aufgabe der eigenen Steuereinheit der Maschine, der Not-Halt des Fensters ist kein Sicherheitskreis.

## 📋 Auf einen Blick

| | |
|---|---|
| Version | `0.1.6` |
| Reifegrad | Funktionsfähig |
| Tests | 493 |
| Technologie-Stack | Rust |
| Auf echter Hardware | Nein: standardmäßig steuert es eine simulierte Maschine; eine echte GRBL-Steuerung (`--grbl`) wurde nie erprobt |

**Springe zu:** [Überblick](#-überblick) · [Architektur](#-architektur) · [Schnellstart](#-schnellstart) · [Tests](#-tests) · [Struktur](#-struktur-des-repositorys) · [Verwandte](#-verwandte-projekte)

## 🚦 Projektstatus

Jede Zeile ist ein Teil dieses Repositorys, und die Ampel zeigt, wie weit er heute ist. Sie beruht auf dem, was das Repository zeigen kann (Tests, Builds, Läufe), nicht auf Plänen.

🟢 Fertig und überprüft · 🟡 Funktioniert, mit bekannten Grenzen · 🟠 Teilweise fertig oder noch nicht überprüft · 🔴 Nicht gemacht oder funktioniert nicht

| Bereich | Stand | Was das hier bedeutet |
|---|---|---|
| Basis-Bibliothek | 🟡 | Ein Rust-Crate, das `scene`, `texts` und `theme` aus `eh3d-view` unter dem Namen dieses Programms erneut exportiert; es braucht den Ordner von EH3D-COMMON daneben. |
| Szene und Ansichten | 🟡 | In EH3D-COMMON geschrieben und getestet; das Fenster dieses Programms zeichnet die Bahn eines Programms auf den Tisch und die Position des Werkzeugs (auf einem Rechner gesehen). |
| Themen und Texte | 🟡 | Elf Themen und die Wörter der Oberfläche in 37 Sprachen, im Fenster dieses Programms gesehen. |
| Tests | 🟡 | 493 Tests bestehen auf einem Rechner (`cargo test --features gui`): die Sitzung (Jog, Nullpunkt, Referenz, ein Programm, Pause, Alarm, Not-Halt), die Bahn eines Programms und die Befehlszeile; die simulierte Steuerung wird in EH3D-COMMON getestet. |
| Fenster und Jog | 🟡 | Läuft auf einem Rechner mit der simulierten Steuerung: Position, Jog-Feld, Schritte, Vorschub, Referenz, Nullpunkt, Entsperren, eine Befehlszeile und ein Protokoll; keine echte Maschine. |
| Maschinensteuerung | 🟠 | Nur der Simulator: ein Programm läuft Zeile für Zeile mit Pause, Stopp und Not-Halt; von hier ist noch keine echte Steuerung angeschlossen. |
| Dokumentation | 🟡 | README in acht Sprachen, die beiden Bilder, drei Beispielprogramme und das Changelog. |

## 🎯 Überblick

* Das **Fenster** (`src/gui.rs`, `phoenix-cnc`): die Position der Maschine (Werkstück und Maschine), ein Jog-Feld mit Schritten und Vorschub, Referenz, Nullpunkt und Entsperren, der Not-Halt, eine Befehlszeile mit Protokoll und das Programm mit seiner Bahn auf dem Tisch.
* Die **Sitzung** (`src/session.rs`): was das Programm mit jeder Steuerung von `eh3d-controller` tut: Jog, Nullpunkt, Referenz, eine Zeile senden, ein Programm Zeile für Zeile ausführen, Pause, Stopp und die Alarme; sie wird an der simulierten Steuerung getestet.
* Die **Bahn eines Programms** (`src/gcode.rs`): die geraden Bewegungen (`G0`, `G1`, absolut oder relativ, in mm oder Zoll), vor dem Lauf gezeichnet; Bögen werden nicht gezeichnet.
* Die **simulierte Steuerung** (`eh3d_controller::sim`, in EH3D-COMMON): eine Maschine im Speicher mit Verfahrgrenzen, die einen Alarm auslösen; die Steuerungsfamilien des Marktes sind in `eh3d-controller` aufgeführt, GRBL ist die erste geschriebene und die anderen erscheinen als noch nicht angesteuert.
* Die **Themen und Wörter** von `eh3d-view`: Forge (Standard), Neon Arcade, Glass Pro, das klassische Windows-Grau und eines, das Windows folgt, und die Oberfläche auf Englisch, Spanisch, Italienisch, Deutsch, Französisch, Portugiesisch, Chinesisch und Japanisch. Was es werden soll: ein Programm zum Bohren, Fräsen und für 2D- und 3D-CNC auf den Engines von EH3D-DRILLPRO und EH3D-ROUTEPRO; dieser Teil existiert noch nicht.
* Die **Menüs, die Einstellungen und die Dateien**: die Menüs Datei, Ansicht, Programm, Maschine und Hilfe mit dem, was PlanetCNC darin hat (öffnen, zuletzt geöffnet, speichern, bearbeiten, die Nullpunkte, die gespeicherten Positionen, die Spindel und das Kühlmittel); ein Einstellungsfenster (Einheiten, Sprache, elf Themen, Verfahrgrenzen, Jog), das zwischen den Läufen gemerkt wird; und die Fenster Öffnen und Speichern der Suite (`eh3d-filedialog`: Orte, Sortierung, Filter, die Endung einer gespeicherten Datei und die Frage vor dem Ersetzen).
* **GRBL an einer seriellen Schnittstelle:** mit `--features serial` gebaut, steuert `phoenix-cnc --grbl PORT[@BAUD]` einen GRBL-Controller (standardmäßig 115200 Baud) statt des Simulators, und `phoenix-cnc --ports` listet die seriellen Ports; ein Build ohne das Feature oder ein Port, der sich nicht öffnet, sagt es und beendet sich. Nur gegen einen skriptgesteuerten Controller erprobt, nie gegen einen echten. Mit einer echten Steuerung wird `--start` (die Datei öffnen und beim Öffnen des Fensters starten) abgelehnt, solange nicht auch `--start-real` angegeben ist; `--start-real` allein ist ein Fehler. `busy_wait` in der Einstellungsdatei (standardmäßig 4 Stunden) ist die längste Wartezeit einer Zeile auf ihre Antwort, solange die Maschine meldet, dass sie arbeitet; der Stopp wird dabei gehört.
* **Ein Fenster, das dem Bildschirm folgt:** es öffnet sich maximiert, und von 720p über 1080p bis zu größeren und breiteren Bildschirmen wächst oder schrumpft alles mit.
* **Weitermachen nach einem Ausfall:** Während ein Programm läuft, wird die erreichte Zeile in einer kleinen Datei gespeichert; fällt das Fenster, der Rechner oder die Verbindung aus, fragt der nächste Start, ob ab dieser Zeile weitergemacht oder verworfen wird. Bis der Bediener antwortet, wird nichts an die Steuerung gesendet.
* **An den EH3D-Server gesendet:** Mit der Adresse des Servers, dem Token dieses Programms und der Kennung der Maschine in den Einstellungen (Abschnitt Server des Einstellungsfensters oder `server_address`, `server_token` und `server_machine` in der Einstellungsdatei) sendet **Programm > An den Server senden** den Text des Programms als G-Code-Auftrag; der Server führt ihn auf seiner Maschine aus (ein GRBL-Controller an seiner seriellen Schnittstelle oder sein Simulator) und das Fenster verfolgt ihn: Zustand, Prozent und erreichte Zeile in der Kopfleiste, im Panel und im Protokoll. Start, Pause und Stopp des Fensters erreichen den Auftrag auf dem Server, das Schließen des Fensters stoppt ihn nicht, und ein angehaltener Auftrag wird ab der Zeile fortgesetzt, die der Server erreicht hat (der nächste Start des Fensters bietet es an, wie bei einem Programm des Controllers): der Server gibt die geltenden Codes zurück und wartet vor der ersten Bewegung auf den Bediener. Spindel und Kühlmittel werden nur mit `server_restore_outputs = yes` wieder gestartet. Gegen einen Stellvertreter-Server geprüft; die eigenen Tests des Servers senden dieselben Anfragen an seinen Simulator. Nie an einer Maschine.
* **Fortsetzen nach einem Fehler:** Ein halb gelaufenes Programm wird zuerst in seinen Zustand zurückgesetzt: Einheiten und absolute Maße, ein sicheres Z (`resume_safe_z` in der Einstellungsdatei, 5 mm), die Spindel mit der Zeit bis zur Drehzahl (`resume_spindle_wait`, 3 s), das Kühlmittel, der erreichte Punkt und der Modalzustand; die Wiederherstellungsdatei enthält den SHA-256 des Programms und setzt nicht fort, wenn sich das Programm geändert hat. Die Zeile, ab der es weitergeht, liegt `resume_backoff` Zeilen (standardmäßig 16) vor der zuletzt gesendeten, weil die Steuerung einige Zeilen in ihrem Puffer hält; ein Eintauchen auf die Tiefe ohne Vorschub des Programms nutzt `resume_plunge_feed` (standardmäßig 100 mm/min).
* **Zeilen für die Steuerung selbst werden standardmäßig nicht gesendet:** eine Programmzeile, die mit `$` beginnt (eine Einstellung oder ein Befehl von GRBL, etwa `$130=9999`, `$X` oder `$RST=*`), hält das Programm an dieser Zeile an, ohne sie zu senden; mit `program_system_commands = yes` in der Einstellungsdatei werden sie gesendet.
* **Die Hilfen des Fensters:** eine **Zusammenfassung vor dem Start eines Programms** (die Bewegungen, der Schnittweg, die Zeit bei den Geschwindigkeiten der Einstellungen, die Reichweite der Achsen und eine Prüfung gegen den Verfahrweg der Maschine, mit den Warnungen, die sich die Suite teilt: ein winziges oder weit von der Ecke entferntes Programm, größer als der Tisch und Schnitte vor jedem Vorschub; die Bögen werden entlang ihrer Kurve gemessen und jeder Werkzeugwechsel addiert seine Zeit; **Trotzdem starten**, oder eine Ablehnung, wenn das Programm den Verfahrweg verlässt, die `reject_out_of_travel = no` zur Warnung macht, und `confirm_start = no` schaltet die Zusammenfassung ab); **Tasten der Tastatur** (Strg+O öffnen, Strg+S speichern, Strg+E bearbeiten, F5 Start, Leertaste Pause, Esc Stopp (bei offenem Fenster schließt Esc es, und mit der Tastatur in einem Textfeld bleiben Leertaste und Esc dem Feld), F1 listet sie auf; `shortcuts = no` schaltet sie ab); eine **Werkzeugleiste** mit gezeichneten Bildern und ihren Namen; Zahlen, die mit Minus und Plus gestuft oder **eingetippt** werden; die Bahn **gezogen und gezoomt von oben oder gedreht und gezoomt in 3D** mit dem Werkzeug; eine **Vorschau der G-Code-Datei** im Öffnen-Fenster; die **Aufzeichnung eines Absturzes** (`eh3d-crash.log`); und ein Abschnitt Sicherheit in den Einstellungen (`rapid`, `summary_tiny`, `summary_far` und `summary_margin` in der Einstellungsdatei).
* **Eine Warteschlange von Programmen und der Verfahrweg der Steuerung:** mehrere im Öffnen-Fenster mit Strg+Klick gewählte, zusammen abgelegte oder in der Befehlszeile genannte Dateien (`phoenix-cnc a.nc b.nc`) laufen nacheinander, jede nach ihrer eigenen Zusammenfassung; das Fenster listet sie mit ihrem Zustand auf, erlaubt das Entfernen, Überspringen und Umordnen der wartenden, und die Warteschlange hält an, wenn ein Programm nicht fertig wird. Bei einer GRBL-Steuerung liest das Programm beim Verbinden `$130`, `$131` und `$132` (den größten Verfahrweg von X, Y und Z) und nimmt für Zusammenfassung und Ablehnung den kleineren Wert aus diesem und `travel_x`, `travel_y` und `travel_z`; antwortet die Steuerung nicht oder liefert sie die Werte nicht, gelten die Einstellungen und das Protokoll sagt es (`read_controller_travel = no` schaltet das Lesen ab).
* **Prüfungen des Programms, weiche Grenzen des Jog und der Taster:** die Zusammenfassung sagt auch, was im Programm falsch aussieht (ein Eilgang, der unter der Oberfläche endet, ein Schnitt bei stehender Spindel, eine ohne Drehzahl gestartete Spindel, ein Werkzeugwechsel, ein Programm ohne Ende sowie ein Vorschub oder eine Spindeldrehzahl über `lint_max_feed` oder `lint_max_rpm`, die standardmäßig 5000 mm/min und 24000 U/min betragen und bei 0 aus sind; `lint = no` schaltet die Prüfungen aus und `lint_rapid_floor` legt das Z fest, unter dem ein Eilgang gemeldet wird). Ein Jog-Schritt, der den Verfahrweg überschreiten würde, wird bis zur Grenze gekürzt, und einer von der Grenze nach außen wird abgelehnt (`jog_soft_limits` ist standardmäßig `homing`: die Grenze gilt erst nach dem Referenzieren; `always` wendet sie von Anfang an an und `no` schaltet sie aus). **Maschine > Z antasten** führt den Zyklus auf einer Tastplatte aus (`probe_feed`, `probe_depth`, `probe_retract`, `probe_plate` und `probe_clearance` in der Einstellungsdatei) und **Maschine > Raster antasten** tastet ein Raster über den Schnitten des Programms ab (`probe_grid` mm Abstand), um eine Höhenkarte zu bauen; **Programm mit der Höhenkarte korrigieren** teilt jede Schnittbewegung in Stücke von höchstens `heightmap_segment` mm und hebt oder senkt jedes um die Höhe darunter (ein Programm in absoluten Millimetern ohne Bögen; eines, das den abgetasteten Bereich verlässt, wird abgelehnt), und **Höhenkorrektur rückgängig machen** stellt das Programm wieder her. Beide Antastungen fragen vorher, weil sich die Maschine bewegt, und eine Steuerung ohne Taster lehnt sie ab. Nur gegen Attrappen erprobt; nichts lief auf einer echten Maschine.
* **Die Programmwerkzeuge, im Fenster und auf der Befehlszeile:** **Programm > Programm prüfen...** listet zeilenweise auf, was die Prüfungen finden (die Nummer neben jedem Fund zeigt diese Zeile im Programmfeld), **Programm > Sperrboxen...** bearbeitet die Stellen, an die das Werkzeug nicht fahren darf (ein Programm, das in eine hineinfährt, wird nicht gestartet, außer mit `keepout_reject = no`), und **Programm > Statistik und Simulation...** spielt die Bahn auf dem Papier ab, mit dem Cursor in der Zeile, während das Programmfeld die Restzeit nennt. Die Warteschlange wiederholt ein Programm (ein Feld mit der Zahl der Durchläufe), fragt den Bediener oder wartet `queue_pause` Sekunden zwischen zwei Durchläufen, zeigt ihre Gesamtzeit und wird zwischen den Läufen in einer Datei gehalten (sie kommt wartend zurück: nichts startet von selbst). Das Menü **Makros** führt nach Rückfrage die Makros aus `phoenix-cnc-macros.txt` (neben der Einstellungsdatei) aus; **Maschine > Höhenkarte speichern...** und **Maschine > Höhenkarte laden...** halten die Karte in einer Datei (nie ohne Rückfrage überschrieben), und **Datei > Sitzungsprotokoll exportieren...** speichert das Protokoll als Text, CSV oder JSON (nie über eine Datei). Ihre Werte (`progcheck`, `keepout`, `keepout_margin`, `keepout_reject`, `queue_repeat_max`, `queue_confirm_between`, `queue_pause`, `queue_remember`, `sim_speed`, `macros_confirm`, `macros_max_lines`, `log_max_entries` und `tool_change_time`) stehen in der Einstellungsdatei, und das Fenster behält sie, wenn es die übrigen Einstellungen speichert. `phoenix-cnc --report DATEI` gibt dieselben Funde mit Statistik und Zeit eines Programms aus, ohne das Fenster zu öffnen (Exitcode 0, wenn nichts zu lesen ist, 1, wenn etwas zu lesen ist, 2, wenn die Datei nicht gelesen werden kann; `--lang` wählt die Sprache), und `--version` und `--help` antworten sofort. Nur am Simulator erprobt; nichts lief auf einer echten Maschine.
* **Das Einstellungsfenster hat drei Gruppen für die Programmwerkzeuge:** **Prüfungen** (die erste und die zweite Prüfung, der Abstand und die Ablehnung der Sperrzonen, die Untergrenze der Eilgangbewegungen sowie der höchste Vorschub und die höchste Spindeldrehzahl), **Taster und Höhenkarte** (der Taster, das Raster, die Länge der Stücke eines korrigierten Schnitts und die Grenzen des Handfahrens) und **Warteschlange, Makros und Protokoll** (die Wiederholungen und die Pause der Warteschlange, die Geschwindigkeit der Simulation, die Einträge des Sitzungsprotokolls, die Makros und die Zeit eines Werkzeugwechsels). Jede Zahl bleibt im Bereich, den die Einstellungsdatei annimmt (der Bereich steht im Tooltip), **Standardwerte dieser Gruppe** setzt eine Gruppe zurück, und beim Speichern bleiben alle anderen Schlüssel der Datei erhalten.
* **Ein Arbeitsbericht als eine HTML-Seite:** **Datei > Arbeitsbericht exportieren...** (oder `phoenix-cnc --report DATEI.NC --html BERICHT.html`) schreibt eine Seite, die alles in sich trägt, ohne Skript, ohne Links und ohne getrennte Bilder: die Statistik und die Zeit des Programms, die Werkzeuge, was auffällig ist mit seiner Zeile, die Bahn von oben mit umkreisten Zeilen, die eine Warnung haben, und die Höhenkarte als Farbraster, wenn eine gemessen wurde. Sie folgt der Sprache, die Farben folgen der hellen oder dunklen Einstellung des Browsers, und eine vorhandene Datei wird nie überschrieben.
* **Die Bahn lässt sich einfärben, nach Werkzeug filtern und anklicken:** unter dem Titel der Bahn (im Plan und in 3D) wählt eine Zeile, wie eingefärbt wird (**erledigt / offen**, **Art der Bewegung**: Eilgang, Schnitt, Eintauchen und Bogen; **Fortschritt des Programms**, von Blau nach Rot entlang der Strecke; **Werkzeug**; oder **Vorschub**, vom langsamsten zum schnellsten), zeigt ein Kästchen für jedes Werkzeug des Programms (ein Werkzeug wird mit seinem Kästchen aus- oder eingeblendet, das letzte sichtbare bleibt) und bietet **Auf die Auswahl zoomen** für den Plan und für 3D (in 3D rückt die Bewegung in die Mitte der Ansicht, mit der jeweiligen Drehung). Ein Klick auf eine Bewegung wählt sie aus (sie wird dick gezeichnet), das Programmfeld zeigt ihre Zeile, und die Zeile nennt die Zeilennummer, ihren Code, die Art der Bewegung, ihre Länge, ihren Vorschub und ihr Werkzeug. Plan und 3D-Ansicht behalten Ziehen, Rad und Doppelklick. Für Bilder starten `--color MODUS` (`state`, `kind`, `progress`, `tool` oder `feed`) und `--select ZEILE` (eine Zeile des Programms, ab 1) das Fenster mit dieser Einfärbung und dieser ausgewählten Bewegung, zusammen mit `--shot`.
* **Ein Editor für das Programm:** der Programmeditor (Strg+E) zeigt den Text mit den **Zeilennummern** am Rand (Zeilen, zu denen die Prüfung etwas zu sagen hat, sind eingefärbt), **Suchen und Ersetzen** (mit oder ohne Groß-/Kleinschreibung; Enter sucht das nächste; eines oder alle ersetzen, die Anzahl wird genannt), **Gehe zu Zeile**, die **Hinweise der Prüfung für den Text, wie er gerade geschrieben ist** (ein Klick auf die Nummer eines Hinweises springt zu seiner Zeile) und **Anwenden**, das den Text ins Programm übernimmt (nicht während ein Programm läuft). **Anwenden und speichern** fragt im Editor selbst, mit Ersetzen und Abbrechen, bevor es die Datei überschreibt, aus der das Programm kam (die Frage verschwindet, wenn sich der Text ändert), und **Anwenden und speichern unter ...** läuft über das Speichern-Fenster, das vor dem Ersetzen einer vorhandenen Datei fragt. Die Regeln (`src/editor.rs`) werden ohne Fenster getestet.
* **Maschinenprofile:** im Abschnitt Allgemein der Einstellungen lässt sich eine Maschine als Profil speichern (Verfahrweg der drei Achsen, Spindeldrehzahl und ihre Warngrenze, Vorschub des Jog, Eilgangsgeschwindigkeit und Warngrenze des Vorschubs sowie die Taster-Werte) und mit einem Klick wieder übernehmen; OK behält die Wahl, Abbrechen lässt die Einstellungen, wie sie waren. Die Einstellungsdatei enthält je Profil eine Zeile `profile = NAME | tx=0,500 | ty=0,300 | tz=-100,50 | spindle=10000 | ...` (bis zu 16, Namen bis 40 Zeichen, ein ungültiger Wert behält den Standard) und `profile_active = NAME` für das benutzte. `phoenix-cnc --profile NAME` startet das Fenster (oder `--report`) mit den Werten dieses Profils; ein nicht gespeicherter Name wird im Protokoll gemeldet (bei `--report` abgelehnt) und die Einstellungen bleiben wie sie sind. Die Regeln (`src/profiles.rs`) werden ohne Fenster getestet.
* **Programm transformieren:** **Programm > Programm transformieren ...** verschiebt (X, Y, Z), dreht (Grad, um einen Punkt), skaliert und spiegelt (X, Y) das geladene Programm, und **Programm > Transformation rückgängig machen** stellt es wieder her. Die Zeilen werden an Ort und Stelle umgeschrieben (unveränderte Wörter, Kommentare und Abstände bleiben, wie sie waren), Bögen behalten ihre Mittelpunkte, und eine ungerade Spiegelung macht aus einem Uhrzeigersinn-Bogen einen Gegenuhrzeigersinn-Bogen (die Codes `G2` und `G3` werden getauscht). Ein Programm, das die Änderung falsch machen könnte, wird mit Zeile und Grund abgelehnt: relative Maße (`G91`), eine andere Ebene als XY, Bohrzyklen und Tastbewegungen, Codes mit Maschinenpositionen oder Nullpunktverschiebung (`G10`, `G28`, `G30`, `G53`, `G92`, `G43.1`), absolute Bogenmittelpunkte (`G90.1`) oder ein Maßstab, der nicht über 0 liegt. Das Programm beginnt, wie bei der Bahn angenommen, im Ursprung. Ein laufendes Programm wird nie geändert. `--window transform` öffnet das Fenster. Lässt die erste Bewegung des Programms X oder Y weg, warnt das Fenster und fragt, wo das Werkzeug vor dem Programm steht (standardmäßig der Ursprung); die weggelassene Koordinate wird von dort genommen und geschrieben, sobald die Änderung sie verschiebt. Die Regeln (`src/transform.rs`) werden ohne Fenster getestet, auch dass die Bahn des neuen Programms die transformierte alte Bahn ist.
* **Lesezeichen und Start ab einer Zeile:** Eine Zeile im Programmfeld anklicken wählt sie aus, ein Doppelklick setzt ein Lesezeichen (eine markierte Zeile hat einen Stern hinter der Nummer); oder **Programm > Ausgewählte Zeile mit Lesezeichen versehen**, **Nächstes Lesezeichen** und **Vorheriges Lesezeichen**. **Programm > Lesezeichen und ab einer Zeile starten ...** (`--window bookmarks`) listet die Lesezeichen, springt zu einem und startet von dort oder von der ausgewählten Zeile. Bevor etwas vorbereitet wird, liest das Fenster die Zeilen vor dem Start und sagt, was das Programm dort eingestellt hatte: Zoll, eine andere Ebene als XY, die Werkstückkoordinaten, die Werkzeuglängenkorrektur, das zuletzt verlangte Werkzeug (es wird kein Werkzeugwechsel gemacht), die Spindel mit Drehzahl, das Kühlmittel, einen verschobenen Nullpunkt und eine Schnittbewegung vor jedem Vorschub; ein Start bei relativen Maßen (`G91`), einem noch aktiven Bearbeitungszyklus oder einer noch aktiven Fräserradiuskorrektur wird abgelehnt. **Start vorbereiten** verlangt ein Häkchen, dass Werkzeug, Spindel und Nullpunkt geprüft wurden (`startline_confirm = no` in der Einstellungsdatei oder das Kästchen im Abschnitt Warteschlange, Makros und Protokoll schaltet es ab), bringt die Steuerung wie bei einem mittendrin verlassenen Programm in diesen Zustand (auf die sichere Höhe, Spindel mit Wartezeit, über den Punkt des Programms) und lässt das Programm in der Zeile angehalten: Die Maschine bewegt sich erst bei Fortsetzen, und ein laufendes Programm wird nie angefasst. Lesezeichen gehören zum geladenen Programm. Die Regeln (`src/bookmarks.rs`, `src/startline.rs`) werden ohne Fenster getestet.
* **Ein Werkzeugwechsler mit Verriegelung und Ausblasen, in der Simulation:** Eine Zeile `atc = on | safe=5 | rack=0,0,-5 | pitch=20,0 | lock=3,0.5 | blow=4,1` in der Einstellungsdatei (aus, solange nicht `on` steht) beschreibt einen Wechsler: das Magazin (X, Y, Z des Platzes von Werkzeug 1 und der Schritt zum nächsten), die sichere Höhe, den Ausgangspin und die Wartezeit der Verriegelung sowie den Ausgangspin und die Sekunden des Ausblasens. Für jeden Wechsel in einem Programm werden die Schritte (Spindel anhalten, altes Werkzeug ablegen, Verriegelung lösen, leere Spindel ausblasen, neues Werkzeug nehmen, Verriegelung schließen), die G-Code-Zeilen (`M65 P` / `M64 P`, `G4`) und die Zeit berechnet; `--report` und das Statistik-Fenster listen die Wechsel auf, und die Schätzung nutzt diese Zeit statt `tool_change_time`. Es wird nichts an eine Maschine gesendet. Der Platz jedes Werkzeugs kann statt aus dem Abstand aus einer Tabelle kommen: `phoenix-cnc-atc.txt` neben der Einstellungsdatei enthält für jedes Werkzeug des Magazins eine Zeile `tool = NUMMER | pos=X,Y` (`examples/atc.txt` ist ein Muster; bis zu 64 Werkzeuge mit den Nummern 1 bis 255, Plätze mindestens 1 mm voneinander entfernt, Datei bis 16 KiB); ein Werkzeug, das nicht darin steht, behält Magazin und Abstand, und jede ausgelassene Zeile wird mit Nummer und Grund in `--report` und im Statistikfenster genannt. Die Regeln (`src/atc.rs`) werden ohne Fenster getestet.
* **Eine Materialbibliothek für die Prüfungen:** Der Abschnitt Prüfungen der Einstellungen hat eine Liste **Material** (`material = NAME` in der Einstellungsdatei); ein Programm wird dann am empfohlenen Bereich für Schnittvorschub und Spindeldrehzahl dieses Materials gemessen, und die Prüfung meldet mit Zeile und in den acht Sprachen, wenn eine Bewegung in X oder Y unter oder über dem Vorschubbereich läuft (ein Eintauchen allein in Z zählt nicht) oder eine Spindeldrehzahl unter oder über dem Drehzahlbereich liegt. Die Werte stehen in einer Textdatei, nicht im Programm: `examples/materials.txt` ist eingebaut und gilt, solange es keine eigene Datei gibt, und `phoenix-cnc-materials.txt` neben der Einstellungsdatei ersetzt sie (je Material eine Zeile `material = NAME | feed=VON,BIS | rpm=VON,BIS`, bis zu 64, jeder Bereich darf fehlen). `phoenix-cnc --report` nutzt das Material der Einstellungen ebenfalls. Die Bereiche sind nur Startvorschläge: Sie warnen und halten ein Programm nie an. Die Regeln (`src/materials.rs`) werden ohne Fenster getestet.
* **Feste Ansichten des Viewers:** **Ansicht > Ansicht von oben, Vorderansicht, Seitenansicht** und **Isometrische Ansicht** (oder **Strg+1** bis **Strg+4**) zeigen die Bahn in dieser Ansicht: Die Ansicht von oben ist der Plan, Vorder- und Seitenansicht blicken längs einer Achse auf Tischhöhe, und die isometrische ist die Startansicht des Fensters. `--view top|front|side|iso` (neben `plan` und `3d`) startet das Fenster in einer davon, passend zu `--shot`.
* **Tab vervollständigt den Code im Befehlsfeld:** solange das Feld unten im Fenster die Tastatur hat, vervollständigt Tab den Code, der gerade getippt wird (`G38` ergibt `G38.`, `M30` ergibt `M30 `, `$J` ergibt `$J=`) oder schreibt, wenn mehrere passen, was sie gemeinsam haben, und listet sie neben dem Feld auf (`G5` listet `G53` bis `G59`). Es wirkt auf das letzte Wort der Zeile, unterscheidet nicht zwischen Groß- und Kleinschreibung und sendet nichts: Die Zeile geht weiter mit der Eingabetaste ab.

## 🧩 Rolle in EH3D

EH3D-PHOENIX-CNC ist das Maschinensteuerungsprogramm von EH3D, heute auf einer simulierten Maschine. Es hängt per Pfad von EH3D-COMMON ab (der Ordner `EH3D-COMMON` muss neben diesem liegen: die Ansicht, die Themen, die Wörter und die Steuerungen) und noch von keinem anderen EH3D-Repository.

## 🏛️ Architektur

<p align="center">
  <img src="images/architecture.svg" alt="Architektur von EH3D-PHOENIX-CNC: wer es nutzen wird, was es enthält und worauf es sich stützt" width="100%">
</p>

Wie die Teile zusammenpassen: wer es nutzen wird, was dieses Repository enthält und worauf es sich stützt. Das Bild wird aus `docs/architecture.json` gezeichnet, gegen den Code geprüft und kennzeichnet Künftiges als künftig.

## 🖼️ Screenshots

<p align="center">
  <img src="images/screens/phoenix-cnc-01-running.png" alt="Ein Programm läuft auf der simulierten Maschine" width="100%">
</p>

<p align="center"><em>Ein Programm läuft auf der simulierten Maschine: die erledigten Zeilen sind grün, die aktuelle Zeile hell, die Bahn ist auf den Tisch gezeichnet und das Werkzeug steht, wo die Maschine ist.</em></p>

<p align="center">
  <img src="images/screens/phoenix-cnc-02-midnight.png" alt="Thema Midnight mit einem beendeten Programm" width="100%">
</p>

<p align="center"><em>Das Thema Mitternacht mit einem langen, beendeten Programm.</em></p>

<p align="center">
  <img src="images/screens/phoenix-cnc-03-german-light.png" alt="Deutsche Oberfläche mit dem hellen Thema Paper" width="100%">
</p>

<p align="center"><em>Die Oberfläche auf Deutsch mit dem hellen Thema Papier: jedes Wort des Fensters folgt der Sprache.</em></p>

<p align="center">
  <img src="images/screens/phoenix-cnc-04-settings.png" alt="Einstellungsfenster" width="100%">
</p>

<p align="center"><em>Das Einstellungsfenster, mit den Abschnitten links wie der Baum von PlanetCNC: die Sprache, die elf Themen und was gezeigt wird.</em></p>

<p align="center">
  <img src="images/screens/phoenix-cnc-05-open.png" alt="Fenster Öffnen mit Orten, zuletzt verwendeten Dateien und der Ordnerliste" width="100%">
</p>

<p align="center"><em>Das Fenster Öffnen der Suite: Orte, zuletzt geöffnet, die Liste des Ordners mit Datum und Größe, der Name und der Typ.</em></p>

## 🔒 Sicherheit und Schutz

Dieses Programm öffnet eine serielle Schnittstelle nur, wenn es für GRBL gestartet wird (`--grbl`, ein Build mit `--features serial`), und eine Netzverbindung nur zum EH3D-Server, dessen Adresse in den Einstellungen steht, wenn der Bediener ihm ein Programm sendet; das Token liegt als Text in der Einstellungsdatei. Es öffnet keine Dateien anderer Personen außer der G-Code-Datei und der Höhenkarte, die der Bediener wählt (als Text gelesen, nie von selbst ausgeführt), und seinen eigenen Dateien neben den Einstellungen (die Makros, die zwischen den Läufen gehaltene Warteschlange, die Zeile, die ein Programm erreicht hatte); es schreibt eine Datei nur dort, wo der Bediener es sagt (das Programm, die Höhenkarte, das exportierte Protokoll), und schreibt das Protokoll nie über eine Datei. Die Makros, die es ausführt, sind G-Code-Zeilen, nach denen es vorher fragt, und die zurückkehrende Warteschlange startet nie von selbst. Es ist kein Sicherheitssystem. Siehe [SECURITY.md](SECURITY.md).

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

`cargo test --features gui` führt 493 Tests aus: die Sitzung (Jog und Nullpunkt, ein Programm Zeile für Zeile, Pause und Weiter, eine abgelehnte Zeile, der Not-Halt und der Alarm, das Protokoll der Befehle, die Referenz), die Bahn eines Programms und die Befehlszeile; die Ansicht und die simulierte Steuerung werden in EH3D-COMMON getestet. Die CI führt außerdem `cargo fmt --check` und `cargo clippy` mit Warnungen als Fehlern aus, und `python -I tools/ci_validate.py` prüft Manifest, Versionen, Changelog und die acht READMEs.

## 🏗️ Build und Veröffentlichung

Die Version steht in `Cargo.toml` und in `eh3d.project.json`, und jede Version hat ihren Eintrag in [CHANGELOG.md](CHANGELOG.md). Es gibt noch kein Programm zu verpacken, also keine fertigen Binärdateien.

## 📂 Struktur des Repositorys

<p align="center">
  <img src="images/repository-structure.svg" alt="Repository-Struktur von EH3D-PHOENIX-CNC" width="100%">
</p>

<details>
<summary>Textversion des Baums</summary>

```text
EH3D-PHOENIX-CNC/
├── src/
│   ├── lib.rs                                   die Module und die Ansicht von eh3d-view unter dem Namen dieses Programms
│   ├── gui.rs                                   das Fenster: Position, Jog, Programm, Bahn, Befehle und Protokoll
│   ├── session.rs                               was mit einer Steuerung getan wird: Jog, Nullpunkt, Referenz, ein Programm Zeile für Zeile, Pause, Stopp, Alarme
│   ├── queue.rs                                 die Warteschlange: Reihenfolge, Zustand, Durchläufe, Stopp bei Fehler, Zustandsdatei
│   ├── progcheck.rs                             zweite Programmprüfung: Einheiten, Versätze, Antasten, Ebenen
│   ├── boxes.rs                                 Sperrboxen und Verfahrgrenzen, Bewegung für Bewegung
│   ├── sim.rs                                   der Pfad in der Zeit: Werkzeug zu jedem Zeitpunkt, Restzeit
│   ├── estimate.rs                              die Zeit eines Programms, einmal gemessen und gemerkt: der geladene Pfad und die Dateien der Warteschlange
│   ├── stats.rs                                 Programmstatistik: Strecken, Zeit, Werkzeugwechsel
│   ├── jobs.rs                                  Wiederholungen und Pausen zwischen Warteschlangenprogrammen
│   ├── macros.rs                                die Makrobibliothek: benannter G-Code, der vor dem Start fragt
│   ├── logbook.rs                               Sitzungsprotokoll als Text, CSV oder JSON, nie über eine Datei
│   ├── report.rs                                der Bericht von --report: Statistik, Zeit, Auffälliges
│   ├── workreport.rs                            der Arbeitsbericht als HTML-Seite: Statistik, Warnungen, Bahn, Höhenkarte
│   ├── tuning.rs                                die Werte dieser Werkzeuge in der Einstellungsdatei
│   ├── ranges.rs                                der Bereich jedes Schlüssels dieser Werkzeuge, für Fenster und Datei
│   ├── words.rs                                 die Meldungen dieser Werkzeuge in acht Sprachen
│   ├── gcode.rs                                 die geraden Bewegungen eines G-Code-Programms, um seine Bahn zu zeichnen
│   ├── heightmap.rs                             die Höhenkarte des Materials und die Korrektur eines Programms
│   ├── jog.rs                                   die weichen Grenzen des Jog: ein Schritt über den Verfahrweg hinaus wird gekürzt oder abgelehnt
│   ├── lint.rs                                  was an einem Programm auffällt: Eilgänge, Spindel, Vorschub, Drehzahl
│   ├── probe.rs                                 der Taster: der Z-Zyklus auf einer Tastplatte und das Raster von Antastungen für die Höhenkarte
│   ├── view.rs                                  der Plan, der gezogen und gezoomt wird, und die gedrehte 3D-Ansicht, als Zahlen
│   ├── viewer.rs                                der Bahnbetrachter: Arten, Werkzeuge, Fortschritt, Farben, Klicks
│   ├── plan.rs                                  die Prüfung vor dem Start: Zeit, Reichweite und Verfahrweg der Bewegungen
│   ├── cli.rs                                   die Befehlszeile: die Datei, die Sprache, das Thema und die Bildoptionen
│   ├── complete.rs                              Tab im Befehlsfeld: vervollständigt die G-, M- und $-Codes
│   ├── editor.rs                                der Programmeditor: Suchen, Ersetzen, Zeile, Hinweise
│   ├── profiles.rs                              Maschinenprofile: Verfahrweg, Spindel, Vorschübe und Taster je Maschine
│   ├── transform.rs                             Programm verschieben, drehen, skalieren, spiegeln, mit Regeln
│   ├── atc.rs                                   der Werkzeugwechsler in der Simulation: Verriegelung, Ausblasen, Schritte, G-Code und Zeit
│   ├── bookmarks.rs                             Lesezeichen auf den Zeilen eines Programms: setzen, nächstes, vorheriges
│   ├── startline.rs                             was ein Programm in einer Zeile eingestellt hat und wann der Start dort abgelehnt wird
│   ├── materials.rs                             die Materialbibliothek: empfohlene Vorschub- und Drehzahlbereiche für die Prüfungen
│   ├── bin/phoenix-cnc.rs                       das Programm, das das Fenster öffnet
│   ├── server.rs                                die Verbindung zum EH3D-Server: senden, verfolgen, anhalten, stoppen
│   ├── settings.rs                              was zwischen Läufen gemerkt wird
│   ├── gui/dialogs.rs                           Menüs, Einstellungen, Öffnen, Speichern
│   ├── gui/prefs.rs                             die Einstellungsgruppen der Werkzeuge: Prüfungen, Taster, Warteschlange
│   ├── gui/remote.rs                            das Programm auf dem Server: Start, Pause, Stopp und Fortsetzen
│   ├── gui/views.rs                             die Fenstermitte: die Bahn von oben oder in 3D, mit dem Werkzeug
│   ├── gui/pathview.rs                          die Zeile des Betrachters: Farben, Ebenen, Zoom, angeklickte Zeile
│   ├── gui/editwin.rs                           das Editorfenster: Zeilennummern, Suchen, Hinweise, Speichern
│   ├── gui/xform.rs                             das Transformationsfenster und Rückgängig im Programm-Menü
│   ├── gui/lines.rs                             das Lesezeichen-Fenster und der Start ab einer Zeile, mit seiner Frage
│   ├── gui/summary.rs                           das Zusammenfassungsfenster vor dem Start und die Tastenliste
│   ├── gui/probe.rs                             der Taster im Fenster: Rückfragen und Höhenkarten-Einträge
│   ├── gui/numfield.rs                          eine Zahl, die mit den Tasten Minus und Plus oder durch Eintippen gesetzt wird
│   ├── gui/keys.rs                              die Tasten der Tastatur und die Tabelle, die Menüs und Tastenliste lesen
│   ├── gui/queue.rs                             die Warteschlange im Fenster: Durchläufe, Wiederholungen, Rückfrage oder Wartezeit dazwischen
│   ├── gui/check.rs                             das Prüffenster: was die Prüfungen finden, Zeile für Zeile
│   ├── gui/keepout.rs                           der Editor der Sperrboxen und die Ablehnung eines Programms, das in eine hineinfährt
│   ├── gui/sim.rs                               Statistik und Simulation: das Werkzeug auf dem Pfad, die Zeile, die Restzeit
│   ├── gui/macros.rs                            das Makro-Menü und die Rückfrage vor dem Ausführen eines Makros
│   ├── gui/files.rs                             die Höhenkarte und das Sitzungsprotokoll als Dateien, über die Dateifenster
│   └── gui/icons.rs                             die Bilder der Schaltflächen, mit Linien gezeichnet, und die Schaltfläche der Werkzeugleiste
├── examples/                                    drei Beispiel-G-Code-Programme, vom Quadrat bis zur langen Spirale
├── tests/                                       weitere Tests zu G-Code und Einstellungen
├── tools/                                       CI-Prüfung, README-Prüfung und die Bild-Werkzeuge
├── docs/                                        die Quellen der Bilder von Architektur und Repository-Struktur
├── .github/                                     CI-Workflow, Vorlagen für Issues und Pull Requests
├── images/                                      Banner, die beiden Bilder und die Screenshots
├── build.sh build.bat                           bauen die Bibliothek
├── run.sh run.bat                               melden, dass es noch kein Programm gibt, und führen die Tests aus
├── eh3d.project.json                            Manifest: Version, Rolle, Reifegrad und Build-Befehl
├── Cargo.toml Cargo.lock                        das Rust-Paket und seine festgelegten Abhängigkeiten
├── rust-toolchain.toml                          die festgelegte Rust-Toolchain
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
