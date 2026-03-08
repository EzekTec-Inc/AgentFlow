---
name: document_processor
version: 1.0.0
description: "Process documents: extract entities, identify context, and convert formats"
tools:
  - name: "convert_text"
    description: "Convert text documents using pandoc"
    command: "pandoc"
    args: ["{{input_file}}", "-o", "{{output_file}}"]
  - name: "convert_image"
    description: "Convert images using imagemagick"
    command: "convert"
    args: ["{{input_file}}", "{{output_file}}"]
---
