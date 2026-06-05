import subprocess

with open('pr_description.md', 'r') as f:
    lines = f.readlines()

title = lines[0].strip()
body = ''.join(lines[1:]).strip()

with open('pr_description.md', 'w') as f:
    f.write(body)

# Run pr.py and provide the title
process = subprocess.Popen(['python3', 'pr.py'], stdin=subprocess.PIPE, text=True)
process.communicate(input=f"{title}\n")
