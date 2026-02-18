#!/usr/bin/env node

const path = require("path");
const { spawnSync } = require("child_process");

const binaryName = process.platform === "win32" ? "tandem-tui.exe" : "tandem-tui";
const binaryPath = path.join(__dirname, "native", binaryName);

const child = spawnSync(binaryPath, process.argv.slice(2), { stdio: "inherit" });

if (child.error) {
  console.error("tandem-tui binary is missing. Reinstall with: npm i -g @frumu/tandem-tui");
  console.error(child.error.message);
  process.exit(1);
}

process.exit(child.status ?? 1);
