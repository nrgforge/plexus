#!/usr/bin/env python3
"""
extract_content.py — Read a file, detect MIME type, extract text, chunk by sections.

Input (stdin): JSON with file_path and optional section boundaries.
  Wrapped in llm-orc envelope (ScriptAgentInput or legacy wrapper).

  Inner payload:
    {
      "file_path": "/path/to/file",
      "sections": [{"label": "...", "start_line": 1, "end_line": 50}],
      "existing_concepts": ["concept1", "concept2"]
    }

Output (stdout): JSON array of text chunks for fan-out.
  Each chunk:
    {
      "content": "actual text content...",
      "file_path": "/path/to/file",
      "section_label": "Introduction",
      "mime_type": "text/markdown",
      "existing_concepts": ["concept1"]
    }

When sections are present, each section becomes a separate chunk (line-range extraction).
When no sections are present, the file is chunked by a simple size heuristic.
"""
import json
import mimetypes
import os
import sys


def unwrap_input(raw_json):
    """Unwrap llm-orc envelope to get actual data and parameters.

    Handles three formats:
    1. ScriptAgentInput: {"agent_name": "...", "input_data": "<json>", ...}
    2. Legacy wrapper:   {"input": "<json or dict>", "parameters": {...}, ...}
    3. Direct:           {"file_path": "...", ...}

    Returns (data_dict, parameters_dict).
    """
    envelope = json.loads(raw_json) if raw_json.strip() else {}

    # Format 1: ScriptAgentInput
    input_data = envelope.get("input_data", "")
    if isinstance(input_data, str) and input_data.strip():
        try:
            return json.loads(input_data), envelope.get("parameters", {}) or {}
        except json.JSONDecodeError:
            pass

    # Format 2: Legacy wrapper
    if "input" in envelope and "parameters" in envelope:
        inner = envelope["input"]
        params = envelope.get("parameters", {}) or {}
        if isinstance(inner, str) and inner.strip():
            try:
                return json.loads(inner), params
            except json.JSONDecodeError:
                return envelope, params
        if isinstance(inner, dict):
            return inner, params

    # Format 3: Direct
    return envelope, {}


def detect_mime(file_path):
    """Detect MIME type from file extension."""
    mime, _ = mimetypes.guess_type(file_path)
    return mime or "application/octet-stream"


def read_file(file_path):
    """Read file content as text. Returns (lines, error)."""
    try:
        with open(file_path, "r", encoding="utf-8", errors="replace") as f:
            return f.readlines(), None
    except FileNotFoundError:
        return [], f"File not found: {file_path}"
    except PermissionError:
        return [], f"Permission denied: {file_path}"
    except Exception as e:
        return [], f"Error reading {file_path}: {e}"


def chunk_by_sections(lines, sections, file_path, mime_type, existing_concepts):
    """Split lines into chunks based on section boundaries."""
    chunks = []
    for section in sections:
        start = max(0, section.get("start_line", 1) - 1)  # 1-indexed to 0-indexed
        end = section.get("end_line", len(lines))
        content = "".join(lines[start:end]).strip()
        if content:
            chunks.append({
                "content": content,
                "file_path": file_path,
                "section_label": section.get("label", f"lines {start+1}-{end}"),
                "mime_type": mime_type,
                "existing_concepts": existing_concepts,
            })
    return chunks


def chunk_by_size(lines, file_path, mime_type, existing_concepts, max_lines=100):
    """Split lines into roughly equal chunks when no sections are provided."""
    if len(lines) <= max_lines:
        # Small file — single chunk
        return [{
            "content": "".join(lines).strip(),
            "file_path": file_path,
            "section_label": "full_document",
            "mime_type": mime_type,
            "existing_concepts": existing_concepts,
        }]

    chunks = []
    for i in range(0, len(lines), max_lines):
        batch = lines[i:i + max_lines]
        content = "".join(batch).strip()
        if content:
            chunks.append({
                "content": content,
                "file_path": file_path,
                "section_label": f"lines {i+1}-{min(i+max_lines, len(lines))}",
                "mime_type": mime_type,
                "existing_concepts": existing_concepts,
            })
    return chunks


def main():
    raw = sys.stdin.read()
    data, params = unwrap_input(raw)

    file_path = data.get("file_path", "")
    sections = data.get("sections", [])
    existing_concepts = data.get("existing_concepts", [])

    if not file_path:
        print(json.dumps({"error": "No file_path provided"}))
        sys.exit(1)

    # Detect MIME type
    mime_type = detect_mime(file_path)

    # Only process text-like files
    if not (mime_type.startswith("text/") or mime_type in (
        "application/json", "application/xml", "application/javascript",
        "application/typescript", "application/x-yaml", "application/toml",
        "application/octet-stream",  # fallback — try anyway
    )):
        print(json.dumps([{
            "content": f"[Binary file: {mime_type}]",
            "file_path": file_path,
            "section_label": "binary",
            "mime_type": mime_type,
            "existing_concepts": existing_concepts,
        }]))
        return

    # Read file
    lines, error = read_file(file_path)
    if error:
        print(json.dumps({"error": error}))
        sys.exit(1)

    if not lines:
        print(json.dumps([]))
        return

    # Chunk
    max_lines = params.get("max_chunk_lines", 100)
    if sections:
        chunks = chunk_by_sections(lines, sections, file_path, mime_type, existing_concepts)
    else:
        chunks = chunk_by_size(lines, file_path, mime_type, existing_concepts, max_lines)

    print(json.dumps(chunks, indent=2))


if __name__ == "__main__":
    main()
