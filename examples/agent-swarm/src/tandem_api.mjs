import fs from "node:fs";

export function loadDotEnv(path) {
  if (!fs.existsSync(path)) return;
  const raw = fs.readFileSync(path, "utf8");
  for (const line of raw.split(/\r?\n/)) {
    if (!line || line.trim().startsWith("#")) continue;
    const idx = line.indexOf("=");
    if (idx < 0) continue;
    const key = line.slice(0, idx).trim();
    const value = line.slice(idx + 1).trim();
    if (!(key in process.env)) process.env[key] = value;
  }
}

export function createApi(baseUrl, token = "") {
  const root = baseUrl.replace(/\/$/, "");
  const baseHeaders = { "content-type": "application/json" };
  if (token) {
    baseHeaders.authorization = `Bearer ${token}`;
    baseHeaders["x-tandem-token"] = token;
  }

  async function req(method, path, body) {
    const resp = await fetch(`${root}${path}`, {
      method,
      headers: baseHeaders,
      body: body == null ? undefined : JSON.stringify(body),
    });
    if (!resp.ok) {
      const text = await resp.text();
      throw new Error(`${method} ${path} failed: ${resp.status} ${text}`);
    }
    const text = await resp.text();
    return text ? JSON.parse(text) : {};
  }

  return {
    get: (path) => req("GET", path),
    post: (path, body) => req("POST", path, body),
    put: (path, body) => req("PUT", path, body),
    patch: (path, body) => req("PATCH", path, body),

    async streamEvents(onEvent, { signal } = {}) {
      const resp = await fetch(`${root}/event`, { headers: baseHeaders, signal });
      if (!resp.ok || !resp.body) throw new Error(`GET /event failed: ${resp.status}`);

      const reader = resp.body.getReader();
      const decoder = new TextDecoder();
      let buffer = "";
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        buffer += decoder.decode(value, { stream: true });

        let split;
        while ((split = buffer.indexOf("\n\n")) >= 0) {
          const rawEvent = buffer.slice(0, split);
          buffer = buffer.slice(split + 2);
          for (const line of rawEvent.split("\n")) {
            if (!line.startsWith("data:")) continue;
            const payload = line.slice(5).trim();
            if (!payload) continue;
            try {
              onEvent(JSON.parse(payload));
            } catch {
              // ignore malformed payloads
            }
          }
        }
      }
    },
  };
}
