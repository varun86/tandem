# PPT Feature Plan: AI-Generated Presentations in Tandem

## Overview

This feature enables users to generate PowerPoint presentations directly from AI prompts. The system uses a JSON-first approach where the AI generates a structured representation of slides, which Tandem previews in-app and can export to a standard `.pptx` file.

## Architecture

### 1. JSON Intermediate Representation (IR)

All presentations are stored and managed as `.tandem.ppt.json` files. This format serves as the "Source of Truth" for:

- AI generation output.
- Frontend rendering for previews.
- Backend input for binary `.pptx` generation.

### 2. Backend (Rust)

A dedicated Tauri command `generate_pptx` will handle the conversion.

- **Library:** `pptx` crate.
- **Responsibility:** Maps the JSON IR to OpenXML structures and saves the file to the user's workspace.

### 3. Frontend (React)

A `PresentationPreview` component will be added to the file browser.

- **Rendering:** Uses Tailwind CSS to mimic slide layouts (16:9 aspect ratio).
- **Interactivity:** Allows users to flip through slides and trigger the "Export" command.

## Data Schema

```typescript
interface Presentation {
  title: string;
  author?: string;
  theme?: "light" | "dark" | "corporate";
  slides: Slide[];
}

interface Slide {
  id: string;
  layout: "title" | "content" | "section" | "blank";
  title?: string;
  subtitle?: string; // For title/section layouts
  content?: string[]; // Bullet points for content layout
  image_url?: string;
}
```

## Implementation Phases

### Phase 1: Schema & Types

- Define Rust structs in `src-tauri/src/presentation.rs`.
- Define TypeScript interfaces in `src/lib/presentation.ts`.

### Phase 2: Backend Command

- Implement `generate_pptx(json: String, path: String)` in `src-tauri/src/commands.rs`.
- Add `pptx` dependency to `Cargo.toml`.

### Phase 3: Frontend UI

- Create `src/components/files/PresentationPreview.tsx`.
- Update `src/components/files/FilePreview.tsx` to handle `.tandem.ppt.json` extensions.

### Phase 4: AI Tooling

- Add `create_presentation` tool to the AI agent's capability list.
- Define a system prompt for generating compliant JSON.

## User Workflow

1. **Request:** User asks "Create a 5-slide presentation on quantum computing."
2. **Generation:** AI uses the `create_presentation` tool to write a `.tandem.ppt.json` file.
3. **Preview:** Tandem automatically opens the file previewer showing the slides.
4. **Export:** User clicks "Export to PPTX" and selects a save location.
