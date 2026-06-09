with open("pr_description.md", "r") as f:
    lines = f.readlines()

title = lines[0].strip()
body = "".join(lines[1:]).strip()

with open("pr.py", "w") as f:
    f.write(f'''import sys

print(f"Submitting PR with title: {title}\\n\\n{body}")
''')
