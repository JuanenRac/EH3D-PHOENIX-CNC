# ADR 0001: the script engine of the post-processors

Status: accepted as a direction. Nothing of the engine is written yet, because the crates it needs are not part of what this repository
builds today; this note fixes the choice, the limits and the tests that are missing, so that the work starts from a decision and not from
a discussion.

## Context

A post-processor has two parts. The declarative part (the rules of a file beside the settings: a header, a footer, words to replace, words
to drop) is always available and is done. The second part is a script, for what a rule cannot say (renumbering the lines, a tool change
written in the words of one machine, a comment with the time of the job). A script runs code that somebody else wrote on the computer of the
operator, so it widens the surface of attack of a program that drives a machine. That is why:

* the switch `post_scripts` of the motion file is **off** by default; with it off a rule that names a script is not run, and the program says
  so (the label that says scripts are off);
* with it on and no engine in the build, the program says that too (the label that says there is no engine), and runs nothing;
* the same switch must be reachable from Studio, with the same conservative default.

The language chosen for the scripts is Lua: small, well known, easy to cut down to nothing but string and table work.

## The two ways to run Lua from Rust

| | pure Rust interpreter (`piccolo`) | binding to the C implementation (`mlua`, with the Lua or Luau source vendored) |
|---|---|---|
| unsafe and C code in the build | none in the interpreter | a C compiler is needed on every platform, and the C code runs in the process |
| limit of work | fuel: the interpreter is asked for a number of steps and returns control when they are used, so an endless loop ends by construction | a hook that counts instructions (Lua) or an interrupt callback (Luau); it works, but it is one more thing to get right |
| limit of memory | the allocator of Rust can be wrapped | an allocator function can be given to Lua; a memory limit is documented |
| standard library | partial and changing: the string functions of the posts have to be checked one by one | complete |
| speed | slower | faster (irrelevant for scripts that rewrite a few thousand lines) |
| maturity | before 1.0: the API moves | stable |
| sandbox | nothing is there until it is added: the libraries that touch files or the system are simply not loaded | Luau has a sandbox mode; Lua needs the dangerous globals removed by hand |

## Decision

1. The engine is **`piccolo`**, the interpreter written in Rust, behind a small trait of our own (`ScriptEngine`: give it the text of the
   script, the lines of the program and the limits; get the lines back or a failure), so that the engine can be changed without touching
   the post-processors. The reasons: no C code in the process that drives a machine, no C compiler in the build of the three systems, and a
   limit of work that is part of how it runs.
2. If a spike on the real fixtures shows that its standard library cannot do what the sample posts need, the fallback is `mlua` with Luau
   (its sandbox and its interrupt callback are the nearest to what is asked of `piccolo`). The trait of point 1 is what makes the fallback a
   change of one file.
3. The sandbox is built by **loading only what is needed**: the base functions for values and iteration, `string`, `table`, `math` without
   the random functions. Nothing that reads or writes a file, runs a program, reads the clock, opens a socket or loads code from text is
   loaded: no `io`, `os`, `package`, `require`, `debug`, `load`, `dofile`, `loadfile`.
4. A script gets the lines of the program and the settings that concern it, as values, and returns the lines. It cannot see the settings
   file, the tool table or the path of anything.
5. Every limit is a key of the motion file, with a conservative default (the names are the proposal): `post_script_fuel` (steps),
   `post_script_memory_kb`, `post_script_max_lines`, `post_script_max_bytes` (what the script may give back) and `post_script_max_file`
   (the size of the script that is read).
6. The engine fails **closed**: a script that does not end, uses too much, gives back something that is not a list of lines, or raises an
   error leaves the program as it was, says why in the eight languages, and nothing is sent to the machine from a half result.
7. The switch stays off by default, in the motion file and in Studio. Turning it on is a decision of the operator for each machine.

## What exists now

* the declarative rules, tested;
* the switch `post_scripts` and the way it is read and written;
* `Post::script_state`, which says whether a script may run (off, on without engine) and the two labels that say it in eight languages;
* a test that a post-processor that names a script with the switch off or without an engine runs nothing.

## Tests that are missing

Each of these is written before the engine is accepted, and each one must fail when the thing it protects is taken away.

1. Every global the sandbox must not have (`io`, `os`, `package`, `require`, `debug`, `load`, `loadstring`, `dofile`, `loadfile`,
   `collectgarbage`, `getmetatable` of the string table, `setmetatable` on the globals) is `nil` inside a script.
2. A script that calls `io.open`, `os.execute`, `os.time` or `require "socket"` ends with a failure and not with an effect; a file that it
   tried to create does not exist afterwards.
3. `while true do end` ends with the failure of the work limit, in a time that does not depend on the speed of the computer, and the
   program is unchanged.
4. A script that builds a very long string (`string.rep`, a loop that keeps appending) ends with the failure of the memory limit.
5. A script that gives back more lines or more bytes than the limits is refused as a whole.
6. A deep recursion and a chain of coroutines end with a failure and do not overflow the stack of the program.
7. The same script on the same program gives the same lines twice, and on the three systems (no clock, no random numbers, the same
   formatting of numbers).
8. A script that is not valid text, has a syntax error, raises an error, returns nothing or returns something that is not a list of strings:
   each is a failure told in the eight languages, with the number of the line of the script when there is one, and the program is unchanged.
9. The order of work is fixed and tested: the rules first and the script after, or the other way round as the post-processor says, and a
   failure of the script does not leave the effect of the rules applied.
10. The file of the script is looked for beside the settings or at the path that was given, is refused over the size limit, and a link that
    points out of the folder is not followed.
11. With `post_scripts` off a post-processor that names a script behaves exactly as it does today, whether the engine is in the build or not.
12. A test with a fixed seed throws made-up scripts (random pieces of valid and invalid Lua) at the engine and requires that every one of
    them ends, within the limits, with lines or with a failure, and never with a panic.
13. A test that the sample scripts of the documentation run and give the lines the documentation shows.
14. The licences of the engine and of everything it brings are on the list that the CI of the suite accepts, and the version is pinned.

## Consequences

* The first piece of work is the spike on the fixtures (point 2 of the decision), which needs the crate in the build: it cannot be done in
  an environment that has no access to the registry.
* Until the engine is in, nothing changes for the operator: scripts are off and the program says so.
* Studio needs the same switch and the same limits as controls; that part belongs to the repository of Studio.
