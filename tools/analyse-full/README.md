# analyse-full

analyse-full captures the output of a live Ghost Tap loaded in FL Studio, runs the
ghost-audio maximum-quality analysis profile, and prints the complete result as Markdown.
The Tap writes a lossless 32-bit floating-point WAV at FL Studio's active sample rate; there is
no system-audio loopback or sample-rate conversion in this path.

Load Ghost Tap on the signal path you want to measure and make sure FL Studio is processing it.
Then run this from the workspace root:

    cargo run -p analyse-full --release

The default capture is 10 seconds. Start playback before running the command or within 30 seconds
after it is armed. Capture begins when the Tap sees real signal and includes its short pre-roll.

Choose another length or Tap instance with:

    cargo run -p analyse-full --release -- --length 15 --tap-instance 1

--duration is accepted as an alias for --length. Ghost Tap currently accepts lengths from
0.05 through 20 seconds; the effective upper limit can be lower at unusually high FL Studio
sample rates because the plugin uses a fixed real-time-safe capture buffer.

When exactly one Tap is live it is selected automatically. If more than one is live, the command
prints their instance IDs and requires --tap-instance so it cannot silently analyze the wrong path.

Progress is written to stderr and Markdown is written to stdout, so a report can be saved without
mixing in progress messages:

    cargo run -q -p analyse-full --release -- --length 10 > analysis.md

The captured WAV remains in Ghost Tap's artifact directory and its path is included in the report.
