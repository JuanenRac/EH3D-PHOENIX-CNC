# Changelog

All notable changes to this project are documented here.

## [Unreleased]

- **Renamed:** The project is now called EH3D-PHOENIX-CNC (it was EH3D-MOTIONCNC); it joins the PHOENIX family of programs.
- **The application icon and the logos of the program:** `images/brand/` holds the icon (SVG, PNG sizes, `.ico` and `.icns`), the horizontal and vertical logos in a light and a dark version and the social preview image.
- **Release builds are checked and attested:** `release-binaries.yml` takes the owner's go-ahead only (a manual start from main, the commit must be part of main), builds the same packages as before, writes `SHA256SUMS` and a build manifest, attests their provenance, verifies the artifact as a stranger would, prepares a draft release of unsigned files only after the owner approves it (a dry run is the default) and opens an issue `build-failed` when anything does not finish.
- **Shared checks updated:** the validator also compares the badge and the Version row of the README with the manifest, fails when one of its checks cannot run, and the CHANGELOG step of the workflow reads the base branch from the environment.
- **More tests:** how a G-code program is read (forms of numbers, comments, modal moves, units), what is saved to go on after a crash, the settings file as an operator may edit it, the queue of programs, the movements of the views and the command line.
- **The settings file is safer to edit and to write:** a line break in a folder or in a recent file name can no longer add a line to it, a switch written as No, OFF or False is read as off, and a file saved with a byte order mark is read from its first line.
- **The workflows are hardened:** every action is pinned by its commit with its version in a comment, the checkouts keep no credentials, and the release workflow runs only for the owner on main, with no cache and the values of the matrix passed by environment.

## [0.0.9] - Queue of programs, Esc, Space and F5 keys, and the travel of GRBL
- **A queue of programs:** several files chosen in the Open window with Ctrl+click, dropped together or given on the command line run one after the other; each one shows its summary before it starts, the window lists them with their state and lets you remove, skip and reorder the ones that wait, and the queue stops when a program does not finish.
- **New keys:** Space pauses and Esc stops the job (F5 still starts); with the keyboard in a box of text they leave the job alone, and with a window open Esc only closes the window. F6 and F8 are free.
- **The travel of a GRBL controller shortens the one in the settings:** on connecting, `$130` to `$132` are read and the smaller travel of each axis is used for the summary and the refusal; if the controller does not answer or does not give them the settings are used and the log says so. `read_controller_travel` turns the reading off.

## [0.0.8] - Summary before a start, shortcuts, toolbar and a 3D view

- **A summary before a program starts:** the moves, the cutting distance, the time at the rapid speed and the feed of the program, the reach of the axes and a check against the travel of the machine (the zero of the work taken into account), with the warnings the suite shares (a program that is tiny or far from the corner of the travel, bigger than the table, arcs the estimate leaves out, cuts before any feed). **Start anyway** when something looks wrong; a program that leaves the travel is refused (`reject_out_of_travel = no` makes it a warning, `confirm_start = no` turns the summary off). It comes before Start and before sending the program to the server; the measure is `eh3d-summary` of EH3D-COMMON.

- **Keys of the keyboard:** Ctrl+O, Ctrl+S, Ctrl+E, Ctrl+, (settings), F5 start, F6 pause, F8 stop, Ctrl+Q, F1 (the list), F3 (plan or 3D), Ctrl+0 (reset the view) and Esc (closes a window, but never a question that needs an answer); with a window open only Esc, F1 and Ctrl+Q act; they are written beside the menu entries and `shortcuts = no` turns them off.

- **A toolbar with drawn pictures and names:** open, save, edit, start, pause, stop, rewind and settings, greyed out where they do not apply; the pictures are drawn with lines, so they take the colour of the theme (`show_toolbar` in the settings).

- **Numbers that are stepped or written:** the spindle speed, the limits of the travel, the feed and the rapid speed in the settings take the minus and plus buttons or the value written (a point or a comma, Enter or a click elsewhere accepts it, Esc leaves it as it was, the range is kept).

- **A Safety section in the settings window:** the summary on or off, refusing a program that leaves the travel, and the rapid speed (`confirm_start`, `reject_out_of_travel`, `rapid`); the thresholds of the checks live in the settings file (`summary_tiny`, `summary_far`, `summary_margin`).

- **The path from above is dragged and zoomed:** the wheel zooms about the pointer, dragging moves it and a double click or Ctrl+0 brings it back.

- **The path in 3D:** the table with its grid, the volume the axes reach, the path with the part that is done and the tool with a line down to the table, turned and tilted with the pointer and zoomed with the wheel (`2D` and `3D` tabs, F3, `--view 3d`; the choice is remembered in `view_3d`).

- **The path and the tool are drawn where the machine is:** the program speaks in work coordinates, so its path is moved by the zero of the work before it is drawn on the table of the machine.

- **A picture of the G-code file in the Open window:** the path of the file chosen, its size and its moves, read once for each file and only up to 2 MB (`open_preview`, and a switch in the window).

- **A file that cannot be opened is said in the language:** the log gives the file and the reason.

- **A crash is recorded:** a panic is appended to `eh3d-crash.log` in the settings folder with the program, its version and the place (`eh3d_view::crash`, shared).

- **The words:** the summary, the safety section, the keys and the toolbar in the 37 languages (they live in the texts of EH3D-COMMON). 31 tests more (the measure of a program, the settings, the pointer movements of both views, the numbers, the keys, the summary and the picture of a file).
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
