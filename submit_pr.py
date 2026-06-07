import os

with open("pr_description.md", "r") as f:
    lines = f.readlines()

title = lines[0].strip()
body = "".join(lines[1:]).strip()

with open("pr.py", "r") as f:
    pr_script = f.read()

# Replace hardcoded title if exists, otherwise assume it takes args or we need to modify it
# The instructions say "rewrite it using a Python script to dynamically extract the title from the first line of pr_description.md and the body from the remaining lines before running it."
# Let's see what pr.py actually looks like first.
