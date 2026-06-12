#![forbid(unsafe_code)]

use clap::CommandFactory;
use clap_complete::{generate, shells::Bash};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

mod args {
    use clap::{Parser, Subcommand};
    use std::path::PathBuf;

    include!("src/args.rs");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=src/args.rs");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?);
    generate_bash_completion(&out_dir)?;

    let target_completion_dir = target_profile_dir(&out_dir)?.join("completions");
    generate_bash_completion(&target_completion_dir)?;

    Ok(())
}

fn generate_bash_completion(output_dir: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(output_dir)?;
    let mut command = args::Cli::command();
    let output_path = output_dir.join("oxidepdf.bash");
    let mut bytes = Vec::new();
    generate(Bash, &mut command, "oxidepdf", &mut bytes);
    fs::write(&output_path, bytes)?;
    Ok(output_path)
}

fn target_profile_dir(out_dir: &Path) -> io::Result<PathBuf> {
    let mut current = out_dir;
    while let Some(parent) = current.parent() {
        if current.file_name().is_some_and(|name| name == "build") {
            return Ok(parent.to_path_buf());
        }
        current = parent;
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "could not locate target profile directory from OUT_DIR",
    ))
}
