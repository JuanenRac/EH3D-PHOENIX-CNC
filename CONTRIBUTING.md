# Contributing to EH3D-PHOENIX-CNC

Thank you for looking. This repository is the base of a future machine-control program, so changes are small and carefully tested.

1. **Nothing moves a real machine without proof.** There is no machine control here yet. When there is, a change that sends bytes to a real control unit needs a note of what was observed on hardware and a test with the simulator.
2. **Every behaviour has a test**, including the refusals. The view of a job (scene, themes, words) lives in `eh3d-view` of EH3D-COMMON and is tested there; a change to it is made in that repository.
3. **Before a pull request:** `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test` and `python -I tools/ci_validate.py` must pass. The build needs the folder `EH3D-COMMON` next to this one.
4. **Texts exist in eight languages** (English, Spanish, French, Italian, German, Chinese, Japanese and European Portuguese) wherever the program shows text to people. The README files keep the same structure and the two pictures are drawn from `docs/architecture.json` and `docs/structure.json`.
5. **No files of other people's software.** Do not add programs, manuals or demo files of existing machine-control software.
6. Commits are written in English, small and about one thing.
