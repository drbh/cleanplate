# /// script
# dependencies = [
#   "huggingface_hub",
#   "tqdm"
# ]
# ///

from huggingface_hub import HfApi
from tqdm import tqdm
import time
from collections import defaultdict
import json

models = []
api = HfApi()
for x in tqdm(api.list_models(
    tags=["conversational"],
    library="transformers",
    # sort="downloads",
    # direction=-1,
    expand=["config"],
)):
    models.append(x)

    if len(models) % 10_000 == 0:
        print(f" Downloaded {len(models)} models")
        time.sleep(1) # to avoid rate limit


template_to_model_ids = defaultdict(list)
for m in models:
    if m.config is None:
        continue
    tokenizer_config = m.config.get('tokenizer_config')
    if not tokenizer_config:
        continue
    chat_template = tokenizer_config.get('chat_template')
    if not chat_template: continue
    if isinstance(chat_template, list):
        continue
    template_to_model_ids[chat_template].append(m.id)

# Sort by number of models using the same template (test more common first)
template_to_model_ids = dict(sorted(template_to_model_ids.items(), key=lambda x: len(x[1]), reverse=True))

# print info on number of templates and total models
print(f"Found {len(template_to_model_ids)} templates")
print(f"Found {sum(len(v) for v in template_to_model_ids.values())} models")

with open('chat_template_to_model_ids.json', 'w') as f:
    json.dump(template_to_model_ids, f, indent=2)
