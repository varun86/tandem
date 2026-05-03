#!/usr/bin/env node
import { appendFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(new URL("..", import.meta.url).pathname);
const fixturePath = resolve(
  root,
  "docs/fixtures/bug-monitor-external-log-intake/service.log.jsonl"
);

const baseUrl = (process.env.TANDEM_BASE_URL || "http://localhost:3000/api/engine").replace(
  /\/$/,
  ""
);
const token = process.env.TANDEM_TOKEN || "";
const projectId = process.env.BUG_MONITOR_DEMO_PROJECT_ID || "external-demo";
const sourceId = process.env.BUG_MONITOR_DEMO_SOURCE_ID || "service-jsonl";
const timeoutMs = Number(process.env.BUG_MONITOR_SMOKE_TIMEOUT_MS || 60_000);
const startedAt = Date.now();
const fingerprint = `external-demo-smoke-${startedAt}`;

function headers() {
  return {
    "content-type": "application/json",
    ...(token ? { authorization: `Bearer ${token}` } : {}),
  };
}

async function post(path) {
  const response = await fetch(`${baseUrl}${path}`, {
    method: "POST",
    headers: headers(),
  });
  if (!response.ok) {
    const body = await response.text().catch(() => "");
    throw new Error(`${path} failed (${response.status}): ${body}`);
  }
  return response.json().catch(() => ({}));
}

async function get(path) {
  const response = await fetch(`${baseUrl}${path}`, { headers: headers() });
  if (!response.ok) {
    const body = await response.text().catch(() => "");
    throw new Error(`${path} failed (${response.status}): ${body}`);
  }
  return response.json().catch(() => ({}));
}

function appendDemoFailure() {
  const line = {
    timestamp: new Date().toISOString(),
    level: "error",
    service: "external-demo",
    event: "external_service_crash",
    message: "smoke test worker failed while processing external log intake",
    error: "SmokeError: external log intake smoke marker",
    stack:
      "SmokeError: external log intake smoke marker\n    at smokeTest (docs/fixtures/bug-monitor-external-log-intake/service.log.jsonl:1:1)",
    fingerprint,
  };
  appendFileSync(fixturePath, `${JSON.stringify(line)}\n`, "utf8");
}

async function main() {
  appendDemoFailure();
  await post(
    `/bug-monitor/log-sources/${encodeURIComponent(projectId)}/${encodeURIComponent(
      sourceId
    )}/reset-offset`
  );

  while (Date.now() - startedAt < timeoutMs) {
    const payload = await get("/bug-monitor/incidents?limit=50");
    const incidents = Array.isArray(payload?.incidents) ? payload.incidents : [];
    const match = incidents.find((incident) => incident?.fingerprint === fingerprint);
    if (match) {
      console.log(
        JSON.stringify(
          {
            ok: true,
            fingerprint,
            incident_id: match.incident_id,
            status: match.status,
            draft_id: match.draft_id || null,
          },
          null,
          2
        )
      );
      return;
    }
    await new Promise((resolveTimeout) => setTimeout(resolveTimeout, 2_000));
  }

  throw new Error(`Timed out waiting for Bug Monitor incident ${fingerprint}`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
