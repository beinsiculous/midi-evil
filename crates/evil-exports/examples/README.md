# Public native project audio

From the workspace root:

```sh
cargo run -p evil-exports --example project_audio -- midi song.evil song.mid
cargo run -p evil-exports --example project_audio -- wav song.evil mix.wav
cargo run -p evil-exports --example project_audio -- note song.evil note.wav 0 74 480
```

The last form solos instrument index 0 on MIDI key 74, velocity 127, for 480 ticks.
It uses the project's sample rate, gain, tempo and patch/seed settings, then averages
the stereo render into PCM16 mono. Patch paths stay relative to the source project.
The source project is never changed. The MIDI path uses evil-exports' atomic writer;
WAV destinations are rendering intermediates and should be staged outside runtime assets.
No private compatibility crates or desktop editor changes are required.

Isolated notes omit track automation and arrangement, retain instrument and project
mix settings, and are limited to ten tempo-mapped seconds. An unmapped drum key
fails before writing. Exports print their destination and WAV renderer diagnostics.

Each operation requires exactly the arguments shown; unknown operations and surplus
arguments are rejected before the project is opened.
