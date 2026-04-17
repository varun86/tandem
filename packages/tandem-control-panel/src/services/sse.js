const channels = new Map();

const _metrics = {
  channels_open: 0,
  channels_total: 0,
  events_received: 0,
  errors_total: 0,
};

const INITIAL_RECONNECT_DELAY_MS = 1000;
const MAX_RECONNECT_DELAY_MS = 15000;

function buildKey(url, withCredentials) {
  return `${withCredentials ? "cred" : "anon"}:${url}`;
}

function recomputeMetrics() {
  let open = 0;
  for (const ch of channels.values()) {
    if (!ch.closed) open += 1;
  }
  _metrics.channels_open = open;
  _metrics.channels_total = channels.size;
}

function clearReconnectTimer(channel) {
  if (channel.reconnectTimer !== null) {
    clearTimeout(channel.reconnectTimer);
    channel.reconnectTimer = null;
  }
}

function notifyMessage(channel, data) {
  _metrics.events_received += 1;
  const event = { data };
  for (const listener of [...channel.listeners]) {
    try {
      listener(event);
    } catch {
      // listener failures are isolated
    }
  }
}

function notifyError(channel, error) {
  _metrics.errors_total += 1;
  for (const listener of [...channel.errorListeners]) {
    try {
      listener(error);
    } catch {
      // ignore callback failures
    }
  }
}

function consumeSsePayload(channel, text) {
  const trimmed = String(text || "").trim();
  if (!trimmed || trimmed === ": keep-alive" || trimmed.startsWith(":")) return;
  notifyMessage(channel, trimmed);
}

async function pumpSseBody(channel, body, signal) {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let currentData = "";

  try {
    while (true) {
      if (signal.aborted || channel.closed || channel.refs === 0) break;
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split("\n");
      buffer = lines.pop() ?? "";

      for (const rawLine of lines) {
        const line = rawLine.replace(/\r$/, "");
        if (line.startsWith("data:")) {
          currentData += line.slice(5).trimStart();
          continue;
        }
        if (line === "") {
          if (currentData) {
            consumeSsePayload(channel, currentData);
            currentData = "";
          }
        }
      }
    }

    if (currentData) {
      consumeSsePayload(channel, currentData);
    }
  } finally {
    reader.releaseLock();
  }
}

async function connectChannel(channel, url, withCredentials, key) {
  if (channel.closed || channel.refs === 0 || channel.running) return;
  channel.running = true;
  clearReconnectTimer(channel);

  const controller = new AbortController();
  channel.controller = controller;
  let shouldBackoff = false;

  try {
    const response = await fetch(url, {
      credentials: withCredentials ? "include" : "omit",
      headers: {
        Accept: "text/event-stream",
        "Cache-Control": "no-cache",
      },
      signal: controller.signal,
    });

    if (channel.closed || channel.refs === 0 || controller.signal.aborted) return;

    if (!response.ok) {
      const body = await response.text().catch(() => "");
      notifyError(
        channel,
        new Error(`SSE connect failed (${response.status} ${response.statusText}): ${body}`)
      );
      shouldBackoff = true;
      return;
    }

    const contentType = String(response.headers.get("content-type") || "").toLowerCase();
    if (!contentType.includes("text/event-stream") || !response.body) {
      shouldBackoff = true;
      return;
    }

    channel.reconnectDelayMs = INITIAL_RECONNECT_DELAY_MS;
    await pumpSseBody(channel, response.body, controller.signal);
  } catch (error) {
    if (!channel.closed && channel.refs > 0 && !controller.signal.aborted) {
      notifyError(channel, error);
      shouldBackoff = true;
    }
  } finally {
    if (channel.controller === controller) {
      channel.controller = null;
    }
    channel.running = false;

    if (!channel.closed && channel.refs > 0) {
      const delayMs = shouldBackoff
        ? channel.reconnectDelayMs
        : INITIAL_RECONNECT_DELAY_MS;
      if (shouldBackoff) {
        channel.reconnectDelayMs = Math.min(
          Math.max(delayMs, INITIAL_RECONNECT_DELAY_MS) * 2,
          MAX_RECONNECT_DELAY_MS
        );
      } else {
        channel.reconnectDelayMs = INITIAL_RECONNECT_DELAY_MS;
      }

      channel.reconnectTimer = setTimeout(() => {
        channel.reconnectTimer = null;
        void connectChannel(channel, url, withCredentials, key);
      }, delayMs);
    } else {
      channels.delete(key);
      recomputeMetrics();
    }
  }
}

function ensureChannel(url, withCredentials = true) {
  const key = buildKey(url, withCredentials);
  const existing = channels.get(key);
  if (existing && !existing.closed) return existing;

  const channel = {
    controller: null,
    errorListeners: new Set(),
    listeners: new Set(),
    reconnectDelayMs: INITIAL_RECONNECT_DELAY_MS,
    reconnectTimer: null,
    refs: 0,
    closed: false,
    running: false,
  };

  channels.set(key, channel);
  recomputeMetrics();
  return channel;
}

export function subscribeSse(url, onMessage, options = {}) {
  const withCredentials = options.withCredentials !== false;
  const key = buildKey(url, withCredentials);
  const channel = ensureChannel(url, withCredentials);
  channel.refs += 1;
  channel.listeners.add(onMessage);
  if (typeof options.onError === "function") channel.errorListeners.add(options.onError);
  void connectChannel(channel, url, withCredentials, key);

  return () => {
    const current = channels.get(key);
    if (!current) return;
    current.listeners.delete(onMessage);
    if (typeof options.onError === "function") current.errorListeners.delete(options.onError);
    current.refs = Math.max(0, current.refs - 1);
    if (current.refs === 0) {
      current.closed = true;
      current.controller?.abort();
      clearReconnectTimer(current);
      channels.delete(key);
      recomputeMetrics();
    }
  };
}

export function closeAllSseChannels() {
  for (const channel of channels.values()) {
    channel.closed = true;
    channel.refs = 0;
    channel.controller?.abort();
    clearReconnectTimer(channel);
    channel.listeners.clear();
    channel.errorListeners.clear();
  }
  channels.clear();
  recomputeMetrics();
}

export function getSseMetrics() {
  return {
    ..._metrics,
    channels: {
      open: _metrics.channels_open,
      total: _metrics.channels_total,
    },
  };
}
