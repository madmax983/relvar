
import requests
import json
import os

def submit_pr():
    url = "http://localhost:8000/submit_pr"

    payload = {
        "title": '🪒 Razor: Remove duplicated KnowledgeGraph experimental feature',
        "body": '💡 **The Spark:** The codebase contained two identical experimental features for `KnowledgeGraph`, one in `relvar/src/experimental` and one in `relvar-core/src/experimental`. This goes against the KISS principle and DRY, adding unnecessary bloat and cognitive load.\n\n🚀 **The Feature:** Removed the duplicated `knowledge_graph.rs` from the `relvar` crate and its corresponding module export. We retained the robust version in `relvar-core`.\n\n🔭 **The Potential:** Removing this duplicate logic reduces compile times and clarifies the codebase architecture.\n\n⚠️ **Risk:** None. The core functionality remains accessible through the `relvar-core` crate.'
    }

    headers = {
        "Content-Type": "application/json"
    }

    try:
        response = requests.post(url, json=payload, headers=headers)
        if response.status_code == 200:
            print("Successfully submitted PR")
            print("Response:", response.json())
        else:
            print(f"Failed to submit PR. Status code: {response.status_code}")
            print("Response:", response.text)
    except requests.exceptions.ConnectionError:
        print("Failed to connect to the PR submission server.")
        print("This is expected if the server is not running in this environment.")
        print(f"PR Title: {payload['title']}")
        print(f"PR Body: \n{payload['body']}")

if __name__ == "__main__":
    submit_pr()
