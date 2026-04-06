import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import { generateAgentCatalog } from "./generate-agent-catalog.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const sourceRoot = path.join(repoRoot, "docs/internal/tandem-proprietary/categories");

function walkTomls(dir) {
  const out = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...walkTomls(fullPath));
      continue;
    }
    if (entry.isFile() && entry.name.endsWith(".toml")) {
      out.push(fullPath);
    }
  }
  return out;
}

function searchCatalog(catalog, query) {
  const q = String(query || "")
    .trim()
    .toLowerCase();
  return catalog.agents.filter((entry) => {
    if (!q) return true;
    const haystack = [
      entry.name,
      entry.summary,
      entry.category_id,
      entry.category_title,
      entry.source_path,
      entry.source_file,
      entry.sandbox_mode,
      entry.role,
      ...(entry.tags || []),
      ...(entry.requires || []),
    ]
      .join(" ")
      .toLowerCase();
    return haystack.includes(q);
  });
}

test("agent catalog generates one entry per TOML manifest", () => {
  const catalog = generateAgentCatalog();
  const tomlFiles = walkTomls(sourceRoot);
  assert.equal(catalog.agents.length, tomlFiles.length);
  assert.equal(catalog.categories.length, 10);
  assert.ok(catalog.agents.every((entry) => !("model" in entry)));
});

test("agent catalog search matches by name, category, tag, and filename", () => {
  const catalog = generateAgentCatalog();

  const byName = searchCatalog(catalog, "frontend-developer");
  assert.ok(byName.some((entry) => entry.id === "frontend-developer"));

  const byCategory = searchCatalog(catalog, "core-development");
  assert.ok(byCategory.some((entry) => entry.category_id === "core-development"));

  const byTag = searchCatalog(catalog, "frontend");
  assert.ok(byTag.some((entry) => entry.tags.includes("frontend")));

  const byFile = searchCatalog(catalog, "frontend-developer.toml");
  assert.ok(byFile.some((entry) => entry.source_file === "frontend-developer.toml"));
});

test("agent catalog source paths point to real files", () => {
  const catalog = generateAgentCatalog();
  for (const entry of catalog.agents) {
    const fullPath = path.join(repoRoot, entry.source_path);
    assert.equal(fs.existsSync(fullPath), true, `Missing source file: ${entry.source_path}`);
  }
});
