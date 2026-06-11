import subprocess

def main():
    with open('pr_description.md', 'r') as f:
        lines = f.readlines()

    title = lines[0].strip()
    description = "".join(lines[1:]).strip()

    with open('temp_pr_body.md', 'w') as f:
        f.write(description)

    subprocess.run(['python3', 'pr.py', '--title', title, '--body-file', 'temp_pr_body.md'])

if __name__ == "__main__":
    main()
