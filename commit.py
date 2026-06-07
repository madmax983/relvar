import os
import subprocess

def run(cmd):
    subprocess.run(cmd, shell=True, check=True)

with open('pr_description.md', 'r') as f:
    lines = f.read().splitlines()

title = lines[0].strip()
description = "\n".join(lines[1:]).strip()

import json
description_escaped = json.dumps(description)

python_script = f"""
import os
os.environ["TITLE"] = {json.dumps(title)}
os.environ["DESCRIPTION"] = {description_escaped}
"""

with open('submit.py', 'w') as f:
    f.write(python_script)
