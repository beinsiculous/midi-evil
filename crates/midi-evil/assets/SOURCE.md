# MIDI Evil branding

MIDI Evil uses the Dr. Maxwell icon and full MIDI Evil splash selected by the user from the Insiculous branding assets.

Source masters from the user-selected Insiculous branding set (retained unmodified):

- `ai_midi_evil_icon_64x64.aseprite`
- `ai_midi_evil_icon_64x64.png`
- `ai_midi_evil_splash_320x192.aseprite`
- `ai_midi_evil_splash_320x192.png`

`midi-evil-icon.aseprite` and `midi-evil-splash.aseprite` are copies of the editable source masters. Their matching PNG files were exported in Aseprite for use by the app. No NanoStudio artwork is used in the rendered UI.

## Maxwell workstation controls

`maxwell-controls.aseprite` is an original, layered 144 × 16 control-icon atlas
created in Aseprite for the desktop redesign. It uses the existing Maxwell
ivory/red palette; no reference-app pixels are included. The nine 16 × 16 cells
are song, piano roll, mixer, synth, play, stop, save, keyboard, and pads. The app samples
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

## Maxwell performance keyboard

`maxwell-keyboard.aseprite` is the editable 160 × 96 performance-page badge,
assembled in Aseprite from the unchanged selected Maxwell icon and new pixel
circuitry, meter bars, and EVIL KEYS lettering. `maxwell-keyboard.png` is its
app-ready export. The keys, wheel, and XY pads are drawn by egui; no artwork from
the reference screenshot is used.

Recreate the badge and keyboard atlas cell from the repository root:

```bash
aseprite -b --script crates/midi-evil/assets/maxwell-keyboard.lua
```

After editing the badge master, export it directly:

```bash
aseprite -b crates/midi-evil/assets/maxwell-keyboard.aseprite \
  --save-as crates/midi-evil/assets/maxwell-keyboard.png
```

## Maxwell rhythm lab

`maxwell-beats.aseprite` is the editable 160 × 96 Evil Beats badge, created in
Aseprite from the unchanged selected Maxwell icon, original pad-grid circuitry,
and pixel lettering. `maxwell-beats.png` is its app-ready export. Pad surfaces and
waveforms are drawn with egui. No reference-app artwork or drum samples are used.

```bash
aseprite -b --script crates/midi-evil/assets/maxwell-beats.lua
aseprite -b crates/midi-evil/assets/maxwell-beats.aseprite \
  --save-as crates/midi-evil/assets/maxwell-beats.png
```
