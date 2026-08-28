// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import {
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rename,
  rm,
  stat,
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
} from "./packaged-driver.mjs";

if (process.platform !== "linux") {
  throw new Error("The randomized durable fault sequence requires a Linux runner");
}
await Promise.all([stat(executable), stat(sidecar)]);

const stepsPerSeed = Number.parseInt(
  process.env.OMEGAT_RANDOM_FAULT_STEPS ?? "16",
  10,
);
assert(
  Number.isSafeInteger(stepsPerSeed)
    && stepsPerSeed >= 12
    && stepsPerSeed <= 256,
);
const seeds = (
  process.env.OMEGAT_RANDOM_FAULT_SEEDS
  ?? process.env.OMEGAT_RANDOM_FAULT_SEED
  ?? "6695,1592639215,3512640997"
)
  .split(",")
  .map((seed) => Number.parseInt(seed.trim(), 10) >>> 0);
assert(seeds.length > 0 && seeds.every((seed) => seed > 0));

const limits = {
  OMEGAT_TEST_CONFIG_HISTORY_LIMIT: "2",
  OMEGAT_TEST_CONFIG_DEDUPE_HOT_LIMIT: "2",
  OMEGAT_TEST_CONFIG_ARCHIVE_SEGMENT_LIMIT: "1",
  OMEGAT_TEST_CONFIG_ARCHIVE_COMPACTION_SEGMENT_LIMIT: "2",
  OMEGAT_TEST_CONFIG_ARCHIVE_COMPACTION_BATCH_LIMIT: "64",
  OMEGAT_TEST_CONFIG_ARCHIVE_BATCH_PREFIX_HEX: "1",
  OMEGAT_TEST_PRODUCT_HISTORY_RECENT_LIMIT: "2",
  OMEGAT_TEST_PRODUCT_HISTORY_HOT_LIMIT: "2",
  OMEGAT_TEST_PRODUCT_HISTORY_SEGMENT_LIMIT: "1",
  OMEGAT_TEST_PRODUCT_HISTORY_COMPACTION_SEGMENTS: "2",
  OMEGAT_TEST_PRODUCT_HISTORY_COMPACTION_RECORDS: "64",
  OMEGAT_TEST_PRODUCT_HISTORY_PREFIX_HEX: "1",
};
const rpc = (client, method, params = {}) =>
  client.evaluate(
    `window.omegat.rpc(${JSON.stringify(method)}, ${JSON.stringify(params)})`,
    true,
  );
const startRpc = (client, method, params) =>
  client.evaluate(`(() => {
    const pending = window.omegat.rpc(
      ${JSON.stringify(method)},
      ${JSON.stringify(params)}
    );
    pending.catch(() => {});
    window.__omegatRandomDurableFault = pending;
    return true;
  })()`);
const projectPaths = (root) => {
  const directory = join(root, ".repositories", "transactions");
  return {
    directory,
    active: join(directory, "active.json"),
    activeRecovery: join(directory, ".active.previous.json"),
    history: join(directory, "history.ndjson"),
    hot: join(directory, "history-hot.json"),
    hotRecovery: join(directory, ".history-hot.recovery.json"),
    manifest: join(directory, "history-manifest.json"),
    manifestRecovery: join(directory, ".history-manifest.recovery.json"),
    archive: join(directory, "history-archive"),
    owner: join(directory, "renderer-owner.json"),
    ownerRecovery: join(directory, ".renderer-owner.recovery.json"),
  };
};
const configPaths = (config) => {
  const directory = join(config, "transactions", "shared-config");
  return {
    directory,
    active: join(directory, "active.json"),
    activeRecovery: join(directory, "active.recovery.json"),
    history: join(directory, "history.ndjson"),
    hot: join(directory, "history-hot.json"),
    hotRecovery: join(directory, ".history-hot.recovery.json"),
    manifest: join(directory, "history-manifest.json"),
    manifestRecovery: join(directory, ".history-manifest.recovery.json"),
    archive: join(directory, "history-archive"),
  };
};
const assertReplicasEqual = async (left, right) => {
  const exists = [await pathExists(left), await pathExists(right)];
  assert.equal(exists[0], exists[1], `${left} and ${right} existence diverged`);
  if (exists[0]) {
    assert((await readFile(left)).equals(await readFile(right)));
  }
};
const assertNoCandidates = async (directory) => {
  if (!await pathExists(directory)) return;
  const names = await readdir(directory, { recursive: true });
  assert.deepEqual(names.filter((name) => name.endsWith(".tmp")), []);
};
const terminal = (row) =>
  ["completed", "cancelled", "request_cancelled", "failed"].includes(row.status);

const durableHistoryRows = async (paths) => {
  const rows = [];
  if (await pathExists(paths.hot)) {
    rows.push(...JSON.parse(await readFile(paths.hot, "utf8")).records);
  }
  if (await pathExists(paths.archive)) {
    for (const name of await readdir(paths.archive)) {
      if (!name.endsWith(".json")) continue;
      const segment = JSON.parse(
        await readFile(join(paths.archive, name), "utf8"),
      );
      rows.push(...segment.records);
    }
  }
  return rows;
};

const terminalRows = async (paths, batchId) =>
  (await durableHistoryRows(paths))
    .filter((row) => row.batch_id === batchId && terminal(row));

const historyGcStatus = async (paths) => {
  await assertReplicasEqual(paths.hot, paths.hotRecovery);
  await assertReplicasEqual(paths.manifest, paths.manifestRecovery);
  const manifest = JSON.parse(await readFile(paths.manifest, "utf8"));
  const archiveFiles = await readdir(paths.archive);
  assert(manifest.generation > 0, `${paths.directory} did not advance GC generation`);
  assert(
    archiveFiles.length > 0,
    `${paths.directory} has no immutable history generation`,
  );
  return {
    generation: manifest.generation,
    segmentCount: manifest.segments.length,
    archiveFiles: archiveFiles.length,
  };
};

async function createProject(display, config, root, text) {
  let app = await launchPackagedRenderer(display, config, null, limits);
  try {
    await rpc(app.client, "project.create", {
      root,
      source_lang: "en",
      target_lang: "fr",
      sentence_seg: false,
    });
    await writeFile(join(root, "source", "source.txt"), text, "utf8");
    await rpc(app.client, "project.reload");
  } finally {
    await terminatePackaged(app);
    app = undefined;
  }
}

async function recoverProject(display, config, root, batchId) {
  let recovered = await launchPackaged(display, config, root, limits);
  try {
    await waitFor(`project transaction ${batchId} recovery`, async () =>
      !await pathExists(projectPaths(root).active) ? true : undefined
    );
    assert.equal((await rpc(recovered.client, "project.props")).root, root);
  } finally {
    await terminatePackaged(recovered);
    recovered = undefined;
  }
}

async function projectFault(
  display,
  workDir,
  config,
  root,
  rootIndex,
  index,
  afterPublish,
) {
  const label = `random-project-${index}`;
  const marker = join(workDir, `${label}.marker`);
  const point = afterPublish ? "after_atomic_publish" : "before_atomic_publish";
  let owner = await launchPackaged(display, config, root, {
    ...limits,
    OMEGAT_TEST_PRODUCT_TRANSACTION_OPERATION: "project.save",
    OMEGAT_TEST_PRODUCT_TRANSACTION_POINT: point,
    OMEGAT_TEST_PRODUCT_TRANSACTION_MARKER: marker,
  });
  let killed;
  try {
    assert.equal(await startRpc(owner.client, "project.save", {}), true);
    await waitFor(`${label} ${point}`, () => pathExists(marker));
    const active = JSON.parse(
      await readFile(projectPaths(root).active, "utf8"),
    );
    const expectedActiveStatus = afterPublish ? "sidecar_committed" : "pending";
    const envelope = active.batches.find((row) =>
      row.payload?.operation === "project.save"
      && row.status === expectedActiveStatus
    );
    assert(envelope, `${label} did not publish its ${expectedActiveStatus} envelope`);
    const batchId = envelope.batch_id;
    killed = await killPackaged(owner);
    owner = undefined;
    await recoverProject(display, config, root, batchId);
    const rows = await terminalRows(projectPaths(root), batchId);
    assert.equal(rows.length, 1, `${batchId} did not converge to one terminal`);
    assert.equal(rows[0].status, afterPublish ? "completed" : "cancelled");
    await assertReplicasEqual(
      projectPaths(root).owner,
      projectPaths(root).ownerRecovery,
    );
    const releasedOwner = JSON.parse(
      await readFile(projectPaths(root).owner, "utf8"),
    );
    assert.equal(releasedOwner.released, true);
    await assertNoCandidates(projectPaths(root).directory);
    return {
      index,
      kind: "project.save",
      root,
      rootIndex,
      batchId,
      point,
      killed,
      terminalStatus: rows[0].status,
      ownerReleased: true,
    };
  } finally {
    await terminatePackaged(owner);
  }
}

async function configFault(
  display,
  workDir,
  config,
  root,
  rootIndex,
  index,
  historyFault,
  random,
) {
  const batchId = `random-config-${index}`;
  const marker = join(workDir, `${batchId}.marker`);
  const requestedTheme = `random-theme-${index}`;
  const replica = random() % 2 === 0 ? "active.recovery.json" : "active.json";
  const point = random() % 2 === 0 ? "after_rename" : "after_parent_fsync";
  const fault = historyFault
    ? {
        OMEGAT_TEST_CONFIG_TRANSACTION_OPERATION: "prefs.patch",
        OMEGAT_TEST_CONFIG_TRANSACTION_POINT: "after_terminal_history_publish",
        OMEGAT_TEST_CONFIG_TRANSACTION_MARKER: marker,
      }
    : {
        OMEGAT_TEST_DURABLE_FILE_NAME: replica,
        OMEGAT_TEST_DURABLE_FILE_POINT: point,
        OMEGAT_TEST_DURABLE_FILE_MARKER: marker,
      };
  let owner = await launchPackagedRenderer(display, config, root, {
    ...limits,
    ...fault,
  });
  let recovered;
  let killed;
  try {
    assert.equal(await startRpc(owner.client, "prefs.patch", {
      theme: requestedTheme,
      config_transaction_retry_batch_id: batchId,
    }), true);
    await waitFor(`${batchId} config fault`, () => pathExists(marker));
    killed = await killPackaged(owner);
    owner = undefined;
    recovered = await launchPackagedRenderer(display, config, root, limits);
    await waitFor(`${batchId} config recovery`, async () => {
      const prefs = await rpc(recovered.client, "prefs.get");
      return prefs.theme === requestedTheme ? true : undefined;
    });
    const rows = await terminalRows(configPaths(config), batchId);
    assert.equal(rows.length, 1, `${batchId} did not converge to one terminal`);
    assert.equal(rows[0].status, "completed");
    await assertReplicasEqual(
      configPaths(config).active,
      configPaths(config).activeRecovery,
    );
    await assertNoCandidates(configPaths(config).directory);
    return {
      index,
      kind: "prefs.patch",
      root,
      rootIndex,
      point: historyFault ? "after_terminal_history_publish" : point,
      replica: historyFault ? null : replica,
      killed,
      exactTerminalRows: rows.length,
    };
  } finally {
    await Promise.all([
      terminatePackaged(recovered),
      terminatePackaged(owner),
    ]);
  }
}

async function moveProject(display, config, oldRoot, newRoot, rootIndex) {
  const beforePaths = projectPaths(oldRoot);
  const immutableBefore = new Map();
  if (await pathExists(beforePaths.archive)) {
    for (const name of await readdir(beforePaths.archive)) {
      immutableBefore.set(
        name,
        (await readFile(join(beforePaths.archive, name))).toString("base64"),
      );
    }
  }
  await rename(oldRoot, newRoot);
  await recoverProject(display, config, newRoot, `move-root-${rootIndex}`);
  const afterPaths = projectPaths(newRoot);
  for (const [name, bytes] of immutableBefore) {
    assert.equal(
      (await readFile(join(afterPaths.archive, name))).toString("base64"),
      bytes,
      `project move rewrote immutable history segment ${name}`,
    );
  }
  await assertReplicasEqual(afterPaths.owner, afterPaths.ownerRecovery);
  return {
    rootIndex,
    oldRoot,
    newRoot,
    immutableSegmentsVerified: immutableBefore.size,
    mutableScopeRebased: false,
  };
}

const makeRandom = (seed) => {
  let state = seed || 1;
  return () => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    return state >>> 0;
  };
};

async function runSeed(display, suiteDir, seed) {
  const workDir = join(suiteDir, `seed-${seed}`);
  const config = join(workDir, "config");
  const roots = [join(workDir, "project-a"), join(workDir, "project-b")];
  const random = makeRandom(seed);
  await mkdir(config, { recursive: true });
  await createProject(display, config, roots[0], `random source a seed ${seed}`);
  await createProject(display, config, roots[1], `random source b seed ${seed}`);
  const trace = [];
  let projectMove;
  for (let index = 0; index < stepsPerSeed; index += 1) {
    if (index === 6) {
      const oldRoot = roots[0];
      roots[0] = join(workDir, "project-a-after-history-move");
      projectMove = await moveProject(
        display,
        config,
        oldRoot,
        roots[0],
        0,
      );
    }
    const scenario = index < 8
      ? index % 2
      : index < 12
        ? 2 + index % 2
        : random() % 4;
    const rootIndex = index < 8 ? index % roots.length : random() % roots.length;
    const root = roots[rootIndex];
    if (scenario === 0 || scenario === 1) {
      trace.push(await projectFault(
        display,
        workDir,
        config,
        root,
        rootIndex,
        index,
        scenario === 1,
      ));
    } else {
      trace.push(await configFault(
        display,
        workDir,
        config,
        root,
        rootIndex,
        index,
        scenario === 3,
        random,
      ));
    }
  }
  assert(projectMove, `seed ${seed} did not execute the project move`);
  const movedRecent = (await readFile(projectPaths(roots[0]).history, "utf8"))
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
  assert(
    movedRecent.every((row) => row.project_root === roots[0]),
    "post-move transactions retained stale mutable history scope",
  );
  projectMove.mutableScopeRebased = true;
  const projectHistoryGc = [];
  for (const root of roots) {
    assert.equal(await pathExists(projectPaths(root).active), false);
    await assertReplicasEqual(
      projectPaths(root).owner,
      projectPaths(root).ownerRecovery,
    );
    await assertNoCandidates(projectPaths(root).directory);
    projectHistoryGc.push(await historyGcStatus(projectPaths(root)));
  }
  const configHistoryGc = await historyGcStatus(configPaths(config));
  await assertNoCandidates(configPaths(config).directory);
  assert.deepEqual(
    [...new Set(
      trace
        .filter((row) => row.kind === "project.save")
        .map((row) => row.rootIndex),
    )].toSorted(),
    [0, 1],
  );
  assert(
    trace.some((row) =>
      row.kind === "project.save" && row.root === projectMove.newRoot
    ),
    `seed ${seed} did not continue transactions after project move`,
  );
  return {
    seed,
    steps: stepsPerSeed,
    roots,
    trace,
    projectMove,
    historyGc: {
      projects: projectHistoryGc,
      config: configHistoryGc,
    },
  };
}

const suiteDir = await mkdtemp(join(tmpdir(), "omegat-random-durable-faults-"));
const display = await startPackagedDisplay();
const startedAt = Date.now();
try {
  const runs = [];
  for (const seed of seeds) {
    runs.push(await runSeed(display.display, suiteDir, seed));
  }
  const trace = runs.flatMap((run) =>
    run.trace.map((row) => ({ ...row, seed: run.seed }))
  );
  console.log(JSON.stringify({
    result: "passed",
    driver: "packaged-random-durable-faults-linux",
    package: executable,
    platform: process.platform,
    seeds,
    seed: seeds[0],
    stepsPerSeed,
    steps: trace.length,
    durationMs: Date.now() - startedAt,
    runs,
    trace,
    allQueuesDrained: true,
    allTerminalDecisionsExact: true,
    ownerReleaseTombstonesVerified: true,
    historyGcVerified: true,
    projectMoveCombinedWithFaultSequence: true,
    platformsNotRun: ["windows", "macos"],
  }, null, 2));
} finally {
  await stopPackagedDisplay(display);
  if (process.env.OMEGAT_KEEP_E2E !== "1") {
    await rm(suiteDir, { recursive: true, force: true });
  } else {
    console.error(`kept randomized durable fault workdir: ${suiteDir}`);
  }
}
