import assert from "node:assert/strict";
import test from "node:test";

import { buildSystemdUnits } from "../lib/setup/services/systemd.js";

test("buildSystemdUnits points both services at the shared runner", () => {
  const units = buildSystemdUnits({
    nodePath: "/usr/bin/node",
    envFile: "/home/test/.config/tandem/control-panel.env",
    homeDir: "/home/test",
    serviceUser: "test",
  });
  assert.match(units.engineUnit, /service-runner\.js engine --env-file/);
  assert.match(units.panelUnit, /service-runner\.js panel --env-file/);
  assert.match(units.panelUnit, /After=network-online.target tandem-engine\.service/);
});
