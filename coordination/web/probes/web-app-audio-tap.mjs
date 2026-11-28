// What the web gate hears. `installAudioTap` is installed before the page's own scripts in every
// row: it counts the audio contexts the page constructs, hears everything connected into a
// context's destination — an AudioWorklet records each sample against the context's own clock —
// and passes it on at no volume, since headless Firefox plays through the machine's real audio
// server and a gate is not to be heard. `onsets` finds the notes in what it recorded.
//
// The probe steers the tap through `window.midiEvilAudio`: `constructorThrows` makes the
// constructor throw, as a browser with no audio does; `holdSuspended()` suspends every context
// and keeps each suspended, the page's resume() answered and ignored, until `release()`. A row
// about a page nobody has touched reads none of this until its own input is past —
// page.evaluate is itself a gesture to both browsers — so with `reportThroughConsole` the tap and
// the bridge's two elements also report through the console, which is no gesture.

// Runs in the page, so it closes over nothing of this module's.
export function installAudioTap({ reportThroughConsole = false } = {}) {
  const NativeContext = window.AudioContext;
  const audio = (window.midiEvilAudio = {
    constructed: 0,
    contexts: [],
    chunks: [],
    frames: [],
    inputs: [],
    constructorThrows: false,
    suspended: false,
    errors: [],
  });
  const report = (line) => {
    if (reportThroughConsole) console.log(`midi-evil-audio: ${line}`);
  };
  // Long enough for any row's recording, short enough that a row leaves no page holding minutes.
  const KEPT_SECONDS = 40;
  const HEARD = 0.01;
  const QUIET_SECONDS = 0.3;
  const workletSource = `
    class MidiEvilTap extends AudioWorkletProcessor {
      constructor() { super(); this.chunk = new Float32Array(4096); this.filled = 0; this.frame = 0; }
      process(inputs) {
        const input = inputs[0];
        if (this.filled === 0) this.frame = currentFrame;
        for (let sample = 0; sample < 128; sample += 1) {
          let sum = 0;
          for (const channel of input ?? []) sum += channel[sample];
          this.chunk[this.filled + sample] = input && input.length ? sum / input.length : 0;
        }
        this.filled += 128;
        if (this.filled === this.chunk.length) {
          this.port.postMessage({ frame: this.frame, samples: this.chunk }, [this.chunk.buffer]);
          this.chunk = new Float32Array(4096);
          this.filled = 0;
        }
        return true;
      }
    }
    registerProcessor("midi-evil-tap", MidiEvilTap);`;
  const workletUrl = URL.createObjectURL(new Blob([workletSource], { type: "text/javascript" }));
  const nativeConnect = AudioNode.prototype.connect;
  const nativeResume = NativeContext.prototype.resume;
  const nativeSuspend = NativeContext.prototype.suspend;
  let sounding = false;
  let lastSoundAt = 0;

  // Each chunk the worklet sends: its first frame on its context's clock, when it reached the
  // page, and its loudest sample. Whether the tap hears anything now changes on the chunks.
  function received(record, { frame, samples }) {
    let peak = 0;
    for (const sample of samples) peak = Math.max(peak, Math.abs(sample));
    const arrivedAt = performance.now();
    audio.chunks.push({ context: record.index, frame, sampleRate: record.sampleRate, arrivedAt, peak, samples });
    while (audio.chunks.length && arrivedAt - audio.chunks[0].arrivedAt > KEPT_SECONDS * 1000) audio.chunks.shift();
    if (peak > HEARD) {
      lastSoundAt = arrivedAt;
      if (!sounding) report(`sound, context ${record.index}`);
      sounding = true;
    } else if (sounding && arrivedAt - lastSoundAt > QUIET_SECONDS * 1000) {
      sounding = false;
      report("silence");
    }
  }

  class TappedContext extends NativeContext {
    constructor(...options) {
      if (audio.constructorThrows) throw new DOMException("the gate's tap refuses to make a context", "NotSupportedError");
      super(...options);
      const record = { index: audio.contexts.length, sampleRate: this.sampleRate, states: [this.state], workletReady: false };
      audio.constructed += 1;
      audio.contexts.push(record);
      report(`constructed ${audio.constructed}, ${this.state}`);
      this.addEventListener("statechange", () => {
        record.states.push(this.state);
        report(`context ${record.index} ${this.state}`);
      });
      if (audio.suspended) nativeSuspend.call(this);
      const collector = this.createGain();
      const passedOn = this.createGain();
      passedOn.gain.value = 0;
      const sink = this.createGain();
      sink.gain.value = 0;
      nativeConnect.call(collector, passedOn);
      nativeConnect.call(passedOn, this.destination);
      nativeConnect.call(sink, this.destination);
      this.midiEvilCollector = collector;
      this.audioWorklet
        .addModule(workletUrl)
        .then(() => {
          const listener = new AudioWorkletNode(this, "midi-evil-tap");
          listener.port.onmessage = (message) => received(record, message.data);
          nativeConnect.call(collector, listener);
          nativeConnect.call(listener, sink);
          record.workletReady = true;
        })
        .catch((error) => audio.errors.push(`the tap's worklet: ${error}`));
      audio.live = this;
    }

    resume() {
      if (audio.suspended) return Promise.resolve();
      return nativeResume.call(this);
    }
  }
  window.AudioContext = TappedContext;
  AudioNode.prototype.connect = function (target, ...rest) {
    if (target instanceof AudioDestinationNode && this.context.midiEvilCollector) {
      nativeConnect.call(this, this.context.midiEvilCollector);
      return target;
    }
    return nativeConnect.call(this, target, ...rest);
  };

  audio.holdSuspended = () => {
    audio.suspended = true;
    if (audio.live) nativeSuspend.call(audio.live);
  };
  audio.release = () => {
    audio.suspended = false;
  };
  audio.state = () => audio.live?.state ?? "none";
  // The loudest sample of the chunks that reached the page between two of its clock's readings.
  audio.peakBetween = (from, to = Infinity) =>
    audio.chunks.filter((chunk) => chunk.arrivedAt >= from && chunk.arrivedAt <= to).reduce((peak, chunk) => Math.max(peak, chunk.peak), 0);
  // Whether the worklet has been recording, and for how long, since `from`.
  audio.listenedSince = (from) => audio.chunks.filter((chunk) => chunk.arrivedAt >= from).length * 4096 / (audio.live?.sampleRate ?? 48000);
  // Every sample one context recorded since `from`, with the first one's frame, for the onset
  // detector; null when nothing was recorded.
  audio.recording = (from) => {
    const chunks = audio.chunks.filter((chunk) => chunk.arrivedAt >= from && chunk.context === audio.contexts.length - 1);
    if (!chunks.length) return null;
    const samples = [];
    for (const chunk of chunks) for (const sample of chunk.samples) samples.push(sample);
    const contiguous = chunks.every((chunk, index) => index === 0 || chunk.frame === chunks[index - 1].frame + 4096);
    return { firstFrame: chunks[0].frame, sampleRate: chunks[0].sampleRate, samples, contiguous };
  };
  // The longest wait between two animation frames that overlaps `from` to `to`: how long the page
  // answered nothing then. A wait that began before `from` or ended after `to` counts, so a stall
  // that spans the whole window is never missed for want of a frame inside it.
  audio.longestFrameBetween = (from, to = Infinity) => {
    let longest = 0;
    for (let index = 1; index < audio.frames.length; index += 1) {
      const [before, after] = [audio.frames[index - 1], audio.frames[index]];
      if (after >= from && before <= to) longest = Math.max(longest, after - before);
    }
    return longest;
  };
  audio.block = (milliseconds) => {
    const until = performance.now() + milliseconds;
    while (performance.now() < until) {
      // The page's thread is held, as a long render holds it.
    }
  };
  const frame = (time) => {
    audio.frames.push(time);
    if (audio.frames.length > 20000) audio.frames.splice(0, 10000);
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
  // When the page's listeners heard each press: a key sent while the page is held is heard only
  // once it is let go.
  for (const name of ["keydown", "pointerdown"]) {
    window.addEventListener(name, (event) => audio.inputs.push({ name, key: event.key ?? "", heardAt: performance.now() }), true);
  }
  if (reportThroughConsole) {
    document.addEventListener("DOMContentLoaded", () => {
      for (const id of ["midi-evil-current", "midi-evil-live"]) {
        const element = document.getElementById(id);
        new MutationObserver(() => console.log(`${id}: ${element.textContent}`)).observe(element, { childList: true, characterData: true, subtree: true });
      }
    });
  }
}

// Onsets in a recording: the peak of every 2.5 ms, held over 12.5 ms so a low note's own period
// leaves no dips, and an onset wherever that rises above 1.15 times the median of the 200 ms
// before it and above 0.02, at least 250 ms after the last. Good to about 12 ms. Returns their
// times on the context's clock, in seconds.
export function onsets(recording) {
  if (!recording) return [];
  const { samples, sampleRate, firstFrame } = recording;
  const hop = Math.round(sampleRate * 0.0025);
  const hops = Math.floor(samples.length / hop);
  const peaks = new Float32Array(hops);
  for (let index = 0; index < hops; index += 1) {
    let peak = 0;
    for (let sample = index * hop; sample < (index + 1) * hop; sample += 1) peak = Math.max(peak, Math.abs(samples[sample]));
    peaks[index] = peak;
  }
  const held = peaks.map((_, index) => Math.max(...peaks.slice(Math.max(0, index - 4), index + 1)));
  const audible = peaks.findIndex((peak) => peak > 1e-4);
  if (audible < 0) return [];
  const look = Math.round(0.2 / 0.0025);
  const found = [];
  let previous = -Infinity;
  for (let index = audible; index < hops; index += 1) {
    const before = Array.from(held.slice(Math.max(audible, index - look), index)).sort((first, second) => first - second);
    const baseline = before.length >= 8 ? before[Math.floor(before.length / 2)] : 0;
    const time = (firstFrame + index * hop) / sampleRate;
    if (held[index] > 0.02 && held[index] > 1.15 * Math.max(baseline, 1e-4) && time - previous > 0.25) {
      found.push(time);
      previous = time;
    }
  }
  return found;
}
