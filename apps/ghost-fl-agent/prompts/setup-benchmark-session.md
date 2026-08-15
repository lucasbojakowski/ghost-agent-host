You are setting up **Ghost FL Studio Benchmark Session A** as a broad raw-agent integration test.

This setup is allowed to make substantial changes, but it must run only in a **fresh or disposable** FL Studio project.

Before changing anything:

1. Inspect the current session, channel names, tempo, and mixer routing.
2. If the project already contains meaningful user work, a large custom structure, or anything that does not look fresh/disposable, make **no changes** and finish with `BENCHMARK_SETUP_ABORTED`, explaining why.
3. Do not reset the whole project and do not remove existing channels/effects just to force the fixture. Prefer renaming/reusing fresh default channels and adding what is missing.
4. When adding a generator or effect, discover the exact installed plugin name through the browser tool first. Never guess plugin names.

If the project is suitable, create the following reproducible benchmark fixture.

## Tempo

Set the project tempo to **124 BPM**.

## Channel Rack

Ensure the first nine working channels exist and are named, in order:

1. Kick
2. Snare
3. Closed Hat
4. Open Hat
5. Perc
6. Bass
7. Chords
8. Lead
9. Vox Chop

Reuse fresh/default channels where practical. If more channels are required, inspect the Plugin database and add available stock FL generators until there are enough. The exact generator is not important for this structural benchmark; exact discovered plugin names are mandatory. Do not remove extra existing fresh-project channels unless the user explicitly asks later.

Route those nine channels to mixer tracks 1 through 9 respectively.

## Mixer names

Rename mixer inserts 1 through 15 exactly as follows:

1. Kick
2. Snare
3. Closed Hat
4. Open Hat
5. Perc
6. Bass
7. Chords
8. Lead
9. Vox
10. Drum Bus
11. Music Bus
12. Vocal Bus
13. Reverb
14. Delay
15. Parallel Comp

## Mixer colors

Use coherent group colors so later organization tests have real project structure to inspect:

- tracks 1–5, drums: `#E05A47`
- tracks 6–8, music: `#5E8CFF`
- track 9 and Vocal Bus 12: `#C56CF0`
- Drum Bus 10: `#F29F67`
- Music Bus 11: `#72A7FF`
- Reverb 13, Delay 14, Parallel Comp 15: `#58C9B9`

## Routing fixture

Create this deliberately imperfect but coherent routing state. Inspect routing before each family of changes and preserve unrelated routes.

- Kick 1 -> Master directly.
- Snare 2 -> Master directly.
- Closed Hat 3 -> Drum Bus 10, not directly to Master.
- Open Hat 4 -> Drum Bus 10, not directly to Master.
- Perc 5 -> Master directly. This is intentional and will be repaired by a later benchmark task.
- Bass 6 -> Master directly. This is intentional and will be repaired by a later benchmark task.
- Chords 7 -> Music Bus 11, not directly to Master.
- Lead 8 -> Music Bus 11, not directly to Master.
- Vox 9 -> Vocal Bus 12, not directly to Master.
- Drum Bus 10 -> Master.
- Music Bus 11 -> Master.
- Vocal Bus 12 -> Master.
- Reverb 13 -> Master.
- Delay 14 -> Master.
- Parallel Comp 15 -> Master.

Do not create sends into Reverb, Delay, or Parallel Comp yet. Those buses should exist but remain unused so later tests can exercise send/routing creation.

## Playlist labels

Rename playlist tracks 1 through 9:

1. Drums
2. Bass
3. Chords
4. Lead
5. Vox Chop
6. FX
7. Automation
8. Prints
9. References

Do not create or delete playlist content just to satisfy this naming fixture.

## Optional processor fixture

If, and only if, browser discovery clearly exposes a stock FL dynamics processor with an unambiguous exact name, place one instance in **Vocal Bus / mixer track 12 / slot 1**. Do not alter its parameters. If no suitable exact stock dynamics plugin can be established confidently, leave the slot empty and report that as an optional omission. Do not substitute a third-party plugin.

## Final verification

After setup, re-read enough native state to verify the result rather than relying on memory. At minimum verify:

- tempo;
- channel names and channel-to-mixer assignments;
- mixer names/effects visible in session context where available;
- full mixer routing;
- playlist track names.

Summarize the actual tool-driven result and any deviations.

Finish with exactly one of these markers:

- `BENCHMARK_SETUP_GREEN` — all required structural fixture items were verified; optional processor may be omitted.
- `BENCHMARK_SETUP_PARTIAL` — safe setup ran but one or more required structural items could not be established or verified; list each deviation.
- `BENCHMARK_SETUP_ABORTED` — the starting project was not clearly fresh/disposable, so no setup mutations were made.
