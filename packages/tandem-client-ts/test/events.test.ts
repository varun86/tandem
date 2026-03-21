import { readFileSync } from "node:fs";
import { describe, it, expect } from "vitest";
import { TandemClient } from "../src/client.js";
import { EngineEventSchema } from "../src/normalize/index.js";

const CONTRACT_PATH = "../../contracts/events.json";

describe("EngineEventSchema Contracts", () => {
  let eventsContract: Array<{ type: string; required: string[] }>;

  try {
    const raw = readFileSync(CONTRACT_PATH, "utf-8");
    eventsContract = JSON.parse(raw);
  } catch (err) {
    throw new Error(`Failed to load events.json from ${CONTRACT_PATH}: ${err}`);
  }

  it("has loaded contract definitions", () => {
    expect(eventsContract.length).toBeGreaterThan(0);
  });

  eventsContract.forEach((def) => {
    it(`validates and normalizes '${def.type}' events correctly`, () => {
      // Mock a tolerant wire payload
      const mockWirePayload: Record<string, unknown> = {
        type: def.type,
        timestamp: "2024-01-01T00:00:00Z",
        properties: { custom: "data" },
      };

      // Populate wire-specific required ID fields
      if (def.required.includes("sessionId")) {
        mockWirePayload.sessionID = "s_123"; // Wire name
      }
      if (def.required.includes("runId")) {
        mockWirePayload.run_id = "r_456"; // Wire name
      }

      // Parse through boundary normalized schema
      const result = EngineEventSchema.safeParse(mockWirePayload);
      expect(result.success).toBe(true);

      if (!result.success) return; // For TS narrow
      const event = result.data;

      // Assert canonical structure guarantees
      expect(event.type).toBe(def.type);
      expect(event.properties).toEqual({ custom: "data" });

      if (def.required.includes("sessionId")) {
        expect(event.sessionId).toBe("s_123");
      }
      if (def.required.includes("runId")) {
        expect(event.runId).toBe("r_456");
      }
    });
  });
});

describe("Coder SDK coverage", () => {
  it("lists coder runs with normalized query parameters", async () => {
    const client = new TandemClient({
      baseUrl: "http://localhost:39731",
      token: "test-token",
    });

    const originalFetch = globalThis.fetch;
    let requestedUrl = "";
    globalThis.fetch = (async (input) => {
      requestedUrl = String(input);
      return new Response(JSON.stringify({ runs: [{ coder_run_id: "coder-1" }] }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }) as typeof fetch;

    try {
      const result = await client.coder.listRuns({
        limit: 10,
        workflowMode: "issue_triage",
        repoSlug: "user123/tandem",
      });
      expect(requestedUrl).toContain("/coder/runs?");
      expect(requestedUrl).toContain("limit=10");
      expect(requestedUrl).toContain("workflow_mode=issue_triage");
      expect(requestedUrl).toContain("repo_slug=user123%2Ftandem");
      expect(result.runs[0]?.coder_run_id).toBe("coder-1");
      expect(result.count).toBe(1);
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  it("posts approve run requests to the coder endpoint", async () => {
    const client = new TandemClient({
      baseUrl: "http://localhost:39731",
      token: "test-token",
    });

    const originalFetch = globalThis.fetch;
    let requestInit: RequestInit | undefined;
    globalThis.fetch = (async (_input, init) => {
      requestInit = init;
      return new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }) as typeof fetch;

    try {
      const result = await client.coder.approveRun("coder-2", "ship it");
      expect(requestInit?.method).toBe("POST");
      expect(String(requestInit?.body)).toContain("ship it");
      expect(result.ok).toBe(true);
    } finally {
      globalThis.fetch = originalFetch;
    }
  });
});

describe("High-value parity coverage", () => {
  it("exposes new top-level namespaces", () => {
    const client = new TandemClient({
      baseUrl: "http://localhost:39731",
      token: "test-token",
    });

    expect(client.browser).toBeTruthy();
    expect(client.workflows).toBeTruthy();
    expect(client.bugMonitor).toBeTruthy();
  });

  it("hits browser status and install endpoints", async () => {
    const client = new TandemClient({ baseUrl: "http://localhost:39731", token: "test-token" });
    const originalFetch = globalThis.fetch;
    const urls: string[] = [];
    globalThis.fetch = (async (input) => {
      urls.push(String(input));
      return new Response(JSON.stringify({ ok: true, runnable: true }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }) as typeof fetch;

    try {
      await client.browser.status();
      await client.browser.install();
      await client.browser.smokeTest({ url: "https://example.com" });
      expect(urls[0]).toContain("/browser/status");
      expect(urls[1]).toContain("/browser/install");
      expect(urls[2]).toContain("/browser/smoke-test");
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  it("hits workflow list, run, and patch hook endpoints", async () => {
    const client = new TandemClient({ baseUrl: "http://localhost:39731", token: "test-token" });
    const originalFetch = globalThis.fetch;
    const calls: Array<{ url: string; method: string }> = [];
    globalThis.fetch = (async (input, init) => {
      calls.push({ url: String(input), method: String(init?.method ?? "GET") });
      return new Response(JSON.stringify({ workflows: [], count: 0, run: {}, hook: {} }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }) as typeof fetch;

    try {
      await client.workflows.list();
      await client.workflows.run("wf-1");
      await client.workflows.patchHook("hook-1", { enabled: false });
      expect(calls[0]?.url).toContain("/workflows");
      expect(calls[1]?.url).toContain("/workflows/wf-1/run");
      expect(calls[1]?.method).toBe("POST");
      expect(calls[2]?.url).toContain("/workflow-hooks/hook-1");
      expect(calls[2]?.method).toBe("PATCH");
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  it("uses canonical bug monitor routes", async () => {
    const client = new TandemClient({ baseUrl: "http://localhost:39731", token: "test-token" });
    const originalFetch = globalThis.fetch;
    const urls: string[] = [];
    globalThis.fetch = (async (input) => {
      urls.push(String(input));
      return new Response(JSON.stringify({ status: {}, drafts: [], count: 0, ok: true }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }) as typeof fetch;

    try {
      await client.bugMonitor.getStatus();
      await client.bugMonitor.listDrafts({ limit: 3 });
      await client.bugMonitor.approveDraft("draft-1", "yes");
      expect(urls[0]).toContain("/bug-monitor/status");
      expect(urls[1]).toContain("/bug-monitor/drafts?limit=3");
      expect(urls[2]).toContain("/bug-monitor/drafts/draft-1/approve");
      expect(urls.every((url) => !url.includes("/failure-reporter/"))).toBe(true);
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  it("returns raw TOML from the MCP catalog endpoint", async () => {
    const client = new TandemClient({ baseUrl: "http://localhost:39731", token: "test-token" });
    const originalFetch = globalThis.fetch;
    globalThis.fetch = (async () =>
      new Response("name = 'demo'\n", {
        status: 200,
        headers: { "Content-Type": "text/plain" },
      })) as typeof fetch;

    try {
      const toml = await client.mcp.catalogToml("demo");
      expect(toml).toContain("name = 'demo'");
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  it("encodes resource keys for get and patch", async () => {
    const client = new TandemClient({ baseUrl: "http://localhost:39731", token: "test-token" });
    const originalFetch = globalThis.fetch;
    const urls: string[] = [];
    globalThis.fetch = (async (input) => {
      urls.push(String(input));
      return new Response(JSON.stringify({ key: "a/b", ok: true, rev: 1, value: {} }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }) as typeof fetch;

    try {
      await client.resources.get("a/b");
      await client.resources.patchKey("a/b", { value: { ok: true } });
      expect(urls[0]).toContain("/resource/a%2Fb");
      expect(urls[1]).toContain("/resource/a%2Fb");
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  it("posts routine and automation artifacts to artifact endpoints", async () => {
    const client = new TandemClient({ baseUrl: "http://localhost:39731", token: "test-token" });
    const originalFetch = globalThis.fetch;
    const calls: string[] = [];
    globalThis.fetch = (async (input) => {
      calls.push(String(input));
      return new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }) as typeof fetch;

    try {
      await client.routines.addArtifact("run-r", { uri: "file://a", kind: "report" });
      await client.automations.addArtifact("run-a", { uri: "file://b", kind: "report" });
      expect(calls[0]).toContain("/routines/runs/run-r/artifacts");
      expect(calls[1]).toContain("/automations/runs/run-a/artifacts");
    } finally {
      globalThis.fetch = originalFetch;
    }
  });
});
