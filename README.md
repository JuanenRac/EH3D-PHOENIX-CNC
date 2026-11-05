<p align="center">
  <img src="images/EH3D_PHOENIX_CNC_BANNER.svg" alt="EH3D-PHOENIX-CNC banner" width="100%">
</p>

# <img src="images/brand/icon-64.png" alt="" width="40" height="40" align="absmiddle"> EH3D-PHOENIX-CNC
<p align="center">🇺🇸 <b>English</b> | <a href="README_spa.md">🇪🇸 Español</a> | <a href="README_fra.md">🇫🇷 Français</a> | <a href="README_ita.md">🇮🇹 Italiano</a> | <a href="README_deu.md">🇩🇪 Deutsch</a> | <a href="README_zho.md">🇨🇳 中文</a> | <a href="README_jpn.md">🇯🇵 日本語</a> | <a href="README_por.md">🇵🇹 Português</a></p>

### The shared base of the EH3D machine-control programs: the view of a job, the themes and the words of the interface in eight languages

<p align="center">
  <img src="https://img.shields.io/badge/License-GPL%203.0--or--later-blue.svg" alt="GPL 3.0 or later">
  <img src="https://img.shields.io/badge/Stack-Rust-DEA584.svg" alt="Rust">
  <img src="https://img.shields.io/badge/Languages-8-00E5FF.svg" alt="Languages">
  <img src="https://img.shields.io/badge/Version-0.1.0-0f6cbd.svg" alt="Version">
  <img src="https://img.shields.io/badge/Maturity-scaffolding-FFB020.svg" alt="Maturity">
  <img src="https://img.shields.io/badge/Tests-126-0f6cbd.svg" alt="Tests">
</p>

---

**Honesty check - what actually runs today:** a Rust program with a window (`phoenix-cnc`) that drives a **simulated** machine: it shows the position of the machine (work and machine), moves it by hand with a jog pad, sets the zero, homes, takes the commands typed by the operator, runs a G-code program line by line (with pause, stop and the emergency stop), draws the path of the program on the table with the lines done marked, and stops at what the controller refuses. The window speaks 37 languages and wears eleven themes. The controller interface (`eh3d-controller`, in EH3D-COMMON) has GRBL written against a scripted line and lists the other families as not driven yet. 126 tests run on a computer. **Nothing here has run on a real machine**, a serial port is opened only for GRBL and a network connection only when a program is sent to the server, and arcs and the other codes are neither drawn nor run by the simulator.

---

> **Safety and security status:** this program drives only a simulated machine: it opens no serial port and no network connection. When a real controller is connected it will have to prove itself on a simulator and then on a real machine before it is called working, and the limit switches and an emergency stop stay the job of the machine's own control unit; the emergency stop of the window is not a safety circuit.

## 📋 At a glance

| | |
|---|---|
| Version | `0.1.0` |
| Maturity | Scaffolding |
| Tests | 126 |
| Stack | Rust |
| On real hardware | No: it drives only a simulated machine |

**Jump to:** [Overview](#-overview) · [Architecture](#-architecture) · [Quick start](#-quick-start) · [Testing](#-testing) · [Structure](#-repository-structure) · [Related](#-related-projects)

## 🚦 Project status

Each line is one part of this repository and the light says how far it is today. It is written from what the repository can show (tests, builds, runs), not from what is planned.

🟢 Done and verified · 🟡 Works, with known limits · 🟠 Partly done or not verified yet · 🔴 Not done or not working

| Area | State | What it means here |
|---|---|---|
| Base library | 🟡 | A Rust crate that re-exports `scene`, `texts` and `theme` of `eh3d-view` under this program's name; it needs the folder of EH3D-COMMON next to it. |
| Scene and views | 🟡 | Written and tested in EH3D-COMMON; the window of this program draws the path of a program on the table and the position of the tool (seen on a computer). |
| Themes and texts | 🟡 | Eleven themes and the words of the interface in 37 languages, seen in the window of this program. |
| Tests | 🟡 | 126 tests pass on a computer (`cargo test --features gui`): the session (jog, zero, home, a program, pause, alarm, emergency stop), the path of a program and the command line; the simulated controller is tested in EH3D-COMMON. |
| Window and jog | 🟡 | Works on a computer with the simulated controller: position, jog pad, steps, feed, home, zero, unlock, a line of commands and a log; no real machine. |
| Machine control | 🟠 | Only the simulator: a program runs line by line with pause, stop and the emergency stop; no real controller is connected from here yet. |
| Documentation | 🟡 | README in eight languages, the two pictures, three sample programs and the changelog. |

## 🎯 Overview

* The **window** (`src/gui.rs`, `phoenix-cnc`): the position of the machine (work and machine), a jog pad with its steps and feed, home, zero and unlock, the emergency stop, a line of commands with its log, and the program with its path on the table.
* The **session** (`src/session.rs`): what the program does with any controller of `eh3d-controller`: jog, zero, home, send a line, run a program line by line, pause, stop and the alarms; it is tested on the simulated controller.
* The **path of a program** (`src/gcode.rs`): the straight moves (`G0`, `G1`, absolute or relative, in mm or inches) drawn before the program runs; arcs are not drawn.
* The **simulated controller** (`eh3d_controller::sim`, in EH3D-COMMON): a machine in memory with travel limits that raise an alarm; the controller families of the market are listed in `eh3d-controller`, GRBL is the first one written and the others show as not driven yet.
* The **themes and the words** of `eh3d-view`: Forge (the default), Neon Arcade, Glass Pro, the classic Windows grey and one that follows Windows, and the interface in English, Spanish, Italian, German, French, Portuguese, Chinese and Japanese. What it will be: one program to drill, to route and to do 2D and 3D CNC work on the engines of EH3D-DRILLPRO and EH3D-ROUTEPRO; that part does not exist yet.
* The **menus, the settings and the files**: the menus File, View, Program, Machine and Help with what PlanetCNC has in them (open, recent files, save, edit, the zeros, the stored positions, the spindle and the coolant); a settings window (units, language, eleven themes, travel limits, jog) that is kept between runs; and the Open and Save windows of the suite (`eh3d-filedialog`: places, sorting, filters, the extension of a saved file and the question before a file is replaced).
* **GRBL on a serial port:** built with `--features serial`, `phoenix-cnc --grbl PORT[@BAUD]` drives a GRBL controller (115200 baud by default) instead of the simulator and `phoenix-cnc --ports` lists the serial ports; a build without the feature, or a port that does not open, says so and leaves. It has been tried only against a scripted controller, never against a real one.
* **A window that follows the screen:** it opens maximized, and from 720p to 1080p and on to larger and wider screens everything grows or shrinks with it.
* **Going on after a failure:** while a program runs, the line it has reached is saved to a small file; if the window, the computer or the link goes down, the next start asks whether to go on from that line or to leave it. Nothing is sent to the controller until the operator answers.
* **Sent to the EH3D server:** with the address of the server, the token of this program and the id of the machine in the settings (section Server of the settings window, or `server_address`, `server_token` and `server_machine` in the settings file), **Program > Send to the server** sends the text of the program as a G-code job; the server runs it on its machine (a GRBL controller on its serial port, or its simulator) and the window follows it: the state, the percentage and the line it has reached in the top bar, the panel and the log. Start, Pause and Stop of the window reach the job on the server, closing the window does not stop it, and a job that stopped is taken up again from the line the server reached (the next start of the window offers it, as it does for a program of the controller): the server gives the codes in force back and waits for the operator before the first move. The spindle and the coolant are started again only with `server_restore_outputs = yes`. Tested against a stand-in server; the server's own tests send the same requests to its simulator. Never run on a machine.
* **Resuming after a failure:** a program left in the middle is first put back in its state: units and absolute distances, a safe Z (`resume_safe_z` in the settings file, 5 mm), the spindle with the time to reach its speed (`resume_spindle_wait`, 3 s), the coolant, the point it had reached and the modal state; the recovery file keeps the SHA-256 of the program and refuses to go on if the program changed.
* **The helpers of the window:** a **summary before a program starts** (the moves, the cutting distance, the time at the speeds of the settings, the reach of the axes and a check against the travel of the machine, with the warnings the suite shares: a program that is tiny or far from the corner, bigger than the table, arcs the estimate leaves out and cuts before any feed; **Start anyway**, or a refusal when the program leaves the travel, which `reject_out_of_travel = no` turns into a warning, and `confirm_start = no` turns the summary off); **keys of the keyboard** (Ctrl+O open, Ctrl+S save, Ctrl+E edit, F5 start, Space pause, Esc stop (with a window open Esc closes it, and with the keyboard in a box of text Space and Esc are left to the box), F1 lists them; `shortcuts = no` turns them off); a **toolbar** with drawn pictures and their names; numbers that are stepped with minus and plus or **written**; the path **dragged and zoomed from above or turned and zoomed in 3D** with the tool; a **picture of the G-code file** in the Open window; the **record of a crash** (`eh3d-crash.log`); and a Safety section in the settings (`rapid`, `summary_tiny`, `summary_far` and `summary_margin` in the settings file).
* **A queue of programs and the travel of the controller:** several files chosen in the Open window with Ctrl+click, dropped together or given on the command line (`phoenix-cnc a.nc b.nc`) run one after the other, each after its own summary; the window lists them with their state, lets you remove, skip and reorder the ones that wait, and the queue stops when a program does not finish. With a GRBL controller the program reads `$130`, `$131` and `$132` (the longest travel of X, Y and Z) when it connects and uses the smaller of that and `travel_x`, `travel_y` and `travel_z` for the summary and the refusal; if the controller does not answer or does not give them, the settings are used and the log says so (`read_controller_travel = no` turns the reading off).

## 🧩 Role in EH3D

EH3D-PHOENIX-CNC is the machine-control program of EH3D, today working on a simulated machine. It depends by path on EH3D-COMMON (the folder `EH3D-COMMON` must sit next to this one: the view, the themes, the words and the controllers) and on no other EH3D repository yet.

## 🏛️ Architecture

<p align="center">
  <img src="images/architecture.svg" alt="EH3D-PHOENIX-CNC architecture: who will use it, what it holds and what it relies on" width="100%">
</p>

How the parts fit together: who will use it, what this repository holds and what it relies on. The picture is drawn from `docs/architecture.json`, is checked against the code and marks what is future as future.

## 🖼️ Screenshots

<p align="center">
  <img src="images/screenshots/program-running.png" alt="a program running" width="100%">
</p>

<p align="center"><em>A program running on the simulated machine: the lines done are green, the line that goes now is bright, the path is drawn on the table and the tool is where the machine is.</em></p>

<p align="center">
  <img src="images/screenshots/spiral-neon.png" alt="Midnight theme" width="100%">
</p>

<p align="center"><em>The Midnight theme, with a long program that has finished.</em></p>

<p align="center">
  <img src="images/screenshots/square-german.png" alt="German interface, light theme" width="100%">
</p>

<p align="center"><em>The interface in German with the light Paper theme: every word of the window follows the language.</em></p>

<p align="center">
  <img src="images/screenshots/settings-window.png" alt="settings window" width="100%">
</p>

<p align="center"><em>The settings window, with the sections on the left like the tree of PlanetCNC: the language, the eleven themes and what is shown.</em></p>

<p align="center">
  <img src="images/screenshots/open-window.png" alt="Open window" width="100%">
</p>

<p align="center"><em>The Open window of the suite: places, recent files, the list of the folder with its dates and sizes, the file name and the type.</em></p>

## 🔒 Security & Safety

This program opens a serial port only when it is started for GRBL (`--grbl`, a build with `--features serial`) and a network connection only to the EH3D server whose address is in its settings, when the operator sends a program to it; the token is kept as text in the settings file. It opens no file of other people except the G-code file the operator chooses (read as text, never run by itself) and it is not a safety system. See [SECURITY.md](SECURITY.md).

## 🚀 Quick start

You need a stable Rust toolchain and the folder `EH3D-COMMON` next to this one.

```bash
cargo build --release --features gui
cargo test --features gui
cargo clippy --all-targets --features gui -- -D warnings
cargo fmt --all --check
python -I tools/ci_validate.py
cargo run --release --features gui -- examples/02_frame_and_pocket.nc
```

`build.bat` / `build.sh` build the program, `test.bat` / `test.sh` run the tests and `run.bat` / `run.sh` open the window on a sample program. The scripts wait for a key at the end unless `NOPAUSE` is set.

## 🧪 Testing

`cargo test --features gui` runs 126 tests: the session (jog and zero, a program run line by line, pause and go on, a refused line, the emergency stop and the alarm, the log of the commands, homing), the path of a program and the command line; the view and the simulated controller are tested in EH3D-COMMON. The CI also runs `cargo fmt --check` and `cargo clippy` with warnings as errors, and `python -I tools/ci_validate.py` checks the manifest, the versions, the changelog and the eight READMEs.

## 🏗️ Build & Release

The version lives in `Cargo.toml` and in `eh3d.project.json`, and each version has its entry in [CHANGELOG.md](CHANGELOG.md). There is no program to package yet, so there are no ready-made binaries.

## 📂 Repository Structure

<p align="center">
  <img src="images/repository-structure.svg" alt="Repository structure of EH3D-PHOENIX-CNC" width="100%">
</p>

<details>
<summary>Text version of the tree</summary>

```text
EH3D-PHOENIX-CNC/
├── src/
│   ├── lib.rs                                   the modules, and the view of eh3d-view re-exported under this program's name
│   ├── gui.rs                                   the window: position, jog, program, path, commands and log
│   ├── session.rs                               what is done with a controller: jog, zero, home, a program, pause, stop, alarms
│   ├── queue.rs                                 the queue of programs: order, state, skip, remove, reorder, stop on failure
│   ├── gcode.rs                                 the straight moves of a G-code program, to draw its path
│   ├── view.rs                                  the plan that is dragged and zoomed and the 3D view that is turned, as numbers
│   ├── plan.rs                                  the check before a start: time, reach and travel of the moves
│   ├── cli.rs                                   the command line: the file, the language, the theme and the picture options
│   ├── bin/phoenix-cnc.rs                       the program that opens the window
│   ├── server.rs                                the link to the EH3D server: send, follow, hold, stop
│   ├── settings.rs                              what is kept between runs
│   ├── gui/dialogs.rs                           menus, settings, Open, Save, editor
│   ├── gui/remote.rs                            the program on the server: Start, Pause, Stop and going on
│   ├── gui/views.rs                             the centre of the window: the path from above or in 3D, with the tool
│   ├── gui/summary.rs                           the summary window before a start and the list of keys
│   ├── gui/numfield.rs                          a number set with the minus and plus buttons or by writing it
│   ├── gui/keys.rs                              the keys of the keyboard and the table the menus and the list of keys read
│   ├── gui/queue.rs                             the queue in the window: files run one after the other, each after its summary
│   └── gui/icons.rs                             the pictures of the buttons, drawn with lines, and the toolbar button
├── examples/                                    three sample G-code programs, from a square to a long spiral
├── tests/                                       more tests of the G-code and the settings
├── tools/                                       CI check, README check and the picture tools
├── docs/                                        the sources of the Architecture and Repository Structure pictures
├── .github/                                     CI workflow, issue and pull-request templates
├── images/                                      banner, the two pictures and the screenshots
├── build.sh build.bat                           build the library
├── run.sh run.bat                               say that there is no program yet and run the tests
├── eh3d.project.json                            manifest: version, role, maturity and build command
├── Cargo.toml Cargo.lock                        the Rust package and its locked dependencies
├── CHANGELOG.md                                 what changed in each version
├── SECURITY.md SUPPORT.md                       how to report a vulnerability and where to ask
└── CONTRIBUTING.md CODE_OF_CONDUCT.md LICENSE   how to contribute, the code of conduct and the licence
```

</details>

## 🗺️ Roadmap & Known limits

**Known limits today**

* Nothing here has run on a real machine: the controller is a simulator (or GRBL on a serial port, never tried) and the server is reached only by the tests.
* The simulator takes only basic G-code (`G0`, `G1`, `G90`, `G91`, `G20`, `G21`, `G92`, `M3` to `M9`); arcs, canned cycles and probing are refused, and the path of a program does not draw arcs.
* It needs the folder `EH3D-COMMON` next to it, because it depends on it by path.

**Next**

* A real controller: GRBL over a serial port first, then the other families, then the controller of HYDRA-UMC.
* Arcs and the rest of the G-code in the simulator and in the path.
* A 3D view of the path with the height of the cuts.
* Probing, the tool changer and the materials of the machine.
* Join the engines of EH3D-DRILLPRO and EH3D-ROUTEPRO so that one program drills and routes.

## 🌍 Languages

README and texts exist in English, Spanish, French, Italian, German, Chinese, Japanese and European Portuguese.

## 🔗 Related Projects

* [EH3D-COMMON](https://github.com/JuanenRac/EH3D-COMMON): where the view (`eh3d-view`) and the ZIP of jobs (`eh3d-archive`) live
* [EH3D-DRILLPRO](https://github.com/JuanenRac/EH3D-DRILLPRO): the drilling program whose engine will be joined here
* [EH3D-ROUTEPRO](https://github.com/JuanenRac/EH3D-ROUTEPRO): the routing program whose engine will be joined here

## 📚 Documentation

* [Changelog](CHANGELOG.md)
* [Security policy](SECURITY.md) and [support](SUPPORT.md)
* The view of a job is documented in EH3D-COMMON, where it lives.

## 🤝 Contributing & Community

* Before a pull request run `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` and `python -I tools/ci_validate.py`: all must pass. See [CONTRIBUTING.md](CONTRIBUTING.md) and the [code of conduct](CODE_OF_CONDUCT.md).
* The README exists in eight languages: a change to the text is made in all eight files, and the two pictures are drawn from `docs/architecture.json` and `docs/structure.json` by `tools/make_architecture.py` and `tools/make_structure.py`.
* Do not describe as working what has not been run on a machine.
* Report a vulnerability privately: [SECURITY.md](SECURITY.md). Questions, ideas and reports: electrohobby3d@gmail.com ([SUPPORT.md](SUPPORT.md)).

## 👤 AUTHOR

**JuanenRac (Electro Hobby 3D)** · electrohobby3d@gmail.com

## 📜 LICENSE

Software: GPL-3.0-or-later. See [LICENSE](LICENSE). Hardware designs of the suite use CERN OHL-S.
