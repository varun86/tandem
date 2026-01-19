# Tandem v1.0.0 Release Notes

## Highlights

- AI-powered plan-and-execute workflow for creating PowerPoint presentations
- Slides tool category to guide LLMs with structured presentation instructions
- Presentation previewer with navigation, thumbnails, and speaker notes
- One-click export to PPTX using the built-in Rust backend

## Features

### Presentations

- Two-phase flow: outline planning (reviewable) then JSON execution
- `.tandem.ppt.json` format as source of truth for slides
- Themes: light, dark, corporate, minimal
- Layouts: title, content, section, blank
- Slide navigation via arrows, buttons, and thumbnail strip
- Speaker notes toggle
- Export to `.pptx` powered by `ppt-rs`

### Chat + Controls

- Context toolbar below the chat input for quick model/tool selection
- Tool category picker with badges for enabled capabilities
- Model selector grouped by provider and context size

### File Handling

- Automatic detection and preview of `.tandem.ppt.json` files
- Dedicated presentation preview experience in the file browser

## Known Limitations

- No image embedding yet in exported slides
- Basic layout options only; advanced positioning is not included

## Next Up

- Image and chart support
- More layout templates and theme customization
- PDF export and batch export
