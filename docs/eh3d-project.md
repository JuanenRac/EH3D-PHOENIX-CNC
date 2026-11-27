# The `.eh3d` project and the branch `cnc/`

An `.eh3d` file is the project of the EH3D Fabrication Suite: one ZIP file with a branch for each program (`pcb/`, `cad/`, `3dp/`, `laser/`, `cnc/`...). The format is defined in `docs/FORMAT_EH3D.md` of EH3D-COMMON and read and written by its crate `eh3d-project`. `phoenix-cnc` writes only `cnc/`; every other branch, known or not, keeps its bytes. Nothing in a project is run and these commands move no machine.

## The commands

| Command | What it does |
|---|---|
| `project list FILE.eh3d [--json]` | the branches, who wrote each one, their format, and what is out of date or wrong |
| `project save FILE.eh3d [--program P] [--drawing D] [--tools T] [--motion M] [--name N] [--id ID]` | puts the files in `cnc/`; the project is created when it is not there (the name and the identifier are only for a new one); what is not given stays |
| `project import FILE.eh3d FILE [--motion M]` | takes a G-code program or a DXF, SVG or HPGL drawing, keeps it as it is under `cnc/origin/` with `report.json`; a drawing also gives the program that cuts along it, with the values of the motion file |
| `project take FILE.eh3d BRANCH NAME` | copies a file of another branch to `cnc/taken/BRANCH/NAME` with a reference to the state of its source; a program or a drawing becomes the one of the work, a mesh is only kept |
| `project open FILE.eh3d [--into DIR]` | reads and checks `cnc/` and says what it holds; with `--into` writes its files |
| `project extract FILE.eh3d BRANCH DIR` | writes the files of any branch, byte for byte |

The exit code is 0 when there is nothing to read, 1 when something is out of date, wrong or was not read, and 2 on an error. A file that is there is never written over by `open` or `extract`. The project is replaced in one step (a temporary file, then a rename).

What every program does with its branch (open or create, save, import with a report, take, out-of-date notices, list and extract) is the module `appbranch` of the crate `eh3d-project`; this program says what its files are. `list` says when a branch is signed. Writing the branch removes a signature of the project, and a signature of the branch when there is one; the command says so. These commands neither sign nor check signatures: `eh3d-project` of EH3D-COMMON does.

`save` refuses a motion file that gives a value to `klipper.api_key` and writes nothing: a project is passed on and the credential must not travel in it. Take the line out or save without `--motion`.

## In the window

File > Suite project (.eh3d) does the same with the window: Open suite project, Save suite project, Save suite project as, Import a file and keep the original, Take from another branch, Extract a branch and Branches of the project. A project can also be opened with the file window, dropped on the window, picked from the recent files or given on the command line.

- Open takes the program of `cnc/` as the program of the window; a project without one leaves the window with no program. A project that cannot be read changes nothing. Open, import and take are refused while a program runs or the machine moves, and nothing here moves it.
- Save puts the program of the window. The table of tools and the motion file of the machine go only when their boxes in the project bar are ticked, and a motion file with `klipper.api_key` is not put. What the project holds already is not written again, so a program taken from another branch keeps its reference. The drawing of the branch is kept as it is.
- Save as to a new file copies the whole project first, so every branch goes with it. A file that is there keeps its own branches: only `cnc/` is written, after a question.
- The window asks before writing a project it did not open, a `cnc/` that changed since it was read, or one whose signature (of the project or of the branch) the write removes. Nothing is written until *Write* is pressed: Escape and *Cancel* say no, and no button has the keyboard, so Enter does not write. The commands do not ask.
- A project that another program saved since the window read or wrote it is **not written over**, also when it was saved while a question waited: Save, Save as over that file, Import and Take say so in the language of the window and offer to read it again, to save under another name or to close. The content counts, not the date, what this program writes itself (the panel «Attachments» included) does not count, and reading it again for the bar is not reading the work again.
- A bar under the top bar names the project and says what is OUT OF DATE and what is signed. It is read again when the window gets the keyboard back.
- The table of tools and the motion file of a project are never applied to the machine; extract the branch to use them. A program is never written over a project as text.

## Attachments

The panel «Attachments» of the window (View, or File > Suite project) is the shared panel `eh3d-attachpanel` of EH3D-COMMON (its guide is `docs/ATTACHMENTS_PANEL.md` there). It works on the project the window has open, and the window reads the project again after a write. It writes only the branch `attachments/`; the branch `cnc/` and the others keep their bytes. A write that removes a signature is asked about first. Dangerous files and kinds the suite does not know are extracted or opened only after a confirmation, programs and scripts are never opened, nothing is downloaded, and a settings file that holds `server_token` is not attached. PDF pages need the PDF library beside the program, which the packages carry with its licences; a build from source gets it with `python ../EH3D-COMMON/tools/fetch_pdfium.py --out target/release`. A write is refused, with nothing written, when another program saved the project after the panel read it. Its limits are read from the settings (`attachments.max_mb`, `viewer.max_*_mb`; among them `viewer.max_preview_mb`, 16 by default, is the text a whole preview keeps).

## What is in the branch

```
cnc/
  branch.json          written by the container: program, format 1.0, content hash, references
  cnc.json             which file is the program, the drawing, the tools, the motion file
  programs/NAME.nc     a program given to `save`, or made from an imported drawing
  drawing/NAME         a drawing given to `save`
  settings/tools.txt
  settings/motion.txt
  origin/NAME          the original of the last import
  origin/report.json   what was read, not read and lost on export
  taken/BRANCH/NAME    copies of files of other branches
```

Fields of `cnc.json` and of the record that this version does not know, and files it does not know, are kept. A branch of another format than 1.0 is not written (`EH3D-PKG-024`); a sealed branch or project is not written (`EH3D-PKG-020`).

## Out of date

A copy made with `take` keeps the content hash and the format of the branch it came from. When that branch changes, `list` and `open` say `OUT OF DATE`; take the file again to bring it up to date. Nothing is ever changed on its own, and a program made from a drawing is not made again by itself.

## Limits

An import keeps one original at a time: a second import replaces the first and says so. Gerber and Excellon files of a board are not imported into a project yet. A G-code program holds cutting moves, not the entities of the drawing it was made from; the original is kept.
