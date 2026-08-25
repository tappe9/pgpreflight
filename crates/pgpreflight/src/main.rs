#![forbid(unsafe_code)]

use std::process::ExitCode;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    pgpreflight::run().await
}
