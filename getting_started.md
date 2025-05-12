## Reproducible Example

The following example demonstrates how to reproduce the analysis via a few simple commands.

**Step 1: Clone the repository** and make sure you have `cargo` and `uv` installed.

**Step 2: Dump all of the templates** next run the small Python script (Thank you @xenova for sharing this script) to dump all the templates from the `huggingface` repository. This will create a JSON file with all the templates and their corresponding model IDs.

```bash
uv run scripts/dump-templates.py
# Reading inline script metadata from `scripts/dump-templates.py`
# 8001it [00:01, 7673.83it/s] Downloaded 10000 models
# 18805it [00:03, 8814.73it/s] Downloaded 20000 models
# 29065it [00:05, 8958.19it/s] Downloaded 30000 models
# 38001it [00:07, 8095.41it/s] Downloaded 40000 models
# 49001it [00:09, 7085.82it/s] Downloaded 50000 models
# 58001it [00:11, 6916.77it/s] Downloaded 60000 models
# 69001it [00:14, 3733.74it/s] Downloaded 70000 models
# 79001it [00:17, 6246.81it/s] Downloaded 80000 models
# 89001it [00:19, 7777.62it/s] Downloaded 90000 models
# 99001it [00:21, 6848.28it/s] Downloaded 100000 models
# 108001it [00:23, 6434.79it/s] Downloaded 110000 models
# 119001it [00:26, 6292.06it/s] Downloaded 120000 models
# 129001it [00:28, 6516.23it/s] Downloaded 130000 models
# 139001it [00:30, 7046.90it/s] Downloaded 140000 models
# 144798it [00:32, 4402.24it/s]
# Found 1717 templates
# Found 93652 models
```

**Step 3: Analyze the templates** run the small Rust program to analyze the templates. This will read the JSON file created in the previous step and attempt to extract the required input variables from each template. 

```bash
cargo run --release --example extract
# Reading templates from: chat_template_to_model_ids.json
# Found 1717 templates to analyze
# Total unique model IDs: 93652

# Analysis complete! Results saved to: template_analysis_results.json
# Shape frequency analysis saved to: shape_frequency_results.json

# Summary:
# Total templates: 1717
# Successfully analyzed: 1692
# Total number of model IDs: 93585
# Failed: 25
# Total number of model IDs of failures: 67
# Unique object shapes found: 252
# | index | template_count | model_id_count | Pct of models | Covered |
# | ----- | -------------- | -------------- | ------------- | ------- |
# | 01    | 140            | 15601          | 16.66%        | 16.66%  |
# | 02    | 231            | 15249          | 16.28%        | 32.94%  |
# | 03    | 86             | 10064          | 10.75%        | 43.69%  |
# | 04    | 72             | 7352           | 7.85%         | 51.54%  |
# | 05    | 27             | 6821           | 7.28%         | 58.82%  |
# | 06    | 63             | 5441           | 5.81%         | 64.63%  |
# | 07    | 29             | 5226           | 5.58%         | 70.21%  |
# | 08    | 14             | 3914           | 4.18%         | 74.39%  |
# | 09    | 214            | 2929           | 3.13%         | 77.52%  |
# | 10    | 5              | 2481           | 2.65%         | 80.17%  |
# | 11    | 26             | 2162           | 2.31%         | 82.48%  |
# | 12    | 20             | 2094           | 2.24%         | 84.71%  |
# | 13    | 74             | 1760           | 1.88%         | 86.59%  |
# | 14    | 54             | 1307           | 1.40%         | 87.99%  |
# | 15    | 22             | 1263           | 1.35%         | 89.33%  |
# | 16    | 15             | 1227           | 1.31%         | 90.65%  |
# | 17    | 29             | 1043           | 1.11%         | 91.76%  |
# | 18    | 2              | 585            | 0.62%         | 92.38%  |
# | 19    | 8              | 527            | 0.56%         | 92.95%  |
# | 20    | 20             | 459            | 0.49%         | 93.44%  |
# | 21    | 3              | 400            | 0.43%         | 93.86%  |
# | 22    | 2              | 385            | 0.41%         | 94.27%  |
# | 23    | 6              | 337            | 0.36%         | 94.63%  |
# | 24    | 4              | 309            | 0.33%         | 94.96%  |
# | 25    | 3              | 292            | 0.31%         | 95.28%  |
```

**Step 4: Review the results** the results of the analysis are saved in two files and the `shape_frequency_results.json` file contains the frequency of each unqiue input sorted by the number of models that use it. 

We can see in the summary that the top 4 input types account for (15601 + 15249 + 10064 + 7352 = 48_266) ~51% of the models we analyzed. This suggest a high degree of overlap between the templates and a small subset of high impact inputs.

Recommendation: choose a subset of these inputs and create a type that captures the most common use cases. This can either be a union of the common types or some new type that captures the most common use cases...
