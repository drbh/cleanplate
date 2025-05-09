# cleanplate

A tool for analyzing Jinja2 templates and automatically generating JSON schema that describes the variables used in the template.

## Overview

`cleanplate` parses Jinja templates using MiniJinja and performs static analysis to:
1. Extract all variables used in the template
2. Categorize them as external (required context) or internal (defined in template)
3. Track object attributes and nested properties
4. Generate a JSON representation of the expected data structure

## Usage

```bash
# Analyze default template (templates/example.jinja)
cleanplate

# Analyze a specific template
cleanplate --file path/to/template.jinja
```

## Example

given this template

```jinja
{% set loop_messages = messages %}

{% for message in loop_messages %}
  {% set content = '<|start_header_id|>' + message['role'] + '<|end_header_id|>\n\n'+ message['content'] | trim + '<|eot_id|>' %}

  {% if loop.index0 == 0 %}
    {% set content = bos_token + content %}
  {% endif %}

  {{ content }}
{% endfor %}

{% if add_generation_prompt %}
  {{ '<|start_header_id|>assistant<|end_header_id|>\n\n' }}
{% endif %}
```

the tool generates this stdout

```txt
=== Variable Analysis Report ===

External Variables (required context):
  add_generation_prompt
  bos_token
  messages

Internal Variables (defined in template):
  content
  loop_messages

Loop Variables:
  message (from loop_messages)
```

and prints an outline of the expected data structure

```json
{
  "add_generation_prompt": "",
  "bos_token": "",
  "messages": [
    {
      "content": "",
      "role": ""
    }
  ]
}
```

>[!IMPORTANT]
> the critical thing to note is the ability handle indirection (`set loop_messages`) and nested properties (`message['role']`). In most cases querying for variables will only retrun the top level keys, but this tool visits each node in the template and builds a complete picture of the data structure.

### Implementation notes

- **Single‑pass analysis** — depth‑first walk over the Minijinja AST; linear *O(n)*.
- **First‑touch classification** (`VariableTracker`)
  - **Read** – value comes from the render context
  - **Set** – template‑local assignment
  - **Alias** – `set x = y`; `x` forwards to `y` (cycle‑safe)
  - **LoopVar(iterable)** – induction variable and its iterable path
- **State buckets**
  `external_vars` (inputs) • `internal_vars` (locals) • `loop_vars` (`item → items`) • `object_attrs` (observed properties) • `object_aliases` (alias → canonical)
- **Hierarchy capture** — a read of `user.address.city` stores every segment so parents exist even if never referenced directly.
- **Alias & loop‑aware JSON**
  - `resolve_alias_chain` walks aliases until the real variable (guards against cycles).
  - `find_iterated_var` flags iterable objects so the generated schema uses `[ { … } ]`.
- **Edge handling** — ignores `loop.*`, numeric subscripts, and normalises `obj['key']` → `obj.key`.
- **Deterministic output** — ordering via `BTreeSet`; leaves default to `""`.
**API** — `analyze(template) -> TemplateAnalysis` returns all buckets plus the synthesized JSON skeleton.


## Development

- **Testing**: run `cargo test` be
- **Debugging**: use the `--verbose` flag to see detailed of the variable tracker during runtime.