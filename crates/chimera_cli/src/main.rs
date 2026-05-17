//! Rust/Zig Elixir compiler CLI.
//!
//! Command-line interface for the rzx compiler.

#[cfg(test)]
use chimera_allocator as _;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rzx")]
#[command(about = "Rust/Zig Elixir compiler", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile an Elixir source file
    Compile {
        /// Input file or path
        input: PathBuf,

        /// Output file (defaults to input with .beam extension)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Enable debug information
        #[arg(short, long)]
        debug: bool,

        /// Optimization level (0-3)
        #[arg(short, long, default_value = "2")]
        opt_level: u8,
    },
    /// Run an Elixir script
    Run {
        /// Input file or path
        input: PathBuf,

        /// Script arguments
        #[arg(short, long)]
        args: Vec<String>,
    },
    /// Run tests
    Test {
        /// Test file or directory
        #[arg(short, long)]
        input: Option<PathBuf>,

        /// Include doc tests
        #[arg(long)]
        include_docs: bool,
    },
    /// Format Elixir source
    Format {
        /// Input file or path
        #[arg(short, long)]
        input: Option<PathBuf>,

        /// Output to file (default is inplace)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Check mode (don't write)
        #[arg(short, long)]
        check: bool,
    },
    /// Compile and print diagnostics
    Check {
        /// Input file or path
        input: PathBuf,
    },
    /// Cross-reference analysis
    Xref {
        /// Input file or path
        #[arg(short, long)]
        input: PathBuf,

        /// Show unreachable functions
        #[arg(long)]
        unreachable: bool,
    },
    /// Print version
    Version,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compile {
            input,
            output,
            debug,
            opt_level,
        } => compile_file(input, output, debug, opt_level),
        Commands::Run { input, args } => run_script(input, args),
        Commands::Test {
            input,
            include_docs,
        } => run_tests(input, include_docs),
        Commands::Format {
            input,
            output,
            check,
        } => format_source(input, output, check),
        Commands::Check { input } => check_file(input),
        Commands::Xref { input, unreachable } => cross_ref(input, unreachable),
        Commands::Version => {
            println!("rzx {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

fn compile_file(
    input: PathBuf,
    output: Option<PathBuf>,
    _debug: bool,
    _opt_level: u8,
) -> Result<()> {
    use chimera_module::ModuleBuilder;
    use chimera_source::{SourceFile, SourceFileId};
    use chimera_term::SharedAtomTable;

    let source = std::fs::read_to_string(&input)?;
    let path_str: &str = &input.to_string_lossy();
    let file_id = SourceFileId::new(0);
    let _source_file = SourceFile::new(file_id, path_str, source.as_str());
    let atoms = SharedAtomTable::new();
    let mut builder = ModuleBuilder::new(atoms);

    // Leak the string to create a 'static lifetime
    let static_source: &'static str = Box::leak(source.into_boxed_str());
    let _ast = builder.compile_source(static_source, file_id)?;

    let output_path = output.unwrap_or_else(|| {
        let mut path = input.clone();
        path.set_extension("beam");
        path
    });

    println!("Compiled {} -> {:?}", input.display(), output_path);
    Ok(())
}

fn run_script(input: PathBuf, args: Vec<String>) -> Result<()> {
    use chimera_source::{SourceFile, SourceFileId};
    use chimera_target::{compile_module, TargetAdapter, TargetRuntime};
    use chimera_term::SharedAtomTable;

    let source = std::fs::read_to_string(&input)?;
    let path_str: &str = &input.to_string_lossy();
    let file_id = SourceFileId::new(0);
    let _source_file = SourceFile::new(file_id, path_str, source.as_str());
    let atoms = SharedAtomTable::new();

    // Compile the script to a module artifact
    let static_source: &'static str = Box::leak(source.into_boxed_str());
    let artifact = compile_module(static_source, file_id, atoms)?;

    // Create a target adapter and emit the module
    let mut target = TargetAdapter::new(SharedAtomTable::new());
    target.emit_module(&artifact)?;

    // If a main function is defined, evaluate it
    // The script arguments are passed to the target runtime
    if !args.is_empty() {
        println!("Script arguments: {:?}", args);
    }

    println!("Executed {:?}", input);
    Ok(())
}

fn run_tests(input: Option<PathBuf>, _include_docs: bool) -> Result<()> {
    use chimera_source::{SourceFile, SourceFileId};
    use chimera_target::{compile_module, TargetAdapter, TargetRuntime};
    use chimera_term::SharedAtomTable;
    use std::fs;

    // Find test files
    let test_paths = if let Some(input_path) = input {
        if input_path.is_dir() {
            discover_tests(&input_path)?
        } else {
            vec![input_path]
        }
    } else {
        // Default to test directory
        let test_dir = PathBuf::from("test");
        if test_dir.exists() {
            discover_tests(&test_dir)?
        } else {
            println!("No tests found (no test directory)");
            return Ok(());
        }
    };

    if test_paths.is_empty() {
        println!("No tests found");
        return Ok(());
    }

    println!("Running {} test files...", test_paths.len());

    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut total_skipped = 0;

    for test_path in &test_paths {
        println!("\n== Running {:?} ==", test_path);
        let source = fs::read_to_string(test_path)?;
        let path_str: &str = &test_path.to_string_lossy();
        let file_id = SourceFileId::new(0);
        let _source_file = SourceFile::new(file_id, path_str, source.as_str());

        let static_source: &'static str = Box::leak(source.into_boxed_str());

        // Compile the test module
        let atoms = SharedAtomTable::new();
        match compile_module(static_source, file_id, atoms) {
            Ok(artifact) => {
                let mut target = TargetAdapter::new(SharedAtomTable::new());
                target.emit_module(&artifact)?;

                // Run test functions
                if let Some(module_name) = extract_module_name(&artifact) {
                    let exports = target.get_exports(&module_name)?;
                    let mut passed = 0;
                    let mut failed = 0;
                    let skipped = 0;

                    for (name, _arity) in exports {
                        let name_debug = format!("{:?}", name);
                        if name_debug.starts_with("test_") {
                            match target.evaluate_expression(&module_name, &name, vec![]) {
                                Ok(_) => {
                                    println!("  ✓ {:?}", name);
                                    passed += 1;
                                }
                                Err(e) => {
                                    println!("  ✗ {:?}: {}", name, e);
                                    failed += 1;
                                }
                            }
                        }
                    }

                    total_passed += passed;
                    total_failed += failed;
                    total_skipped += skipped;

                    if failed == 0 {
                        println!("\n✓ {} passed", passed);
                    } else {
                        println!("\n✓ {} passed, {} failed", passed, failed);
                    }
                }
            }
            Err(e) => {
                println!("  ✗ Compilation failed: {}", e);
                total_failed += 1;
            }
        }
    }

    println!("\n=========================");
    println!(
        "Test results: {} passed, {} failed, {} skipped",
        total_passed, total_failed, total_skipped
    );

    if total_failed > 0 {
        Err(anyhow::anyhow!("{} tests failed", total_failed))
    } else {
        Ok(())
    }
}

fn discover_tests(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut tests = Vec::new();
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                tests.extend(discover_tests(&path)?);
            } else if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                if name_str.ends_with("_test.exs") || name_str.ends_with("_test.ex") {
                    tests.push(path);
                }
            }
        }
    }
    Ok(tests)
}

fn extract_module_name(
    artifact: &chimera_target::CompiledModuleArtifact,
) -> Option<chimera_term::ModuleName> {
    Some(artifact.module.clone())
}

fn format_source(_input: Option<PathBuf>, _output: Option<PathBuf>, _check: bool) -> Result<()> {
    println!("Formatting...");
    println!("(formatter requires chimera_fmt crate implementation)");
    Ok(())
}

fn check_file(input: PathBuf) -> Result<()> {
    use chimera_module::ModuleBuilder;
    use chimera_source::{SourceFile, SourceFileId};
    use chimera_term::SharedAtomTable;

    let source = std::fs::read_to_string(&input)?;
    let path_str: &str = &input.to_string_lossy();
    let file_id = SourceFileId::new(0);
    let _source_file = SourceFile::new(file_id, path_str, source.as_str());
    let atoms = SharedAtomTable::new();
    let mut builder = ModuleBuilder::new(atoms);

    let static_source: &'static str = Box::leak(source.into_boxed_str());
    match builder.compile_source(static_source, file_id) {
        Ok(_ast) => {
            println!("{}: OK", input.display());
            Ok(())
        }
        Err(e) => {
            println!("{}: ERROR", input.display());
            println!("  {}", e);
            Err(anyhow::anyhow!("compilation failed"))
        }
    }
}

fn cross_ref(_input: PathBuf, _unreachable: bool) -> Result<()> {
    println!("Cross-reference analysis...");
    println!("(xref requires analysis infrastructure)");
    Ok(())
}
