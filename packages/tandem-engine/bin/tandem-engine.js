#!/usr/bin/env node

const path = require("path");
const { spawnSync } = require("child_process");

const binaryName = process.platform === "win32" ? "tandem-engine.exe" : "tandem-engine";
const binaryPath = path.join(__dirname, "native", binaryName);

const child = spawnSync(binaryPath, process.argv.slice(2), { stdio: "inherit" });

if (child.error) {
  console.error("tandem-engine binary is missing. Reinstall with: npm i -g @frumu/tandem");
  console.error(child.error.message);
  process.exit(1);
}

process.exit(child.status ?? 1);
