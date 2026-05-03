import { describe, expect, it } from "vitest";
import { TandemClient } from "../src/client.js";
import type {
  BugMonitorConfigResponse,
  BugMonitorIntakeKeyCreateInput,
  BugMonitorIntakeKeyCreateResponse,
  BugMonitorIntakeKeyDisableResponse,
  BugMonitorIntakeKeyListResponse,
  BugMonitorStatusResponse,
} from "../src/public/index.js";

describe("Bug Monitor external project public types", () => {
  it("accept monitored project config and structured log watcher status", () => {
    const config: BugMonitorConfigResponse = {
      bug_monitor: {
        enabled: true,
        repo: "frumu-ai/tandem",
        monitored_projects: [
          {
            project_id: "aca",
            name: "ACA",
            enabled: true,
            repo: "frumu-ai/aca",
            workspace_root: "/home/evan/aca",
            mcp_server: "github",
            log_sources: [
              {
                source_id: "coder-worker",
                path: "logs/coder-worker.jsonl",
                format: "json",
                minimum_level: "error",
                start_position: "end",
                watch_interval_seconds: 5,
              },
            ],
          },
        ],
      },
    };
    const status: BugMonitorStatusResponse = {
      status: {
        config: config.bug_monitor,
        log_watcher: {
          running: true,
          enabled_projects: 1,
          enabled_sources: 1,
          sources: [
            {
              project_id: "aca",
              source_id: "coder-worker",
              path: "/home/evan/aca/logs/coder-worker.jsonl",
              healthy: true,
              offset: 2048,
              file_size: 4096,
              total_candidates: 1,
              total_submitted: 1,
            },
          ],
        },
      },
    };

    expect(config.bug_monitor.monitored_projects?.[0]?.log_sources?.[0]?.source_id).toBe(
      "coder-worker"
    );
    expect(status.status.log_watcher?.sources?.[0]?.healthy).toBe(true);
  });

  it("accepts scoped intake key management payloads", () => {
    const createInput: BugMonitorIntakeKeyCreateInput = {
      project_id: "aca",
      name: "ACA CI",
      scopes: ["bug_monitor:report"],
    };
    const listResponse: BugMonitorIntakeKeyListResponse = {
      keys: [
        {
          key_id: "intake-key-1",
          project_id: "aca",
          name: "ACA CI",
          key_hash: "[redacted]",
          enabled: true,
          scopes: ["bug_monitor:report"],
          created_at_ms: 1,
          last_used_at_ms: null,
        },
      ],
    };
    const createResponse: BugMonitorIntakeKeyCreateResponse = {
      key: listResponse.keys[0]!,
      raw_key: "tbm_intake_secret",
    };
    const disableResponse: BugMonitorIntakeKeyDisableResponse = {
      key: { ...listResponse.keys[0]!, enabled: false },
    };

    expect(createInput.project_id).toBe("aca");
    expect(createResponse.raw_key).toContain("tbm_intake_");
    expect(disableResponse.key.enabled).toBe(false);
  });

  it("calls scoped intake key endpoints with typed payloads", async () => {
    const client = new TandemClient({ baseUrl: "http://localhost:39731", token: "test-token" });
    const originalFetch = globalThis.fetch;
    const calls: Array<{ url: string; method: string; body?: string }> = [];
    globalThis.fetch = (async (input, init) => {
      calls.push({
        url: String(input),
        method: String(init?.method ?? "GET"),
        body: typeof init?.body === "string" ? init.body : undefined,
      });
      return new Response(
        JSON.stringify({
          keys: [],
          key: {
            key_id: "intake-key-1",
            project_id: "aca",
            name: "ACA CI",
            key_hash: "[redacted]",
            enabled: true,
            scopes: ["bug_monitor:report"],
          },
          raw_key: "tbm_intake_secret",
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }
      );
    }) as typeof fetch;

    try {
      await client.bugMonitor.listIntakeKeys();
      await client.bugMonitor.createIntakeKey({
        project_id: "aca",
        name: "ACA CI",
        scopes: ["bug_monitor:report"],
      });
      await client.bugMonitor.disableIntakeKey("intake/key 1");

      expect(calls[0]).toMatchObject({
        url: "http://localhost:39731/bug-monitor/intake/keys",
        method: "GET",
      });
      expect(calls[1]).toMatchObject({
        url: "http://localhost:39731/bug-monitor/intake/keys",
        method: "POST",
      });
      expect(calls[1]?.body).toBe(
        JSON.stringify({
          project_id: "aca",
          name: "ACA CI",
          scopes: ["bug_monitor:report"],
        })
      );
      expect(calls[2]).toMatchObject({
        url: "http://localhost:39731/bug-monitor/intake/keys/intake%2Fkey%201/disable",
        method: "POST",
      });
    } finally {
      globalThis.fetch = originalFetch;
    }
  });
});
