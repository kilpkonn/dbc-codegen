use clap::Parser;
use dbc_codegen::{Config, FileStyle};
use heck::ToSnakeCase;
use std::ffi::OsStr;
use std::fs::File;
use std::path::Path;
use std::{path::PathBuf, process::exit};

/// Generate Rust `struct`s from a `dbc` file.
#[derive(Debug, Parser)]
#[command(version)]
struct Cli {
    /// Path to a `.dbc` file
    dbc_path: PathBuf,

    /// Target directory to write Rust source file(s) to
    out_path: PathBuf,

    /// Specify output path to root module file.
    module_file: Option<String>,

    /// Enable debug printing
    #[arg(long)]
    debug: bool,
}

fn main() {
    let args = Cli::parse();
    let files = list_files(&args.dbc_path);

    if !args.out_path.is_dir() {
        eprintln!(
            "Output path needs to point to a directory (checked {})",
            args.out_path.display()
        );
        exit(exitcode::CANTCREAT);
    }

    let file_style = match args.module_file.is_some() {
        false => FileStyle::Standalone,
        true => FileStyle::Shared {
            common_types_import: "use super::CanError;",
        },
    };
    let config = Config::builder().file_style(file_style).debug_prints(true);

    let mut modules = vec![];

    for file in files {
        let dbc_file = match std::fs::read(&file) {
            Ok(it) => it,
            Err(e) => {
                eprintln!(
                    "could not read `{}`: {}, ignoring",
                    args.dbc_path.display(),
                    e
                );
                continue;
            }
        };
        let dbc_file_name = file
            .file_name()
            .unwrap_or_else(|| args.dbc_path.as_ref())
            .to_string_lossy();

        let rust_mod = file.file_stem().unwrap().to_str().unwrap().to_snake_case();
        let messages_path = args.out_path.join(format!("{rust_mod}.rs"));
        modules.push(rust_mod);
        let mut messages_file = File::create(&messages_path).unwrap_or_else(|e| {
            eprintln!(
                "Could not create `{}` file in {}: {:?}",
                messages_path.display(),
                args.out_path.display(),
                e
            );
            exit(exitcode::CANTCREAT);
        });

        let config = config
            .clone()
            .dbc_name(&dbc_file_name)
            .dbc_content(&dbc_file)
            .build();

        dbc_codegen::codegen(config, &mut messages_file).unwrap_or_else(|e| {
            eprintln!("could not convert `{}`: {}", file.display(), e);
            if args.debug {
                eprintln!("details: {:?}", e);
            }
            // exit(exitcode::NOINPUT)
        })
    }

    if let Some(module_file) = args.module_file {
        let module_file = args.out_path.join(module_file);
        let module_file = File::create(&module_file).unwrap_or_else(|e| {
            eprintln!(
                "Could not create `{}` file in {}: {:?}",
                module_file.display(),
                args.out_path.display(),
                e
            );
            exit(exitcode::CANTCREAT);
        });

        let modules = modules
            .into_iter()
            .map(|m| format!("pub mod {m};"))
            .collect::<Vec<_>>()
            .join("\n");
        let config = config.dbc_name("").dbc_content(&[]).build();
        dbc_codegen::codegen_shared(config, &modules, module_file).unwrap_or_else(|e| {
            eprintln!("could not generate module file: {}", e);
            if args.debug {
                eprintln!("details: {:?}", e);
            }
            exit(exitcode::NOINPUT)
        });
    }
}

fn list_files(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    path.read_dir()
        .unwrap()
        .map(Result::unwrap)
        .filter_map(|it| {
            let ft = it.file_type().unwrap();
            if ft.is_dir() {
                return Some(list_files(&it.path()));
            }
            if ft.is_file() {
                let path = it.path();
                if path.extension().and_then(OsStr::to_str) == Some("dbc") {
                    return Some(vec![path]);
                }
            }

            None
        })
        .flatten()
        .collect()
}
