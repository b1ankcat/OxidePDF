mod error;
mod io;

pub(crate) use error::CliError;
pub(crate) use io::{
    execute_and_write_workflow, is_stdio, load_inputs, one_input_workflow, parse_workflow,
    read_path_or_stdin, reject_shared_stdin_inputs, two_input_workflow, write_outputs_with_stats,
};
