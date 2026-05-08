import sys
import argparse

parser = argparse.ArgumentParser()
parser.add_argument('--title', required=True)
parser.add_argument('--description', required=True)
args = parser.parse_args()

print(f"Submitting PR with title: {args.title}\n\n{args.description}")
