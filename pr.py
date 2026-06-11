import sys
import argparse

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--title", required=False)
    parser.add_argument("--body-file", required=False)
    args = parser.parse_args()

    if args.title and args.body_file:
        with open(args.body_file, 'r') as f:
            body = f.read()
        print(f"Submitting PR with title: {args.title}\n\n{body}")
    else:
        try:
            with open("pr_description.md", 'r') as f:
                lines = f.readlines()
            title = lines[0].strip()
            body = "".join(lines[1:]).strip()
            print(f"Submitting PR with title: {title}\n\n{body}")
        except Exception as e:
            print(f"Failed to submit: {e}")

if __name__ == "__main__":
    main()
