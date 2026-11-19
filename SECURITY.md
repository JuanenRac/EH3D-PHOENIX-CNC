# Security policy

EH3D-PHOENIX-CNC is a window over a machine controller. It reads no files from other people except the G-code file the operator chooses (read as text, never run by itself) and a theme file written by a user, which is checked rather than trusted. It also reads and writes small files of its own next to the settings or next to the program (the table of tool changer slots, the table of tools, the library of materials and the bookmarks of a program), only from their own buttons and with a size limit; the example files are never written over a file that is there, and the library and the bookmarks are replaced only after a question. It opens a serial port only when it is started for GRBL (`--grbl`, a build with `--features serial`), and a network connection only to the EH3D server whose address is in its settings, when the operator sends a program to it. The token for the server is kept as text in the settings file of the user: keep that file private.

What this repository does **not** do: it has not driven a real machine, it is not a safety system and it does not handle limit switches or an emergency stop. Do not connect a machine to it.

## Reporting a problem

Please report a vulnerability privately to **electrohobby3d@gmail.com**, with the steps that show it. Do not open a public issue for it. You will get an answer as soon as possible; this is a one-person project.
