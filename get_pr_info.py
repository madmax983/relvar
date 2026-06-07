import sys

with open('pr_description.md', 'r') as f:
    lines = f.readlines()

title = lines[0].strip()
body = "".join(lines[1:]).strip()

with open('title.txt', 'w') as f:
    f.write(title)

with open('body.txt', 'w') as f:
    f.write(body)
