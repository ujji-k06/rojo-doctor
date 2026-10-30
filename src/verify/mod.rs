pub mod pipeline;
pub mod tools;

use std::process::ExitCode;

use crate::cli::VerifyArgs;
pub use pipeline::{
    execute_verify, print_verify_report, StageReport, StageStatus, VerifyReport, VerifyVerdict,
};

pub fn run(args: VerifyArgs) -> ExitCode {
    let format = args.format;
    let verbose = args.verbose;
    let (report, exit_code) = execute_verify(args);
    print_verify_report(&report, format, verbose);
    exit_code
}
