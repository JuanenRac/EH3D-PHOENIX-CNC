# EH3D-PHOENIX-CNC: status

## Current state

EH3D-PHOENIX-CNC works on a computer against simulated machines: a docking window and a command line that drive a controller behind a guard that fails closed, run a program with pause, stop and the emergency stop, resume it after a failure, check it with a long list of checks, draw and time it, import drawings and boards, and send it to the server. GRBL, grblHAL and Marlin have been tried against simulators of their dialects, on simulated cables that cut, spoil and silence the lines, from fixed seeds. Nothing has run on a real machine.

## Recent work

A safety pass over the link to the server, the queue and the starts in the middle of a program. The emergency stop of the window also goes to the machine of the server; a running program cannot be replaced; a stop or an alarm stops the queue, also between two runs; a restart says its way of moving; and a start is refused where an arc goes on unsaid, with `G93`, `G95` or `G43`. Decisions taken the cautious way: the emergency stop is sent to the server whenever a job is not known to be over (a latched machine needs an administrator to release it), and what cannot be written back is refused instead of guessed. All of it with simulators and a stand-in server. Board files with an error are refused, and so is a drill file whose format, zeros or units would be supposed (`pcb_allow_assumed_drill`, off).

The window has a panel «Attachments» (the shared `eh3d-attachpanel`): the data sheets, manuals and other files of the project are listed, attached, linked to a part, extracted and removed after a question, and shown beside the list (`eh3d-project.md`). The panel itself is tested in EH3D-COMMON; it has not been tried on a screen by a person.

The menu File of the window opens, saves, imports, takes and extracts an `.eh3d` project and asks before it writes one that is not its own. The command `project` keeps the work in the branch `cnc/` of an `.eh3d` project (`eh3d-project.md`): the other branches keep their bytes, copies taken from them are told as out of date and imported files keep their original with a report. The window became a docking window of thirteen panels with four workspaces; the guard that fails closed is in the session and in the window; grblHAL, Marlin and Klipper (through Moonraker over HTTP) are driven like GRBL; the Gerber and Excellon files of a board are cut as isolation, profile and drill programs; and a record decides the engine of the scripts of the post-processors. The README files, this page and the register of requirements agree on what exists and what is missing; the register is the one that says it. It does not write over a project another program saved since it was read, and offers to read it again or to save under another name.

## Capabilities

The official state of each capability is in `REQUIREMENTS.md`. The register has 167 requirements: 158 implemented, 2 prototype, 7 planned and 0 validated. By progress: 158 done, 2 partial, 1 pending and 6 blocked. This page never states a state that `REQUIREMENTS.md` does not.

## Open questions

* Which real machine is the first one: the GRBL machine of the workshop or another.
* Whether the scripts of the post-processors use Lua in a sandbox, as the decision record proposes.
* How the 3D view should show the height of the cuts.

## What it needs from other repositories

* EH3D-COMMON: the controller interface and the simulated controller, the view, the docking frame, the server client and the project container. The workflows take its tag v0.3.7.
* EH3D-CORE-LIBS: the readers of Gerber and Excellon files. The workflows take its commit 551aea3fcf793867b2503fff76823b24513a5f50, until a release holds the notices of what the drill reader supposes.
* A real machine with GRBL, grblHAL, Marlin or Klipper, a probe, a tool setter and a changer, for the proofs that no simulator can give.

## Known technical debt and omitted tests

* 1239 tests (`#[test]` lines of `src/` and `tests/`; nine of them keep this register true); all of them run on a computer against simulated machines, with fixed seeds where chance is involved.
* No test has driven a real controller: the serial port is built behind a feature and has never been opened on a machine.
* The look of the window has been seen on a computer by its pictures only; no operator has used it at a machine.
* The look-ahead and the time estimates are checked for consistency, never against the time a real machine takes.
