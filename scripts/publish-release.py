import os
import sys
import json
import urllib.request

def publish_release():
    token = os.environ.get("GITHUB_TOKEN")
    if not token:
        print("GITHUB_TOKEN not found in environment. Please set GITHUB_TOKEN or create the release via web interface.")
        print("Release tag v1.3.0 is already created and pushed to GitHub!")
        return

    repo = "JunMystery/Agent-Guidance-Rust"
    tag = "v1.3.0"
    url = f"https://api.github.com/repos/{repo}/releases"
    headers = {
        "Authorization": f"token {token}",
        "Accept": "application/vnd.github.v3+json",
        "User-Agent": "Python-AgentGuidance-Release"
    }

    # 1. Create Release
    payload = {
        "tag_name": tag,
        "name": tag,
        "body": "Release v1.3.0: Performance optimizations, composite workflow actions, ONNX runtime acceleration, pre-tokenized passage cache, and cross-platform concurrency.",
        "draft": False,
        "prerelease": False
    }

    req = urllib.request.Request(url, data=json.dumps(payload).encode("utf-8"), headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            upload_url = data["upload_url"].split("{")[0]
            print(f"Created GitHub release v1.3.0: {data['html_url']}")

            # 2. Upload asset
            zip_path = "dist/agent-guidance-windows-x86_64.zip"
            if os.path.exists(zip_path):
                with open(zip_path, "rb") as f:
                    zip_data = f.read()
                
                upload_req = urllib.request.Request(
                    f"{upload_url}?name=agent-guidance-windows-x86_64.zip",
                    data=zip_data,
                    headers={
                        "Authorization": f"token {token}",
                        "Content-Type": "application/zip",
                        "User-Agent": "Python-AgentGuidance-Release"
                    },
                    method="POST"
                )
                with urllib.request.urlopen(upload_req) as upload_resp:
                    print("Successfully uploaded agent-guidance-windows-x86_64.zip asset!")
    except Exception as e:
        print(f"Release publishing error: {e}")

if __name__ == "__main__":
    publish_release()
