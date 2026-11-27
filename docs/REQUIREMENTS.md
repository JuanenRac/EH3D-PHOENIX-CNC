# EH3D-PHOENIX-CNC: requirements

This register is the single source of the state of each capability: `STATUS.md` and the README files cite it and never state a state that it does not. A state is one of `planned`, `prototype`, `implemented` and `validated`. `validated` means that a person has run the capability on a real machine and the evidence of the manifest says `bench` or `field`; nothing here is validated, because nothing of this program has run on a real machine: the controller is a simulator, a simulated GRBL, grblHAL or Marlin on a simulated cable, or a server in memory.

The register has 164 requirements: 155 implemented, 2 prototype, 7 planned and 0 validated. By progress: 155 done, 2 partial, 1 pending and 6 blocked. High coverage is not proof of correctness, so each row says how it is checked: the test that names it, and what it shows.

## How to read the register

* **ID:** `PCNC-NNN`, never reused and never renumbered; a new capability takes the next number.
* **State:** `implemented` is code that is complete and tested on a computer, `prototype` is part of the capability, `planned` is not written.
* **Progress:** `done`, `partial`, `pending` (can be written without hardware) or `blocked` (waits for hardware or for another program of the suite).
* **Evidence:** `simulator` when the test runs against a simulated controller, a simulated cable or a server in memory, `host-test` when it checks a calculation, a reader or a screen-less piece, `none` when nothing is written yet.
* **Test:** the acceptance criterion: `file::name` is the test `name` of `tests/file.rs` (`src/file::name` is a test inside `src/file.rs`), which must exist. For a capability that is not written the cell says `planned:` and the name of the test that will show it; the day that test exists, the row must change.
* **Limits:** what is not done or not proven; a row that is `partial` or `blocked` always says it.

The check is `tests/reqs.rs`: it reads this table, finds each test and fails when a state, a test or a count does not agree.

## The machine, the jog and the emergency stop

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| PCNC-001 | Jog by a step and make the work position nothing with the zero. | implemented | done | simulator | `src/session::a_jog_moves_by_the_step_and_zero_makes_the_work_position_nothing` | - |
| PCNC-002 | A step past the limit of the travel is shortened. | implemented | done | simulator | `src/jog::a_step_past_the_limit_is_shortened` | - |
| PCNC-003 | A step from the limit outwards is refused. | implemented | done | simulator | `src/jog::a_step_from_the_limit_outwards_is_refused` | - |
| PCNC-004 | The limits that need homing hold only after the machine was homed. | implemented | done | simulator | `src/session::the_limits_that_need_homing_hold_only_after_the_machine_was_homed` | - |
| PCNC-005 | The travel of the controller shortens the travel of the settings and never lengthens it. | implemented | done | simulator | `src/session::the_travel_of_the_controller_shortens_the_travel_of_the_settings_and_never_lengthens_it` | - |
| PCNC-006 | Homing and an alarm from the travel are shown. | implemented | done | simulator | `src/session::homing_and_an_alarm_from_the_travel_are_shown` | - |
| PCNC-007 | The emergency stop ends the run and blocks it until the machine is unlocked. | implemented | done | simulator | `src/session::the_emergency_stop_ends_the_run_and_blocks_it_until_unlocked` | - |
| PCNC-008 | The emergency stop is heard while a line waits for its answer. | implemented | done | simulator | `src/session::the_emergency_stop_is_heard_while_a_line_waits_for_its_answer` | - |
| PCNC-009 | A controller with no urgent channel says that the stop cannot be given out of turn. | implemented | done | simulator | `src/session::a_controller_with_no_urgent_channel_says_that_the_stop_cannot_be_given_out_of_turn` | - |
| PCNC-010 | A jog is refused while a program runs and the line typed by hand is logged. | implemented | done | simulator | `src/session::a_jog_is_refused_while_a_program_runs_and_the_mdi_line_is_logged` | - |
| PCNC-011 | Jogs that cross do not trip the guard because the move in progress is cancelled first. | implemented | done | simulator | `src/session::jogs_that_cross_are_normal_and_do_not_trip_the_guard_because_the_move_in_progress_is_cancelled_first` | - |
| PCNC-012 | Jogs in any order from a fixed seed never trip the guard and never send two moves in a row. | implemented | done | simulator | `src/session::jogs_in_any_order_from_a_fixed_seed_never_trip_the_guard_and_never_send_two_moves_in_a_row` | - |
| PCNC-013 | Space pauses and Escape stops the job. | implemented | done | simulator | `src/gui/keys::space_pauses_and_escape_stops_the_job` | - |

## Running a program

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| PCNC-014 | A program runs line by line and ends by itself. | implemented | done | simulator | `src/session::a_program_runs_line_by_line_and_ends_by_itself` | - |
| PCNC-015 | A pause holds the program and start goes on. | implemented | done | simulator | `src/session::a_pause_holds_the_program_and_start_goes_on` | - |
| PCNC-016 | A refused line stops the program where it is and says which. | implemented | done | simulator | `src/session::a_refused_line_stops_the_program_where_it_is_and_says_which` | - |
| PCNC-017 | A line that starts with a dollar sign stops the program unless the settings allow it. | implemented | done | simulator | `src/session::a_program_line_that_starts_with_a_dollar_stops_the_program_unless_it_is_allowed` | - |
| PCNC-018 | A program left in the middle goes on paused with its codes rebuilt. | implemented | done | simulator | `src/session::a_program_left_in_the_middle_goes_on_paused_with_its_codes_rebuilt` | - |
| PCNC-019 | A resumed program lowers the tool only when the operator says so. | implemented | done | simulator | `src/session::a_resumed_program_lowers_the_tool_only_when_the_operator_says_so` | - |
| PCNC-020 | The recovery file tells a changed program from the same one by its hash. | implemented | done | simulator | `src/recovery::the_hash_tells_a_changed_program_from_the_same_one` | - |
| PCNC-021 | Start from a line: the state the program had is written back before the chosen line. | implemented | done | simulator | `src/restart::the_state_the_program_had_is_written_back_before_the_chosen_line` | - |
| PCNC-022 | A start from a line that cannot be rebuilt is refused with the line that caused it. | implemented | done | simulator | `src/restart::where_the_state_cannot_be_rebuilt_the_start_is_refused_with_the_line_that_caused_it` | - |
| PCNC-023 | Several programs run one after the other, each after its own summary. | implemented | done | simulator | `src/gui/queue::two_files_run_one_after_the_other_and_each_one_shows_its_summary_first` | - |
| PCNC-024 | A program that stops before its end stops the queue. | implemented | done | simulator | `src/queue::a_program_that_does_not_finish_stops_the_queue_until_the_operator_goes_on` | - |
| PCNC-025 | A program repeats as many times as it is told, asking or waiting between the runs. | implemented | done | simulator | `src/jobs::between_two_runs_it_is_a_confirmation_a_wait_or_nothing` | - |
| PCNC-026 | The queue is kept in a file and comes back waiting. | implemented | done | simulator | `src/gui/queue::the_queue_is_kept_in_a_file_and_comes_back_waiting` | - |
| PCNC-027 | The summary before a start waits for the operator, and a paused program goes on without it. | implemented | done | host-test | `src/gui/summary::a_fresh_start_waits_for_the_summary_and_a_paused_program_goes_on_without_it` | - |

## The guard, the controllers and the cable

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| PCNC-028 | The guard stops the machine when the silence goes on, and an order that moves is never sent twice. | implemented | done | simulator | `src/guard::an_order_that_moves_is_never_sent_twice_and_the_silence_stops_the_machine` | - |
| PCNC-029 | A position that jumps or is not a number is not believed. | implemented | done | simulator | `src/guard::a_position_that_jumps_or_is_not_a_number_is_not_believed` | - |
| PCNC-030 | A second move order while one is active is a stop, and a renewal is not. | implemented | done | simulator | `src/guard::a_second_move_order_while_one_is_active_is_a_stop_and_a_renewal_is_not` | - |
| PCNC-031 | The switches make the guard less strict and the defaults are the strict ones. | implemented | done | simulator | `src/guard::the_switches_make_the_guard_less_strict_and_the_defaults_are_the_strict_ones` | - |
| PCNC-032 | Made-up schedules of faults never send a move twice nor leave a move after a stop. | implemented | done | simulator | `src/guard::made_up_schedules_of_faults_never_send_a_move_twice_nor_leave_a_move_after_a_trip` | - |
| PCNC-033 | The guard is on by default and a switch of the motion file takes it away. | implemented | done | simulator | `src/link::the_guard_is_on_by_default_and_the_switch_takes_it_away` | - |
| PCNC-034 | A controller that falls silent stops the machine and the window says so until the operator clears it. | implemented | done | simulator | `src/gui/shell::a_controller_that_falls_silent_stops_the_machine_and_the_window_says_so_until_the_operator_clears_it` | - |
| PCNC-035 | A real controller does not start a program by itself unless it is said twice. | implemented | done | host-test | `src/cli::a_real_controller_does_not_start_a_program_by_itself_unless_it_is_said_twice` | - |
| PCNC-036 | GRBL: a move past the travel is an alarm and the lock is cleared with unlock. | implemented | done | simulator | `src/link::a_simulated_grbl_alarms_on_a_move_past_its_travel_and_unlock_clears_the_lock` | - |
| PCNC-037 | grblHAL says so in its build, starts locked and is told from GRBL. | implemented | done | simulator | `src/dialect::grbl_and_grblhal_are_told_apart_by_the_build_and_the_banner` | - |
| PCNC-038 | Marlin: a line spoiled on the wire is never executed when the lines are numbered. | implemented | done | simulator | `src/dialcheck::a_cable_that_spoils_the_lines_fails_the_check_of_marlin_without_numbers_and_not_with_them` | - |
| PCNC-039 | Marlin: an urgent reset is the emergency stop and leaves it halted. | implemented | done | simulator | `src/link::an_urgent_reset_stops_a_simulated_marlin_and_it_refuses_the_moves_after_it` | - |
| PCNC-040 | A simulated cable cuts, spoils and silences the lines from a fixed seed, and the answers still come back whole. | implemented | done | simulator | `src/dialcheck::the_check_passes_on_a_cable_that_cuts_and_adds_noise_and_marlin_numbered` | - |
| PCNC-041 | Each dialect passes its battery of steps against its simulator, in the language of the person. | implemented | done | simulator | `src/dialcheck::every_dialect_passes_its_check_with_the_cable_of_the_defaults` | - |
| PCNC-042 | A silent controller is stopped for every dialect. | implemented | done | simulator | `src/dialcheck::the_silent_controller_is_stopped_for_every_dialect` | - |
| PCNC-043 | Every alarm and error of the controllers is told in the eight languages. | implemented | done | host-test | `src/ctltext::every_alarm_and_every_error_of_the_table_has_all_eight_languages_and_they_differ` | - |
| PCNC-044 | The Portuguese of the controllers is that of Portugal. | implemented | done | host-test | `src/ctltext::the_portuguese_of_the_controllers_is_that_of_portugal` | - |
| PCNC-045 | Drive a real GRBL, grblHAL or Marlin machine on a serial port. | planned | blocked | none | `planned:real_machine::a_real_controller_runs_a_program_and_stops_on_the_emergency_stop` | The serial port is built behind a feature and has never been opened on a machine; it needs a person at a real machine with the emergency stop in reach. |

## The server

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| PCNC-046 | A program is sent to the server as a job, started and followed to its end. | implemented | done | simulator | `src/server::a_program_is_sent_started_and_followed_to_its_end` | - |
| PCNC-047 | A refusal of the server is told by its code in the eight languages. | implemented | done | simulator | `src/server::a_refusal_is_told_by_its_code_in_each_of_the_eight_languages_with_the_detail_of_the_server` | - |
| PCNC-048 | A login that is gone ends the following at once and says to log in. | implemented | done | simulator | `src/server::a_login_that_is_gone_ends_the_following_at_once_and_says_to_log_in` | - |
| PCNC-049 | A server that is not there is tried again with growing pauses and the link says so. | implemented | done | simulator | `src/server::a_server_that_is_not_there_is_tried_again_with_growing_pauses_and_the_link_says_so` | - |
| PCNC-050 | The token of the server is never shown. | implemented | done | host-test | `src/serverlink::the_store_is_shared_by_its_clones_and_never_shows_the_token` | - |
| PCNC-051 | The emergency stop is sent to the machine on the server with no body and says whether the machine is latched. | implemented | done | simulator | `src/cncroutes::the_emergency_stop_is_sent_with_no_body_and_says_whether_the_machine_is_latched` | - |
| PCNC-052 | The release of the stop needs an administrator and a confirmation, and sends nothing without them. | implemented | done | simulator | `src/cncroutes::the_release_needs_an_administrator_and_a_confirmation_and_sends_nothing_without_them` | - |
| PCNC-053 | A jog on the server is made exactly when it is inside the limits. | implemented | done | simulator | `src/cncroutes::for_values_from_a_fixed_seed_a_jog_is_made_exactly_when_it_is_inside_the_limits` | - |
| PCNC-054 | Reach a real EH3D server with a real machine registered on it. | planned | blocked | none | `planned:real_server::a_job_sent_to_a_real_server_reaches_a_real_machine` | The server is reached only by tests in memory; it needs the server deployed and a machine registered. |

## Checks of a program before it runs

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| PCNC-055 | The summary reads the moves, the reach and the time and refuses a program that is out of the travel when the settings say so. | implemented | done | host-test | `src/plan::a_move_past_the_travel_is_found_and_refused_only_when_the_settings_say_so` | - |
| PCNC-056 | A rapid below the surface is counted. | implemented | done | host-test | `src/lint::a_rapid_below_the_surface_is_counted` | - |
| PCNC-057 | A cut with the spindle off, and a spindle with no speed, are found. | implemented | done | host-test | `src/lint::a_cut_with_the_spindle_off_and_a_spindle_with_no_speed_are_found` | - |
| PCNC-058 | A feed or a speed above the limit is found and the limit is a setting. | implemented | done | host-test | `src/lint::a_feed_or_a_speed_above_the_limit_is_found_and_the_limit_is_a_setting` | - |
| PCNC-059 | Each check has its own switch and the others keep working. | implemented | done | host-test | `src/lint::each_of_the_new_checks_has_its_own_switch_and_the_others_keep_working` | - |
| PCNC-060 | The dangerous commands are named with their line. | implemented | done | host-test | `src/progcheck::the_dangerous_commands_are_named_with_their_line` | - |
| PCNC-061 | A program that enters a keep-out box is refused with its line and the box. | implemented | done | host-test | `src/gui/keepout::a_program_that_enters_a_box_is_refused_with_its_line_and_the_box` | - |
| PCNC-062 | An arc is tested along its curve and not by its chord against the boxes. | implemented | done | host-test | `src/boxes::an_arc_is_tested_along_its_curve_and_not_by_its_chord` | - |
| PCNC-063 | Canned cycles are worked out into their moves. | implemented | done | host-test | `src/cycles::a_simple_drilling_cycle_goes_down_to_the_depth_and_back_to_the_r_plane` | - |
| PCNC-064 | Cutter compensation is worked out in simulation and its misuse is said with the line. | prototype | partial | host-test | `src/cutcomp::a_square_cut_on_the_left_is_shifted_inside_by_the_radius` | The shifted path is a model of what a controller does; no controller has been compared with it. |
| PCNC-065 | The offsets that a G10 line writes, and the work systems in use, are listed. | implemented | done | host-test | `src/wcs::the_offsets_a_g10_line_writes_are_listed_with_their_numbers` | - |
| PCNC-066 | A tool length that the table does not have, or a compensation with no tool, is named. | implemented | done | host-test | `src/offsets::an_h_that_the_table_does_not_have_is_named` | - |
| PCNC-067 | A cut deeper than the reach of the holder is named with its line and the deepest depth. | implemented | done | host-test | `src/holder::a_cut_deeper_than_the_reach_is_named_with_its_line_and_the_deepest_depth` | - |
| PCNC-068 | The life of a tool is followed pass by pass, and a worn-out tool is found. | implemented | done | host-test | `src/life::the_minutes_already_used_count_and_the_pass_where_the_life_runs_out_is_named` | - |
| PCNC-069 | A harder material uses more of the life of a tool. | implemented | done | host-test | `src/consumption::a_harder_material_uses_more_of_the_life_of_the_tool` | - |
| PCNC-070 | The library of materials is valid and holds the program to the ranges of the material in use. | implemented | done | host-test | `src/materials::the_shipped_library_is_valid_and_every_material_has_both_ranges` | - |
| PCNC-071 | What a macro leaves on when it ends is named with its line. | implemented | done | host-test | `src/macrocheck::what_is_left_on_is_named_with_its_line` | - |
| PCNC-072 | A system line drops the whole macro. | implemented | done | host-test | `src/macros::a_system_line_drops_the_whole_macro` | - |
| PCNC-073 | A program is checked against the format of the controller it is for. | implemented | done | host-test | `src/postcheck::a_line_longer_than_the_buffer_counts_without_its_spaces_and_comments` | - |
| PCNC-074 | Thousands of made-up programs pass through every analysis without a panic. | implemented | done | host-test | `src/fuzz::a_thousand_made_up_programs_pass_through_every_analysis_without_a_panic` | - |

## Path, time, edit and report

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| PCNC-075 | The path of a program is drawn in order with rapids told from cuts. | implemented | done | host-test | `src/gcode::moves_are_drawn_in_order_with_rapids_told_from_cuts` | - |
| PCNC-076 | An arc is followed along its curve from end to end. | implemented | done | host-test | `src/gcode::an_arc_by_its_centre_is_followed_along_its_curve_from_end_to_end` | - |
| PCNC-077 | The look-ahead can only make the plan slower, never unsafe. | implemented | done | host-test | `src/lookahead::the_window_of_the_look_ahead_can_only_make_the_plan_slower_never_unsafe` | - |
| PCNC-078 | The parts of the time add up to the time of the statistics plus the dwells. | implemented | done | host-test | `src/timesplit::the_parts_add_up_to_the_time_of_the_statistics_plus_the_dwells` | - |
| PCNC-079 | The tool is found at any moment of the simulation of the program. | implemented | done | host-test | `src/sim::the_tool_is_found_at_any_moment` | - |
| PCNC-080 | The plan is the true view from above and a click on it gives back the point of the table. | implemented | done | host-test | `src/view::the_plan_is_the_true_view_from_above_with_x_to_the_right_and_y_up` | - |
| PCNC-081 | A click on a move selects it and shows its line. | implemented | done | host-test | `src/gui/pathview::a_click_on_a_move_selects_it_and_shows_its_line` | - |
| PCNC-082 | The work report is one page of HTML with no script and no link. | implemented | done | host-test | `src/workreport::the_page_is_one_document_with_no_script_and_no_link_and_the_name_is_escaped` | - |
| PCNC-083 | The editor finds the matches in order, and capitals count only when asked. | implemented | done | host-test | `src/editor::the_matches_are_found_in_order_and_capitals_count_only_when_asked` | - |
| PCNC-084 | Bookmarks are kept next to the program and come back when it is opened again. | implemented | done | host-test | `src/bookmarks::the_text_of_the_file_reads_back_for_the_same_program_and_not_for_another` | - |
| PCNC-085 | Tab finishes the code in the command box and lists the ones that fit. | implemented | done | host-test | `src/complete::several_codes_give_their_common_start_and_the_list` | - |
| PCNC-086 | Every G-code word has its help in the eight languages. | implemented | done | host-test | `src/gcodehelp::every_entry_has_its_text_in_the_eight_languages` | - |
| PCNC-087 | The log of the session is exported in the format of the extension and never over a file. | implemented | done | host-test | `src/gui/files::the_log_of_the_session_is_exported_in_the_format_of_the_extension_and_never_over_a_file` | - |
| PCNC-088 | The log in CSV defuses formulas. | implemented | done | host-test | `src/logbook::csv_quotes_what_it_must_and_defuses_formulas` | - |

## Probe, height map, tool setter and changer

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| PCNC-089 | A grid of probe touches measures a tilted board. | implemented | done | simulator | `src/probe::a_grid_measures_a_tilted_board` | - |
| PCNC-090 | A touch that misses ends the grid and puts the distances back. | implemented | done | simulator | `src/session::a_touch_that_misses_ends_the_grid_and_puts_the_distances_back` | - |
| PCNC-091 | A probe moves nothing without the confirmation and not on a controller without a probe. | implemented | done | simulator | `src/session::a_probe_moves_nothing_without_the_confirmation_and_not_on_a_controller_without_a_probe` | - |
| PCNC-092 | A cut is split and follows the surface of the height map. | implemented | done | host-test | `src/heightmap::a_cut_is_split_and_follows_the_surface` | - |
| PCNC-093 | A height map is written and read back. | implemented | done | host-test | `src/heightmap::the_map_is_written_and_read_back` | - |
| PCNC-094 | The tool setter moves nothing unconfirmed and gives the length of a believable touch. | implemented | done | simulator | `src/session::the_tool_setter_moves_nothing_unconfirmed_and_gives_the_length_of_a_believable_touch` | - |
| PCNC-095 | The length goes into the table of tools and only into the line of that tool. | implemented | done | host-test | `src/toolsetter::the_length_goes_into_the_table_of_tools_in_its_own_format_and_nothing_else_changes` | - |
| PCNC-096 | A tool changer with a lock and a blow works in simulation: the old tool is put back, the lock blown and the new tool taken. | prototype | partial | simulator | `src/atc::a_change_puts_the_old_tool_back_blows_and_takes_the_new_one` | Only the model of a changer; it is off by default and no real changer has been run. |
| PCNC-097 | A change that goes through a keep-out box refuses the start while the changer is on. | implemented | done | host-test | `src/gui/keepout::a_tool_change_that_goes_through_a_box_refuses_the_start_while_the_changer_is_on` | - |
| PCNC-098 | Probe, set a tool and change a tool on a real machine. | planned | blocked | none | `planned:real_probe::a_real_probe_a_real_setter_and_a_real_changer_give_the_expected_numbers` | Needs a real machine with a probe, a tool setter and a changer. |

## Importing, converting and post-processing

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| PCNC-099 | DXF lines, polylines with bulges, circles and arcs are read with their units. | implemented | done | host-test | `src/import::dxf_lines_polylines_circles_and_arcs_are_read_with_bulges_and_units` | - |
| PCNC-100 | SVG shapes and paths are read in millimetres with Y up. | implemented | done | host-test | `src/import::svg_shapes_and_paths_are_read_in_mm_with_y_up` | - |
| PCNC-101 | HPGL paths are read in plotter units, absolute and relative. | implemented | done | host-test | `src/import::hpgl_paths_are_read_in_plotter_units_absolute_and_relative` | - |
| PCNC-102 | An entity of XML is never expanded and a bomb of them costs nothing. | implemented | done | host-test | `src/import::an_entity_of_xml_is_never_expanded_and_a_bomb_of_them_costs_nothing` | - |
| PCNC-103 | A drawing becomes a program that cuts pass by pass and goes to the origin. | implemented | done | host-test | `src/import::a_drawing_becomes_a_program_that_cuts_pass_by_pass_and_goes_to_the_origin` | - |
| PCNC-104 | A copper layer becomes the isolation round the copper. | implemented | done | host-test | `src/pcb::a_copper_layer_becomes_the_isolation_round_the_copper` | - |
| PCNC-105 | The outline of a board is cut on the outside, in passes, and a cut-out on the inside. | implemented | done | host-test | `src/pcb::the_outline_is_cut_on_the_outside_of_the_board_and_in_passes` | - |
| PCNC-106 | A drill file becomes the holes, tool after tool, with the pecks of the settings. | implemented | done | host-test | `src/pcb::a_drill_file_becomes_the_holes_tool_after_tool_with_the_pecks_of_the_setting` | - |
| PCNC-107 | Bytes made up from a fixed seed never panic the board importer. | implemented | done | host-test | `src/pcb::made_up_bytes_from_a_fixed_seed_never_panic_and_always_end_in_an_answer` | - |
| PCNC-108 | A post-processor applies header, footer, numbers and word rules in order. | implemented | done | host-test | `src/post::header_footer_numbers_and_word_rules_are_applied_in_order` | - |
| PCNC-109 | A program is converted between millimetres and inches line by line, and nothing a machine can show is lost. | implemented | done | host-test | `src/units::there_and_back_again_loses_nothing_a_machine_can_show` | - |
| PCNC-110 | Two equal programs do not differ and a longer cut is a line of the comparison. | implemented | done | host-test | `src/compare::a_longer_cut_a_new_tool_and_a_deeper_pass_are_each_a_line` | - |
| PCNC-111 | A shift, a turn, a scale and a mirror transform a program and undo brings it back. | implemented | done | host-test | `src/transform::a_turn_and_its_opposite_bring_the_program_back` | - |
| PCNC-112 | Run the scripts of a post-processor in a sandbox with every limit a key of the motion file. | planned | pending | none | `planned:post::a_script_runs_in_a_sandbox_with_its_limits` | Decided in a record, not built: this build has no engine and a post-processor that asks for a script says so. |

## Settings, languages and the window

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| PCNC-113 | The settings are written and read back and what is wrong keeps the default. | implemented | done | host-test | `src/settings::the_settings_are_written_and_read_back` | - |
| PCNC-114 | Every value of the motion file has its range, the defaults are the cautious ones and a value out of range is named. | implemented | done | host-test | `src/motioncfg::values_are_read_and_the_wrong_ones_are_named_and_keep_their_default` | - |
| PCNC-115 | Machine profiles are saved, used and deleted without touching what does not belong to a machine. | implemented | done | host-test | `src/profiles::using_a_profile_does_not_touch_what_does_not_belong_to_a_machine` | - |
| PCNC-116 | A machine template changes its values and leaves the rest of the settings. | implemented | done | host-test | `src/templates::applying_a_template_changes_its_values_and_leaves_the_rest_of_the_settings` | - |
| PCNC-117 | Each workspace shows what its task needs. | implemented | done | host-test | `src/panels::each_workspace_shows_what_its_task_needs` | - |
| PCNC-118 | A damaged layout never stops the window and the factory one comes back. | implemented | done | host-test | `src/panels::a_damaged_layout_never_stops_the_window_and_the_factory_one_comes_back` | - |
| PCNC-119 | Every workspace and every panel draws with a program loaded and without one. | implemented | done | host-test | `src/gui/shell::every_workspace_draws_with_a_program_loaded_and_without_one` | - |
| PCNC-120 | The Chinese and the Japanese words of the panels are never the English. | implemented | done | host-test | `src/paneltext::the_chinese_and_the_japanese_are_never_the_english` | - |
| PCNC-121 | The Portuguese of the messages is that of Portugal. | implemented | done | host-test | `src/words::portuguese_is_the_one_of_portugal` | - |
| PCNC-122 | The help of the command line names every option it reads. | implemented | done | host-test | `src/cli::the_help_names_every_option_the_command_line_reads` | - |
| PCNC-123 | Every tool of the command line gives 0 or 1 on every example in every language with no hole left. | implemented | done | host-test | `tests/tools_coverage::every_tool_gives_0_or_1_on_every_example_in_every_language_with_no_hole_left` | - |
| PCNC-124 | The README and both scripts show the same sample, and the arguments of the README are accepted. | implemented | done | host-test | `tests/readme::the_readme_and_both_scripts_show_the_same_sample` | - |
| PCNC-125 | A person looks at the window on a screen at a machine and finds it usable. | planned | blocked | none | `planned:operator::an_operator_runs_a_job_with_the_window_at_a_machine` | Needs a person at a screen, at a machine. |
| PCNC-126 | The controller of HYDRA-UMC drives a machine from this program. | planned | blocked | none | `planned:hydra::the_controller_of_hydra_umc_runs_a_program` | Needs the contract with HYDRA-UMC and its controller. |

## Klipper through Moonraker

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| PCNC-127 | Klipper is driven through Moonraker over HTTP from `--klipper`, behind the guard, and the credential goes in one header and nowhere else. | implemented | done | simulator | `tests/klipper::klipper_is_driven_over_http_from_the_option_in_the_guard_and_the_credential_goes_in_one_header` | - |
| PCNC-128 | A failure of the HTTP link of Klipper has a number and is told in the eight languages. | implemented | done | simulator | `tests/klipper::nothing_listening_is_a_failure_of_the_link_with_a_number_and_words_in_each_language` | - |
| PCNC-129 | An address outside the local network, and a name, are refused unless the switch of the motion file is on, before any connection. | implemented | done | simulator | `tests/klipper::an_address_outside_the_local_network_needs_the_switch_and_a_name_needs_it_too` | - |
| PCNC-130 | The `klipper.*` keys of the motion file are ranged, a fault names its line and the credential is never shown. | implemented | done | host-test | `src/motioncfg::the_klipper_keys_are_read_checked_and_the_credential_is_never_shown` | - |
| PCNC-131 | A simulated Klipper is chosen with `--sim klipper` and sits in the guard like the other dialects. | implemented | done | simulator | `src/link::klipper_is_simulated_with_sim_and_sits_in_the_guard_like_the_others` | - |
| PCNC-132 | A refusal of Klipper is logged in the language of the window. | implemented | done | simulator | `src/session::a_refusal_of_klipper_is_logged_in_the_language_of_the_window` | - |
| PCNC-133 | Drive a real Klipper through a real Moonraker. | planned | blocked | none | `planned:real_machine::a_real_klipper_runs_a_program_and_stops_on_the_emergency_stop` | The HTTP link has only met a stand-in Moonraker in the tests; it needs a machine with Klipper and a person at it with the emergency stop in reach. |
| PCNC-134 | A starting point of a material of the shared database is told for an operation and a cutter of a diameter: between two known points it is a straight line and says so, outside them it is a refusal, and it says that nothing was tried on a machine. | implemented | done | host-test | `shared::a_starting_point_from_a_seed_stays_between_the_points_it_comes_from_and_never_goes_outside_them` | The points of the database are starting values (the seed was never tried on a machine); this program draws from it and does not judge it. |
| PCNC-135 | `--feeds` tells the point in the eight languages and refuses what it cannot answer with the exit code 2. | implemented | done | host-test | `shared::feeds_tells_the_point_with_its_chip_load_and_what_a_person_should_know_in_eight_languages` | - |
| PCNC-136 | The materials that a router works are listed, all or by family, with the points each has. | implemented | done | host-test | `shared::materials_lists_what_the_database_has_for_a_router_and_each_family_on_its_own` | - |
| PCNC-137 | The chip load and the cutting speed of a point are worked out and a chip too thin or too thick for the diameter is said. | implemented | done | host-test | `shared::the_chip_load_follows_the_feed_the_speed_and_the_teeth_and_the_verdict_only_goes_up` | The limits (0.2 % and 8 % of the diameter) are a rule of thumb for small cutters, never measured on a machine. |
| PCNC-138 | The material of the settings can be one of the shared database and the check of a program is held to the feed and the speed of its points. | implemented | done | host-test | `shared::the_material_of_the_settings_can_be_one_of_the_shared_database` | - |
| PCNC-139 | The format of a program can be checked for Marlin and for Klipper, from what their documentation lists. | implemented | done | host-test | `shared::the_formats_of_marlin_and_klipper_say_what_a_milling_program_takes_from_them` | The lists are read from the documentation, not from a controller: a code that is not listed is a reason to read the line. |
| PCNC-140 | The program, the drawing, the table of tools and the motion file saved in the branch cnc of a project come back byte for byte. | implemented | done | host-test | `eh3dfile::a_saved_branch_comes_back_byte_for_byte` | - |
| PCNC-141 | Writing the branch cnc leaves the bytes of every other branch, known or not, as they were. | implemented | done | host-test | `eh3dfile::the_other_branches_keep_their_bytes_whatever_this_program_does` | - |
| PCNC-142 | A copy taken from another branch keeps the hash and the format of its source and is told as out of date when the source changes. | implemented | done | host-test | `eh3dfile::a_copy_taken_from_cad_is_told_as_out_of_date_and_taken_again` | - |
| PCNC-143 | An imported G-code program or drawing keeps its original byte for byte with a report of what was read, not read and lost on export. | implemented | done | host-test | `eh3dfile::an_imported_drawing_becomes_a_program_beside_the_original` | - |
| PCNC-144 | A branch of a newer format or a sealed one is not written, with the code of the format, and the file stays as it was. | implemented | done | host-test | `eh3dfile::a_branch_of_a_newer_format_or_sealed_is_not_written` | - |
| PCNC-145 | A damaged project is an error and never a crash, with damage made from a fixed seed. | implemented | done | host-test | `eh3dfile::damaged_projects_are_errors_and_never_a_crash` | - |
| PCNC-146 | A signed branch is listed as signed, and a change that removes the signature says so. | implemented | done | host-test | `eh3dfile::a_signed_branch_is_listed_and_a_change_says_the_signature_is_gone` | - |
| PCNC-147 | The window opens, saves and saves under another name a project, keeping the bytes of every other branch. | implemented | done | host-test | `src/gui/suite::saving_under_another_name_keeps_every_branch_and_another_project_is_asked_about` | - |
| PCNC-148 | The window asks before it writes a signed branch or a project it did not open, and says that a signature was removed. | implemented | done | host-test | `src/gui/suite::a_signed_branch_is_asked_about_and_the_removed_signature_is_told` | - |
| PCNC-149 | The window takes a program of another branch, keeps what is out of date in sight and keeps the reference when it saves. | implemented | done | host-test | `src/gui/suite::a_program_taken_from_cad_is_told_as_out_of_date_and_a_save_keeps_its_reference` | - |
| PCNC-150 | A project never replaces a running program, and the files of the machine go into one only when asked and never with a credential. | implemented | done | host-test | `src/gui/suite::the_files_of_the_machine_go_only_when_asked_and_never_with_a_credential` | - |
| PCNC-151 | The command line never puts a motion file that holds the credential of a link into a project: the save is refused and nothing is written. | implemented | done | host-test | `eh3dfile::the_command_line_never_puts_the_credential_of_a_link_in_a_project` | - |
| PCNC-152 | The window has the panel of the attachments of the project, and it is shown and drawn like every other panel. | implemented | done | host-test | `src/gui/shell::every_panel_can_be_shown_and_drawn_in_turn` | - |
| PCNC-153 | The limits of the panel of the attachments are read from the settings file, kept when it is written and given to the panel. | implemented | done | host-test | `src/settings::the_lines_of_the_limits_of_the_attachments_are_kept_and_give_the_limits` | - |
| PCNC-154 | A file dropped on the panel of the attachments is attached there, and the files the window opens are left to the window. | implemented | done | host-test | `src/gui/dialogs::the_files_the_window_opens_are_told_apart_from_the_ones_of_the_attachments` | The drop itself is tested in EH3D-COMMON with made-up events, not on a desktop. |
| PCNC-155 | The emphasised words of the window take the text colour of the theme, and words are readable on the panels, the title bars and the tab bars of every theme, light or dark. | implemented | done | host-test | `src/gui::the_words_that_stand_out_are_read_on_every_theme` | The colours are checked as numbers, not by a person on a screen. |
| PCNC-156 | The emergency stop of the window also goes to the machine of the server while a job is not known to be over, falls back to the plain stop and says when neither arrived. | implemented | done | host-test | `src/server::the_emergency_stop_goes_to_the_machine_and_falls_back_to_the_plain_stop` | Only against a stand-in server. |
| PCNC-157 | A stop that does not reach the server says that the machine may still be moving, each time it fails. | implemented | done | host-test | `src/server::a_stop_that_does_not_arrive_says_the_machine_may_still_move_every_time` | - |
| PCNC-158 | A job is made again only when the server says the old one ended or that it does not have it. | implemented | done | host-test | `src/server::a_new_job_is_made_only_when_the_server_says_it_does_not_have_the_old_one` | - |
| PCNC-159 | A program that runs or waits paused is not replaced, cleared or rewound. | implemented | done | host-test | `src/gui/queue::a_running_program_is_not_replaced_cleared_rewound_or_overtaken_by_the_queue` | - |
| PCNC-160 | A stop or an emergency stop during the wait between two runs ends it and nothing starts. | implemented | done | host-test | `src/gui/queue::a_stop_or_an_emergency_stop_during_the_wait_ends_it_and_nothing_starts` | - |
| PCNC-161 | A stop after the last line went out stops the queue. | implemented | done | host-test | `src/gui/queue::a_stop_after_the_last_line_went_out_stops_the_queue` | - |
| PCNC-162 | A restarted program says its way of moving after the pause. | implemented | done | host-test | `src/restart::a_line_that_names_only_axes_still_cuts_after_a_restart` | - |
| PCNC-163 | A resume is refused where an arc goes on unsaid, with `G93` or `G95` and with `G43`. | implemented | done | host-test | `src/session::a_resume_is_refused_where_an_arc_a_feed_mode_or_a_table_offset_cannot_be_written_back` | - |
| PCNC-164 | The feed and the length offset are written back in the units they were given in. | implemented | done | host-test | `src/restart::the_feed_and_the_offset_are_written_in_the_units_they_were_given_in` | - |
