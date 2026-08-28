// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import {
  mkdir,
  mkdtemp,
  readFile,
  readdir,
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

const steps = Number.parseInt(process.env.OMEGAT_RANDOM_FAULT_STEPS ?? "24", 10);
assert(Number.isSafeInteger(steps) && steps >= 8 && steps <= 256);
const initialSeed = Number.parseInt(
  process.env.OMEGAT_RANDOM_FAULT_SEED ?? "6695",
  10,
) >>> 0;
let randomState = initialSeed || 1;
const random = () => {
  randomState ^= randomState << 13;
  randomState ^= randomState >>> 17;
  randomState ^= randomState << 5;
  return randomState >>> 0;
};

const limits = {
  OMEGAT_TEST_CONFIG_HISTORY_LIMIT: "256",
  OMEGAT_TEST_CONFIG_DEDUPE_HOT_LIMIT: "256",
  OMEGAT_TEST_PRODUCT_HISTORY_RECENT_LIMIT: "256",
  OMEGAT_TEST_PRODUCT_HISTORY_HOT_LIMIT: "256",
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
const parseRows = async (path) => {
  if (!await pathExists(path)) return [];
  return (await readFile(path, "utf8"))
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
};
const projectPaths = (root) => {
  const directory = join(root, ".repositories", "transactions");
  return {
    directory,
    active: join(directory, "active.json"),
    activeRecovery: join(directory, ".active.previous.json"),
    history: join(directory, "history.ndjson"),
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

async function projectFault(display, workDir, config, root, index, afterPublish) {
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
    const rows = (await parseRows(projectPaths(root).history))
      .filter((row) => row.batch_id === batchId && terminal(row));
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

async function configFault(display, workDir, config, root, index, historyFault) {
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
    const rows = (await parseRows(configPaths(config).history))
      .filter((row) => row.batch_id === batchId && terminal(row));
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

const workDir = await mkdtemp(join(tmpdir(), "omegat-random-durable-faults-"));
const config = join(workDir, "config");
const roots = [join(workDir, "project-a"), join(workDir, "project-b")];
const display = await startPackagedDisplay();
const startedAt = Date.now();
try {
  await mkdir(config, { recursive: true });
  await createProject(display.display, config, roots[0], "random source a");
  await createProject(display.display, config, roots[1], "random source b");
  const trace = [];
  for (let index = 0; index < steps; index += 1) {
    const scenario = index < 4 ? index : random() % 4;
    const root = index < 2 ? roots[index] : roots[random() % roots.length];
    if (scenario === 0 || scenario === 1) {
      trace.push(await projectFault(
        display.display,
        workDir,
        config,
        root,
        index,
        scenario === 1,
      ));
    } else {
      trace.push(await configFault(
        display.display,
        workDir,
        config,
        root,
        index,
        scenario === 3,
      ));
    }
  }
  for (const root of roots) {
    assert.equal(await pathExists(projectPaths(root).active), false);
    await assertReplicasEqual(
      projectPaths(root).owner,
      projectPaths(root).ownerRecovery,
    );
  }
  assert.equal(
    trace.filter((row) => row.kind === "project.save").length > 0,
    true,
  );
  assert.equal(
    trace.filter((row) => row.kind === "prefs.patch").length > 0,
    true,
  );
  assert.deepEqual(
    [...new Set(
      trace
        .filter((row) => row.kind === "project.save")
        .map((row) => row.root),
    )].toSorted(),
    roots.toSorted(),
  );
  console.log(JSON.stringify({
    result: "passed",
    driver: "packaged-random-durable-faults-linux",
    package: executable,
    platform: process.platform,
    seed: initialSeed,
    steps,
    durationMs: Date.now() - startedAt,
    roots,
    trace,
    allQueuesDrained: true,
    allTerminalDecisionsExact: true,
    ownerReleaseTombstonesVerified: true,
    platformsNotRun: ["windows", "macos"],
  }, null, 2));
} finally {
  await stopPackagedDisplay(display);
  if (process.env.OMEGAT_KEEP_E2E !== "1") {
    await rm(workDir, { recursive: true, force: true });
  } else {
    console.error(`kept randomized durable fault workdir: ${workDir}`);
  }
}
