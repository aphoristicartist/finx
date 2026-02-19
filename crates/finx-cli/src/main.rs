mod cli;
mod commands;
mod error;
mod metadata;
mod output;

use clap::Parser;
use std::future::Future;
use std::process::ExitCode;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use crate::cli::Cli;
use crate::error::CliError;

fn main() -> ExitCode {
    match block_on(run()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}

async fn run() -> Result<ExitCode, CliError> {
    let cli = Cli::parse();

    let envelope = commands::run(&cli).await?;
    if cli.stream {
        output::render_stream(&envelope, cli.explain)?;
    } else {
        output::render(&envelope, cli.format, cli.pretty)?;
    }

    if cli.strict && (!envelope.meta.warnings.is_empty() || !envelope.errors.is_empty()) {
        return Err(CliError::StrictModeViolation {
            warning_count: envelope.meta.warnings.len(),
            error_count: envelope.errors.len(),
        });
    }

    if !envelope.errors.is_empty() {
        return Ok(ExitCode::from(3));
    }

    Ok(ExitCode::SUCCESS)
}

fn block_on<F>(future: F) -> F::Output
where
    F: Future,
{
    let waker = noop_waker();
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);

    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn noop_waker() -> Waker {
    // SAFETY: The vtable functions never dereference the data pointer and are no-op operations.
    unsafe { Waker::from_raw(noop_raw_waker()) }
}

fn noop_raw_waker() -> RawWaker {
    RawWaker::new(std::ptr::null(), &NOOP_RAW_WAKER_VTABLE)
}

unsafe fn noop_raw_waker_clone(_: *const ()) -> RawWaker {
    noop_raw_waker()
}

unsafe fn noop_raw_waker_wake(_: *const ()) {}

unsafe fn noop_raw_waker_wake_by_ref(_: *const ()) {}

unsafe fn noop_raw_waker_drop(_: *const ()) {}

static NOOP_RAW_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    noop_raw_waker_clone,
    noop_raw_waker_wake,
    noop_raw_waker_wake_by_ref,
    noop_raw_waker_drop,
);
