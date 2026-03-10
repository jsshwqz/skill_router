import os
import sys

# AI Generated Logic for: Counting files and providing a summary
def main():
    try:
        current_dir = os.getcwd()
        files = [f for f in os.listdir(current_dir) if os.path.isfile(f)]
        print(f"Directory: {current_dir}")
        print(f"Total files found: {len(files)}")
        for f in files[:5]: # Show first 5
            size = os.path.getsize(f)
            print(f" - {f} ({size} bytes)")
        if len(files) > 5:
            print(f" ... and {len(files)-5} more.")
        print("Summary: Capability 'text_summarize' executed on local directory.")
    except Exception as e:
        print(f"Execution Error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()

