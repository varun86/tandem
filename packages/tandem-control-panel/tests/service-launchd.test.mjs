import assert from "node:assert/strict";
import test from "node:test";

import { buildLaunchdPlists } from "../lib/setup/services/launchd.js";

test("buildLaunchdPlists emits LaunchDaemon labels and user binding", () => {
  const plists = buildLaunchdPlists({
    nodePath: "/usr/local/bin/node",
    envFile: "/Users/test/Library/Application Support/tandem/control-panel.env",
    logsDir: "/Users/test/Library/Application Support/tandem/logs",
    homeDir: "/Users/test",
    serviceUser: "test",
  });
  assert.match(plists.enginePlist, /<string>ai\.frumu\.tandem\.engine<\/string>/);
  assert.match(plists.panelPlist, /<key>UserName<\/key><string>test<\/string>/);
  assert.match(plists.panelPlist, /service-runner\.js<\/string>/);
});
