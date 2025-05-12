use clap::Parser;
use cleanplate::analyze;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::path::PathBuf;

/// A tool for batch processing `MiniJinja` templates from a JSON file
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Cli {
    /// The input JSON file containing templates
    #[clap(
        short,
        long,
        value_parser,
        default_value = "chat_template_to_model_ids.json"
    )]
    input: PathBuf,

    /// The output JSON file to save the analysis results
    #[clap(
        short,
        long,
        value_parser,
        default_value = "template_analysis_results.json"
    )]
    output: PathBuf,

    /// The output JSON file to save the shape frequency analysis
    #[clap(
        short,
        long,
        value_parser,
        default_value = "shape_frequency_results.json"
    )]
    shape_output: PathBuf,

    /// Enable verbose output with debug tracing
    #[clap(short, long)]
    verbose: bool,
}

// Structure to track both template count and associated model IDs
#[derive(Serialize)]

struct ShapeData {
    template_count: usize,
    model_ids: HashSet<String>,
    // avoid serializing HashSet directly
    #[serde(skip_serializing)]
    templates: Vec<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    // Parse command line arguments
    let cli = Cli::parse();

    // Expand the tilde in the path if present
    let input_path = if cli.input.starts_with("~/") {
        let home = dirs::home_dir().expect("Could not find home directory");
        home.join(cli.input.strip_prefix("~/").unwrap())
    } else {
        cli.input
    };

    // Read the input JSON file
    println!("Reading templates from: {}", input_path.display());
    let json_content = match fs::read_to_string(&input_path) {
        Ok(content) => content,
        Err(err) => {
            eprintln!("Error reading JSON file: {err}");
            eprintln!("Path: {}", input_path.display());
            return Err(err.into());
        }
    };

    // Parse the JSON
    let templates_map: HashMap<String, Value> = match serde_json::from_str(&json_content) {
        Ok(map) => map,
        Err(err) => {
            eprintln!("Error parsing JSON: {err}");
            return Err(err.into());
        }
    };

    let template_count = templates_map.len();
    println!("Found {template_count} templates to analyze");

    let mut total_model_ids = 0;
    let mut total_model_ids_set = HashSet::new();
    // Count the total number of unique model IDs
    for model_ids in templates_map.values() {
        if let Some(id_array) = model_ids.as_array() {
            for id_value in id_array {
                if let Some(id_str) = id_value.as_str() {
                    total_model_ids_set.insert(id_str.to_string());
                }
            }
        }
    }
    total_model_ids = total_model_ids_set.len();
    println!("Total unique model IDs: {total_model_ids}");
    println!("");

    // Create a vector to store analysis results as a list of objects
    let mut analysis_results = Vec::new();

    // Create a map to track shape data (count and associated model IDs)
    let mut shape_data: HashMap<String, ShapeData> = HashMap::new();

    // Process each template
    for (template_key, model_ids) in &templates_map {
        // println!("Analyzing template: {template_key}");

        // Clone the template key to avoid ownership issues
        let template_name = template_key.clone();

        // Analyze the template
        match analyze(&template_name, cli.verbose) {
            Ok(analysis) => {
                // Get the object shapes as a string to use as a key for frequency counting
                let shape_json_str = serde_json::to_string(&analysis.object_shapes_json)?;

                // Create a HashSet for the model IDs of this template
                let mut template_model_ids = HashSet::new();

                // Handle model IDs properly, avoiding temporary value issues
                if let Some(id_array) = model_ids.as_array() {
                    for id_value in id_array {
                        if let Some(id_str) = id_value.as_str() {
                            template_model_ids.insert(id_str.to_string());
                        }
                    }
                }

                // Update shape data in our map
                let entry = shape_data
                    .entry(shape_json_str.clone())
                    .or_insert(ShapeData {
                        template_count: 0,
                        model_ids: HashSet::new(),
                        templates: Vec::new(),
                    });

                entry.template_count += 1;
                entry.model_ids.extend(template_model_ids);
                entry.templates.push(template_name.clone());

                // Create a result object for this template
                let template_analysis = json!({
                    "template": template_name,
                    "model_ids": model_ids,
                    "external_vars": analysis.external_vars,
                    "internal_vars": analysis.internal_vars,
                    "loop_vars": analysis.loop_vars,
                    "object_shapes_json": analysis.object_shapes_json,
                    "status": "success"
                });

                analysis_results.push(template_analysis);
            }
            Err(err) => {
                // eprintln!("Error analyzing template '{template_name}': {err}");
                // Add error information to the results
                let error_analysis = json!({
                    "template": template_name,
                    "model_ids": model_ids,
                    "error": err.to_string(),
                    "status": "error"
                });

                analysis_results.push(error_analysis);
            }
        }
    }

    // Write the analysis results to the output file as a JSON array
    let output_json = serde_json::to_string_pretty(&analysis_results)?;
    fs::write(&cli.output, output_json)?;

    // Create a vector of shape frequency results, with both counts
    let mut shape_frequency_results = Vec::new();
    for (shape_str, data) in shape_data {
        // Parse the shape string back to JSON
        let shape_json: Value = serde_json::from_str(&shape_str)?;

        // TODO: include the templates in the output (too many for now)
        // Create a list of template names for reference
        let template_names = Vec::<String>::new(); // data.templates;
        shape_frequency_results.push(json!({
            "object_shapes_json": shape_json,
            "template_count": data.template_count,
            "model_id_count": data.model_ids.len(),
            "templates": template_names
        }));
    }

    // TODO: revisit configurable sorting options

    // // Sort by template count first, then by model ID count (both descending)
    // shape_frequency_results.sort_by(|a, b| {
    //     let count_a = a["template_count"].as_i64().unwrap_or(0);
    //     let count_b = b["template_count"].as_i64().unwrap_or(0);

    //     let model_count_a = a["model_id_count"].as_i64().unwrap_or(0);
    //     let model_count_b = b["model_id_count"].as_i64().unwrap_or(0);

    //     // Primary sort by template count, secondary by model ID count
    //     count_b
    //         .cmp(&count_a)
    //         .then(model_count_b.cmp(&model_count_a))
    // });

    // Sort by model_id_count only
    shape_frequency_results.sort_by(|a, b| {
        let model_count_a = a["model_id_count"].as_i64().unwrap_or(0);
        let model_count_b = b["model_id_count"].as_i64().unwrap_or(0);

        // Sort by model ID count in descending order
        model_count_b.cmp(&model_count_a)
    });

    // Write the shape frequency results to the separate output file
    let shape_output_json = serde_json::to_string_pretty(&shape_frequency_results)?;
    fs::write(&cli.shape_output, shape_output_json)?;

    println!(
        "Analysis complete! Results saved to: {}",
        cli.output.display()
    );
    println!(
        "Shape frequency analysis saved to: {}",
        cli.shape_output.display()
    );

    // Print a summary
    let success_count = analysis_results
        .iter()
        .filter(|v| v["status"] == "success")
        .count();

    let unique_shapes_count = shape_frequency_results.len();

    let total_number_of_model_ids = analysis_results
        .iter()
        .filter(|v| v["status"] == "success")
        .map(|v| v["model_ids"].as_array().unwrap().len())
        .sum::<usize>();

    let total_number_of_models_of_failures = analysis_results
        .iter()
        .filter(|v| v["status"] == "error")
        .map(|v| v["model_ids"].as_array().unwrap().len())
        .sum::<usize>();

    println!("\nSummary:");
    println!("Total templates: {template_count}");
    println!("Successfully analyzed: {success_count}");
    println!("Total number of model IDs: {total_number_of_model_ids}");
    println!("Failed: {}", template_count - success_count);
    println!("Total number of model IDs of failures: {total_number_of_models_of_failures}");
    println!("Unique object shapes found: {unique_shapes_count}");

    // Print the top 5 most common shapes (if available)
    if !shape_frequency_results.is_empty() {
        // loop until 95% of the models are covered
        let mut covered = 0.0;
        let mut total = 0.0;
        println!(
            "| index | {:^14} | {:^14} | {:^13} | {:^9} |",
            "template_count", "model_id_count", "Pct of models", "Covered"
        );
        println!(
            "|{:-<7}|{:-<16}|{:-<16}|{:-<15}|{:-<11}|",
            "", "", "", "", ""
        );
        for (i, result) in shape_frequency_results.iter().enumerate() {
            let model_count = result["model_id_count"].as_f64().unwrap_or(0.0);
            total += model_count;
            let contrib = model_count / total_model_ids as f64 * 100.0;
            covered += contrib;
            println!(
                "| {:^5} | {:^14} | {:^14} | {:^13} | {:^9} |",
                format!("{:02}", i + 1),
                format!("{:.2}", result["template_count"]),
                format!("{:.2}", result["model_id_count"]),
                format!("{:.2}%", contrib),
                format!("{:.2}%", covered)
            );
            if covered >= 95.0 {
                break;
            }
        }
    }

    Ok(())
}

// How many templates are needed for a given target percent
// 50% in 4
// 80% in 10
// 90% in 16
// 95% in 25
// 99% in 62