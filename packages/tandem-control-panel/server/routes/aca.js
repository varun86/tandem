function copyRequestHeaders(req) {
  const headers = new Headers();
  for (const [key, value] of Object.entries(req.headers || {})) {
    if (!value) continue;
    const lower = key.toLowerCase();
    if (["host", "content-length", "cookie", "authorization"].includes(lower)) {
      continue;
    }
    if (Array.isArray(value)) headers.set(key, value.join(", "));
    else headers.set(key, value);
  }
  return headers;
}

export function createAcaApiHandler(deps) {
  const { PORTAL_PORT, ACA_BASE_URL, getAcaToken, sendJson } = deps;

  return async function handleAcaApi(req, res) {
    const baseUrl = String(ACA_BASE_URL || "").trim().replace(/\/+$/, "");
    if (!baseUrl) {
      sendJson(res, 503, {
        ok: false,
        error: "ACA integration is not configured. Set ACA_BASE_URL to enable ACA-backed coding.",
      });
      return true;
    }

    const incoming = new URL(req.url, `http://127.0.0.1:${PORTAL_PORT}`);
    const targetPath = incoming.pathname.replace(/^\/api\/aca/, "") || "/";
    const targetUrl = `${baseUrl}${targetPath}${incoming.search}`;
    const token = String(getAcaToken?.() || "aca-proxy").trim();
    const needsAuth = targetPath !== "/health";

    const headers = copyRequestHeaders(req);
    if (needsAuth && token) headers.set("authorization", `Bearer ${token}`);
    if (!headers.has("accept")) headers.set("accept", "*/*");

    const hasBody = !["GET", "HEAD"].includes(req.method || "GET");

    let upstream;
    try {
      upstream = await fetch(targetUrl, {
        method: req.method,
        headers,
        body: hasBody ? req : undefined,
        duplex: hasBody ? "half" : undefined,
      });
    } catch (error) {
      sendJson(res, 502, {
        ok: false,
        error: `ACA unreachable: ${error instanceof Error ? error.message : String(error)}`,
      });
      return true;
    }

    const responseHeaders = {};
    upstream.headers.forEach((value, key) => {
      const lower = key.toLowerCase();
      if (["content-encoding", "transfer-encoding", "connection"].includes(lower)) return;
      responseHeaders[key] = value;
    });

    try {
      res.writeHead(upstream.status, responseHeaders);
      if (!upstream.body) {
        res.end();
        return true;
      }
      for await (const chunk of upstream.body) {
        if (res.writableEnded || res.destroyed) break;
        res.write(chunk);
      }
      if (!res.writableEnded && !res.destroyed) {
        res.end();
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (res.headersSent) {
        const lower = message.toLowerCase();
        if (lower.includes("terminated") || lower.includes("aborted")) {
          if (!res.writableEnded && !res.destroyed) res.end();
          return true;
        }
        if (!res.destroyed && !res.writableEnded) {
          res.destroy(error instanceof Error ? error : undefined);
        }
        return true;
      }
      sendJson(res, 502, {
        ok: false,
        error: `ACA proxy stream failed: ${message}`,
      });
    }

    return true;
  };
}
