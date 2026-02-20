#!/usr/bin/env python3
"""Add Z.ai metadata to PDF files"""
import sys
import os
from pypdf import PdfReader, PdfWriter

def add_metadata(input_path, output_path=None, title=None):
    """Add Z.ai metadata to a PDF file"""
    if output_path is None:
        output_path = input_path
    
    if title is None:
        title = os.path.splitext(os.path.basename(input_path))[0]
    
    reader = PdfReader(input_path)
    writer = PdfWriter()
    
    for page in reader.pages:
        writer.add_page(page)
    
    writer.add_metadata({
        '/Title': title,
        '/Author': 'Z.ai',
        '/Creator': 'Z.ai',
        '/Subject': 'IG Broker Trading Bot - Feature Summary and POC Status'
    })
    
    with open(output_path, 'wb') as f:
        writer.write(f)
    
    print(f"✓ Added Z.ai metadata to: {output_path}")

if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: python add_zai_metadata.py <input.pdf> [-o output.pdf] [-t title]")
        sys.exit(1)
    
    input_file = sys.argv[1]
    output_file = None
    title = None
    
    i = 2
    while i < len(sys.argv):
        if sys.argv[i] == '-o' and i + 1 < len(sys.argv):
            output_file = sys.argv[i + 1]
            i += 2
        elif sys.argv[i] == '-t' and i + 1 < len(sys.argv):
            title = sys.argv[i + 1]
            i += 2
        else:
            i += 1
    
    add_metadata(input_file, output_file, title)
