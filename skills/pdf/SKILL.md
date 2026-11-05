---
name: pdf
description: Make a good-looking PDF (one-pagers, reports, handouts, letters) from HTML, and read PDFs.
---
# Making PDFs

1. Write the document as one self-contained HTML file in `tmp/` (inline CSS, no external fonts
   unless you download them into `tmp/`). Design for paper:
   - Letter is 612 × 792 px, A4 is 595 × 842 px (CSS px = PDF points). Set
     `html,body{margin:0}` and give the body its own padding (36–54 px).
   - System fonts that look good: `-apple-system`, `"New York"`, `Georgia`, `"Avenir Next"`,
     `"SF Mono"` for code. 10–11.5 pt body text, 1.4 line height.
   - "One page" means it must fit 792 px (Letter) of content height: check, then trim.
2. Render: `openclicky-helper pdf tmp/doc.html output/Name.pdf letter` (or `a4`). It prints the
   page count; if it isn't what you wanted, change the HTML and render again.
3. Check it: `openclicky-helper text output/Name.pdf` to read it back, and
   `qlmanage -t -s 1200 -o tmp output/Name.pdf` then `view_image tmp/Name.pdf.png` to look at it.
4. Put it where they asked with `device_save_file` (never overwrites), and list the saved path
   under ARTIFACTS.

Reading a PDF: `device_read_file` (from their Mac) or `openclicky-helper text file.pdf`.
