import assert from "node:assert/strict";
import { createServer } from "node:http";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawn } from "node:child_process";
import test from "node:test";

function getFreePort() {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      server.close(() => resolve(address.port));
    });
    server.on("error", reject);
  });
}

async function waitForReady(url, timeoutMs = 15000) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    try {
      const res = await fetch(`${url}/api/system/health`);
      if (res.ok) return;
    } catch {
      // retry
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(`Timed out waiting for ${url}`);
}

async function request(url, path, opts = {}) {
  const { method = "GET", body, cookie, json = true, headers = {} } = opts;
  const target = new URL(path, url);
  const res = await fetch(target, {
    method,
    headers: {
      ...(cookie ? { cookie } : {}),
      ...headers,
      ...(body != null && !(body instanceof FormData) ? { "content-type": "application/json" } : {}),
    },
    ...(body != null
      ? { body: body instanceof FormData ? body : JSON.stringify(body) }
      : {}),
  });
  if (!json) return res;
  const text = await res.text();
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch {
    parsed = { raw: text };
  }
  return { status: res.status, ok: res.ok, headers: res.headers, json: () => parsed, text: () => text };
}

function extractCookie(res) {
  const setCookie = res.headers.get("set-cookie") || "";
  return setCookie.split(",")[0].split(";")[0].trim();
}

test("KB proxy forwards uploads and config through the control panel", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "tcp-kb-"));
  const secretFile = join(root, "kb_admin_api_key");
  await writeFile(secretFile, "kb-secret\n", "utf8");

  const enginePort = await getFreePort();
  const kbPort = await getFreePort();
  const panelPort = await getFreePort();
  const engineToken = "engine-token";

  const fakeEngine = await new Promise((resolve) => {
    const server = createServer((req, res) => {
      if ((req.url || "").startsWith("/global/health")) {
        res.writeHead(200, { "content-type": "application/json" });
        res.end(JSON.stringify({ ready: true, healthy: true, version: "fake-engine" }));
        return;
      }
      res.writeHead(404);
      res.end();
    });
    server.listen(enginePort, "127.0.0.1", () => resolve(server));
  });
  t.after(() => fakeEngine.close());

  let seenAuth = "";
  let seenBody = "";
  const fakeKb = await new Promise((resolve) => {
    const server = createServer((req, res) => {
      const chunks = [];
      req.on("data", (chunk) => chunks.push(Buffer.from(chunk)));
      req.on("end", () => {
        seenAuth = String(req.headers.authorization || "");
        seenBody = Buffer.concat(chunks).toString("utf8");

        if ((req.url || "").startsWith("/admin/collections")) {
          res.writeHead(200, { "content-type": "application/json" });
          res.end(JSON.stringify({ collections: [{ collection_id: "acme", document_count: 1 }] }));
          return;
        }

        if ((req.url || "").startsWith("/admin/documents")) {
          res.writeHead(200, { "content-type": "application/json" });
          res.end(
            JSON.stringify({
              document: {
                doc_id: "acme/welcome",
                collection_id: "acme",
                title: "Welcome",
              },
            })
          );
          return;
        }

        if ((req.url || "").startsWith("/admin/reindex")) {
          res.writeHead(200, { "content-type": "application/json" });
          res.end(JSON.stringify({ ok: true }));
          return;
        }

        res.writeHead(404, { "content-type": "application/json" });
        res.end(JSON.stringify({ ok: false, error: "not found" }));
      });
    });
    server.listen(kbPort, "127.0.0.1", () => resolve(server));
  });
  t.after(() => fakeKb.close());

  const baseUrl = `http://127.0.0.1:${panelPort}`;
  const panel = spawn(process.execPath, ["bin/setup.js"], {
    cwd: new URL("..", import.meta.url),
    env: {
      ...process.env,
      TANDEM_CONTROL_PANEL_PORT: String(panelPort),
      TANDEM_ENGINE_URL: `http://127.0.0.1:${enginePort}`,
      TANDEM_CONTROL_PANEL_AUTO_START_ENGINE: "0",
      TANDEM_API_TOKEN: engineToken,
      TANDEM_KB_ADMIN_URL: `http://127.0.0.1:${kbPort}`,
      TANDEM_KB_ADMIN_API_KEY_FILE: secretFile,
      TANDEM_KB_DEFAULT_COLLECTION_ID: "acme",
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  t.after(() => {
    if (!panel.killed) panel.kill("SIGTERM");
  });

  await waitForReady(baseUrl);

  const login = await request(baseUrl, "/api/auth/login", {
    method: "POST",
    body: { token: engineToken },
    json: false,
  });
  assert.equal(login.status, 200);
  const cookie = extractCookie(login);

  const config = await request(baseUrl, "/api/knowledgebase/config", { cookie });
  assert.equal(config.status, 200);
  assert.equal(config.json().default_collection_id, "acme");

  const collections = await request(baseUrl, "/api/knowledgebase/collections", { cookie });
  assert.equal(collections.status, 200);
  assert.equal(collections.json().collections[0].collection_id, "acme");

  const form = new FormData();
  form.set("collection_id", "acme");
  form.set("file", new Blob(["Hello from the KB proxy."] , { type: "text/markdown" }), "welcome.md");

  const upload = await request(baseUrl, "/api/knowledgebase/documents", {
    method: "POST",
    body: form,
    cookie,
    json: false,
    headers: {},
  });
  assert.equal(upload.status, 200);
  const payload = await upload.json();
  assert.equal(payload.document.doc_id, "acme/welcome");
  assert.equal(seenAuth, "Bearer kb-secret");
  assert.match(seenBody, /collection_id/);
  assert.match(seenBody, /welcome\.md/);
});
