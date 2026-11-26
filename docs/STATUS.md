# EH3D-PHOENIX-CNC: status

## Current state

EH3D-PHOENIX-CNC works on a computer against simulated machines: a docking window and a command line that drive a controller behind a guard that fails closed, run a program with pause, stop and the emergency stop, resume it after a failure, check it with a long list of checks, draw and time it, import drawings and boards, and send it to the server. GRBL, grblHAL and Marlin have been tried against simulators of their dialects, on simulated cables that cut, spoil and silence the lines, from fixed seeds. Nothing has run on a real machine.

## Recent work

The command `project` keeps the work in the branch `cnc/` of an `.eh3d` project (`eh3d-project.md`): the other branches keep their bytes, copies taken from them are told as out of date and imported files keep their original with a report. The window became a docking window of twelve panels with four workspaces; the guard that fails closed is in the session and in the window; grblHAL, Marlin and Klipper (through Moonraker over HTTP) are driven like GRBL; the Gerber and Excellon files of a board are cut as isolation, profile and drill programs; and a record decides the engine of the scripts of the post-processors. The README files, this page and the register of requirements agree on what exists and what is missing; the register is the one that says it.

## Capabilities

The official state of each capability is in `REQUIREMENTS.md`. The register has 145 requirements: 136 implemented, 2 prototype, 7 planned and 0 validated. By progress: 136 done, 2 partial, 1 pending and 6 blocked. This page never states a state that `REQUIREMENTS.md` does not.

## Open questions

* Which real machine is the first one: the GRBL machine of the workshop or another.
* Whether the scripts of the post-processors use Lua in a sandbox, as the decision record proposes.
* How the 3D view should show the height of the cuts.

## What it needs from other repositories

* EH3D-COMMON: the controller interface and the simulated controller, the view, the docking frame and the server client.
* EH3D-CORE-LIBS: the readers of Gerber and Excellon files.
* A real machine with GRBL, grblHAL, Marlin or Klipper, a probe, a tool setter and a changer, for the proofs that no simulator can give.

## Known technical debt and omitted tests

* 1199 tests (`#[test]` lines of `src/` and `tests/`; nine of them keep this register true); all of them run on a computer against simulated machines, with fixed seeds where chance is involved.
* No test has driven a real controller: the serial port is built behind a feature and has never been opened on a machine.
* The look of the window has been seen on a computer by its pictures only; no operator has used it at a machine.
* The look-ahead and the time estimates are checked for consistency, never against the time a real machine takes.
