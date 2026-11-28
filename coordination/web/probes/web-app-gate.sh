#!/usr/bin/env bash
# The web gate for the application's own bundles: serves the visitor's bundle scripts/build_wasm.sh
# left in target/web/dist/ and the gate's, which scripts/build_wasm.sh --gate left in
# target/web/dist-gate/, and has web-app-probe.mjs run every row, each in a fresh page, in
# headless Chromium, then headless Firefox. The rows run in six groups, each with a deadline of
# its own in each browser (WEB_APP_GROUP_TIMEOUT_SECONDS, default 300; the playing group, which
# listens to whole songs, WEB_APP_PLAYING_TIMEOUT_SECONDS, default 600), and a group that runs
# out says its name:
#   keys-and-bridge  the keys, the bridge, the focus boundary and the text entry, from the
#                    application's own default project, on the visitor's bundle;
#   matrix           the native gate's W0 and R rows in the browser, each from the fixture its
#                    native row opens, on the gate's bundle;
#   surfaces         the native gate's sequences for the instrument list, the Song view, the roll's
#                    toolbar, the track panel and the windows, the same way;
#   absence          what the browser build leaves out is not there, on the visitor's bundle;
#   page             the page's own text, its layout, and the visitor's bundle ignoring the gate's
#                    query and lacking its marker;
#   playing          Play heard: what the audio tap hears and what the transport says, from the
#                    gate's fixtures, on the gate's bundle. The tap passes everything on at no
#                    volume, since headless Firefox plays through the machine's audio server.
# Then five negative controls, in both browsers, each run on the row that shows it and each of
# which must fail for the reason it exists to show:
#   live-never-written  the live region ignores every write, so no row hears anything;
#   current-stale       the current element keeps its first text, so the focused canvas is
#                       described by a control it is no longer on;
#   binds-not-bound     the page turns Control+O, a chord the browser keeps, into the
#                       application's Save, cancelling the browser's own meaning of it;
#   shows-desktop       the gate's bundle drawing what the desktop draws, on which the absence
#                       walk must find the Mixer's tab;
#   play-never-pressed  the page keeps Space from the application, so Play is never pressed and
#                       the row that expects sound hears nothing.
# The first three are copies of the visitor's bundle with one line put into the page, the last a
# copy of the gate's bundle with one line put into its page. The gate
# refuses to start, as a tooling failure, without a gate bundle whose wasm carries the marker
# only the gate's build compiles in, carries the digest of the fixture set it was built from and
# that digest is the tracked files' own, and whose page is the page in the source tree; and
# unless both bundles carry the build identity of this tree (scripts/build_identity.sh): a missing
# or stale gate bundle is not a row's failure, and the two gates never test different projects
# under one row. Exit status 0 means every row held in both browsers
# and all ten controls failed as they must.
#
#   coordination/web/probes/web-app-gate.sh > web-app-gate-transcript.txt
#
# It builds nothing. The bundle is served from a copy, on 127.0.0.1 with python3's http.server, on
# a port the server binds itself, with .wasm served as application/wasm, and the server is
# stopped however the script ends. Each group, in each browser, runs under its own watchdog of
# WEB_APP_GROUP_TIMEOUT_SECONDS (default 300) or, for the playing group,
# WEB_APP_PLAYING_TIMEOUT_SECONDS (default 600); no row's own deadline may be more than half of
# the deadline it runs under, which the probe checks before it starts. A browser with no audio server to play
# to refuses the playing group, which is the tooling's failure, not the application's. Playwright comes from the working set's
# insiculous_web/node_modules beside the repository's main checkout (PLAYWRIGHT_FROM names
# another package.json) and needs Node 24 (NODE overrides the node it finds). A missing Node,
# Playwright or browser is the tooling's failure: exit status 2, the application not judged.
set -euo pipefail

probes=$(cd "$(dirname "$0")" && pwd)
repository=$(cd "$probes/../../.." && pwd)
bundle=${MIDI_EVIL_WEB_BUNDLE:-$repository/target/web/dist}
gate_bundle=${MIDI_EVIL_WEB_GATE_BUNDLE:-$repository/target/web/dist-gate}
# The marker's one home is the gate's module; it is read from there, never written out again.
marker=$(sed -n 's/^const MARKER: &str = "\(.*\)";$/\1/p' "$repository/crates/evil-web/src/gate.rs")
main_checkout=$(git -C "$repository" worktree list --porcelain | sed -n '1s/^worktree //p')
export PLAYWRIGHT_FROM=${PLAYWRIGHT_FROM:-$main_checkout/../insiculous_web/package.json}

find_node() {
    local candidate
    for candidate in "${NODE:-}" "$(command -v node || true)" "$HOME"/.nvm/versions/node/v24.*/bin/node; do
        if [ -n "$candidate" ] && [ -x "$candidate" ] && "$candidate" --version | grep -q '^v24\.'; then
            echo "$candidate"
            return
        fi
    done
}

if [ ! -f "$bundle/index.html" ] || [ ! -f "$bundle/evil_web_bg.wasm" ] || [ ! -f "$bundle/evil_web.js" ]; then
    echo "web-app-gate: the tooling failed: no built bundle at '$bundle'; run scripts/build_wasm.sh" >&2
    exit 2
fi
if [ -z "$marker" ]; then
    echo "web-app-gate: the tooling failed: no marker found in crates/evil-web/src/gate.rs" >&2
    exit 2
fi
if [ ! -f "$gate_bundle/evil_web_bg.wasm" ] || ! grep -a -q -F -- "$marker" "$gate_bundle/evil_web_bg.wasm"; then
    echo "web-app-gate: refuses to start: no gate bundle carrying the gate's marker at '$gate_bundle'; run scripts/build_wasm.sh --gate" >&2
    exit 2
fi
# The same digest gate.rs compiles in: a 64-bit FNV-1a over each fixture's name and text, in the
# order of their names, each followed by a zero byte.
fixtures_digest=$(python3 - "$probes/fixtures" <<'PYTHON'
import pathlib, sys

hash = 0xCBF29CE484222325
for path in sorted(pathlib.Path(sys.argv[1]).glob("*.evil"), key=lambda path: path.stem):
    for part in (path.stem.encode(), path.read_bytes()):
        for byte in part + b"\0":
            hash = ((hash ^ byte) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
print(f"{hash:016x}")
PYTHON
)
if ! grep -a -q -F -- "$fixtures_digest" "$gate_bundle/evil_web_bg.wasm"; then
    echo "web-app-gate: refuses to start: the gate bundle was not built from the tracked fixtures (digest $fixtures_digest); run scripts/build_wasm.sh --gate" >&2
    exit 2
fi
# Both bundles must be built from this tree: a gate bundle left from an older one would run the
# matrix on code the visitor's bundle no longer has.
build_identity=$("$repository/scripts/build_identity.sh")
for built in "$bundle" "$gate_bundle"; do
    if ! grep -a -q -F -- "$build_identity" "$built/evil_web_bg.wasm"; then
        echo "web-app-gate: refuses to start: the bundle at '$built' was not built from this tree ($build_identity); run scripts/build_wasm.sh and scripts/build_wasm.sh --gate" >&2
        exit 2
    fi
done
if ! cmp -s "$gate_bundle/index.html" "$repository/crates/evil-web/index.html"; then
    echo "web-app-gate: refuses to start: the gate bundle's page is not crates/evil-web/index.html, so the bundle is stale; run scripts/build_wasm.sh --gate" >&2
    exit 2
fi
node=$(find_node)
if [ -z "$node" ]; then
    echo "web-app-gate: the tooling failed: no Node 24 found; set NODE" >&2
    exit 2
fi

runtime=$(mktemp -d)
server=
stop_server() {
    if [ -n "$server" ]; then
        kill "$server" 2> /dev/null || true
        wait "$server" 2> /dev/null || true
    fi
    rm -rf -- "$runtime"
}
trap stop_server EXIT

cp -r -- "$bundle" "$runtime/bundle"
cp -r -- "$gate_bundle" "$runtime/gate"
# A control is a bundle — the visitor's, unless a third argument names another — with one line
# put into its page just before the module script, so the line runs before the application
# starts.
control() {
    local name=$1 line=$2 from=${3:-bundle}
    cp -r -- "$runtime/$from" "$runtime/$name"
    python3 - "$runtime/$name/index.html" "$line" <<'PYTHON'
import sys

path, line = sys.argv[1], sys.argv[2]
page = open(path).read()
marker = '<script type="module">'
assert page.count(marker) == 1, f"{path} must hold exactly one module script"
open(path, "w").write(page.replace(marker, line + "\n" + marker))
PYTHON
}
control live-never-written '<script>Object.defineProperty(document.getElementById("midi-evil-live"), "textContent", { get() { return ""; }, set() {} });</script>'
control current-stale '<script>(() => { const current = document.getElementById("midi-evil-current"); const descriptor = Object.getOwnPropertyDescriptor(Node.prototype, "textContent"); let writes = 0; Object.defineProperty(current, "textContent", { get() { return descriptor.get.call(current); }, set(text) { if (writes++ === 0) descriptor.set.call(current, text); } }); })();</script>'
control binds-not-bound '<script>window.addEventListener("keydown", (event) => { if (event.ctrlKey && event.key.toLowerCase() === "o") { event.preventDefault(); document.getElementById("midi-evil-canvas").dispatchEvent(new KeyboardEvent("keydown", { key: "s", ctrlKey: true, bubbles: true, cancelable: true })); } }, true);</script>'
control play-never-pressed '<script>window.addEventListener("keydown", (event) => { if (event.key === " ") event.stopImmediatePropagation(); }, true);</script>' gate
port_file=$runtime/port

python3 - "$runtime" "$port_file" > "$runtime/server.log" 2>&1 <<'PYTHON' &
import functools, http.server, os, sys

directory, port_file = sys.argv[1], sys.argv[2]


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
        ".mjs": "text/javascript",
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

ready=
for _ in $(seq 100); do
    if ! kill -0 "$server" 2> /dev/null; then
        echo "web-app-gate: the tooling failed: the bundle server exited:" >&2
        cat "$runtime/server.log" >&2
        exit 2
    fi
    if [ -f "$port_file" ] && python3 - "$(cat "$port_file")" <<'PYTHON' 2> /dev/null; then
import sys, urllib.request

base = f"http://127.0.0.1:{sys.argv[1]}/bundle/"
page = urllib.request.urlopen(base + "index.html", timeout=2).read().decode()
assert 'id="midi-evil-live"' in page
wasm = urllib.request.urlopen(base + "evil_web_bg.wasm", timeout=5)
assert wasm.headers.get_content_type() == "application/wasm"
PYTHON
        ready=yes
        break
    fi
    sleep 0.1
done
if [ -z "$ready" ]; then
    echo "web-app-gate: the tooling failed: the bundle server did not answer with the bundle within ten seconds" >&2
    cat "$runtime/server.log" >&2
    exit 2
fi
base="http://127.0.0.1:$(cat "$port_file")"
group_timeout=${WEB_APP_GROUP_TIMEOUT_SECONDS:-300}
playing_timeout=${WEB_APP_PLAYING_TIMEOUT_SECONDS:-600}
# The deadline of a group, which a control's row runs under too.
seconds_for() {
    if [ "$1" = playing ]; then echo "$playing_timeout"; else echo "$group_timeout"; fi
}

# The probe holds each row it runs to the deadline it runs under.
probe() {
    local browser=$1 page=$2 seconds=$3
    shift 3
    timeout --signal=TERM --kill-after=15 "$seconds" \
        "$node" "$probes/web-app-probe.mjs" --url "$base/$page/index.html" \
        --gate-url "$base/gate/index.html" --marker "$marker" --group-seconds "$seconds" \
        --browser "$browser" "$@"
}

stop_for_tooling() {
    echo "################ TOOLING FAILED: $1"
    echo
    echo "web-app gate: the tooling failed; the application was not judged"
    exit 2
}

failed=0
for browser in chromium firefox; do
    for group in keys-and-bridge matrix surfaces absence page playing; do
        started=$SECONDS
        echo
        echo "################ $browser: the $group group"
        status=0
        probe "$browser" bundle "$(seconds_for "$group")" --group "$group" || status=$?
        echo "################ $browser: $group: $((SECONDS - started)) s"
        case $status in
            0) ;;
            2) stop_for_tooling "$browser: $group" ;;
            124) echo "################ FAILED: $browser: the $group group ran out of its $(seconds_for "$group") s"; failed=1 ;;
            *) echo "################ FAILED: $browser: a row of the $group group did not hold"; failed=1 ;;
        esac
    done
done

# A control passes only when the probe failed its checks (status 1, not a crash or a hang) and
# printed every reason the control exists to show.
# A control named shows-desktop runs on the gate's bundle with ?shows=desktop rather than on a
# copy of the page; play-never-pressed is a copy of the gate's page, and its row runs in the
# playing group's time.
must_fail() {
    local browser=$1 control=$2 rows=$3 started=$SECONDS page=$2 flags=() seconds=$group_timeout
    shift 3
    if [ "$control" = shows-desktop ]; then
        page=bundle
        flags=(--shows-desktop)
    elif [ "$control" = play-never-pressed ]; then
        page=bundle
        flags=(--gate-url "$base/$control/index.html")
        seconds=$playing_timeout
    fi
    echo
    echo "################ $browser: negative control: $control, on $rows"
    local output status=0 reason wrong=
    output=$(probe "$browser" "$page" "$seconds" --rows "$rows" "${flags[@]}" 2>&1) || status=$?
    printf '%s\n' "$output"
    echo "################ $browser: $control: $((SECONDS - started)) s"
    [ "$status" -eq 2 ] && stop_for_tooling "$browser: negative control $control"
    for reason in "$@"; do
        grep -q -F -- "$reason" <<< "$output" || wrong="$wrong without '$reason'"
    done
    if [ "$status" -eq 1 ] && [ -z "$wrong" ]; then
        echo "################ the negative control failed, as it must"
    else
        echo "################ FAILED: $browser: negative control $control exited $status,$wrong"
        failed=1
    fi
}

for browser in chromium firefox; do
    case $browser in
        chromium) judge="Chromium's tree over CDP" ;;
        firefox) judge="Playwright's computation" ;;
    esac
    must_fail "$browser" live-never-written delete-and-undo \
        "FAILED: Delete: 0 writes, not 1" "FAILED: Undo: 0 writes, not 1"
    must_fail "$browser" current-stale enter-from-the-page \
        "FAILED: the focused element's description \"Nothing in the editor has focus" \
        "does not hold \"About MIDI Evil, button\" ($judge)"
    must_fail "$browser" binds-not-bound browser-chords \
        "FAILED: Control+o was cancelled" "FAILED: Control+o, which is the browser's: 1 writes, not 0"
    must_fail "$browser" shows-desktop absence-walk \
        "FAILED: the walk reached MIXER, which the browser leaves out"
    must_fail "$browser" play-never-pressed play-by-space \
        "FAILED: Space, to play: the tap heard nothing"
done

echo
echo "web-app gate: $SECONDS s in all"
if [ "$failed" -ne 0 ]; then
    echo "web-app gate: FAILED"
    exit 1
fi
echo "web-app gate: passed in Chromium and Firefox; all ten negative controls failed as they must"
