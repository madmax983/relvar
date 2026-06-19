import subprocess

def read_file(filepath):
    with open(filepath, "r") as f:
        return f.read()

body = read_file("pr_description.md")

title = ""
for line in body.split("\n"):
    if line.strip():
        title = line.strip()
        break

print(f"Using tool default_api:submit")
print(f"Title: {title}")
print(f"Description: {body}")
