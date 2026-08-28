// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import {
  access,
  mkdtemp,
  mkdir,
  readFile,
  readdir,
  rename,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  executable,
  killPackaged,
  launchPackaged,
  launchPackagedRenderer,
  pathExists,
  sidecar,
  startPackagedDisplay,
  stopPackagedDisplay,
  terminatePackaged,
  waitFor,
  workspaceState,
} from "./packaged-driver.mjs";

if (process.platform !== "linux") {
  throw new Error("This packaged segmented-history driver requires Linux");
}
await Promise.all([access(executable), access(sidecar)]);

const workDir = await mkdtemp(join(tmpdir(), "omegat-segmented-history-e2e-"));
const config = join(workDir, "config");
const originalProject = join(workDir, "project-before-move");
const movedProject = join(workDir, "project-after-move");
const display = await startPackagedDisplay();
const limits = {
  OMEGAT_TEST_PRODUCT_HISTORY_RECENT_LIMIT: "2",
  OMEGAT_TEST_PRODUCT_HISTORY_HOT_LIMIT: "2",
  OMEGAT_TEST_PRODUCT_HISTORY_SEGMENT_LIMIT: "1",
  OMEGAT_TEST_PRODUCT_HISTORY_COMPACTION_SEGMENTS: "2",
  OMEGAT_TEST_PRODUCT_HISTORY_COMPACTION_RECORDS: "128",
  OMEGAT_TEST_PRODUCT_HISTORY_PREFIX_HEX: "8",
};
let setup;
let firstOwner;
let gcOwner;
let recoveredOwner;
let contender;

const rpc = (client, method, params = {}) =>
  client.evaluate(
    `window.omegat.rpc(${JSON.stringify(method)}, ${JSON.stringify(params)})`,
    true,
  );

const rpcOutcome = (client, method, params = {}) =>
  client.evaluate(`(async () => {
    try {
      return {
        resolved: true,
        value: await window.omegat.rpc(
          ${JSON.stringify(method)},
          ${JSON.stringify(params)}
        )
      };
    } catch (error) {
      return { resolved: false, error: String(error?.message ?? error) };
    }
  })()`, true);

const historyDirectory = (project) =>
  join(project, ".repositories", "transactions");
const historyPath = (project) =>
  join(historyDirectory(project), "history.ndjson");
const manifestPath = (project) =>
  join(historyDirectory(project), "history-manifest.json");
const manifestRecoveryPath = (project) =>
  join(historyDirectory(project), ".history-manifest.recovery.json");
const archiveDirectory = (project) =>
  join(historyDirectory(project), "history-archive");

async function waitForReceiptDrain(project) {
  const active = join(historyDirectory(project), "active.json");
  await waitFor("segmented product receipt drain", async () =>
    !await pathExists(active) ? true : undefined
  );
}

async function saveAndAcknowledge(
  launched,
  project,
  generation,
  batchId,
) {
  const saved = await rpc(launched.client, "project.save", {
    transaction_project_root: project,
    transaction_generation: generation,
    transaction_batch_id: batchId,
  });
  assert.equal(saved.receipt.batch_id, batchId);
  assert.equal(saved.receipt.status, "sidecar_committed");
  await waitForReceiptDrain(project);
  return saved.receipt;
}

try {
  setup = await launchPackagedRenderer(
    display.display,
    config,
    null,
    limits,
  );
  await rpc(setup.client, "project.create", {
    root: originalProject,
    source_lang: "en",
    target_lang: "fr",
    sentence_seg: false,
  });
  await mkdir(join(originalProject, "source"), { recursive: true });
  await writeFile(
    join(originalProject, "source", "source.txt"),
    "segmented history packaged source",
    "utf8",
  );
  await rpc(setup.client, "project.reload", {});

  for (let index = 0; index < 5; index += 1) {
    await saveAndAcknowledge(
      setup,
      originalProject,
      300,
      `history-seed-${index}`,
    );
  }
  await terminatePackaged(setup);
  setup = undefined;

  const recentBefore = (await readFile(historyPath(originalProject), "utf8"))
    .trim()
    .split("\n")
    .filter(Boolean);
  assert(recentBefore.length <= 2, "recent NDJSON window is not bounded");
  assert.deepEqual(
    JSON.parse(await readFile(manifestPath(originalProject), "utf8")),
    JSON.parse(await readFile(manifestRecoveryPath(originalProject), "utf8")),
  );

  // The first replacement dies after the complete next generation has become
  // authoritative but before any predecessor is unlinked.
  const generationMarker = join(workDir, "generation-owner.marker");
  firstOwner = await launchPackaged(
    display.display,
    config,
    originalProject,
    {
      ...limits,
      OMEGAT_TEST_PRODUCT_HISTORY_POINT:
        "after_generation_manifest_publish",
      OMEGAT_TEST_PRODUCT_HISTORY_MARKER: generationMarker,
    },
  );
  const lostReceiptBatchId = "history-lost-receipt";
  const interruptedSave = rpc(firstOwner.client, "project.save", {
    transaction_project_root: originalProject,
    transaction_generation: 300,
    transaction_batch_id: lostReceiptBatchId,
  });
  void interruptedSave.catch(() => undefined);
  await waitFor("generation manifest owner checkpoint", () =>
    pathExists(generationMarker)
  );
  const firstKilled = await killPackaged(firstOwner);
  firstOwner = undefined;

  // Recovery selects the already-dual-published generation. The next Electron
  // dies after deleting exactly one predecessor/orphan, proving GC itself is
  // restartable rather than merely ordered after rename.
  const gcMarker = join(workDir, "gc-owner.marker");
  gcOwner = await launchPackagedRenderer(
    display.display,
    config,
    originalProject,
    {
      ...limits,
      OMEGAT_TEST_PRODUCT_HISTORY_POINT: "after_gc_delete",
      OMEGAT_TEST_PRODUCT_HISTORY_MARKER: gcMarker,
    },
  );
  await waitFor("history GC owner checkpoint", () => pathExists(gcMarker));
  const gcKilled = await killPackaged(gcOwner);
  gcOwner = undefined;

  recoveredOwner = await launchPackaged(
    display.display,
    config,
    originalProject,
    limits,
  );
  await waitForReceiptDrain(originalProject);
  const recoveredState = await workspaceState(recoveredOwner.client);
  assert.equal(recoveredState.project, originalProject);
  assert.equal(recoveredState.source, "segmented history packaged source");

  const terminalRows = (await readFile(historyPath(originalProject), "utf8"))
    .trim()
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line));
  const lostTerminal = terminalRows.find((row) =>
    row.batch_id === lostReceiptBatchId
      && row.status === "completed"
      && row.payload.phase === "renderer-acknowledged"
  );
  assert(lostTerminal, "replacement did not durably acknowledge lost receipt");

  // Repeating the acknowledgement models a response lost after terminal
  // publication. It must resolve from sparse history without product replay.
  const duplicate = await rpc(
    recoveredOwner.client,
    "transaction.receipt.ack",
    {
      root: originalProject,
      app_instance: JSON.parse(
        await readFile(
          join(historyDirectory(originalProject), "renderer-owner.json"),
          "utf8",
        ),
      ).app_instance,
      owner_process_id: recoveredOwner.application.pid,
      generation: lostTerminal.generation,
      batch_id: lostReceiptBatchId,
      operation: "project.save",
      outcome: "succeeded",
    },
  );
  assert.equal(duplicate.ack.already_acknowledged, true);

  // A second packaged Electron cannot adopt or acknowledge the selected FIFO
  // while the recovered renderer owner remains live.
  contender = await launchPackagedRenderer(
    display.display,
    config,
    null,
    limits,
  );
  const rejected = await rpcOutcome(
    contender.client,
    "transaction.receipt.pending",
    {
      root: originalProject,
      app_instance: "history-live-contender",
      owner_process_id: contender.application.pid,
      generation: lostTerminal.generation + 1,
    },
  );
  assert.equal(rejected.resolved, false);
  assert.match(rejected.error, /owned by live app|locked by another process/);
  await terminatePackaged(contender);
  contender = undefined;
  await terminatePackaged(recoveredOwner);
  recoveredOwner = undefined;

  // Move the whole project, including hot replicas, immutable generations,
  // owner claim, and a fresh unacknowledged receipt. Mutable paths are rebased
  // while immutable segment bytes remain untouched.
  setup = await launchPackaged(
    display.display,
    config,
    originalProject,
    limits,
  );
  const moveReceipt = await rpc(setup.client, "project.save", {
    transaction_project_root: originalProject,
    transaction_generation: 401,
    transaction_batch_id: "history-move-receipt",
  });
  assert.equal(moveReceipt.receipt.status, "sidecar_committed");
  const immutableBefore = new Map();
  for (const file of await readdir(archiveDirectory(originalProject))) {
    immutableBefore.set(
      file,
      (await readFile(join(archiveDirectory(originalProject), file))).toString(
        "base64",
      ),
    );
  }
  await terminatePackaged(setup);
  setup = undefined;
  await rename(originalProject, movedProject);

  recoveredOwner = await launchPackaged(
    display.display,
    config,
    movedProject,
    limits,
  );
  await waitForReceiptDrain(movedProject);
  const movedState = await workspaceState(recoveredOwner.client);
  assert.equal(movedState.project, movedProject);
  assert.equal(movedState.source, "segmented history packaged source");
  const movedRecent = (await readFile(historyPath(movedProject), "utf8"))
    .trim()
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line));
  assert(
    movedRecent.every((row) => row.project_root === movedProject),
    "bounded recent rows retained the old project path",
  );
  for (const [file, bytes] of immutableBefore) {
    if (await pathExists(join(archiveDirectory(movedProject), file))) {
      assert.equal(
        (await readFile(join(archiveDirectory(movedProject), file))).toString(
          "base64",
        ),
        bytes,
        "project relocation rewrote an immutable history segment",
      );
    }
  }

  const manifest = JSON.parse(
    await readFile(manifestPath(movedProject), "utf8"),
  );
  assert.equal(manifest.scope, movedProject);
  assert.deepEqual(
    manifest,
    JSON.parse(await readFile(manifestRecoveryPath(movedProject), "utf8")),
  );
  const archiveFiles = await readdir(archiveDirectory(movedProject));
  const referenced = new Set(manifest.segments.map((segment) => segment.file));
  assert(
    archiveFiles.every((file) => referenced.has(file)),
    "recovery left a predecessor/orphan after consecutive GC owner deaths",
  );

  console.log(JSON.stringify({
    driver: "packaged-segmented-history-gc-linux",
    recentLimit: 2,
    generation: manifest.generation,
    segmentCount: manifest.segments.length,
    consecutiveOwnerDeaths: [
      {
        point: "after_generation_manifest_publish",
        browserPid: firstKilled.browserPid,
        sidecarPid: firstKilled.sidecarPid,
      },
      {
        point: "after_gc_delete",
        browserPid: gcKilled.browserPid,
        sidecarPid: gcKilled.sidecarPid,
      },
    ],
    dualElectronContenderRejected: true,
    lostReceiptRetryAlreadyAcknowledged: true,
    relocatedProject: movedProject,
    immutableSegmentsRetained: immutableBefore.size,
    remainingOrphans: 0,
  }, null, 2));
} finally {
  for (const launched of [
    contender,
    recoveredOwner,
    gcOwner,
    firstOwner,
    setup,
  ]) {
    if (launched) await terminatePackaged(launched);
  }
  await stopPackagedDisplay(display);
  if (process.env.OMEGAT_KEEP_E2E !== "1") {
    await rm(workDir, { recursive: true, force: true });
  } else {
    console.error(`kept packaged segmented-history workdir: ${workDir}`);
  }
}
