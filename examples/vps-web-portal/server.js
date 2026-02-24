import express from 'express';
import { createProxyMiddleware, fixRequestBody } from 'http-proxy-middleware';
import cors from 'cors';
import dotenv from 'dotenv';
import path from 'path';
import { fileURLToPath } from 'url';
import fs from 'node:fs/promises';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

dotenv.config();

const execFileAsync = promisify(execFile);

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const app = express();
const TANDEM_ENGINE_URL = process.env.VITE_TANDEM_ENGINE_URL || 'http://127.0.0.1:39731';
const SERVER_KEY = process.env.VITE_PORTAL_KEY || 'default-secret-key';
const PORT = process.env.PORT || 3000;
const DEBUG_PROXY_AUTH = process.env.DEBUG_PROXY_AUTH === '1';

const SYSTEM_CONTROL_MODE = process.env.TANDEM_SYSTEM_CONTROL_MODE || 'systemd';
const ENGINE_SERVICE_NAME = process.env.TANDEM_ENGINE_SERVICE_NAME || 'tandem-engine.service';
const ENGINE_CONTROL_SCRIPT = process.env.TANDEM_ENGINE_CONTROL_SCRIPT || '/usr/local/bin/tandem-engine-ctl';
const ARTIFACT_ROOTS = (process.env.TANDEM_ARTIFACT_READ_ROOTS || '/srv/tandem')
  .split(',')
  .map((v) => v.trim())
  .filter(Boolean);
const PORTAL_MAX_ARTIFACT_BYTES = Number.parseInt(
  process.env.TANDEM_PORTAL_MAX_ARTIFACT_BYTES || '1048576',
  10
);
const STRESS_MAX_CONCURRENCY = Number.parseInt(
  process.env.TANDEM_STRESS_MAX_CONCURRENCY || '64',
  10
);

app.use(cors());
app.use(express.json({ limit: '1mb' }));

const extractBearerOrQueryToken = (req) => {
  const headerAuth = req.headers['authorization'];
  if (typeof headerAuth === 'string' && headerAuth.startsWith('Bearer ')) {
    return headerAuth.slice('Bearer '.length);
  }

  if (typeof req.query?.token === 'string' && req.query.token.length > 0) {
    return req.query.token;
  }

  const candidates = [req.originalUrl, req.url];
  for (const raw of candidates) {
    if (!raw || typeof raw !== 'string') continue;
    try {
      const parsed = new URL(raw, 'http://localhost');
      const token = parsed.searchParams.get('token');
      if (token) return token;
    } catch {
      // Ignore malformed values and continue.
    }
  }

  return null;
};

const tokenSource = (req) => {
  const headerAuth = req.headers['authorization'];
  if (typeof headerAuth === 'string' && headerAuth.startsWith('Bearer ')) {
    return 'authorization-header';
  }
  if (typeof req.query?.token === 'string' && req.query.token.length > 0) {
    return 'query-param';
  }
  return 'url-search';
};

const requireAuth = (req, res, next) => {
  const token = extractBearerOrQueryToken(req);

  if (token === SERVER_KEY) {
    req.portalAuthToken = token;
    if (DEBUG_PROXY_AUTH) {
      console.log(
        `[proxy-auth] allow ${req.method} ${req.originalUrl || req.url} source=${tokenSource(req)} token_len=${token.length}`
      );
    }
    next();
    return;
  }

  if (DEBUG_PROXY_AUTH) {
    console.log(
      `[proxy-auth] deny ${req.method} ${req.originalUrl || req.url} source=${tokenSource(req)} token_present=${!!token}`
    );
  }
  res.status(401).json({ error: 'Unauthorized: Invalid SERVER_KEY' });
};

app.options(/^\/engine(\/|$)/, cors());

const handleProxyReq = (proxyReq, req) => {
  const token = req.portalAuthToken || extractBearerOrQueryToken(req);
  if (token) {
    proxyReq.setHeader('Authorization', `Bearer ${token}`);
  }
  if (DEBUG_PROXY_AUTH) {
    const hasAuthHeader = !!proxyReq.getHeader('authorization');
    console.log(
      `[proxy-auth] forward ${req.method} ${req.originalUrl || req.url} token_present=${!!token} auth_header_set=${hasAuthHeader}`
    );
  }

  if (req.headers.accept && req.headers.accept === 'text/event-stream') {
    proxyReq.setHeader('Cache-Control', 'no-cache');
  }

  // express.json() consumes request streams; re-write JSON bodies for proxied PUT/PATCH/POST.
  fixRequestBody(proxyReq, req);
};

const runControlScript = async (action) => {
  if (SYSTEM_CONTROL_MODE !== 'systemd') {
    throw new Error(`Unsupported TANDEM_SYSTEM_CONTROL_MODE='${SYSTEM_CONTROL_MODE}'`);
  }

  const cmd = '/usr/bin/sudo';
  const args = [ENGINE_CONTROL_SCRIPT, action, ENGINE_SERVICE_NAME];
  const { stdout } = await execFileAsync(cmd, args, { timeout: 15000, maxBuffer: 1024 * 1024 });

  let parsed;
  try {
    parsed = JSON.parse(stdout);
  } catch {
    parsed = { ok: true, action, raw: stdout.trim() };
  }
  return parsed;
};

const getControlCapabilities = async () => {
  try {
    await fs.access(ENGINE_CONTROL_SCRIPT);
    return {
      processControl: {
        enabled: SYSTEM_CONTROL_MODE === 'systemd',
        mode: SYSTEM_CONTROL_MODE,
        serviceName: ENGINE_SERVICE_NAME,
        scriptPath: ENGINE_CONTROL_SCRIPT,
      },
      artifactPreview: {
        enabled: true,
        roots: ARTIFACT_ROOTS,
        maxBytes: PORTAL_MAX_ARTIFACT_BYTES,
      },
    };
  } catch {
    return {
      processControl: {
        enabled: false,
        mode: SYSTEM_CONTROL_MODE,
        serviceName: ENGINE_SERVICE_NAME,
        scriptPath: ENGINE_CONTROL_SCRIPT,
        reason: 'control script missing',
      },
      artifactPreview: {
        enabled: true,
        roots: ARTIFACT_ROOTS,
        maxBytes: PORTAL_MAX_ARTIFACT_BYTES,
      },
    };
  }
};

const ensureArtifactPathAllowed = async (uri) => {
  if (!uri || typeof uri !== 'string' || !uri.startsWith('file://')) {
    throw new Error('Only file:// artifact URIs are supported for preview');
  }

  const filePath = decodeURIComponent(uri.slice('file://'.length));
  const realPath = await fs.realpath(filePath);

  const allowed = ARTIFACT_ROOTS.some((root) => realPath.startsWith(root));
  if (!allowed) {
    throw new Error('Artifact path is outside configured TANDEM_ARTIFACT_READ_ROOTS');
  }

  return { filePath, realPath };
};

const toEnvStyle = (providerId) =>
  String(providerId || '')
    .trim()
    .replace(/[^a-zA-Z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .toUpperCase();

const resolveProviderKeyCandidates = (providerId) => {
  const normalized = toEnvStyle(providerId);
  const aliases = {
    OPENROUTER: ['OPENROUTER_API_KEY'],
    OPENAI: ['OPENAI_API_KEY'],
    ANTHROPIC: ['ANTHROPIC_API_KEY'],
    GOOGLE: ['GOOGLE_API_KEY', 'GEMINI_API_KEY'],
    GEMINI: ['GEMINI_API_KEY', 'GOOGLE_API_KEY'],
    XAI: ['XAI_API_KEY'],
    GROQ: ['GROQ_API_KEY'],
    COHERE: ['COHERE_API_KEY'],
    MISTRAL: ['MISTRAL_API_KEY'],
    TOGETHER: ['TOGETHER_API_KEY'],
    PERPLEXITY: ['PERPLEXITY_API_KEY'],
    DEEPSEEK: ['DEEPSEEK_API_KEY'],
  };

  const mapped = aliases[normalized] || [];
  const fallback = normalized ? [`${normalized}_API_KEY`] : [];
  return Array.from(new Set([...mapped, ...fallback]));
};

const maskKeyPreview = (value) => {
  if (!value) return '';
  if (value.length <= 6) return `${value.slice(0, 2)}...`;
  return `${value.slice(0, 6)}...`;
};

const expandUserPath = (raw) => {
  const input = String(raw || '').trim();
  if (!input) return process.env.HOME || '/';
  if (input === '~') return process.env.HOME || '/';
  if (input.startsWith('~/')) {
    return path.join(process.env.HOME || '/', input.slice(2));
  }
  return input;
};

const resolveDirectoryPath = async (rawPath) => {
  const expanded = expandUserPath(rawPath);
  const absolute = path.isAbsolute(expanded) ? expanded : path.resolve(expanded);
  const real = await fs.realpath(absolute).catch(() => absolute);
  const stat = await fs.stat(real);
  if (!stat.isDirectory()) {
    throw new Error('Path is not a directory');
  }
  return real;
};

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const percentile = (values, p) => {
  if (!values.length) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const rank = Math.ceil((p / 100) * sorted.length) - 1;
  const index = Math.min(sorted.length - 1, Math.max(0, rank));
  return sorted[index];
};

const summarizeSamples = (samples) => {
  if (!samples.length) {
    return { avg: 0, p50: 0, p95: 0, p99: 0, min: 0, max: 0 };
  }
  const total = samples.reduce((sum, value) => sum + value, 0);
  return {
    avg: total / samples.length,
    p50: percentile(samples, 50),
    p95: percentile(samples, 95),
    p99: percentile(samples, 99),
    min: Math.min(...samples),
    max: Math.max(...samples),
  };
};

const parsePositiveInt = (raw, fallback, min, max) => {
  const n = Number.parseInt(String(raw ?? ''), 10);
  if (!Number.isFinite(n) || Number.isNaN(n)) return fallback;
  return Math.min(max, Math.max(min, n));
};

const splitCommandLine = (raw) => {
  const input = String(raw || '').trim();
  if (!input) return ['pwd', []];
  const parts = input.split(/\s+/).filter(Boolean);
  if (parts.length === 0) return ['pwd', []];
  return [parts[0], parts.slice(1)];
};

const toSse = (res, event, payload) => {
  res.write(`event: ${event}\n`);
  res.write(`data: ${JSON.stringify(payload)}\n\n`);
};

const PORTAL_STRESS_PERMISSION_RULES = [
  { permission: 'read', pattern: '*', action: 'allow' },
  { permission: 'write', pattern: '*', action: 'allow' },
  { permission: 'edit', pattern: '*', action: 'allow' },
  { permission: 'bash', pattern: '*', action: 'allow' },
  { permission: 'websearch', pattern: '*', action: 'allow' },
  { permission: 'webfetch', pattern: '*', action: 'allow' },
  { permission: 'webfetch_html', pattern: '*', action: 'allow' },
];

const DEFAULT_REMOTE_PROMPT =
  'Fetch https://tandem.frumu.ai/docs/ via webfetch (markdown mode) and summarize the first 20 tokens of the page in one sentence.';
const SHARED_EDIT_MARKDOWN_FIXTURE = Array.from(
  { length: 200 },
  (_, idx) =>
    `- Line ${idx + 1}: Tandem benchmark fixture sentence ${idx + 1} about reliability, latency, and observability.`
).join('\n');

const buildScenarioPrompt = ({ scenario, prompt, filePath, inlineBody }) => {
  let base = DEFAULT_REMOTE_PROMPT;
  if (scenario === 'file') {
    base = `Use the read tool to open ${filePath || '/srv/tandem/docs/overview.md'} and summarize its key sections in one markdown paragraph.`;
  } else if (scenario === 'shared_edit') {
    base =
      'Edit the shared 200-line markdown fixture for clarity. Return improved markdown and a 5-item bullet list of key edits.\n\n' +
      SHARED_EDIT_MARKDOWN_FIXTURE;
  } else if (scenario === 'inline') {
    base = `Summarize the following markdown blob:\n\n${inlineBody || '# Summary\\n- Highlight'}`;
  }
  const suffix = String(prompt || '').trim();
  return suffix ? `${base}\n\n${suffix}` : base;
};

const parseRunIdFromAsyncResponse = (payload) => {
  const direct =
    payload?.id || payload?.runID || payload?.runId || payload?.run_id;
  if (typeof direct === 'string' && direct.trim()) return direct.trim();
  const nested =
    payload?.run?.id ||
    payload?.run?.runID ||
    payload?.run?.runId ||
    payload?.run?.run_id;
  if (typeof nested === 'string' && nested.trim()) return nested.trim();
  return '';
};

const asString = (value) => (typeof value === 'string' && value.trim() ? value.trim() : '');

const pickFirstModelId = (models) => {
  if (!models || typeof models !== 'object') return '';
  const keys = Object.keys(models);
  return keys.length ? keys[0] : '';
};

const resolveEngineModelSpec = async (token) => {
  const [cfg, catalog] = await Promise.all([
    engineRequest({ token, path: '/config/providers', timeoutMs: 15000 }),
    engineRequest({ token, path: '/provider', timeoutMs: 15000 }),
  ]);

  const connected = new Set(Array.isArray(catalog?.connected) ? catalog.connected : []);
  const providerEntries = Array.isArray(catalog?.all) ? catalog.all : [];
  const providerMap = new Map(providerEntries.map((entry) => [entry?.id, entry]));

  const defaultProviderId = asString(cfg?.default) || asString(catalog?.default);
  if (defaultProviderId && connected.has(defaultProviderId)) {
    const defaultModelId = asString(cfg?.providers?.[defaultProviderId]?.default_model);
    if (defaultModelId) {
      return { providerID: defaultProviderId, modelID: defaultModelId };
    }
  }

  for (const providerId of connected) {
    const configured = asString(cfg?.providers?.[providerId]?.default_model);
    const discovered = pickFirstModelId(providerMap.get(providerId)?.models);
    const modelID = configured || discovered;
    if (providerId && modelID) {
      return { providerID: providerId, modelID };
    }
  }

  throw new Error('No connected provider/model configured for server-side stress run');
};

const engineRequest = async ({ token, path, method = 'GET', body, timeoutMs = 25000 }) => {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(`${TANDEM_ENGINE_URL}${path}`, {
      method,
      headers: {
        'Content-Type': 'application/json',
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
      },
      body: body ? JSON.stringify(body) : undefined,
      signal: controller.signal,
    });
    if (!response.ok) {
      const text = await response.text().catch(() => '');
      throw new Error(`Engine ${method} ${path} failed (${response.status}): ${text}`);
    }
    if (response.status === 204) return {};
    return await response.json().catch(() => ({}));
  } catch (error) {
    if (error instanceof DOMException && error.name === 'AbortError') {
      throw new Error(`Engine request timed out after ${timeoutMs}ms: ${path}`);
    }
    throw error;
  } finally {
    clearTimeout(timeout);
  }
};

app.get('/portal/system/capabilities', requireAuth, async (_req, res) => {
  const caps = await getControlCapabilities();
  res.json(caps);
});

app.get('/portal/system/engine/status', requireAuth, async (_req, res) => {
  try {
    const status = await runControlScript('status');
    res.json(status);
  } catch (error) {
    res.status(500).json({ ok: false, error: String(error.message || error) });
  }
});

app.post('/portal/system/engine/:action', requireAuth, async (req, res) => {
  const action = req.params.action;
  if (!['start', 'stop', 'restart'].includes(action)) {
    res.status(400).json({ ok: false, error: 'Unsupported action' });
    return;
  }

  try {
    const result = await runControlScript(action);
    const status = await runControlScript('status');
    res.json({ ok: true, action, result, status });
  } catch (error) {
    res.status(500).json({ ok: false, action, error: String(error.message || error) });
  }
});

app.get('/portal/artifacts/content', requireAuth, async (req, res) => {
  try {
    const uri = typeof req.query.uri === 'string' ? req.query.uri : '';
    const { realPath } = await ensureArtifactPathAllowed(uri);
    const stat = await fs.stat(realPath);

    const full = await fs.readFile(realPath);
    const truncated = full.byteLength > PORTAL_MAX_ARTIFACT_BYTES;
    const body = truncated ? full.subarray(0, PORTAL_MAX_ARTIFACT_BYTES) : full;

    const ext = path.extname(realPath).toLowerCase();
    const kind = ext === '.json' ? 'json' : ext === '.md' ? 'markdown' : 'text';

    res.json({
      ok: true,
      uri,
      path: realPath,
      kind,
      truncated,
      size: stat.size,
      content: body.toString('utf8'),
    });
  } catch (error) {
    res.status(400).json({ ok: false, error: String(error.message || error) });
  }
});

app.get('/portal/provider/key-preview', requireAuth, async (req, res) => {
  const providerId = String(req.query.providerId || '').trim();
  if (!providerId) {
    res.status(400).json({ ok: false, error: 'providerId is required' });
    return;
  }

  const candidates = resolveProviderKeyCandidates(providerId);
  for (const envVar of candidates) {
    const value = process.env[envVar];
    if (typeof value === 'string' && value.trim().length > 0) {
      res.json({
        ok: true,
        present: true,
        envVar,
        preview: maskKeyPreview(value.trim()),
      });
      return;
    }
  }

  res.json({
    ok: true,
    present: false,
    envVar: candidates[0] || null,
    preview: '',
  });
});

app.get('/portal/fs/directories', requireAuth, async (req, res) => {
  try {
    const current = await resolveDirectoryPath(req.query.path);
    const entries = await fs.readdir(current, { withFileTypes: true });
    const directories = entries
      .filter((entry) => entry.isDirectory())
      .map((entry) => ({
        name: entry.name,
        path: path.join(current, entry.name),
      }))
      .sort((a, b) => a.name.localeCompare(b.name));

    const parent = path.dirname(current);
    res.json({
      ok: true,
      current,
      parent: parent !== current ? parent : null,
      directories,
    });
  } catch (error) {
    res.status(400).json({ ok: false, error: String(error.message || error) });
  }
});

app.post('/portal/fs/mkdir', requireAuth, async (req, res) => {
  try {
    const parentPathRaw = typeof req.body?.parentPath === 'string' ? req.body.parentPath : '';
    const explicitPathRaw = typeof req.body?.path === 'string' ? req.body.path : '';
    const nameRaw = typeof req.body?.name === 'string' ? req.body.name.trim() : '';

    let targetPath = '';
    if (explicitPathRaw) {
      targetPath = path.isAbsolute(explicitPathRaw)
        ? explicitPathRaw
        : path.resolve(expandUserPath(explicitPathRaw));
    } else {
      if (!nameRaw) {
        res.status(400).json({ ok: false, error: 'name or path is required' });
        return;
      }
      if (nameRaw.includes('/') || nameRaw.includes('\\') || nameRaw.includes('\0')) {
        res.status(400).json({ ok: false, error: 'name must be a single directory segment' });
        return;
      }
      const parentPath = await resolveDirectoryPath(parentPathRaw);
      targetPath = path.join(parentPath, nameRaw);
    }

    await fs.mkdir(targetPath, { recursive: true });
    const createdPath = await fs.realpath(targetPath).catch(() => targetPath);
    const parentPath = path.dirname(createdPath);
    res.json({
      ok: true,
      path: createdPath,
      parentPath: parentPath !== createdPath ? parentPath : null,
    });
  } catch (error) {
    res.status(400).json({ ok: false, error: String(error.message || error) });
  }
});

const handleStressStream = async (req, res) => {
  const scenario = String(req.query.scenario || 'providerless').trim();
  const scenarioAllowed = new Set(['remote', 'file', 'inline', 'shared_edit', 'providerless']);
  if (!scenarioAllowed.has(scenario)) {
    res.status(400).json({ ok: false, error: 'Unsupported scenario' });
    return;
  }

  const profile = String(req.query.profile || 'soak_mixed').trim();
  const supportedProfiles = new Set([
    'command_only',
    'get_session_only',
    'list_sessions_only',
    'mixed',
    'soak_mixed',
  ]);
  if (!supportedProfiles.has(profile)) {
    res.status(400).json({ ok: false, error: 'Unsupported providerless profile' });
    return;
  }

  const concurrency = parsePositiveInt(req.query.concurrency, 4, 1, STRESS_MAX_CONCURRENCY);
  const durationSeconds = parsePositiveInt(req.query.duration_seconds, 60, 5, 3600);
  const cycleDelayMs = parsePositiveInt(req.query.cycle_delay_ms, 1200, 0, 60000);
  const [commandBin, commandArgs] = splitCommandLine(req.query.command || 'pwd');
  const promptText = buildScenarioPrompt({
    scenario,
    prompt: req.query.prompt,
    filePath: req.query.file_path,
    inlineBody: req.query.inline_body,
  });
  const token = req.portalAuthToken || SERVER_KEY;
  let modelSpec = null;

  try {
    modelSpec = await resolveEngineModelSpec(token);
  } catch (error) {
    res.status(400).json({ ok: false, error: String(error?.message || error) });
    return;
  }

  res.setHeader('Content-Type', 'text/event-stream; charset=utf-8');
  res.setHeader('Cache-Control', 'no-cache, no-transform');
  res.setHeader('Connection', 'keep-alive');
  res.setHeader('X-Accel-Buffering', 'no');
  res.flushHeaders?.();

  let closed = false;
  req.on('close', () => {
    closed = true;
  });
  const heartbeat = setInterval(() => {
    if (closed) return;
    toSse(res, 'ping', { t: Date.now() });
  }, 1000);

  const startedAt = Date.now();
  const stopAt = startedAt + durationSeconds * 1000;
  const latencySamples = [];
  const commandSamples = [];
  const getSamples = [];
  const listSamples = [];
  let completed = 0;
  let errors = 0;
  let lastProgressAt = 0;

  toSse(res, 'open', {
    mode: 'server',
    scenario,
    profile,
    concurrency,
    durationSeconds,
    cycleDelayMs,
    command: `${commandBin}${commandArgs.length ? ` ${commandArgs.join(' ')}` : ''}`,
    model: modelSpec,
  });

  const runProviderlessCycle = async (sessionId) => {
    if (profile === 'command_only') {
      const t = Date.now();
      await engineRequest({
        token,
        path: `/session/${encodeURIComponent(sessionId)}/command`,
        method: 'POST',
        body: { command: commandBin, args: commandArgs },
      });
      const commandMs = Date.now() - t;
      commandSamples.push(commandMs);
      latencySamples.push(commandMs);
      return { latencyMs: commandMs, commandMs };
    }
    if (profile === 'get_session_only') {
      const t = Date.now();
      await engineRequest({ token, path: `/session/${encodeURIComponent(sessionId)}` });
      const getMs = Date.now() - t;
      getSamples.push(getMs);
      latencySamples.push(getMs);
      return { latencyMs: getMs, getMs };
    }
    if (profile === 'list_sessions_only') {
      const t = Date.now();
      await engineRequest({ token, path: '/session?page_size=5' });
      const listMs = Date.now() - t;
      listSamples.push(listMs);
      latencySamples.push(listMs);
      return { latencyMs: listMs, listMs };
    }

    const started = Date.now();
    const commandStart = Date.now();
    const pCommand = engineRequest({
      token,
      path: `/session/${encodeURIComponent(sessionId)}/command`,
      method: 'POST',
      body: { command: commandBin, args: commandArgs },
    }).then(() => Date.now() - commandStart);
    const getStart = Date.now();
    const pGet = engineRequest({ token, path: `/session/${encodeURIComponent(sessionId)}` }).then(
      () => Date.now() - getStart
    );
    const listStart = Date.now();
    const pList = engineRequest({ token, path: '/session?page_size=5' }).then(
      () => Date.now() - listStart
    );

    const [commandMs, getMs, listMs] = await Promise.all([pCommand, pGet, pList]);
    const latencyMs = Date.now() - started;
    commandSamples.push(commandMs);
    getSamples.push(getMs);
    listSamples.push(listMs);
    latencySamples.push(latencyMs);
    return { latencyMs, commandMs, getMs, listMs };
  };

  const runPromptCycle = async (sessionId) => {
    const started = Date.now();
    const startedRun = await engineRequest({
      token,
      path: `/session/${encodeURIComponent(sessionId)}/prompt_async?return=run`,
      method: 'POST',
      body: {
        parts: [{ type: 'text', text: promptText }],
        model: modelSpec,
      },
      timeoutMs: 30000,
    });
    const runId = parseRunIdFromAsyncResponse(startedRun);
    const pollStopAt = Date.now() + 180000;
    while (Date.now() < pollStopAt) {
      const runState = await engineRequest({
        token,
        path: `/session/${encodeURIComponent(sessionId)}/run`,
        timeoutMs: 15000,
      });
      const active = runState?.active || null;
      const activeId =
        active?.runID || active?.runId || active?.run_id || '';
      if (!active) break;
      if (runId && activeId && String(activeId) !== String(runId)) break;
      await sleep(250);
    }
    if (Date.now() >= pollStopAt) {
      throw new Error(`Timed out waiting for run completion (runId=${runId || 'unknown'})`);
    }
    const latencyMs = Date.now() - started;
    latencySamples.push(latencyMs);
    return { latencyMs };
  };

  const runWorker = async (workerId) => {
    let sessionId = '';
    try {
      const created = await engineRequest({
        token,
        path: '/session',
        method: 'POST',
        body: {
          title: `Server stress worker ${workerId}`,
          directory: '.',
          permission: PORTAL_STRESS_PERMISSION_RULES,
        },
      });
      sessionId = created?.id || '';
      if (!sessionId) throw new Error('session creation returned empty id');
    } catch (error) {
      errors += 1;
      toSse(res, 'log', {
        workerId,
        level: 'error',
        message: `worker init failed: ${String(error?.message || error)}`,
      });
      return;
    }

    while (!closed && Date.now() < stopAt) {
      try {
        const sample =
          scenario === 'providerless'
            ? await runProviderlessCycle(sessionId)
            : await runPromptCycle(sessionId);
        completed += 1;
        const now = Date.now();
        if (now - lastProgressAt >= 500 || completed <= 20) {
          lastProgressAt = now;
          toSse(res, 'progress', {
            completed,
            errors,
            lastLatencyMs: sample.latencyMs,
            lastMixedMs: sample.latencyMs,
            lastCommandMs: sample.commandMs || null,
            lastGetMs: sample.getMs || null,
            lastListMs: sample.listMs || null,
          });
        }
      } catch (error) {
        errors += 1;
        toSse(res, 'log', {
          workerId,
          level: 'error',
          message: String(error?.message || error),
        });
      }
      if (closed || Date.now() >= stopAt) break;
      if (cycleDelayMs > 0) {
        await sleep(cycleDelayMs);
      }
    }
  };

  try {
    await Promise.all(Array.from({ length: concurrency }, (_, i) => runWorker(i + 1)));
  } catch (error) {
    toSse(res, 'log', {
      level: 'error',
      message: `stress orchestration error: ${String(error?.message || error)}`,
    });
  }

  const durationSec = Math.max(1, Math.round((Date.now() - startedAt) / 1000));
  const attempts = completed + errors;
  const providerErrorRate = attempts > 0 ? (errors / attempts) * 100 : 0;
  const latency = summarizeSamples(latencySamples);
  const command = summarizeSamples(commandSamples);
  const getSession = summarizeSamples(getSamples);
  const listSessions = summarizeSamples(listSamples);
  const lines = [
    `Soak Results (server-side ${scenario} ${scenario === 'providerless' ? profile : 'prompt'})`,
    `Duration: ${durationSec}s`,
    `Samples: ${latencySamples.length}`,
    `Attempts: ${attempts}`,
    `Completed: ${completed}`,
    `Errors: ${errors}`,
    `Provider error rate: ${providerErrorRate.toFixed(1)}%`,
    `Avg: ${Math.round(latency.avg)}ms`,
    `P50: ${Math.round(latency.p50)}ms`,
    `P95: ${Math.round(latency.p95)}ms`,
    `P99: ${Math.round(latency.p99)}ms`,
    `Min: ${Math.round(latency.min)}ms`,
    `Max: ${Math.round(latency.max)}ms`,
  ];
  if (scenario === 'providerless') {
    lines.push(
      `Command avg/p95: ${Math.round(command.avg)}ms / ${Math.round(command.p95)}ms`,
      `getSession avg/p95: ${Math.round(getSession.avg)}ms / ${Math.round(getSession.p95)}ms`,
      `listSessions avg/p95: ${Math.round(listSessions.avg)}ms / ${Math.round(listSessions.p95)}ms`
    );
  }
  const report = lines.join('\n');

  toSse(res, 'summary', {
    scenario,
    profile,
    durationSeconds: durationSec,
    completed,
    errors,
    attempts,
    providerErrorRate,
    samples: latencySamples.length,
    latency,
    mixed: latency,
    command,
    getSession,
    listSessions,
    report,
  });
  clearInterval(heartbeat);
  res.end();
};

app.get('/portal/stress/stream', requireAuth, handleStressStream);
app.get('/portal/stress/providerless/stream', requireAuth, handleStressStream);

app.use(
  '/engine',
  requireAuth,
  createProxyMiddleware({
    target: TANDEM_ENGINE_URL,
    changeOrigin: true,
    pathRewrite: { '^/engine': '' },
    on: {
      proxyReq: handleProxyReq,
    },
  })
);

app.use(express.static(path.join(__dirname, 'dist')));

app.get('/{*path}', (_req, res) => {
  res.sendFile(path.join(__dirname, 'dist', 'index.html'));
});

const server = app.listen(PORT, () => {
  console.log(`VPS Web Portal proxy running on port ${PORT}`);
  console.log(`Proxying /engine -> ${TANDEM_ENGINE_URL}`);
});

server.on('close', () => {
  console.warn('HTTP server closed');
});

server.on('error', (err) => {
  console.error('HTTP server error:', err);
  process.exit(1);
});

process.on('SIGTERM', () => {
  console.log('Received SIGTERM; shutting down portal');
  server.close(() => process.exit(0));
});
