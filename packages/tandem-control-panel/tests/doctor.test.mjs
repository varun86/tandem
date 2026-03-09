import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { ensureBootstrapEnv } from "../lib/setup/env.js";
import { runDoctor } from "../lib/setup/doctor.js";

test("doctor reports configured panel host and env file", async () => {
  const root = await mkdtemp(join(tmpdir(), "tcp-doctor-"));
  const envFile = join(root, "control-panel.env");
  await ensureBootstrapEnv({
    cwd: root,
    envPath: envFile,
    env: { HOME: root, XDG_CONFIG_HOME: root, XDG_DATA_HOME: root },
  });
  const result = await runDoctor({
    envFile,
    env: { HOME: root, XDG_CONFIG_HOME: root, XDG_DATA_HOME: root },
    cwd: root,
  });
  assert.equal(result.envFile, envFile);
  assert.equal(result.panelHost, "127.0.0.1");
});
