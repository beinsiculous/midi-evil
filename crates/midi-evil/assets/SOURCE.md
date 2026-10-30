# MIDI Evil branding

MIDI Evil uses the Dr. Maxwell icon and full MIDI Evil splash selected by the user from the Insiculous branding assets.

Source files (retained unmodified):

- `/home/jedi/projects/insiculous/deion_assets/ai/branding/midi_evil/ai_midi_evil_icon_64x64.aseprite`
- `/home/jedi/projects/insiculous/deion_assets/ai/branding/midi_evil/ai_midi_evil_icon_64x64.png`
- `/home/jedi/projects/insiculous/deion_assets/ai/branding/midi_evil/ai_midi_evil_splash_320x192.aseprite`
- `/home/jedi/projects/insiculous/deion_assets/ai/branding/midi_evil/ai_midi_evil_splash_320x192.png`

`midi-evil-icon.aseprite` and `midi-evil-splash.aseprite` are copies of the editable source masters. Their matching PNG files were exported in Aseprite for use by the app. No NanoStudio artwork is used in the rendered UI.

## Maxwell workstation controls

`maxwell-controls.aseprite` is an original, layered 128 × 16 control-icon atlas
created in Aseprite for the desktop redesign. It uses the existing Maxwell
ivory/red palette; no reference-app pixels are included. The eight 16 × 16 cells
are song, piano roll, mixer, synth, play, stop, save, and project. The app samples
the exported `maxwell-controls.png` with nearest-neighbor filtering.

The editable Aseprite master is authoritative. `maxwell-controls.lua` records the
initial drawing recipe and can recreate it from the repository root:

```bash
aseprite -b --script crates/midi-evil/assets/maxwell-controls.lua
```

After editing the master, export without rerunning the recipe:

```bash
aseprite -b crates/midi-evil/assets/maxwell-controls.aseprite \
  --save-as crates/midi-evil/assets/maxwell-controls.png
```
