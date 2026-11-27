import { test } from "node:test";
import assert from "node:assert/strict";
import { compareVersions, permissionChips } from "../src/lib/versions.js";

test("versions compare like numbers", () => {
  assert.equal(compareVersions("1.10.0", "1.9.2"), 1);
  assert.equal(compareVersions("1.0.0", "1.0.0"), 0);
  assert.equal(compareVersions("1.0", "1.0.0"), 0);
  assert.equal(compareVersions("1.0.0-beta", "1.0.0"), -1);
  assert.equal(compareVersions("2.0.0", "10.0.0"), -1);
});

test("permission chips", () => {
  assert.deepEqual(permissionChips({ system: ["cpu"], network: ["api.x.com", "setting:url"], storage: true }), [
    "reads cpu", "reaches api.x.com", "web address you type", "saves notes",
  ]);
});
