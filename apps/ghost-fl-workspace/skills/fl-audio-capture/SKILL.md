---
name: fl-audio-capture
description: Safely capture an exact FL Studio mixer signal through Ghost Tap, with explicit insert/slot verification, transport positioning, arming, playback, collection, and result checks.
tools:
  - ghost_tap_list
  - ghost_tap_arm
  - ghost_tap_collect
  - ghost_audio_analyze
  - fl_scripting_search
  - fl_scripting_describe
  - fl_scripting_call
---

# FL audio capture

Use this skill whenever you need to capture live audio from FL Studio through Ghost Tap. Do not treat a visible Ghost Tap window, an old status file, or a remembered mixer slot as proof that the desired signal is being captured.

## Required flow

1. **Identify the exact source signal.** Establish the target mixer insert and whether the desired observation is pre/post a particular processing stage. Inspect FL state rather than guessing track or slot names.
2. **Inspect the target insert's effect slots.** Confirm whether Ghost Tap is already loaded on that exact mixer insert and determine the slot position relative to the processing you intend to measure.
3. **Add or move Ghost Tap only when necessary.** If it is absent, insert it into the intended slot. If it is present in the wrong signal position, ask before changing a meaningful existing chain unless the user already authorized that structural edit. Preserve unrelated effects.
4. **Verify the live tap control plane.** Call `ghost_tap_list`. Match the fresh live instance to the FL state you just established. If there are multiple live taps, do not guess an instance. Use FL inspection and user context to disambiguate.
5. **Set the playlist/transport start position.** Move FL to the exact desired start point before arming. Verify the transport position. If the capture is section-specific, make the bar/beat/timeframe explicit.
6. **Arm before playback.** Call `ghost_tap_arm(instanceId, durationSeconds)`. Keep the returned `requestId`. Arming waits for signal; it does not start FL playback for you.
7. **Start playback through FL tools.** Start transport only after the capture is armed. Do not call a blocking collection tool before playback has started.
8. **Allow the requested timeframe to pass.** The Ghost Tap capture completes from the audio callback once the armed duration has been collected. Avoid unrelated DAW mutations while the measurement is in flight.
9. **Collect the exact request.** Call `ghost_tap_collect(instanceId, requestId, timeoutSeconds)` using the same request id. Never accept an artifact from a different request as the result.
10. **Stop or restore transport when appropriate.** Return FL to a sensible state for the producer unless they explicitly want playback to continue.
11. **Check the artifact before reasoning from it.** Confirm sample rate, frame count, duration, WAV path, request id, and any tap error. Then call `ghost_audio_analyze` on the returned WAV path when acoustic analysis is required.
12. **Verify the result corresponds to the intended signal.** If the capture is silent, truncated, the wrong duration, or acoustically inconsistent with the selected mixer source, inspect routing/slot placement and repeat rather than rationalizing bad data.

## Important failure modes

- A Ghost Tap can be live but located on the wrong mixer insert.
- A correct insert can still produce the wrong measurement if the Tap is in the wrong slot relative to processing.
- `ghost_tap_arm` must happen before playback.
- `ghost_tap_collect` is for an already-started request; do not use it as the step that is supposed to start playback.
- Multiple fresh tap instances require explicit disambiguation.
- Transport position is part of capture provenance. Set and verify it before the take.
- Never infer success merely because a WAV file exists; inspect the returned artifact and analysis.
