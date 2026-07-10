import subprocess
try:
    result = subprocess.run(["cargo", "audit"], capture_output=True, text=True, check=True)
    print("Cargo audit passed!")
except subprocess.CalledProcessError as e:
    print("Cargo audit failed!")
    print(e.stdout)
    print(e.stderr)
