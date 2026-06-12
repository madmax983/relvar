import sys

with open('pr_description.md', 'r', encoding='utf-8') as f:
    lines = f.readlines()

title = lines[0].strip()
body = ''.join(lines[1:]).strip()

with open('pr.py', 'w', encoding='utf-8') as f:
    f.write('''import sys

with open('pr_description.md', 'r', encoding='utf-8') as f:
    lines = f.readlines()

title = lines[0].strip()
body = ''.join(lines[1:]).strip()

print(f"Submitting PR with title: {title}\\n\\n{body}\\n")
''')
