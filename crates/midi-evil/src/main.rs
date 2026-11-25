#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    match midi_evil::run_native() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// A browser build starts from the library, not from a `main`.
#[cfg(target_arch = "wasm32")]
fn main() {}
