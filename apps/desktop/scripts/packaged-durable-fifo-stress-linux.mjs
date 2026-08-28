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
const parseDriverReport = (stdout, driver) => {
  const starts = [0];
  for (let index = stdout.indexOf("\n{"); index >= 0; index = stdout.indexOf("\n{", index + 2)) {
    starts.push(index + 1);
  }
  for (const start of starts.toReversed()) {
    try {
      return JSON.parse(stdout.slice(start).trim());
    } catch {
      // Packaged Electron may emit setup diagnostics before the final report.
    }
  }
  throw new Error(`${driver} did not emit a JSON matrix report`);
};

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
    validate(report) {
      assert.equal(report.result, "passed");
      assert.equal(
        report.mixedQueues.projectMoveRebasedMutableMetadata,
        true,
      );
      assert.equal(
        report.mixedQueues.consecutiveOwnerTakeovers.length,
        2,
      );
      assert(report.mixedQueues.crossRootDispatchOrder.length >= 3);
      for (const file of ["active.recovery.json", "active.json"]) {
        for (const point of [
          "after_candidate_fsync",
          "after_rename",
          "after_parent_fsync",
        ]) {
          assert(
            report.activeReplicaBoundaries.some((boundary) =>
              boundary.file === file && boundary.point === point
            ),
            `missing ${file} ${point} boundary`,
          );
        }
      }
      return {
        ownerSigkills: report.mixedQueues.consecutiveOwnerTakeovers.length,
        crossRootDispatchOrder:
          report.mixedQueues.crossRootDispatchOrder,
        projectRootMoved: true,
        activeReplicaRenameAndFsyncBoundaries: 6,
      };
    },
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
    validate(report) {
      assert.equal(report.simultaneousElectronInstances, true);
      assert.deepEqual(
        report.receiptAckMatrix
          .map(({ receiptType }) => receiptType)
          .toSorted(),
        ["close", "refresh", "save", "team"],
      );
      assert(
        report.resolveCancellationResults.every((row) =>
          row.window === "owner-claim-before-renderer-delivery"
          && row.protocolErrorCode === -32800
        ),
        "post-claim cancellation evidence is incomplete",
      );
      const preClaimCancellation = report.fifoTailCancellationResults.find(
        (row) => row.killBoundary === "after_intent_queue_rename",
      );
      assert(preClaimCancellation, "pre-claim cancellation row is missing");
      assert.equal(preClaimCancellation.protocolErrorCode, -32800);
      const threeWaiters = preClaimCancellation.waitingCancellationTakeover;
      assert.equal(threeWaiters.waitingCallerBrowserPids.length, 3);
      assert.equal(threeWaiters.terminalReadOnlyWaiterWasPreExisting, true);
      assert.equal(threeWaiters.terminalReadOnlyWaiterCreatedTakeover, false);
      assert.equal(threeWaiters.durableRollbackCount, 1);
      assert.equal(threeWaiters.terminalCount, 1);
      assert.equal(threeWaiters.resolveEnvelopeCount, 0);
      assert(
        report.resolveReplacementElections.every((row) =>
          row.survivingLoserThirdElection.launchedAdditionalProcesses === 0
          && row.terminalHeadCount === 1
        ),
        "third waiter owner takeover evidence is incomplete",
      );
      assert(
        report.resolveTerminalCompactionResults.some((row) =>
          row.terminalArchiveCount === 1
          && row.compactedQueueLength === 0
          && row.archiveBoundaryKilledPid
          && row.queueRenameBoundaryKilledPid
        ),
        "archive-fsync/queue-rename cancellation compaction is missing",
      );
      return {
        simultaneousWaitingElectrons:
          threeWaiters.waitingCallerBrowserPids.length,
        cancellationBeforeClaim: true,
        cancellationAfterClaim: true,
        ownerSigkill: true,
        lostAckReceiptTypes:
          report.receiptAckMatrix.map(({ receiptType }) => receiptType),
        terminalArchiveFsyncAndQueueRename: true,
        thirdPreExistingWaiterConverged: true,
        thirdOwnerTakeoverWithoutNewProcess: true,
      };
    },
  },
  {
    driver: "packaged-random-durable-faults-linux.mjs",
    boundaries: [
      "long-random-sequence",
      "project-pre-publication",
      "project-post-publication",
      "config-replica-rename-parent-fsync",
      "config-terminal-history",
      "cross-root-owner-release",
      "multi-seed",
      "history-gc",
      "project-move-during-sequence",
    ],
    validate(report) {
      assert.equal(report.result, "passed");
      assert(report.seeds.length >= 3);
      assert.equal(report.runs.length, report.seeds.length);
      assert(report.stepsPerSeed >= 12);
      assert.equal(report.trace.length, report.steps);
      assert.equal(report.allQueuesDrained, true);
      assert.equal(report.allTerminalDecisionsExact, true);
      assert.equal(report.ownerReleaseTombstonesVerified, true);
      assert.equal(report.historyGcVerified, true);
      assert.equal(report.projectMoveCombinedWithFaultSequence, true);
      assert(
        report.runs.every((run) =>
          run.steps === report.stepsPerSeed
          && run.projectMove.mutableScopeRebased
          && run.projectMove.immutableSegmentsVerified > 0
          && run.historyGc.projects.every((history) => history.generation > 0)
          && run.historyGc.config.generation > 0
        ),
        "multi-seed GC/project-move evidence is incomplete",
      );
      assert.equal(
        report.trace.some((row) =>
          row.kind === "project.save" && row.point === "before_atomic_publish"
        ),
        true,
      );
      assert.equal(
        report.trace.some((row) =>
          row.kind === "project.save" && row.point === "after_atomic_publish"
        ),
        true,
      );
      assert.equal(
        report.trace.some((row) =>
          row.kind === "prefs.patch"
          && row.point === "after_terminal_history_publish"
        ),
        true,
      );
      return {
        seeds: report.seeds,
        stepsPerSeed: report.stepsPerSeed,
        totalSteps: report.steps,
        durationMs: report.durationMs,
        roots: report.runs.map((run) => run.roots),
        ownerReleaseTombstonesVerified: true,
        historyGcVerified: true,
        projectMoveCombinedWithFaultSequence: true,
      };
    },
  },
];

const matrix = [];
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
  const report = parseDriverReport(result.stdout, row.driver);
  matrix.push({
    driver: row.driver,
    boundaries: row.boundaries,
    evidence: row.validate(report),
  });
}

console.log(JSON.stringify({
  result: "passed",
  driver: "packaged-durable-fifo-stress-linux",
  package: executable,
  platform: process.platform,
  electronConcurrency: "real packaged multi-process",
  minimumSimultaneousElectronWaiters: 3,
  matrix,
  platformsNotRun: ["windows", "macos"],
}, null, 2));
