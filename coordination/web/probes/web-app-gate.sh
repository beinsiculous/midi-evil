#!/usr/bin/env bash
# The web gate for the application's own bundle: serves the bundle scripts/build_wasm.sh left in
# target/web/dist/ and has web-app-probe.mjs run every row of web-app-rows.mjs in headless
# Chromium, then headless Firefox, each row in a fresh page starting from the application's own
# default project. Then three negative controls, in both browsers, each made from the same copy of
# the bundle, each run on the row that shows it and each of which must fail for the reason it
# exists to show:
#   live-never-written  the live region ignores every write, so no row hears anything;
#   current-stale       the current element keeps its first text, so the focused canvas is
#                       described by a control it is no longer on;
#   binds-not-bound     the page turns Control+O, a chord the browser keeps, into the
#                       application's Save, cancelling the browser's own meaning of it.
# Exit status 0 means every row held in both browsers and all six controls failed as they must.
#
#   coordination/web/probes/web-app-gate.sh > web-app-gate-transcript.txt
#
# It builds nothing. The bundle is served from a copy, on 127.0.0.1 with python3's http.server, on
# a port the server binds itself, with .wasm served as application/wasm, and the server is
# stopped however the script ends. Each browser run is under a watchdog
# (WEB_APP_GATE_TIMEOUT_SECONDS, default 1500). Playwright comes from the working set's
# insiculous_web/node_modules beside the repository's main checkout (PLAYWRIGHT_FROM names
# another package.json) and needs Node 24 (NODE overrides the node it finds). A missing Node,
# Playwright or browser is the tooling's failure: exit status 2, the application not judged.
set -euo pipefail

probes=$(cd "$(dirname "$0")" && pwd)
repository=$(cd "$probes/../../.." && pwd)
bundle=${MIDI_EVIL_WEB_BUNDLE:-$repository/target/web/dist}
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
# A control is the bundle with one line put into its page just before the module script, so the
# line runs before the application starts.
control() {
    local name=$1 line=$2
    cp -r -- "$runtime/bundle" "$runtime/$name"
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
gate_timeout=${WEB_APP_GATE_TIMEOUT_SECONDS:-1500}

probe() {
    local browser=$1 page=$2
    shift 2
    timeout --signal=TERM --kill-after=15 "$gate_timeout" \
        "$node" "$probes/web-app-probe.mjs" --url "$base/$page/index.html" --browser "$browser" "$@"
}

stop_for_tooling() {
    echo "################ TOOLING FAILED: $1"
    echo
    echo "web-app gate: the tooling failed; the application was not judged"
    exit 2
}

failed=0
for browser in chromium firefox; do
    started=$SECONDS
    echo
    echo "################ $browser: every row"
    status=0
    probe "$browser" bundle || status=$?
    echo "################ $browser: $((SECONDS - started)) s"
    case $status in
        0) ;;
        2) stop_for_tooling "$browser" ;;
        124) echo "################ FAILED: $browser: the run outlived its watchdog"; failed=1 ;;
        *) echo "################ FAILED: $browser: a row did not hold"; failed=1 ;;
    esac
done

# A control passes only when the probe failed its checks (status 1, not a crash or a hang) and
# printed every reason the control exists to show.
must_fail() {
    local browser=$1 page=$2 rows=$3 started=$SECONDS
    shift 3
    echo
    echo "################ $browser: negative control: $page, on $rows"
    local output status=0 reason wrong=
    output=$(probe "$browser" "$page" --rows "$rows" 2>&1) || status=$?
    printf '%s\n' "$output"
    echo "################ $browser: $page: $((SECONDS - started)) s"
    [ "$status" -eq 2 ] && stop_for_tooling "$browser: negative control $page"
    for reason in "$@"; do
        grep -q -F -- "$reason" <<< "$output" || wrong="$wrong without '$reason'"
    done
    if [ "$status" -eq 1 ] && [ -z "$wrong" ]; then
        echo "################ the negative control failed, as it must"
    else
        echo "################ FAILED: $browser: negative control $page exited $status,$wrong"
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
done

echo
echo "web-app gate: $SECONDS s in all"
if [ "$failed" -ne 0 ]; then
    echo "web-app gate: FAILED"
    exit 1
fi
echo "web-app gate: passed in Chromium and Firefox; all six negative controls failed as they must"
