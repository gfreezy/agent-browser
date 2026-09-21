"""Exercise real pipe installation and retained transport on macOS/Windows."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import uuid

binary = str(Path(sys.argv[1]).resolve())
if len(sys.argv) > 2:
    chrome = Path(sys.argv[2])
elif sys.platform == "darwin":
    chrome = Path("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
else:
    chrome = next(
        (Path(os.environ.get(root, "")) / "Google/Chrome/Application/chrome.exe"
         for root in ("PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA")
         if (Path(os.environ.get(root, "")) / "Google/Chrome/Application/chrome.exe").is_file()),
        None,
    )
assert chrome and chrome.is_file(), "Chrome is required for the helper integration test"

with tempfile.TemporaryDirectory(prefix="xfp-pipe-") as directory:
    env = {k: v for k, v in os.environ.items() if not k.startswith("AGENT_BROWSER_")}
    env["AGENT_BROWSER_BACKGROUND_WINDOW"] = "1"
    config = Path(directory) / "config.json"
    config.write_text("{}")
    prefix = [binary, "--config", str(config), "--json", "--headed",
              "--session", "pipe-" + uuid.uuid4().hex[:10],
              "--profile", str(Path(directory) / "profile"),
              "--executable-path", str(chrome)]

    def command(*args):
        result = subprocess.run(prefix + list(args), env=env, capture_output=True,
                                text=True, timeout=90)
        assert result.returncode == 0, result.stdout + result.stderr
        response = json.loads(result.stdout)
        assert response.get("success"), response
        return response["data"]

    try:
        for _ in range(2):
            command("open", "about:blank")
            first = next(t for t in command("tab", "list")["tabs"] if t["active"])
            command("tab", "new", "about:blank")
            command("tab", first["targetId"])
            assert command("eval", "document.visibilityState")["result"] == "visible"
            # A second CLI request and a full restart must not lose the pipe.
            assert command("eval", "6 * 7")["result"] == 42
            command("close")
        print("PASS: helper pipe installation, selected tab, retained connection, profile restart")
    finally:
        subprocess.run(prefix + ["close"], env=env, capture_output=True, timeout=90)
