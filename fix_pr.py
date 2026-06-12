with open("pr_description.md", "r") as f:
    lines = f.readlines()

title = lines[0].strip()
body = "".join(lines[1:]).strip()

with open("pr.py", "w") as f:
    f.write(f"""#!/usr/bin/env python3
import sys

def main():
    print(r'''Submitting PR with title: {title}

{body}
''')

if __name__ == "__main__":
    main()
""")
