import { readFileSync } from "node:fs";
import { describe, it, expect } from "vitest";
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
                properties: { "custom": "data" },
            };

            // Populate wire-specific required ID fields
            if (def.required.includes("sessionId")) {
                mockWirePayload.sessionID = "s_123";  // Wire name
            }
            if (def.required.includes("runId")) {
                mockWirePayload.run_id = "r_456";    // Wire name
            }

            // Parse through boundary normalized schema
            const result = EngineEventSchema.safeParse(mockWirePayload);
            expect(result.success).toBe(true);

            if (!result.success) return; // For TS narrow
            const event = result.data;

            // Assert canonical structure guarantees
            expect(event.type).toBe(def.type);
            expect(event.properties).toEqual({ "custom": "data" });

            if (def.required.includes("sessionId")) {
                expect(event.sessionId).toBe("s_123");
            }
            if (def.required.includes("runId")) {
                expect(event.runId).toBe("r_456");
            }
        });
    });
});
