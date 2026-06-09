import sys
import subprocess

with open("pr_description.md", "r") as f:
    lines = f.readlines()

title = lines[0].strip()
description = "".join(lines[1:]).strip()

with open("pr.py", "w") as f:
    f.write(f"""import os
from pr_agent import submit_pr

submit_pr(
    title={repr(title)},
    description={repr(description)}
)
""")

# Note: We won't actually execute pr.py here as the default_api:submit tool handles the actual git commit/push simulation in this environment.
# We will use the system's submit tool.
