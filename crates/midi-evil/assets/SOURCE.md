# MIDI Evil branding

MIDI Evil uses the Dr. Maxwell icon and full MIDI Evil splash selected by the user from the Insiculous branding assets.

Source masters from the user-selected Insiculous branding set (retained unmodified):

- `ai_midi_evil_icon_64x64.aseprite`
- `ai_midi_evil_icon_64x64.png`
- `ai_midi_evil_splash_320x192.aseprite`
- `ai_midi_evil_splash_320x192.png`

`midi-evil-icon.aseprite` and `midi-evil-splash.aseprite` are copies of the editable source masters. Their matching PNG files were exported in Aseprite for use by the app. No NanoStudio artwork is used in the rendered UI.

## Redistribution audit — 2026-11-27

The selected source revision is Insiculous `deion_assets` commit `de1390b`
(matching `ai/branding/midi_evil/` as refreshed in `de18b6a`). All four local
icon/splash files are byte-identical to that revision:

| File | SHA-256 |
| --- | --- |
| `midi-evil-icon.aseprite` | `f6e242d5679deaf89d82d2afa1cdc30b26d294ee5017d7cb527400c37e30121b` |
| `midi-evil-icon.png` | `4f2d787b934c14ad05429f6dce9f91979546474a46c781a9199e4cc2d117fb03` |
| `midi-evil-splash.aseprite` | `c239a4e10d968434eeca92dbd7a6ab6bb5ac74d84a1eeebd3e5d4c0697970197` |
| `midi-evil-splash.png` | `d158df476e238004554cfd9941e484427774119e6e9a20439552b12cbd83bc05` |

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
