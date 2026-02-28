import { z } from "zod";
import * as Public from "../public/index.js";

export class TandemValidationError extends Error {
  constructor(
    public readonly endpoint: string,
    public readonly status: number,
    public readonly issues: z.ZodIssue[],
    public readonly rawSnippet: string
  ) {
    super(`Tandem API Validation Error [${status}] at ${endpoint}: ${issues.length} issues found.`);
    this.name = "TandemValidationError";
  }
}

// ─── Shared Utilities ─────────────────────────────────────────────────────────

// Flexible ID extractor
export const idNormalizer = z
  .string()
  .or(
    z.object({
      id: z.string().optional(),
      runID: z.string().optional(),
      runId: z.string().optional(),
      run_id: z.string().optional(),
      sessionID: z.string().optional(),
      sessionId: z.string().optional(),
      session_id: z.string().optional(),
      missionID: z.string().optional(),
      missionId: z.string().optional(),
      mission_id: z.string().optional(),
      instanceID: z.string().optional(),
      instanceId: z.string().optional(),
      instance_id: z.string().optional(),
    })
  )
  .transform((val) => {
    if (typeof val === "string") return val;
    return (
      val.id ||
      val.runID ||
      val.runId ||
      val.run_id ||
      val.sessionID ||
      val.sessionId ||
      val.session_id ||
      val.missionID ||
      val.missionId ||
      val.mission_id ||
      val.instanceID ||
      val.instanceId ||
      val.instance_id
    );
  });

// JSON fallback
export const jsonFallback = z.any().transform((val) => val as Public.JsonValue);
export const jsonObjectFallback = z
  .record(z.string(), z.any())
  .transform((val) => val as Public.JsonObject);

// ─── Health Schema ────────────────────────────────────────────────────────────

export const SystemHealthSchema = z
  .object({
    ready: z.boolean().optional(),
    phase: z.string().optional(),
  })
  .passthrough() satisfies z.ZodType<Public.SystemHealth, any, any>;

// ─── Session Schemas ──────────────────────────────────────────────────────────

export const SessionRecordSchema = z
  .object({
    id: z.string(),
    title: z.string(),
    created_at_ms: z.number().optional(),
    createdAtMs: z.number().optional(),
    directory: z.string().optional(),
    workspace_root: z.string().optional(),
    workspaceRoot: z.string().optional(),
    archived: z.boolean().optional(),
  })
  .passthrough()
  .transform(
    (val): Public.SessionRecord => ({
      ...val,
      createdAtMs: val.created_at_ms ?? val.createdAtMs ?? 0,
      workspaceRoot: val.workspace_root ?? val.workspaceRoot,
    })
  );

export const SessionListResponseSchema = z
  .object({
    sessions: z.array(SessionRecordSchema).optional().default([]),
    count: z.number().optional().default(0),
  })
  .passthrough();

export const SessionRunStateResponseSchema = z
  .object({
    active: z
      .object({
        runID: z.string().optional(),
        runId: z.string().optional(),
        run_id: z.string().optional(),
        attachEventStream: z.string().optional(),
      })
      .passthrough()
      .nullable()
      .optional(),
  })
  .passthrough()
  .transform((val): Public.SessionRunStateResponse => {
    if (!val.active) return { active: null };
    return {
      active: {
        ...val.active,
        runId: val.active.runId || val.active.runID || val.active.run_id,
      },
    };
  });

// ─── Run Schemas ──────────────────────────────────────────────────────────────

export const RunNowResponseSchema = z
  .object({
    ok: z.boolean().optional(),
    runID: z.string().optional(),
    runId: z.string().optional(),
    run_id: z.string().optional(),
    status: z.string().optional(),
  })
  .passthrough()
  .transform(
    (val): Public.RunNowResponse => ({
      ...val,
      runId: val.runId || val.runID || val.run_id,
    })
  );

export const RunRecordSchema = z
  .object({
    id: z.string().optional(),
    runID: z.string().optional(),
    runId: z.string().optional(),
    run_id: z.string().optional(),
    routine_id: z.string().optional(),
    automation_id: z.string().optional(),
    status: z.string().optional(),
    started_at_ms: z.number().optional(),
    finished_at_ms: z.number().optional(),
  })
  .passthrough()
  .transform(
    (val): Public.RunRecord => ({
      ...val,
      runId: val.runId || val.runID || val.run_id,
      routineId: val.routine_id,
      automationId: val.automation_id,
      startedAtMs: val.started_at_ms,
      finishedAtMs: val.finished_at_ms,
    })
  );

// ─── Resource Schemas ─────────────────────────────────────────────────────────

export const ResourceWriteResponseSchema = z
  .object({
    ok: z.boolean().default(true),
    rev: z.number().optional(),
  })
  .passthrough();

export const ResourceRecordSchema = z
  .object({
    key: z.string(),
    value: z.any(),
    rev: z.number().optional(),
    updated_at_ms: z.number().optional(),
    updated_by: z.string().optional(),
  })
  .passthrough()
  .transform(
    (val): Public.ResourceRecord => ({
      ...val,
      updatedAtMs: val.updated_at_ms,
      updatedBy: val.updated_by,
    })
  );

export const ResourceListResponseSchema = z
  .object({
    items: z.array(ResourceRecordSchema).optional().default([]),
    count: z.number().optional().default(0),
  })
  .passthrough();

// ─── Memory Schemas ───────────────────────────────────────────────────────────

export const MemoryItemSchema = z
  .object({
    id: z.string().optional(),
    text: z.string().optional(),
    content: z.string().optional(),
    user_id: z.string().optional(),
    userID: z.string().optional(),
    source_type: z.string().optional(),
    sourceType: z.string().optional(),
    tags: z.array(z.string()).optional(),
    source: z.string().optional(),
    session_id: z.string().optional(),
    sessionID: z.string().optional(),
    run_id: z.string().optional(),
    runID: z.string().optional(),
  })
  .passthrough()
  .transform(
    (val): Public.MemoryItem => ({
      ...val,
      text: val.text || val.content,
      content: val.content || val.text,
      userId: val.userID || val.user_id,
      sourceType: val.sourceType || val.source_type,
      sessionId: val.session_id || val.sessionID,
      runId: val.run_id || val.runID,
    })
  );

export const MemoryListResponseSchema = z
  .object({
    items: z.array(MemoryItemSchema).optional().default([]),
    count: z.number().optional().default(0),
  })
  .passthrough();

export const MemorySearchResultSchema = z
  .object({
    id: z.string(),
    text: z.string().optional(),
    content: z.string().optional(),
    score: z.number().optional(),
    source_type: z.string().optional(),
    sourceType: z.string().optional(),
    run_id: z.string().optional(),
    runID: z.string().optional(),
    tags: z.array(z.string()).optional(),
  })
  .passthrough()
  .transform(
    (val): Public.MemorySearchResult => ({
      ...val,
      text: val.text || val.content,
      content: val.content || val.text,
      sourceType: val.sourceType || val.source_type,
      runId: val.runID || val.run_id,
    })
  );

export const MemorySearchResponseSchema = z
  .object({
    results: z.array(MemorySearchResultSchema).optional().default([]),
    count: z.number().optional().default(0),
  })
  .passthrough();

// ─── SSE Schema ───────────────────────────────────────────────────────────────

export const EngineEventSchema = z
  .object({
    type: z.string(),
    properties: z.record(z.string(), z.any()).optional().default({}),
    sessionID: z.string().optional(),
    session_id: z.string().optional(),
    sessionId: z.string().optional(),
    runID: z.string().optional(),
    run_id: z.string().optional(),
    runId: z.string().optional(),
    timestamp: z.string().optional(),
  })
  .passthrough()
  .transform((val): Public.EngineEvent => {
    return {
      ...val,
      properties: val.properties as Record<string, unknown>,
      sessionId: val.sessionId || val.sessionID || val.session_id,
      runId: val.runId || val.runID || val.run_id,
    } as Public.EngineEvent;
  });

// A universal wrapper to safe-parse and throw TandemValidationError
export function parseResponse<T>(
  schema: z.ZodType<T, any, any>,
  rawData: unknown,
  endpoint: string,
  status: number
): T {
  const result = schema.safeParse(rawData);
  if (!result.success) {
    const snippet = JSON.stringify(rawData).substring(0, 200);
    throw new TandemValidationError(endpoint, status, result.error.issues, snippet);
  }
  return result.data;
}
