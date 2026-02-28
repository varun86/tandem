#!/usr/bin/env node

/**
 * Tandem Control Panel Launcher
 * 
 * 1. Spawns the Tandem Engine binary (provided via `tandem-ai`)
 * 2. Starts a lightweight HTTP server to serve the pure HTML/JS frontend UI
 */

import { spawn } from "child_process";
import { createServer } from "http";
import { readFileSync, existsSync } from "fs";
import { join, dirname, extname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const PORTAL_PORT = 39732;
const ENGINE_PORT = 39731;

const log = (msg) => console.log(`[Tandem Setup] ${msg}`);
const err = (msg) => console.error(`[Tandem Setup] ERROR: ${msg}`);

// 1. Spawn Tandem Engine
// We spawn `tandem-engine serve` using the globally available binary path provided by the `@frumu/tandem` dependency.
// In a dev environment where it's linked via workspace, we use npx.
log(`Starting background Tandem Engine on port ${ENGINE_PORT}...`);
const engine = spawn("npx", ["tandem-engine", "serve", "--port", ENGINE_PORT.toString(), "--hostname", "127.0.0.1"], {
    stdio: "inherit",
    shell: true
});

engine.on("error", (e) => {
    err(`Failed to start engine: ${e.message}`);
});

process.on("SIGINT", () => {
    engine.kill("SIGINT");
    process.exit();
});

// 2. Serve Static Frontend UI
const PUBLIC_DIR = join(__dirname, "..", "public");

const MIME_TYPES = {
    ".html": "text/html",
    ".js": "text/javascript",
    ".css": "text/css",
    ".png": "image/png",
    ".svg": "image/svg+xml",
    ".json": "application/json",
};

const server = createServer((req, res) => {
    let filePath = join(PUBLIC_DIR, req.url === "/" ? "index.html" : req.url);

    // Prevent directory traversal attacks
    if (!filePath.startsWith(PUBLIC_DIR)) {
        res.writeHead(403);
        res.end("Forbidden");
        return;
    }

    if (!existsSync(filePath)) {
        // SPA Fallback logic for client-side routing
        if (!extname(filePath)) {
            filePath = join(PUBLIC_DIR, "index.html");
        } else {
            res.writeHead(404);
            res.end("Not Found");
            return;
        }
    }

    try {
        const ext = extname(filePath);
        const mime = MIME_TYPES[ext] || "text/plain";
        const content = readFileSync(filePath);
        res.writeHead(200, { "Content-Type": mime });
        res.end(content);
    } catch (e) {
        res.writeHead(500);
        res.end("Server Error");
    }
});

server.listen(PORTAL_PORT, () => {
    log(`=========================================`);
    log(`Control Panel running at http://localhost:${PORTAL_PORT}`);
    log(`=========================================`);
});
