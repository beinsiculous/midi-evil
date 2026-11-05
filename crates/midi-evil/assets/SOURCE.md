# MIDI Evil branding

MIDI Evil uses the Dr. Maxwell icon and full MIDI Evil splash selected by the user from the Insiculous branding assets.

Source masters from the user-selected Insiculous branding set (retained unmodified):

- `ai_midi_evil_icon_64x64.aseprite`
- `ai_midi_evil_icon_64x64.png`
- `ai_midi_evil_splash_320x192.aseprite`
- `ai_midi_evil_splash_320x192.png`

`midi-evil-icon.aseprite` and `midi-evil-splash.aseprite` are copies of the editable source masters. Their matching PNG files were exported in Aseprite for use by the app. No NanoStudio artwork is used in the rendered UI.

## Redistribution audit — 2026-11-05

The selected source revision is Insiculous `deion_assets` commit `1a7b363`, in
`ai/branding/midi_evil/`. All four local icon/splash files are byte-identical to
that revision:

| File | SHA-256 |
| --- | --- |
| `midi-evil-icon.aseprite` | `37b25cd65397f2cf7ccb6a7976253140d3d9c98b6892fc2b71b0ec469ede7e77` |
| `midi-evil-icon.png` | `d1b1087279206e19e1a2b0d6b3d79b5f01844d680fa19b24854619ccad14d82a` |
| `midi-evil-splash.aseprite` | `064d52f7d6f6330e5b78b72bba2693b5fc70fb20b3bfd184f842b634ebcc7b28` |
| `midi-evil-splash.png` | `619e4f2b259cb06ec3b2914f9c83b10c56d8619aba4b67fda3422c801a8ce69f` |

The source repository's `README.md`, **The AI-asset quarantine rule**, permits
AI-generated assets in **free releases** and requires a human hand-cleanup pass
before any **paid release**. The public/free package audit therefore retains
these selected assets and their provenance. This check is not paid-release
artwork clearance; a paid release must document the required human cleanup or
replace the assets. Later changes to the source masters are not silently
tracked by this repository.

`dr-maxwell.aseprite` and `dr-maxwell.png` are legacy, currently unused copies
of the Insiculous Maxwell icon and follow the same free/paid source policy.
`rusty-logo.aseprite` and `rusty-logo.png` are legacy original project artwork;
their drawing recipe is `tools/aseprite/draw_logo.lua`. These four legacy files
are excluded from the published desktop package.

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
