#!/usr/bin/env bash
# The browser build's first-run smoke: serves the bundle scripts/build_wasm.sh left in
# target/web/dist/ and has web-smoke.mjs open it once in headless Chromium and once in headless
# Firefox, each of which passes only if the canvas itself draws more than one colour within the
# driver's deadline, the page raises and logs no error and no panic, and it never asks for MIDI
# access. Then five negative controls, in both browsers, each made from the same copy of the
# bundle and each of which must fail for the reason it exists to show:
#   empty-wasm       the wasm is an empty file, so the module cannot start;
#   wrong-canvas-id  the page's canvas does not carry the id the entry looks for;
#   logged-error     the page withholds the media query eframe watches the pixel ratio with, so
#                    eframe logs an error and the canvas draws all the same;
#   asks-for-midi    the page itself calls navigator.requestMIDIAccess, and the canvas draws;
#   never-loads      the page holds an image whose request the server never answers, so the page
#                    never finishes loading.
# Exit status 0 means both browsers passed and all ten controls failed as they must; each run's
# time is printed, and the whole smoke's.
#
#   coordination/web/probes/web-smoke.sh > web-smoke-transcript.txt
#
# It deliberately builds nothing, clicks and presses nothing, and judges nothing past starting:
# a gesture's audio, MIDI, files and renders are not part of starting. The canvas screenshots go
# to WEB_SMOKE_SCREENSHOTS (default target/web/smoke/), one per browser and run.
#
# The bundle is served from a copy, so a rebuild during the run cannot hand a page one build's
# glue and another's wasm, on 127.0.0.1 with python3's http.server, on a port the server binds
# itself, with .wasm served as application/wasm whatever the machine's MIME table says; the
# server is stopped however the script ends. Each browser run is under a watchdog
# (WEB_SMOKE_TIMEOUT_SECONDS, default 180). Playwright comes from the working set's
# insiculous_web/node_modules, beside the repository's main checkout, so a worktree elsewhere
# finds it too (PLAYWRIGHT_FROM names another package.json), and needs Node 24 (NODE overrides the
# node it finds). A missing Node, Playwright or browser is the tooling's failure: the script stops
# there with exit status 2, having said nothing about the application. Once the server has
# answered with the bundle, anything a page does — not finishing its load among them — is the
# application's.
set -euo pipefail

probes=$(cd "$(dirname "$0")" && pwd)
repository=$(cd "$probes/../../.." && pwd)
bundle=${MIDI_EVIL_WEB_BUNDLE:-$repository/target/web/dist}
screenshots=${WEB_SMOKE_SCREENSHOTS:-$repository/target/web/smoke}
canvas_id=midi-evil-canvas
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
    echo "web-smoke: the tooling failed: no built bundle at '$bundle'; run scripts/build_wasm.sh" >&2
    exit 2
fi
if ! grep -q "<canvas id=\"$canvas_id\"" "$bundle/index.html"; then
    echo "web-smoke: the tooling failed: $bundle/index.html has no canvas with the id $canvas_id" >&2
    exit 2
fi
node=$(find_node)
if [ -z "$node" ]; then
    echo "web-smoke: the tooling failed: no Node 24 found; set NODE" >&2
    exit 2
fi
mkdir -p "$screenshots"

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

# The bundle is copied once; the passing runs use that copy, and every control is made from it.
cp -r -- "$bundle" "$runtime/bundle"
control() {
    cp -r -- "$runtime/bundle" "$runtime/$1"
}
# Puts a line into a control's page just before its module script, so it runs first.
before_module() {
    local page=$1 line=$2
    python3 - "$runtime/$page/index.html" "$line" <<'PYTHON'
import sys

path, line = sys.argv[1], sys.argv[2]
page = open(path).read()
marker = '<script type="module">'
assert page.count(marker) == 1, f"{path} must hold exactly one module script"
open(path, "w").write(page.replace(marker, line + "\n" + marker))
PYTHON
}
control empty-wasm
: > "$runtime/empty-wasm/evil_web_bg.wasm"
control wrong-canvas-id
sed -i "s/<canvas id=\"$canvas_id\"/<canvas id=\"not-$canvas_id\"/" "$runtime/wrong-canvas-id/index.html"
control logged-error
before_module logged-error '<script>const matchMedia = window.matchMedia.bind(window); window.matchMedia = (query) => query.startsWith("(resolution:") ? null : matchMedia(query);</script>'
control asks-for-midi
before_module asks-for-midi '<script>navigator.requestMIDIAccess?.().catch(() => {});</script>'
# Opening a named pipe with no writer blocks, so the server never answers this image's request.
control never-loads
mkfifo "$runtime/never-loads/unanswered"
before_module never-loads '<img src="unanswered" alt="">'
port_file=$runtime/port

python3 - "$runtime" "$port_file" > "$runtime/server.log" 2>&1 <<'PYTHON' &
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

# Ready means: the server is alive, it bound a port, and it serves this bundle — the application's
# page, and the wasm as application/wasm.
ready=
for _ in $(seq 100); do
    if ! kill -0 "$server" 2> /dev/null; then
        echo "web-smoke: the tooling failed: the bundle server exited:" >&2
        cat "$runtime/server.log" >&2
        exit 2
    fi
    if [ -f "$port_file" ] && python3 - "$(cat "$port_file")" <<'PYTHON' 2> /dev/null; then
import sys, urllib.request

base = f"http://127.0.0.1:{sys.argv[1]}/bundle/"
page = urllib.request.urlopen(base + "index.html", timeout=2).read().decode()
assert "<title>MIDI Evil in the browser</title>" in page
wasm = urllib.request.urlopen(base + "evil_web_bg.wasm", timeout=5)
assert wasm.headers.get_content_type() == "application/wasm"
PYTHON
        ready=yes
        break
    fi
    sleep 0.1
done
if [ -z "$ready" ]; then
    echo "web-smoke: the tooling failed: the bundle server did not answer with the bundle within ten seconds" >&2
    cat "$runtime/server.log" >&2
    exit 2
fi
base="http://127.0.0.1:$(cat "$port_file")"
smoke_timeout=${WEB_SMOKE_TIMEOUT_SECONDS:-180}

smoke() {
    local browser=$1 page=$2
    timeout --signal=TERM --kill-after=15 "$smoke_timeout" \
        "$node" "$probes/web-smoke.mjs" --url "$base/$page/index.html" --browser "$browser" \
        --screenshot "$screenshots/$browser-$page.png"
}

stop_for_tooling() {
    echo "################ TOOLING FAILED: $1"
    echo
    echo "web smoke: the tooling failed; the application was not judged"
    exit 2
}

failed=0
run() {
    local browser=$1 started=$SECONDS
    echo
    echo "################ $browser: the build starts"
    local status=0
    smoke "$browser" bundle || status=$?
    echo "################ $browser: $((SECONDS - started)) s"
    case $status in
        0) ;;
        2) stop_for_tooling "$browser" ;;
        124) echo "################ FAILED: $browser: the run outlived its watchdog"; failed=1 ;;
        *) echo "################ FAILED: $browser: the build did not start"; failed=1 ;;
    esac
}

# A negative control passes only when the driver failed its checks (status 1, not a crash), for
# each reason the control exists to show, and for none it must not show: a reason written
# "!TEXT" must be absent, which is how a control shows the canvas drew and failed all the same.
must_fail() {
    local browser=$1 page=$2 started=$SECONDS
    shift 2
    echo
    echo "################ $browser: negative control: $page"
    local output status=0 reason wrong=
    output=$(smoke "$browser" "$page" 2>&1) || status=$?
    printf '%s\n' "$output"
    echo "################ $browser: $page: $((SECONDS - started)) s"
    [ "$status" -eq 2 ] && stop_for_tooling "$browser: negative control $page"
    for reason in "$@"; do
        if [[ $reason == !* ]]; then
            grep -q -F -- "${reason#!}" <<< "$output" && wrong="$wrong with '${reason#!}'"
        else
            grep -q -F -- "$reason" <<< "$output" || wrong="$wrong without '$reason'"
        fi
    done
    if [ "$status" -eq 1 ] && [ -z "$wrong" ]; then
        echo "################ the negative control failed, as it must"
    else
        echo "################ FAILED: $browser: negative control $page exited $status,$wrong"
        failed=1
    fi
}

did_not_start="FAILED: the console logged an error: MIDI Evil did not start: "
one_colour="FAILED: the canvas holds one colour"
for browser in chromium firefox; do
    run "$browser"
done
for browser in chromium firefox; do
    must_fail "$browser" empty-wasm "$one_colour" "$did_not_start"
    must_fail "$browser" wrong-canvas-id "$one_colour" \
        "${did_not_start}the page has no element with the id $canvas_id"
    must_fail "$browser" logged-error "!$one_colour" \
        "FAILED: the console logged an error: " "Failed to create MediaQueryList"
    must_fail "$browser" asks-for-midi "!$one_colour" "FAILED: the page asked for MIDI access"
    must_fail "$browser" never-loads "FAILED: the page did not finish loading"
done

echo
echo "web smoke: $SECONDS s in all"
if [ "$failed" -ne 0 ]; then
    echo "web smoke: FAILED"
    exit 1
fi
echo "web smoke: passed in Chromium and Firefox; all ten negative controls failed as they must"
