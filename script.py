import re

with open('pr_description.md', 'r') as f:
    lines = f.readlines()

title = lines[0].strip()
body = "".join(lines[1:]).strip()

with open('pr.py', 'r') as f:
    content = f.read()

content = re.sub(r'pr_title = ".*?"', f'pr_title = "{title}"', content)
content = re.sub(r'pr_body = """.*?"""', f'pr_body = """{body}"""', content, flags=re.DOTALL)

with open('pr.py', 'w') as f:
    f.write(content)
