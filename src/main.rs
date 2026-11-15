mod browser;
mod cli;
mod document;
mod human;
mod session;
#[cfg(all(windows, feature = "window"))]
mod window;

fn main() -> std::process::ExitCode {
    match cli::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Surfbird: {}", document::clean(&error));
            std::process::ExitCode::FAILURE
        }
    }
}
