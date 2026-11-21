# EH3D-PHOENIX-CNC: not yet

Good ideas that are not in the current version. Writing them here keeps anybody from implementing them early or twice.

| Idea | Why not yet | Revisit when |
|---|---|---|
| A real machine as the first proof | No real machine has been connected; the proof needs a person at a machine with the emergency stop in reach | the owner has a machine ready for the first run |
| The scripts of the post-processors | The decision record fixes the direction (Lua in a sandbox) but no engine is built; the rules of a post-processor cover the usual cases | a post-processor needs logic that the rules cannot express |
| The controller of HYDRA-UMC | The controller interface lists it, and the controller itself is not written on this side | the contract with HYDRA-UMC is accepted |
| One program that drills and routes with the engines of the two programs of the suite | The engines are separate libraries today and a merged flow needs a decision about the jobs | both engines are used by this program |
| A pendant or a hand wheel | It needs a real device and a decision about the protocol | a device is chosen |
| Closed-loop probing of a part (find the edges and the centre) | The probe works on the table and the tool setter on the length; a part probe needs the real probe first | the real probe has been run once |
| Cameras and live video on the machine | The camera belongs to the vision tool of the suite and to the node of the machine | the vision tool offers a stream |
| Cloud storage of programs | Programs travel through files and the server; a cloud service needs a decision on privacy | the owner asks for it |
| A text editor with the colouring of the G-code | The editor has numbers, find and replace and notes; colouring needs a drawing layer that the window does not have | the window has a text layer with spans |
