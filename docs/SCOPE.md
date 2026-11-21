# EH3D-PHOENIX-CNC: scope

Status: **functional**. EH3D-PHOENIX-CNC is the machine-control program of the PHOENIX family: a window (`phoenix-cnc`) and a command line that drive a controller, run a G-code program on it line by line and check, convert and report a program before it runs. It has run on a computer against simulated machines only. This page says what it is for, what it will not do and where it sits among the other programs of the suite.

## Purpose

One program to drill, to route and to do 2D and 3D CNC work on the machine of a workshop. It shows the position, moves the machine by hand, runs a program with pause, stop and the emergency stop, draws the path, and checks a program (limits, keep-out boxes, cycles, compensation, work systems, tools and their wear) before the first line goes out. Everything that moves a machine goes through a guard that fails closed.

## What it does

* Drives a controller through the interface of `eh3d-controller`: a simulator by default, and GRBL, grblHAL or Marlin on a serial port only when the person names the port (`--grbl`, `--grblhal`, `--marlin`), always behind the guard.
* Runs a G-code program line by line with pause, stop, the emergency stop and the alarms of the controller, jogs by the step inside the travel, sets the zero, homes and unlocks, and resumes a program that was left in the middle only after the operator answers.
* Checks a program before it starts and says what looks wrong with its line: the travel, the keep-out boxes, a rapid below the surface, a cut with the spindle off, a feed or a speed above the limits of the material, canned cycles, cutter compensation, work systems, tool length offsets, the reach of a holder and the wear of the tools.
* Draws the path in the plan and in 3D, times the program with and without a look-ahead, and writes a work report as one page of HTML.
* Reads a drawing (HPGL, DXF, SVG) or the Gerber and Excellon files of a board and writes the G-code for it; converts, compares and reformats programs on the command line.
* Sends a program as a job to the EH3D server, follows it, and gives the emergency stop and the jog routes of a machine on the server.
* Shows the window in twelve panels that can be moved, tabbed, floated and folded, in eleven themes and in many languages (every word of the panels in eight, the summary of a program in 37), and keeps the arrangement.

## What it will not do

* It does not decide that a program is safe to run: the limits, the interlocks and the emergency stop belong to the machine and its controller.
* It does not open a serial port or a network connection unless the person names it, and it does not start a program on a real controller by itself.
* It does not send the lines that address the controller itself (those that start with `$`) unless the settings allow it.
* It does not run the scripts of a post-processor yet (the engine is decided in a record but not built).
* It does not claim that anything was proven on a real machine: the evidence of every capability is `simulator` or `host-test`.

## Where it sits

* **It reads:** G-code programs, drawings and board files, its settings and motion files, the table of tools, the library of materials, the macros, the post-processors and profiles next to the settings, and the answers of the controller or of the server.
* **It writes:** orders to the controller it is driving, the files that the person asks for (a converted program, a report, a log, a height map) and its own settings and recovery files; a file that exists is never written over by accident.
* **It is read by:** the EH3D server reads the jobs it sends; no other program reads it.
* **It depends on:** EH3D-COMMON (the controller interface, the view, the docking frame and the server client) and EH3D-CORE-LIBS (the readers of Gerber and Excellon files), both as sibling folders.

## What it never decides

Whether a program is safe to run, and the limits, the interlocks and the emergency stop of any machine, belong to the machine, its controller and the person who stands at it. The checks of this program make a program refused or a warning louder, never the other way; a clean report is not a statement that the program is safe. Nothing here replaces the emergency stop of the machine.

The requirements of the program, with the state of each and the test that shows it, are in `REQUIREMENTS.md`; the state in a paragraph is in `STATUS.md`.
