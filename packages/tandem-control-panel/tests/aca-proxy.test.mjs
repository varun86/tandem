import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
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
  const { method = "GET", body, cookie, json = true } = opts;
  const target = new URL(path, url);
  const res = await fetch(target, {
    method,
    headers: {
      ...(cookie ? { cookie } : {}),
      ...(body != null ? { "content-type": "application/json" } : {}),
    },
    ...(body != null ? { body: JSON.stringify(body) } : {}),
  });
  if (!json) return res;
  const text = await res.text();
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch {
    parsed = { raw: text };
  }
  return { status: res.status, ok: res.ok, json: () => parsed, text: () => text };
}

function extractCookie(res) {
  const setCookie = res.headers.get("set-cookie") || "";
  return setCookie.split(",")[0].split(";")[0].trim();
}

test("ACA proxy forwards authenticated project requests through the control panel", async (t) => {
  const enginePort = await getFreePort();
  const acaPort = await getFreePort();
  const panelPort = await getFreePort();
  const engineToken = "engine-token";
  const acaToken = "aca-token";

  const fakeEngine = await new Promise((resolve) => {
    const server = createServer((req, res) => {
      if ((req.url || "").startsWith("/global/health")) {
        res.writeHead(200, { "content-type": "application/json" });
        res.end(JSON.stringify({ ready: true, healthy: true, version: "fake-engine" }));
        return;
      }
      if ((req.url || "").startsWith("/context/runs")) {
        res.writeHead(200, { "content-type": "application/json" });
        res.end(JSON.stringify({ runs: [] }));
        return;
      }
      res.writeHead(404);
      res.end();
    });
    server.listen(enginePort, "127.0.0.1", () => resolve(server));
  });
  t.after(() => fakeEngine.close());

  const fakeAca = await new Promise((resolve) => {
    const server = createServer((req, res) => {
      const auth = req.headers.authorization || "";
      if ((req.url || "").startsWith("/health")) {
        res.writeHead(200, { "content-type": "application/json" });
        res.end(JSON.stringify({ status: "healthy", version: "aca-test" }));
        return;
      }
      if (auth !== `Bearer ${acaToken}`) {
        res.writeHead(401, { "content-type": "application/json" });
        res.end(JSON.stringify({ detail: "Invalid API Token" }));
        return;
      }
      if ((req.url || "").startsWith("/projects")) {
        res.writeHead(200, { "content-type": "application/json" });
        res.end(
          JSON.stringify({
            demo: {
              slug: "demo",
              repo_url: "https://example.com/demo.git",
              task_source: { type: "manual", prompt: "Fix the bug" },
            },
          })
        );
        return;
      }
      if ((req.url || "").startsWith("/runs")) {
        res.writeHead(200, { "content-type": "application/json" });
        res.end(JSON.stringify({ runs: [{ run_id: "run-1", project_slug: "demo", status: "running" }] }));
        return;
      }
      if ((req.url || "").startsWith("/mcp")) {
        let body = "";
        req.on("data", (chunk) => {
          body += chunk;
        });
        req.on("end", () => {
          let parsed = {};
          try {
            parsed = JSON.parse(body || "{}");
          } catch {
            parsed = {};
          }
          if (parsed?.method === "tools/call" && parsed?.params?.name === "describe_aca") {
            res.writeHead(200, { "content-type": "application/json" });
            res.end(
              JSON.stringify({
                jsonrpc: "2.0",
                id: parsed.id,
                result: {
                  overview: {
                    summary: "ACA is running.",
                    auth: { mode: "bearer_api_key", required: true },
                    validation: { ok: true, errors: [] },
                    task_source: { type: "manual" },
                    repository: { slug: "demo", path: "repos/demo" },
                    provider: { id: "openai", model: "gpt-4.1-mini" },
                    execution: { backend: "local" },
                    tandem: { base_url: "http://127.0.0.1", startup_mode: "reuse_only", update_policy: "notify" },
                    engine: { healthy: true, running: true, status: "running" },
                    github_mcp: { enabled: true, connected: true, scope: "intake_finalize", remote_sync: "status_comment" },
                    workspace: { summary: {}, workspace: {}, active_project: {}, configured_project: {} },
                    latest_run: { run_id: "run-1", status: "running", is_running: true },
                    allowed_next_actions: ["inspect_latest_run"],
                    doc_refs: [],
                  },
                },
              })
            );
            return;
          }
          res.writeHead(400, { "content-type": "application/json" });
          res.end(JSON.stringify({ error: "unsupported mcp request" }));
        });
        return;
      }
      res.writeHead(404);
      res.end();
    });
    server.listen(acaPort, "127.0.0.1", () => resolve(server));
  });
  t.after(() => fakeAca.close());

  const baseUrl = `http://127.0.0.1:${panelPort}`;
  const panel = spawn(process.execPath, ["bin/setup.js"], {
    cwd: new URL("..", import.meta.url),
    env: {
      ...process.env,
      TANDEM_CONTROL_PANEL_PORT: String(panelPort),
      TANDEM_ENGINE_URL: `http://127.0.0.1:${enginePort}`,
      TANDEM_CONTROL_PANEL_AUTO_START_ENGINE: "0",
      TANDEM_API_TOKEN: engineToken,
      ACA_BASE_URL: `http://127.0.0.1:${acaPort}`,
      ACA_API_TOKEN: acaToken,
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

  const projects = await request(baseUrl, "/api/aca/projects", { cookie });
  assert.equal(projects.status, 200);
  assert.equal(projects.json().demo.slug, "demo");

  const runs = await request(baseUrl, "/api/aca/runs", { cookie });
  assert.equal(runs.status, 200);
  assert.equal(Array.isArray(runs.json().runs), true);
  assert.equal(runs.json().runs[0].project_slug, "demo");

  const overview = await request(baseUrl, "/api/aca/overview", { cookie });
  assert.equal(overview.status, 200);
  assert.equal(overview.json().overview.latest_run.run_id, "run-1");
  assert.equal(overview.json().overview.github_mcp.connected, true);
});
