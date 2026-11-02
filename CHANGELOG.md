# Changelog

All notable changes to this project are documented here.

## [Unreleased]
- **The CI builds against the tag of each sibling:** the sibling repositories are read from `depends_on` of the manifest (the ones they need in turn included) and checked out at the tag the repository is built from, so a build does not move when a sibling changes; a separate job, `Siblings at main`, builds against the main branch of every sibling to show a break before the sibling is tagged and does not hold a merge. The release workflow builds from the same tag.

- **Each dependency on a sibling by path says the version it needs** (`eh3d-view`, `eh3d-controller`, `eh3d-filedialog`: `>=0.0.1`), so a package built from the repository is not rejected for lacking one.

## [0.0.7] - Cut, copy and paste, a stop that stops the machine and a program that goes on from a safe state

- **Cut, copy and paste work in every text box:** the window was built without the clipboard, so none of the keys (Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+Insert, Shift+Insert, Shift+Delete) did anything. All the boxes (the file and folder names, the server address, token and machine, the command line, the program path and the log) take them, and the right button opens a menu with Cut, Copy, Paste, Select all and Undo, greyed out where they do not apply and named in the 37 languages.

- **The stop of a program sent to the server stops the machine:** it asks for `POST /machines/:id/stop` (and cancels the job on a server that does not have it).

- **A program left in the middle goes on from a safe state:** when it is resumed, the controller is first told the units, the absolute distances, a safe Z (`resume_safe_z`, 5 mm by default), the spindle and the time it gets to reach its speed (`resume_spindle_wait`, 3 s), the coolant, the point the program had reached and then the modal state of the program; the recovery file keeps the SHA-256 of the program and refuses to go on from a program that changed.

- **G90.1 and G91.1 are no longer read as G90 and G91:** the preview and the modal panel keep the distance mode when a program sets the arc-centre mode in its header.

## [0.0.6] - Send the program to the server, pause, resume and go on after a failure

- **The program sent to the EH3D server:** the settings (section Server of the window, or `server_address`, `server_token` and `server_machine` in the settings file, the token hidden in the window) say where the server is, who this program is on it and which machine it works for. **Program > Send to the server** (and a button beside Start) sends the text of the program as a G-code job and starts it; the window follows it on a thread of its own (the state, the percentage and the line reached in the top bar, the program panel and the log), Start, Pause and Stop reach the job on the server, a stop is a stop of the machine there, and closing the window leaves the job running. Tested against a stand-in server (the request, the following, the orders, a refusal, the job made again from a line); the server's own tests send the same requests to its simulator.
- **Going on after a failure, through the server:** while a job on the server runs the file next to the settings keeps the id of the job and the line the server reached; at the next start the window offers to go on, follows the job if the server still has it, or sends the program again from the line the failed job reached. The server gives the codes in force back (units, distance mode, plane, coordinate system, feed) and waits for the operator before the first move; the spindle and the coolant start again only with `server_restore_outputs = yes` (off by default).
- **Words:** the Server section of the settings, its three fields and the menu entry in all 37 languages (they live in the texts of EH3D-COMMON). 8 tests more (the settings file, the file the window goes on from, the requests and the following of a job).

## [0.0.5] - No console window on Windows, all 37 languages and going on after a failure

- **The names of the languages come from the texts:** the settings list the 37 languages, each in its own name.

- **No black window on Windows:** the program no longer opens a console; started from a terminal it attaches to it, so the help and the errors are printed there as before.

- **Going on after a failure:** while a program runs, the line it has reached is saved to a small file next to the settings; at the next start the window asks whether to go on from that line or to leave it, and nothing is sent to the controller until the operator answers.
- **Tests:** the file that is saved and read back, the line that is due to be saved and a file that is damaged are checked (30 tests now).

## [0.0.4] - A serial link to GRBL, menus and a left panel like PlanetCNC, and a window that follows the screen

- **The window opens maximized and follows the size of the screen:** from 720p to 1080p and on to larger and wider screens everything grows or shrinks with the window.
- **GRBL on a serial port:** with `--features serial`, `--grbl PORT[@BAUD]` drives a GRBL controller instead of the simulator and `--ports` lists the serial ports; a build without the feature or a port that does not open says so and leaves.
- **Menus, settings and files like PlanetCNC:** the menus File, View, Program, Machine and Help (open, recent files, save, edit the program, the zeros, the stored positions, the spindle and the coolant, what is shown, About); a settings window with its sections (units, language and the eleven themes, travel limits, jog) kept between runs, whose travel limits reach the machine; the Open and Save windows of the suite (`eh3d-filedialog`) with the places, the sorting, the filters, the recent files, the extension of a saved file and the question before a file is replaced; an editor of the program; the position in inches when the settings say so; 22 tests.
- **The left panel made like the one of PlanetCNC:** a column of buttons (set the work zero in XYZ or in XY, go to the stored positions G28 and G30, go to the zero in XY or in Z, home), the position with the tabs Work, Machine, Operator and GCode and the four axes X, Y, Z and A (the letter of an axis sets its zero), the speed and the spindle, the active G-codes and M-codes or the inputs and outputs, and the jog with the A axis; the session keeps the codes in force, the zero of the operator and the stored positions (tested).
## [0.0.3] - The first window: a simulated machine with jog, zero, home, commands and a G-code program with its path, in eight languages and five themes

- **Screenshots in the README:** three pictures of the window (a program running, a second theme and the German interface) in all the eight languages of the README.
- **Documentation, scripts and examples for the window:** the README in eight languages says what runs today (a simulated machine), the build, test and run scripts build and open the window, the CI builds and tests it with the window feature, and three sample G-code programs (a square, a frame and a pocket, a long spiral) are in `examples/`, packaged with the binaries by a release workflow for Windows (64 and 32 bits), Linux and macOS.
- **The session and the path of a program:** `session` runs a program on any controller of `eh3d-controller` (jog, zero, home, MDI, pause, stop, alarm) and `gcode` draws the straight moves of a program; both are tested on the simulated controller (10 tests), and the command line has its own (2).
- **A window with the simulated controller:** `motioncnc` shows the position of the machine (work and machine), a jog pad with its steps and feed, home, zero and unlock, the emergency stop, a line of commands (MDI) with its log, and a G-code program with its path drawn on the table, the lines done marked and the tool where the machine is; it speaks the eight languages and wears the themes of the suite. The program is run line by line, can be paused and stopped, and a line the controller refuses stops it where it is.

## [0.0.2] - The scene moved to the shared view crate, the repository scaffolding in eight languages and the run, build and test scripts

- **A lighter debug build:** the `dev` profile keeps the file and line of a panic (`debug = "line-tables-only"`) but not the full debug symbols, so the `target` folder of a test run is about a tenth of its size.
- Repository scaffolding: the README in eight languages with the project status table, the Architecture and Repository Structure pictures drawn from `docs/architecture.json` and `docs/structure.json`, the CI workflow, the issue and pull-request templates, the code of conduct, the contributing guide, the security policy, the support page and the repository checks in `tools/`.
- The manifest says what the repository is today: a library with no tests of its own, no window and no machine control.
- The view of a job (the scene, the themes and the words of the interface) moved to `eh3d-view` of EH3D-COMMON, where every program of the suite can use it; this crate uses it from there.
- `run` scripts, and `build` and `test` scripts that wait for a key unless `NOPAUSE` is set.

## [0.0.1] - The scene, the themes and the words of the interface

- **The scene of a job on the table:** holes with their drill sizes, routing paths, the progress of the work, a drilling job and a routing job in one scene, the plan view and the isometric view of the machine fitted to the size of the table, and a check that tells what does not fit on the table before a job starts.
- **Themes:** Forge (the default), Neon Arcade, Glass Pro, the classic Windows grey and a theme that follows the colours of Windows (light or dark and the accent colour); a user can write their own in a text file. Every theme is checked for readable contrast.
- **The words of the interface** in English, Spanish, Italian, German, French, Portuguese, Chinese and Japanese.
