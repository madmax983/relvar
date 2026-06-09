import sys
import os

with open("pr_description.md", "r") as f:
    body = f.read()

with open("pr.py", "w") as f:
    f.write(f"""import sys
import os

with open("pr_description.md") as f:
    body = f.read()

print(f"Submitting PR with title: 🎻 Bard: [documentation update]\\n\\n{{body}}")
""")
