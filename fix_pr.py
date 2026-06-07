with open("pr_description.md", "r") as f:
    lines = f.readlines()

title = lines[0].strip()
body = "".join(lines[1:]).strip()
body = body.replace('"', '\\"').replace('\n', '\\n')

with open("pr.py", "w") as f:
    f.write(f"""
def submit_pr():
    print("Submitting PR with title: {title}\\n")
    print("{body}")

if __name__ == '__main__':
    submit_pr()
""")
