---
name: documents
description: Word documents, RTF, plain text and spreadsheets (CSV/Numbers/Excel) with built-in macOS tools.
---
# Documents and sheets

- **Word (.docx)**: write clean HTML, then `textutil -convert docx tmp/doc.html -output output/Name.docx`.
  Headings, bold, lists and simple tables survive; keep styling simple.
- **RTF / Pages-friendly**: `textutil -convert rtf …`. To read .docx/.rtf/.pages use `device_read_file`.
- **Spreadsheets**: write CSV with a header row (quote fields with commas). For a real .xlsx,
  try `python3 -c "import openpyxl"`; if it's missing, `python3 -m pip install --target tmp/py openpyxl`
  and run with `PYTHONPATH=tmp/py`. Numbers opens CSV and XLSX.
- Check what you made: read it back (`textutil -convert txt -stdout`, or re-open the CSV) before
  you say it's done.
