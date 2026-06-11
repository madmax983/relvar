import sys

with open("pr_description.md", "r") as f:
    lines = f.readlines()

title = lines[0].strip()
body = "".join(lines[1:]).strip()

new_pr_script = f"""import sys
import os

title = {repr(title)}
body = {repr(body)}

print(f"Submitting PR with title: {{title}}\\n\\n{{body}}")
"""

with open("pr.py", "w") as f:
    f.write(new_pr_script)
