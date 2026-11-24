#!/bin/bash
# Starts MIDI Evil under XWayland inside a private session bus that has its own AT-SPI bus and its
# own runtime directory, runs atspi-probe.py against it with this script's arguments, and removes
# everything it started. The desktop's accessibility bus and settings, and any MIDI Evil the person
# has open, are never touched. The exit status is the probe's.
#
#   coordination/web/probes/run-probe.sh --press "PIANO ROLL" --key Tab --key shift+Tab
#
# MIDI_EVIL_BINARY (default target/debug/midi-evil) and MIDI_EVIL_FILE (default the tracked Bach
# fixture) choose what is started.
set -u

probes=$(cd "$(dirname "$0")" && pwd)
repository=$(cd "$probes/../../.." && pwd)

inside_private_bus() {
    export XDG_DATA_HOME=$XDG_RUNTIME_DIR/data
    mkdir -p "$XDG_DATA_HOME"
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status ScreenReaderEnabled "<true>" > /dev/null || exit 1
    AT_SPI_BUS_ADDRESS=$(gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.a11y.Bus.GetAddress | sed "s/^('//;s/',)$//")
    export AT_SPI_BUS_ADDRESS
    # The private bus cannot activate the registry: its launcher asks systemd, which a private
    # session bus does not have.
    "$MIDI_EVIL_REGISTRY" > /dev/null &
    registry=$!
    export DISPLAY=$MIDI_EVIL_X_DISPLAY
    unset WAYLAND_DISPLAY
    "$MIDI_EVIL_BINARY" "$MIDI_EVIL_FILE" > "$XDG_RUNTIME_DIR/application.log" 2>&1 &
    application=$!
    # Globals, not locals: the trap runs after this function has returned. The outer cleanup
    # reads the same list in case this shell dies before its trap runs.
    echo "$application $registry" > "$XDG_RUNTIME_DIR/started-pids"
    trap 'kill "$application" "$registry" 2> /dev/null; wait' EXIT
    sleep 2
    if ! kill -0 "$application" 2> /dev/null; then
        echo "run-probe: the application exited before probing" >&3
        exit 1
    fi
    # The probe's own messages go to the caller's stderr; everything else this session prints
    # there is bus chatter, kept in a file and shown only when the run fails.
    python3 "$probes/atspi-probe.py" --application "$(basename "$MIDI_EVIL_BINARY")" "$@" 2>&3
    probe_status=$?
    if ! kill -0 "$application" 2> /dev/null; then
        echo "run-probe: the application died during the run" >&3
        [ "$probe_status" -ne 0 ] || probe_status=1
    fi
    return "$probe_status"
}

if [ "${1-}" = --inside-private-bus ]; then
    shift
    inside_private_bus "$@"
    exit $?
fi

binary=${MIDI_EVIL_BINARY:-$repository/target/debug/midi-evil}
file=${MIDI_EVIL_FILE:-$repository/examples/public-domain/bach-invention-02/bach-invention-02.mid}
if [ ! -x "$binary" ]; then
    echo "run-probe: no application at $binary; build it first" >&2
    exit 1
fi
registry_binary=
for candidate in /usr/libexec/at-spi2-registryd /usr/lib/at-spi2-core/at-spi2-registryd \
    /usr/lib/at-spi2-registryd; do
    if [ -x "$candidate" ]; then
        registry_binary=$candidate
        break
    fi
done
if [ -z "$registry_binary" ]; then
    echo "run-probe: at-spi2-registryd not found; install at-spi2-core" >&2
    exit 1
fi

desktop_display=${DISPLAY:?run-probe: DISPLAY is not set; it names the XWayland server}
# A short path under /tmp, whatever TMPDIR says: the application's single-instance socket lives in
# the runtime directory and a long path overflows a socket address. Being private, it is also why
# this run can neither join nor disturb a MIDI Evil the person has open.
private_runtime=$(mktemp -d /tmp/midi-evil-probe.XXXXXX) || exit 1
cleanup() {
    local started_pid
    for started_pid in $(cat "$private_runtime/started-pids" 2> /dev/null); do
        kill "$started_pid" 2> /dev/null
    done
    rm -rf -- "$private_runtime"
}
trap cleanup EXIT

# GSETTINGS_BACKEND must be set here, outside the private bus: its launcher inherits the bus
# daemon's environment, and with the real backend it stores ScreenReaderEnabled in the desktop's
# own settings. DISPLAY=:99 is a display that does not exist: given the real one, the launcher
# would write its bus address onto the desktop's X root window.
GSETTINGS_BACKEND=memory DISPLAY=:99 XDG_RUNTIME_DIR=$private_runtime \
    MIDI_EVIL_X_DISPLAY=$desktop_display MIDI_EVIL_REGISTRY=$registry_binary \
    MIDI_EVIL_BINARY=$binary MIDI_EVIL_FILE=$file \
    dbus-run-session -- "$0" --inside-private-bus "$@" 3>&2 2> "$private_runtime/session.log"
status=$?
if [ "$status" -ne 0 ]; then
    for log in application.log session.log; do
        if [ -s "$private_runtime/$log" ]; then
            echo "run-probe: $log follows" >&2
            cat "$private_runtime/$log" >&2
        fi
    done
fi
exit "$status"
