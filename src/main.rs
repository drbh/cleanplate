use clap::Parser;
use cleanplate::analyze;
use std::fs;
use std::path::PathBuf;
use std::process;

/// A tool for generating JSON Schema from `MiniJinja` templates
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Cli {
    /// The template file to analyze
    #[clap(short, long, value_parser)]
    file: Option<PathBuf>,

    /// Enable verbose output with debug tracing
    #[clap(short, long)]
    verbose: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let cli = Cli::parse();

    // Get the template file path
    let file_path = cli
        .file
        .unwrap_or_else(|| PathBuf::from("templates/example.jinja"));

    // Read the template file
    let template_content = match fs::read_to_string(&file_path) {
        Ok(content) => content,
        Err(err) => {
            eprintln!("Error reading template file: {err}");
            eprintln!("Path: {}", file_path.display());
            process::exit(1);
        }
    };

    // Analyze the template
    let analysis = match analyze(&template_content, cli.verbose) {
        Ok(a) => a,
        Err(err) => {
            eprintln!("Error analyzing template: {err}");
            process::exit(1);
        }
    };

    // Print the analysis results
    println!("\n=== Variable Analysis Report ===\n");

    // Print external variables (required context)
    println!("External Variables (required context):");
    if analysis.external_vars.is_empty() {
        println!("  None");
    } else {
        for var in &analysis.external_vars {
            println!("  {var}");
        }
    }

    // Print internal variables
    println!("\nInternal Variables (defined in template):");
    let internal_non_loop = analysis
        .internal_vars
        .iter()
        .filter(|v| !analysis.loop_vars.contains_key(*v))
        .collect::<Vec<_>>();

    if internal_non_loop.is_empty() {
        println!("  None");
    } else {
        for var in internal_non_loop {
            println!("  {var}");
        }
    }

    // Print loop variables with their iterables
    println!("\nLoop Variables:");
    let loop_vars = analysis.loop_vars.iter().collect::<Vec<_>>();
    if loop_vars.is_empty() {
        println!("  None");
    } else {
        for (var, iterable) in loop_vars {
            println!("  {var} (from {iterable})");
        }
    }

    // Print JSON Schema
    println!("\nTemplate Data Shape (JSON):");
    println!(
        "{}",
        serde_json::to_string_pretty(&analysis.object_shapes_json)?
    );

    Ok(())
}
