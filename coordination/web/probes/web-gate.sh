#!/bin/bash
# Runs the web gate's mechanical half (plan § W0-web, Gate) in headless Chromium and Firefox:
# the native gate's sequence and its edges; focus into the canvas and out again by keyboard in
# both directions; a held Right across twenty notes driven two ways; leaving the canvas and
# returning, then reading the current note without moving the cursor. Every sequence runs in a
# fresh browser through web-probe.mjs, and every step the gate names carries an expectation, so
# exit status 0 means each key did what the gate says it must. Then the browser-key table, which
# is evidence, not a gate. The person's half — the same keys with Orca — is not this script's.
#
#   coordination/web/probes/web-gate.sh > web-gate-transcript.txt
#
# It builds nothing. It serves the bundle spike/roll-web/build.sh left in the spike worktree's
# spike/roll-web/dist/ (found through `git worktree list`; MIDI_EVIL_WEB_BUNDLE overrides it) on
# 127.0.0.1 with python3's http.server — on a port the server binds itself, with .wasm served as
# application/wasm whatever the machine's MIME table says — checks that the server answers with
# this bundle before anything runs, and stops it however the script exits. Each probe runs under
# a watchdog (WEB_PROBE_TIMEOUT_SECONDS, default 300): a probe that outlives it is sent SIGTERM,
# which Playwright answers by closing its browser, and SIGKILL fifteen seconds later; that counts
# as a failed sequence. Playwright comes from the working set's insiculous_web/node_modules
# (PLAYWRIGHT_FROM names another package.json) and needs Node 24 (NODE overrides the node it
# finds).
#
# The sequences assume the Bach fixture's first track, whose first note is C 5 at bar 1 beat 1.5,
# a sixteenth long, in 4/4 at 64 beats per minute; `--fixture empty` is a one-bar pattern with no
# notes used by one part, `--fixture full` the same bar holding a note on each beat, used by two.
set -u

probes=$(cd "$(dirname "$0")" && pwd)
repository=$(cd "$probes/../../.." && pwd)

find_bundle() {
    local worktree
    worktree=$(git -C "$repository" worktree list --porcelain |
        awk '/^worktree /{path=substr($0, 10)} /^branch refs\/heads\/spike\/w0-accessibility$/{print path}')
    [ -n "$worktree" ] && echo "$worktree/spike/roll-web/dist"
}

find_node() {
    local candidate
    for candidate in "${NODE:-}" "$(command -v node)" "$HOME"/.nvm/versions/node/v24.*/bin/node; do
        if [ -n "$candidate" ] && [ -x "$candidate" ] && "$candidate" --version | grep -q '^v24\.'; then
            echo "$candidate"
            return
        fi
    done
}

bundle=${MIDI_EVIL_WEB_BUNDLE:-$(find_bundle)}
if [ -z "$bundle" ] || [ ! -f "$bundle/index.html" ] || [ ! -f "$bundle/roll_web_bg.wasm" ]; then
    echo "no built bundle at '${bundle}'; run spike/roll-web/build.sh in the spike worktree" >&2
    exit 2
fi
node=$(find_node)
if [ -z "$node" ]; then
    echo "no Node 24 found; set NODE" >&2
    exit 2
fi

runtime=$(mktemp -d)
port_file="$runtime/port"
# The run serves its own copy: a rebuild swapping dist/ mid-run would otherwise hand one page load
# the old glue and the new wasm.
cp -r -- "$bundle" "$runtime/bundle"
python3 - "$runtime/bundle" "$port_file" > "$runtime/server.log" 2>&1 <<'PYTHON' &
import functools, http.server, os, sys

directory, port_file = sys.argv[1], sys.argv[2]


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
    }

    def log_message(self, *arguments):
        pass


server = http.server.ThreadingHTTPServer(
    ("127.0.0.1", 0), functools.partial(Handler, directory=directory)
)
with open(port_file + ".partial", "w") as written:
    written.write(str(server.server_port))
os.rename(port_file + ".partial", port_file)
server.serve_forever()
PYTHON
server=$!
trap 'kill "$server" 2> /dev/null; wait "$server" 2> /dev/null; rm -rf "$runtime"' EXIT

# Ready means: the server is alive, it bound a port, and it serves this bundle — the page carries
# the harness's sentinel and the wasm comes back as application/wasm.
ready=""
for _ in $(seq 100); do
    if ! kill -0 "$server" 2> /dev/null; then
        echo "the bundle server exited:" >&2
        cat "$runtime/server.log" >&2
        exit 2
    fi
    if [ -f "$port_file" ] && python3 - "$(cat "$port_file")" <<'PYTHON' 2> /dev/null; then
import sys, urllib.request

base = f"http://127.0.0.1:{sys.argv[1]}/"
page = urllib.request.urlopen(base + "index.html", timeout=2).read().decode()
assert '<meta name="midi-evil-harness" content="roll-web">' in page
wasm = urllib.request.urlopen(base + "roll_web_bg.wasm", timeout=5)
assert wasm.headers.get_content_type() == "application/wasm"
PYTHON
        ready=yes
        break
    fi
    sleep 0.1
done
if [ -z "$ready" ]; then
    echo "the bundle server did not answer with the harness within ten seconds" >&2
    cat "$runtime/server.log" >&2
    exit 2
fi
url="http://127.0.0.1:$(cat "$port_file")/"
probe_timeout=${WEB_PROBE_TIMEOUT_SECONDS:-300}

probe() {
    timeout --signal=TERM --kill-after=15 "$probe_timeout" \
        "$node" "$probes/web-probe.mjs" --url "$url" "$@"
}

failed=0
run() {
    local browser=$1 title=$2
    shift 2
    echo
    echo "################ $browser: $title"
    probe --browser "$browser" --heading "$title" "$@"
    local status=$?
    if [ "$status" -ne 0 ]; then
        [ "$status" -eq 124 ] && echo "################ the probe outlived its watchdog"
        echo "################ FAILED: $browser: $title"
        failed=1
    fi
}

# A negative control: a sequence that must fail its expectations (exit status 1, not a crash),
# so the probe is shown unable to pass what it exists to catch.
must_fail() {
    local browser=$1 title=$2 wanted=$3
    shift 3
    echo
    echo "################ $browser: negative control: $title"
    local output status
    output=$(probe --browser "$browser" --heading "negative control: $title" "$@" 2>&1)
    status=$?
    printf '%s\n' "$output"
    # Exit status 1 alone is not enough: any stray failed expectation would give it. The control
    # passes only when the probe failed for the reason it exists to show.
    if [ "$status" -eq 1 ] && printf '%s' "$output" | grep -q -F -- "$wanted"; then
        echo "################ the negative control failed, as it must"
    else
        echo "################ FAILED: $browser: negative control exited $status, or not with '$wanted': $title"
        failed=1
    fi
}

# From the page's first stop into the canvas: the roll is where egui's focus starts.
enter=(--key Tab --expect-focus before-editor --key Tab --expect-focus midi-evil-canvas
    --expect-egui roll --expect "Piano roll, no note selected" --expect-announcements 0)

for browser in chromium firefox; do
    run "$browser" "main sequence: focus the roll, read a note, change its pitch and start, delete it, leave and return, read the transport" \
        "${enter[@]}" \
        --key ArrowRight --expect "Piano roll, C 5, bar 1 beat 1.5, 0.25 beats, velocity 127" \
        --expect-announcement "Piano roll, C 5, bar 1 beat 1.5" \
        --key ArrowUp --expect "Piano roll, C sharp 5, bar 1 beat 1.5," \
        --key Control+ArrowRight --expect "Piano roll, C sharp 5, bar 1 beat 1.75," \
        --key Shift+ArrowRight --expect "C sharp 5, bar 1 beat 1.75, 0.5 beats" \
        --key Shift+ArrowLeft --expect "C sharp 5, bar 1 beat 1.75, 0.25 beats" \
        --key Delete --expect "Piano roll, deleted C sharp 5 at bar 1 beat 1.75, C 5, bar 1 beat 2" \
        --expect-announcement "Removed. Piano roll, deleted C sharp 5 at bar 1 beat 1.75" \
        --key Tab --expect-focus after-editor --expect-same --expect-announcements 0 \
        --key Shift+Tab --expect-focus midi-evil-canvas --expect-egui roll \
        --expect "Piano roll, deleted C sharp 5 at bar 1 beat 1.75, C 5, bar 1 beat 2" \
        --expect-announcements 0 \
        --key F6 --expect-egui readout \
        --expect "Transport, stopped, tempo 64 beats per minute, position 0 minutes, 0 seconds" \
        --expect-announcement "Transport, stopped, tempo 64 beats per minute" \
        --key Shift+F6 --expect-egui roll --expect "Piano roll, deleted C sharp 5" \
        --key Escape --expect-focus midi-evil-canvas --expect-egui roll \
        --expect "Piano roll, deleted C sharp 5" --expect-announcement "Piano roll, deleted C sharp 5"

    # The zoom slider is two stops: the slider, and its value box, which egui leaves unnamed. At
    # the value box egui edits text, and the page's focus is on the adopted text entry.
    run "$browser" "focus boundary: into the canvas and out of it by keyboard alone, both directions, through every egui stop" \
        "${enter[@]}" \
        --key Tab --expect-focus after-editor \
        --key Shift+Tab --expect-focus midi-evil-canvas --expect-egui roll \
        --key F6 --expect-egui readout --expect "Transport, " \
        --key Shift+Tab --expect-focus before-editor \
        --key Tab --expect-focus midi-evil-canvas --expect-egui readout \
        --key Tab --expect-egui other --expect "Zoom: slider" \
        --key Tab --expect-focus midi-evil-text-agent --expect "drag value" \
        --key Tab --expect-focus midi-evil-canvas --expect "Quantize" \
        --key Tab --expect "−12" \
        --key Tab --expect "−1" \
        --key Tab --expect "+1" \
        --key Tab --expect "+12" \
        --key Tab --expect "Clean overlaps" \
        --key Tab --expect "Copy" \
        --key Tab --expect-egui roll --expect-focus midi-evil-canvas \
        --key Tab --expect-focus after-editor \
        --key Shift+Tab --expect-focus midi-evil-canvas --expect-egui roll \
        --key Shift+Tab --expect "Copy" \
        --key Shift+Tab --expect "Clean overlaps" \
        --key Shift+Tab --key Shift+Tab --key Shift+Tab --key Shift+Tab --expect "−12" \
        --key Shift+Tab --expect "Quantize" \
        --key Shift+Tab --expect-focus midi-evil-text-agent --expect "drag value" \
        --key Shift+Tab --expect-focus midi-evil-canvas --expect "Zoom: slider" \
        --key Shift+Tab --expect-egui readout --expect-focus midi-evil-canvas \
        --key Shift+Tab --expect-focus before-editor

    run "$browser" "the value box: the text entry is named and described, a typed value is announced when committed, F6 from an editor control reaches the transport, Tab and Shift+Tab still leave" \
        "${enter[@]}" \
        --key F6 --expect-egui readout --expect "Transport, " \
        --key Tab --expect "Zoom: slider 1" \
        --key Tab --expect-focus midi-evil-text-agent --expect-egui other --expect "drag value 1" \
        --expect-name "unnamed drag value" \
        --expect-announcement "drag value 1" \
        --type 2 --expect-focus midi-evil-text-agent --expect "unnamed drag value, editing 2" \
        --expect-announcements 0 \
        --key Enter --expect-focus midi-evil-text-agent --expect "drag value 2" \
        --expect-announcement "drag value 2" \
        --key Shift+Tab --expect "Zoom: slider 2" \
        --key Tab --expect-focus midi-evil-text-agent --expect "drag value 2" \
        --key Tab --expect "Quantize: button" \
        --key F6 --expect-egui readout --expect "Transport, " \
        --key Shift+Tab --expect-focus before-editor \
        --key Tab --expect-focus midi-evil-canvas --expect-egui readout \
        --key Shift+F6 --expect-egui roll --expect "Piano roll, no note selected" \
        --key Tab --expect-focus after-editor

    run "$browser" "the text entry at egui's first stop: Shift+Tab from it leaves the canvas backwards, Tab returns to it" \
        --query edge=value \
        "${enter[@]}" \
        --key F6 --expect-egui readout \
        --key Shift+Tab --expect-focus midi-evil-text-agent --expect "drag value 1" \
        --key Shift+Tab --expect-focus before-editor \
        --key Tab --expect-focus midi-evil-text-agent --expect "drag value 1" \
        --key F6 --expect-focus midi-evil-canvas --expect-egui roll --expect "Piano roll, no note selected"

    run "$browser" "held Right across twenty notes, one repeat-flagged keydown per frame; then leave the canvas, return, and read the current note" \
        "${enter[@]}" \
        --key ArrowRight --expect-place 0 \
        --hold ArrowRight 20 --expect-place 20 --remember held \
        --key Tab --expect-focus after-editor --expect-same --expect-announcements 0 \
        --key Shift+Tab --expect-focus midi-evil-canvas --expect-egui roll --expect-place 20 \
        --expect-remembered held --expect-announcements 0 \
        --reload \
        "${enter[@]}" \
        --key ArrowRight --expect-place 0 \
        --burst ArrowRight 20 --expect-place 20 --expect-remembered held --expect-announcements 1

    run "$browser" "edge: Up on a note at pitch 127" \
        "${enter[@]}" \
        --key ArrowRight --expect "C 5, bar 1 beat 1.5" \
        --key Shift+ArrowUp --key Shift+ArrowUp --key Shift+ArrowUp --key Shift+ArrowUp --expect "C 9," \
        --key Shift+ArrowUp --expect "Piano roll, G 9, bar 1 beat 1.5" \
        --key ArrowUp --expect "Piano roll, already at the top, G 9," \
        --expect-announcement "Cannot go higher." \
        --key ArrowUp --expect-same --expect-announcement "Cannot go higher." \
        --key ArrowUp --expect-same --expect-announcement "Cannot go higher."

    run "$browser" "edge: Ctrl+Left on a note at tick 0" \
        "${enter[@]}" \
        --key ArrowRight --expect "C 5, bar 1 beat 1.5" \
        --key Control+ArrowLeft --key Control+ArrowLeft --expect "Piano roll, C 5, bar 1 beat 1, " \
        --key Control+ArrowLeft --expect "Piano roll, already at the start, C 5, bar 1 beat 1, " \
        --expect-announcement "Cannot move earlier." \
        --key Control+ArrowLeft --expect-same --expect-announcement "Cannot move earlier."

    run "$browser" "edge: Enter then Delete in an empty pattern" --fixture empty \
        --key Tab --key Tab --expect-focus midi-evil-canvas --expect "Piano roll, empty" \
        --key Enter --expect "Piano roll, added, C 4, pattern bar 1 beat 1, 1 beat, velocity 100; used by 1 part" \
        --key Delete --expect "Piano roll, deleted C 4 at pattern bar 1 beat 1; used by 1 part, empty" \
        --expect-announcement "Removed." \
        --key Delete --expect "Piano roll, empty" --expect-announcement "Nothing here yet." \
        --key Delete --expect-same --expect-announcement "Nothing here yet." --expect-valid

    run "$browser" "edge: Enter after the last note of a full pattern, filled by keys" --fixture empty \
        --key Tab --key Tab --expect "Piano roll, empty" \
        --key Enter --key Enter --key Enter \
        --key Enter --expect "Piano roll, added, C 4, pattern bar 1 beat 4, 1 beat, velocity 100; used by 1 part" \
        --key Enter --expect "Piano roll, no room, C 4, pattern bar 1 beat 4, " \
        --expect-announcement "Nothing added: the pattern ends here." \
        --key Enter --expect-same --expect-announcement "Nothing added: the pattern ends here." \
        --expect-valid

    run "$browser" "edge: Enter after the last note of a full shared pattern" --fixture full \
        --key Tab --key Tab --expect "Piano roll, no note selected" \
        --key End --expect "Piano roll, C 4, pattern bar 1 beat 4, 1 beat, velocity 100" \
        --key Enter --expect "Piano roll, no room, C 4, pattern bar 1 beat 4, " \
        --expect-announcement "Nothing added: the pattern ends here." \
        --key Enter --expect-same --expect-announcement "Nothing added: the pattern ends here." \
        --expect-valid \
        --key ArrowUp --expect "Piano roll, C sharp 4, pattern bar 1 beat 4, 1 beat, velocity 100; shared by 2 parts"

    # Moving C 5 onto B 4's onset makes Clean overlaps re-sort the two, so the selected note's
    # index changes under the cursor: the delete must still take C 5. The revision changing is
    # what shows the clean changed something; without it this edge would pass on a no-op.
    run "$browser" "edge: select a note, Clean overlaps by keyboard, return to the roll, delete the same note" \
        "${enter[@]}" \
        --key ArrowRight --key Control+ArrowRight --expect "Piano roll, C 5, bar 1 beat 1.75, " \
        --key Shift+Tab --expect "Copy" \
        --key Shift+Tab --expect "Clean overlaps" \
        --key Enter --expect-changed data-revision \
        --key Tab --key Tab --expect-egui roll --expect "Piano roll, C 5, bar 1 beat 1.75, " \
        --key Delete --expect "Piano roll, deleted C 5 at bar 1 beat 1.75, "
done

for browser in chromium firefox; do
    must_fail "$browser" "--expect at the value box without --expect-focus" \
        "which this step does not name with --expect-focus" \
        "${enter[@]}" --key F6 --key Tab --key Tab --expect "drag value 1"
    must_fail "$browser" "eframe's own text entry, unnamed, at the value box" \
        "has no accessible name" \
        --query agent=eframe --key F6 --key Tab --key Tab --expect-focus role:textbox --expect "drag value 1"
done

for browser in chromium firefox; do
    echo
    echo "################ $browser: the browser-key table (synthetic)"
    if ! probe --browser "$browser" --key-table; then
        echo "################ FAILED: $browser: the browser-key table"
        failed=1
    fi
done

echo
if [ "$failed" -ne 0 ]; then
    echo "################ web gate: FAILED"
    exit 1
fi
echo "################ web gate: every sequence passed in Chromium and Firefox"
