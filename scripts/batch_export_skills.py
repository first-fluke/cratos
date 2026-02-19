#!/usr/bin/env python3
import subprocess
import shutil
import os
import sys

def main():
    # Ensure cratos is built
    print("Building Cratos...")
    subprocess.run(["uv", "run", "cargo", "build"], check=True)
    
    # Get skill list
    print("Fetching active skills...")
    cmd = ["uv", "run", "cargo", "run", "--bin", "cratos", "--", "skill", "list", "--active"]
    result = subprocess.run(cmd, capture_output=True, text=True)
    
    if result.returncode != 0:
        print("Error listing skills:")
        print(result.stderr)
        return

    skills_to_export = []
    
    for line in result.stdout.splitlines():
        line = line.strip()
        if not line: continue
        if line.startswith("Cratos Skills"): continue
        if line.startswith("-----"): continue
        
        parts = line.split()
        if len(parts) < 4: continue
        
        # Icon is usually first char unicode, might be attached or separate depending on terminal
        # But split() should handle it.
        # Format: ICON Name Category Origin ...
        # parts[0] is Icon
        # parts[1] is Name
        # parts[2] is Category
        # parts[3] is Origin
        
        name = parts[1]
        origin = parts[3]
        
        if origin == "built":
            skills_to_export.append(name)

    print(f"Found {len(skills_to_export)} builtin skills to export.")
    
    for skill in skills_to_export:
        print(f"Exporting {skill}...")
        export_cmd = ["uv", "run", "cargo", "run", "--bin", "cratos", "--", "skill", "export", "--markdown", skill]
        subprocess.run(export_cmd, check=True)

    print("Batch export complete.")

if __name__ == "__main__":
    main()
