#!/bin/bash
# Plexus Document Chunker
# Fixed-size line chunking with overlap for fan-out extraction.
# Reads file content directly - accepts file path as input.
#
# llm-orc passes JSON: {"input": "file_path", "parameters": {}, "context": {}}
# This script extracts the file path, reads the file, and chunks it.
#
# Usage: chunker.sh [chunk_lines] [overlap_lines]
#   chunk_lines:   lines per chunk (default: 150, ~2000 words)
#   overlap_lines: overlap between chunks (default: 20)
#
# Outputs JSON array of chunks for llm-orc fan-out processing.
# Chunk IDs indicate line ranges for provenance (e.g., "lines_1-150")

set -euo pipefail

CHUNK_LINES=${1:-150}
OVERLAP_LINES=${2:-20}

# Read stdin and extract file path from llm-orc JSON wrapper
RAW_INPUT=$(cat)

# Extract the input field (file path) from JSON
if echo "$RAW_INPUT" | grep -q '"input"'; then
    # Extract file path from JSON input field
    FILE_PATH=$(echo "$RAW_INPUT" | sed 's/.*"input"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
else
    # Not JSON wrapped, treat as file path directly
    FILE_PATH="$RAW_INPUT"
fi

# Trim whitespace
FILE_PATH=$(echo "$FILE_PATH" | tr -d '\n' | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')

# Check if file exists
if [ ! -f "$FILE_PATH" ]; then
    echo "Error: File not found: $FILE_PATH" >&2
    echo "[]"
    exit 0
fi

# Read and chunk the file
cat "$FILE_PATH" | awk -v chunk="$CHUNK_LINES" -v overlap="$OVERLAP_LINES" -v filepath="$FILE_PATH" '
{
    lines[NR] = $0
}
END {
    # Use standard script agent output pattern for fan-out
    printf "{\"success\": true, \"data\": ["

    if (NR == 0) {
        printf "]}\n"
        exit
    }

    start = 1
    first = 1

    while (start <= NR) {
        end = start + chunk - 1
        if (end > NR) end = NR

        # Build chunk content
        content = ""
        for (i = start; i <= end; i++) {
            if (content != "") content = content "\n"
            content = content lines[i]
        }

        if (content != "") {
            if (!first) printf ","
            first = 0

            # Escape for JSON
            gsub(/\\/, "\\\\", content)
            gsub(/"/, "\\\"", content)
            gsub(/\t/, "\\t", content)
            gsub(/\r/, "\\r", content)
            gsub(/\n/, "\\n", content)

            printf "\n    {\"chunk_id\": \"lines_%d-%d\", \"file\": \"%s\", \"start_line\": %d, \"end_line\": %d, \"content\": \"%s\"}", start, end, filepath, start, end, content
        }

        # If we reached the end, break
        if (end >= NR) break

        # Advance with overlap, ensuring we move forward at least 1 line
        next_start = end + 1 - overlap
        if (next_start <= start) next_start = start + 1
        start = next_start
    }

    printf "\n  ]}\n"
}
'
