//! The page's sound. One audio context for the page's life, made in the first key or pointer
//! press that reaches the page and resumed in every one after, inside the event's own handler,
//! where no browser's rule about gestures can call it late. A preview is one buffer of the whole
//! rendered sound, played on the browser's own audio thread, so it keeps time whatever the page
//! is doing; its position is the context's own clock.

use std::cell::RefCell;
use std::time::Duration;

use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{AudioBufferSourceNode, AudioContext, AudioContextState, AudioScheduledSourceNode};

thread_local! {
    static CONTEXT: RefCell<Option<AudioContext>> = const { RefCell::new(None) };
    /// Why the last attempt to make the context failed, while none has been made.
    static REFUSAL: RefCell<Option<String>> = const { RefCell::new(None) };
    /// Catches a refused resume. One for the page's life, so a press leaks nothing.
    static IGNORE_A_REFUSAL: Closure<dyn FnMut(JsValue)> = Closure::new(|_: JsValue| {});
}

/// Called inside every key and pointer press the page's listeners hear: makes the context the
/// first time a browser allows it, and resumes it every time, since a context made without a
/// gesture the browser counts, or suspended since, starts only when resumed in one. A refused
/// resume is not an error worth a line in the console: the next press tries again.
pub fn wake() {
    CONTEXT.with(|context| {
        let mut context = context.borrow_mut();
        if context.is_none() {
            match AudioContext::new() {
                Ok(made) => {
                    *context = Some(made);
                    REFUSAL.with(|refusal| refusal.borrow_mut().take());
                }
                Err(error) => REFUSAL.with(|refusal| {
                    *refusal.borrow_mut() = Some(describe(&error));
                }),
            }
        }
        if let Some(context) = context.as_ref()
            && let Ok(resumed) = context.resume()
        {
            IGNORE_A_REFUSAL.with(|ignore| {
                let _ = resumed.catch(ignore);
            });
        }
    });
}

fn describe(error: &JsValue) -> String {
    error
        .dyn_ref::<web_sys::js_sys::Error>()
        .map(|error| String::from(error.message()))
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"))
}

/// Plays previews on the page's context.
pub struct PageOutput;

impl midi_evil::PreviewOutput for PageOutput {
    fn play(
        &mut self,
        sound: midi_evil::StereoSound,
        _channel_meters: Vec<(usize, midi_evil::ChannelMeter)>,
        repeats: bool,
    ) -> Result<Box<dyn midi_evil::PlayingPreview>, String> {
        let context = CONTEXT
            .with(|context| context.borrow().clone())
            .ok_or_else(|| {
                REFUSAL
                    .with(|refusal| refusal.borrow().clone())
                    .map_or_else(
                        || "the page has no audio yet; press a key or click in it".to_owned(),
                        |refusal| format!("the browser made no audio context: {refusal}"),
                    )
            })?;
        PagePreview::start(context, &sound, repeats).map_err(|error| describe(&error))
    }
}

struct PagePreview {
    context: AudioContext,
    source: AudioBufferSourceNode,
    started_at_seconds: f64,
    length_seconds: f64,
    repeats: bool,
}

impl PagePreview {
    /// A rate the browser will not make a buffer at fails here, as a device that could not
    /// start. No listener is set on the node, so nothing of it outlives the preview.
    fn start(
        context: AudioContext,
        sound: &midi_evil::StereoSound,
        repeats: bool,
    ) -> Result<Box<dyn midi_evil::PlayingPreview>, JsValue> {
        let frames = sound.left.len();
        // A buffer of no frames is refused; one silent frame is the same preview.
        let length = u32::try_from(frames.max(1)).map_err(|_| "the sound is too long to play")?;
        let buffer = context.create_buffer(2, length, sound.sample_rate as f32)?;
        buffer.copy_to_channel(&sound.left, 0)?;
        buffer.copy_to_channel(&sound.right, 1)?;
        let source = context.create_buffer_source()?;
        source.set_buffer(Some(&buffer));
        source.set_loop(repeats);
        source.connect_with_audio_node(&context.destination())?;
        let started_at_seconds = context.current_time();
        let scheduled: &AudioScheduledSourceNode = &source;
        scheduled.start()?;
        Ok(Box::new(Self {
            context,
            source,
            started_at_seconds,
            length_seconds: frames as f64 / f64::from(sound.sample_rate),
            repeats,
        }))
    }

    fn elapsed_seconds(&self) -> f64 {
        (self.context.current_time() - self.started_at_seconds).max(0.0)
    }
}

impl midi_evil::PlayingPreview for PagePreview {
    fn stop(&self) {
        let scheduled: &AudioScheduledSourceNode = &self.source;
        let _ = scheduled.stop();
        let _ = self.source.disconnect();
    }

    fn is_finished(&self) -> bool {
        !self.repeats && self.elapsed_seconds() >= self.length_seconds
    }

    fn position(&self) -> Duration {
        let elapsed = self.elapsed_seconds();
        let position = if self.repeats && self.length_seconds > 0.0 {
            elapsed % self.length_seconds
        } else {
            elapsed.min(self.length_seconds)
        };
        Duration::from_secs_f64(position)
    }

    /// Firefox makes a context suspended and starts it a moment later, so this is read as the
    /// preview plays, never only when it starts.
    fn device_running(&self) -> bool {
        self.context.state() == AudioContextState::Running
    }
}

impl Drop for PagePreview {
    fn drop(&mut self) {
        midi_evil::PlayingPreview::stop(self);
    }
}
