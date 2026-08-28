// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { stat } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { executable, sidecar } from "./packaged-driver.mjs";

if (process.platform !== "linux") {
  throw new Error("The durable FIFO stress matrix requires a Linux runner");
}
await Promise.all([stat(executable), stat(sidecar)]);

const scripts = dirname(fileURLToPath(import.meta.url));
const rows = [
  {
    driver: "packaged-unified-persistence-mixed-linux.mjs",
    boundaries: [
      "enqueue",
      "active-replica-write-fsync-rename-parent-fsync",
      "cross-root-round-robin",
      "history-append",
      "owner-election",
      "ack",
    ],
  },
  {
    driver: "packaged-compaction-dual-recovery-linux.mjs",
    boundaries: [
      "product-publication",
      "history-append",
      "ack-compaction",
      "cancel-before-lock",
      "cancel-after-lock",
      "cancel-after-rollback",
      "consecutive-owner-death",
    ],
  },
];

for (const row of rows) {
  const result = spawnSync(process.execPath, [join(scripts, row.driver)], {
    cwd: join(scripts, ".."),
    env: process.env,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.stdout) process.stdout.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
  assert.equal(
    result.signal,
    null,
    `${row.driver} terminated by ${result.signal}`,
  );
  assert.equal(
    result.status,
    0,
    `${row.driver} failed with ${result.status}`,
  );
}

console.log(JSON.stringify({
  result: "passed",
  driver: "packaged-durable-fifo-stress-linux",
  package: executable,
  platform: process.platform,
  electronConcurrency: "real packaged multi-process",
  matrix: rows,
  platformsNotRun: ["windows", "macos"],
}, null, 2));
