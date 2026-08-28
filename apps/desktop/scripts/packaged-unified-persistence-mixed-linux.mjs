// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  realpath,
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
  throw new Error("The unified persistence fault matrix requires a Linux runner");
}
await Promise.all([stat(executable), stat(sidecar)]);

const limits = {
  OMEGAT_TEST_CONFIG_HISTORY_LIMIT: "2",
  OMEGAT_TEST_CONFIG_DEDUPE_HOT_LIMIT: "2",
  OMEGAT_TEST_CONFIG_ARCHIVE_SEGMENT_LIMIT: "1",
  OMEGAT_TEST_CONFIG_ARCHIVE_COMPACTION_SEGMENT_LIMIT: "3",
  OMEGAT_TEST_CONFIG_ARCHIVE_COMPACTION_BATCH_LIMIT: "32",
  OMEGAT_TEST_CONFIG_ARCHIVE_BATCH_PREFIX_HEX: "1",
  OMEGAT_TEST_PRODUCT_HISTORY_RECENT_LIMIT: "2",
  OMEGAT_TEST_PRODUCT_HISTORY_HOT_LIMIT: "2",
  OMEGAT_TEST_PRODUCT_HISTORY_SEGMENT_LIMIT: "1",
  OMEGAT_TEST_PRODUCT_HISTORY_COMPACTION_SEGMENTS: "3",
  OMEGAT_TEST_PRODUCT_HISTORY_COMPACTION_RECORDS: "32",
  OMEGAT_TEST_PRODUCT_HISTORY_PREFIX_HEX: "1",
};

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

class SidecarSession {
  constructor(configDir, extraEnv = {}) {
    this.child = spawn(sidecar, [], {
      env: {
        ...process.env,
        OMEGAT_CONFIG_DIR: configDir,
        ...extraEnv,
      },
      stdio: ["pipe", "pipe", "pipe"],
    });
    this.nextId = 1;
    this.pending = new Map();
    this.stdout = "";
    this.stderr = "";
    this.child.stdout.on("data", (chunk) => {
      this.stdout += chunk.toString();
      while (true) {
        const newline = this.stdout.indexOf("\n");
        if (newline < 0) break;
        const line = this.stdout.slice(0, newline).trim();
        this.stdout = this.stdout.slice(newline + 1);
        if (!line) continue;
        const message = JSON.parse(line);
        if (message.id == null) continue;
        const pending = this.pending.get(message.id);
        if (!pending) continue;
        this.pending.delete(message.id);
        if (message.error) pending.reject(new Error(JSON.stringify(message.error)));
        else pending.resolve(message.result);
      }
    });
    this.child.stderr.on("data", (chunk) => {
      this.stderr += chunk.toString();
    });
  }

  request(method, params = {}) {
    const id = this.nextId++;
    return new Promise((resolveRequest, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`sidecar request timed out: ${method}\n${this.stderr}`));
      }, 60_000);
      timeout.unref();
      this.pending.set(id, {
        resolve: (value) => {
          clearTimeout(timeout);
          resolveRequest(value);
        },
        reject: (error) => {
          clearTimeout(timeout);
          reject(error);
        },
      });
      this.child.stdin.write(
        `${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`,
      );
    });
  }

  async close() {
    if (this.child.exitCode !== null) return;
    const exited = new Promise((resolveExit, reject) => {
      this.child.once("error", reject);
      this.child.once("exit", (code, signal) => resolveExit({ code, signal }));
    });
    this.child.stdin.end();
    const result = await exited;
    assert.equal(result.signal, null, `setup sidecar exited by ${result.signal}`);
    assert.equal(result.code, 0, `setup sidecar failed: ${this.stderr}`);
  }
}

async function startRpc(client, method, params) {
  const started = await client.evaluate(`(() => {
    window.__omegatUnifiedPersistenceRequest = window.omegat.rpc(
      ${JSON.stringify(method)},
      ${JSON.stringify(params)}
    );
    window.__omegatUnifiedPersistenceRequest.catch(() => {});
    return true;
  })()`);
  assert.equal(started, true);
}

function configPaths(config) {
  const directory = join(config, "transactions", "shared-config");
  return {
    directory,
    active: join(directory, "active.json"),
    activeRecovery: join(directory, "active.recovery.json"),
    recent: join(directory, "history.ndjson"),
    hot: join(directory, "history-hot.json"),
    hotRecovery: join(directory, ".history-hot.recovery.json"),
    manifest: join(directory, "history-manifest.json"),
    manifestRecovery: join(directory, ".history-manifest.recovery.json"),
    archive: join(directory, "history-archive"),
    legacyDedupe: join(directory, "dedupe.json"),
    legacyDedupeRecovery: join(directory, "dedupe.recovery.json"),
    legacyManifest: join(directory, "manifest.json"),
    legacyManifestRecovery: join(directory, "manifest.recovery.json"),
    legacyArchive: join(directory, "archive"),
    migrationSeed: join(directory, ".history-unified-migration.ndjson"),
  };
}

function projectPaths(project) {
  const directory = join(project, ".repositories", "transactions");
  return {
    directory,
    active: join(directory, "active.json"),
    activeRecovery: join(directory, ".active.previous.json"),
    recent: join(directory, "history.ndjson"),
    hot: join(directory, "history-hot.json"),
    hotRecovery: join(directory, ".history-hot.recovery.json"),
    manifest: join(directory, "history-manifest.json"),
    manifestRecovery: join(directory, ".history-manifest.recovery.json"),
    archive: join(directory, "history-archive"),
    owner: join(directory, "renderer-owner.json"),
    ownerRecovery: join(directory, ".renderer-owner.recovery.json"),
  };
}

async function writeJson(path, value) {
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function legacyTerminal(config, batchId, sequence) {
  return {
    version: 2,
    config_dir: config,
    batch_id: batchId,
    operation: "prefs.patch",
    app_instance: `legacy-${sequence}`,
    owner_process_id: 20_000 + sequence,
    status: "completed",
    payload: { theme: `legacy-${sequence}` },
    result: { legacy_result: sequence },
    error: null,
    updated_unix_ms: Date.now() + sequence,
  };
}

async function seedLegacyConfig(configPath) {
  await mkdir(configPath, { recursive: true });
  const config = await realpath(configPath);
  const paths = configPaths(config);
  await mkdir(paths.directory, { recursive: true });
  const rows = Array.from(
    { length: 4 },
    (_, index) => legacyTerminal(config, `legacy-${index}`, index),
  );
  const dedupe = {
    version: 2,
    config_dir: config,
    revision: 7,
    batches: Object.fromEntries(rows.map((row) => [row.batch_id, row])),
    order: rows.map((row) => row.batch_id),
    updated_unix_ms: Date.now(),
  };
  const manifest = {
    version: 2,
    config_dir: config,
    revision: 3,
    next_segment_id: 1,
    generation: 0,
    segments: [],
    batch_index: {},
    batch_index_complete: true,
    updated_unix_ms: Date.now(),
  };
  const pending = {
    version: 2,
    config_dir: config,
    batch_id: "legacy-pending",
    operation: "prefs.patch",
    app_instance: "legacy-pending-owner",
    owner_process_id: 30_000,
    status: "pending",
    payload: { locale: "fr" },
    result: null,
    error: null,
    updated_unix_ms: Date.now(),
  };
  await Promise.all([
    writeJson(paths.legacyDedupe, dedupe),
    writeJson(paths.legacyDedupeRecovery, dedupe),
    writeJson(paths.legacyManifest, manifest),
    writeJson(paths.legacyManifestRecovery, manifest),
    writeJson(paths.active, {
      version: 2,
      config_dir: config,
      revision: 9,
      batches: [pending],
      updated_unix_ms: Date.now(),
    }),
    writeJson(paths.activeRecovery, {
      version: 2,
      config_dir: config,
      revision: 9,
      batches: [pending],
      updated_unix_ms: Date.now(),
    }),
    writeFile(
      paths.recent,
      `${rows.map((row) => JSON.stringify(row)).join("\n")}\n`,
      "utf8",
    ),
  ]);
  return { config, paths, rows, pending };
}

async function snapshot(path) {
  const info = await stat(path, { bigint: true });
  return {
    bytes: (await readFile(path)).toString("base64"),
    mtimeNs: info.mtimeNs.toString(),
    size: info.size.toString(),
  };
}

async function segmentedRows(paths) {
  const manifest = JSON.parse(await readFile(paths.manifest, "utf8"));
  assert.deepEqual(
    manifest,
    JSON.parse(await readFile(paths.manifestRecovery, "utf8")),
  );
  const rows = [];
  for (const descriptor of [...manifest.segments].sort((a, b) => a.id - b.id)) {
    const bytes = await readFile(join(paths.archive, descriptor.file));
    assert.equal(
      createHash("sha256").update(bytes).digest("hex"),
      descriptor.sha256,
    );
    const segment = JSON.parse(bytes);
    assert.equal(segment.version, 1);
    assert.equal(segment.id, descriptor.id);
    assert.equal(segment.generation, descriptor.generation);
    assert.equal(segment.records.length, descriptor.record_count);
    rows.push(...segment.records);
  }
  const hot = JSON.parse(await readFile(paths.hot, "utf8"));
  assert.deepEqual(hot, JSON.parse(await readFile(paths.hotRecovery, "utf8")));
  rows.push(...hot.records);
  return { manifest, hot, rows };
}

async function assertNoTemporaryFiles(directory) {
  if (!await pathExists(directory)) return;
  const leftovers = (await readdir(directory, { recursive: true }))
    .filter((name) => name.endsWith(".tmp"));
  assert.deepEqual(leftovers, []);
}

function collidingBatchIds() {
  const firstByPrefix = new Map();
  for (let index = 0; index < 256; index += 1) {
    const id = `config-prefix-${index}`;
    const prefix = createHash("sha256").update(id).digest("hex").slice(0, 1);
    if (firstByPrefix.has(prefix)) {
      return [firstByPrefix.get(prefix), id, prefix];
    }
    firstByPrefix.set(prefix, id);
  }
  throw new Error("could not construct one-hex batch prefix collision");
}

async function verifyLegacyMigrationAndPrefixCollision(display, workDir) {
  const seeded = await seedLegacyConfig(join(workDir, "legacy-config"));
  let launched = await launchPackagedRenderer(
    display,
    seeded.config,
    null,
    limits,
  );
  try {
    const prefs = await waitFor("legacy config transaction recovery", async () => {
      const current = await rpc(launched.client, "prefs.get");
      return current.locale === "fr" ? current : undefined;
    });
    const exactRetry = await rpc(launched.client, "prefs.patch", {
      theme: "legacy-0",
      config_transaction_retry_batch_id: "legacy-0",
    });
    assert.deepEqual(exactRetry, { legacy_result: 0 });
    const conflict = await rpcOutcome(launched.client, "prefs.patch", {
      theme: "not-the-legacy-payload",
      config_transaction_retry_batch_id: "legacy-0",
    });
    assert.equal(conflict.resolved, false);
    assert.match(conflict.error, /reused for a different operation or payload/);

    const [first, second, prefix] = collidingBatchIds();
    const newIds = [first, "config-between-a", "config-between-b", second];
    for (const [index, batchId] of newIds.entries()) {
      const result = await rpc(launched.client, "prefs.patch", {
        theme: `unified-${index}`,
        config_transaction_retry_batch_id: batchId,
      });
      assert.equal(result.theme, `unified-${index}`);
    }
    const state = await segmentedRows(seeded.paths);
    const ids = state.rows.map((row) => row.batch_id);
    assert.deepEqual(
      ids,
      [
        ...seeded.rows.map((row) => row.batch_id),
        seeded.pending.batch_id,
        ...newIds,
      ],
    );
    assert.equal(new Set(ids).size, ids.length);
    assert.equal(state.hot.records.length, 2);
    assert(
      state.manifest.partition_index[prefix].length >= 1,
      "sparse manifest dropped a colliding prefix",
    );
    assert(state.rows.every((row) => row.version === 3));
    for (const oldPath of [
      seeded.paths.legacyDedupe,
      seeded.paths.legacyDedupeRecovery,
      seeded.paths.legacyManifest,
      seeded.paths.legacyManifestRecovery,
      seeded.paths.legacyArchive,
      seeded.paths.migrationSeed,
    ]) {
      assert.equal(await pathExists(oldPath), false);
    }
    await assertNoTemporaryFiles(seeded.paths.directory);
    return {
      config: seeded.config,
      legacyRows: seeded.rows.length + 1,
      newRows: newIds.length,
      exactLegacyRetry: true,
      prefixCollision: { prefix, batchIds: [first, second] },
      generation: state.manifest.generation,
    };
  } finally {
    await terminatePackaged(launched);
    launched = undefined;
  }
}

async function createProject(client, root, text) {
  await rpc(client, "project.create", {
    root,
    source_lang: "en",
    target_lang: "fr",
    sentence_seg: false,
  });
  await writeFile(join(root, "source", "source.txt"), text, "utf8");
  await rpc(client, "project.reload");
}

async function createProjectInSession(session, root, text) {
  await session.request("project.create", {
    root,
    source_lang: "en",
    target_lang: "fr",
    sentence_seg: false,
  });
  await writeFile(join(root, "source", "source.txt"), text, "utf8");
  await session.request("project.reload");
}

function parseNdjson(raw) {
  return raw
    .trim()
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

async function runMixedQueueTakeovers(display, workDir, config) {
  const rootA = join(workDir, "prepared-project-a");
  const movedA = join(workDir, "prepared-project-a-moved");
  const rootB = join(workDir, "prepared-project-b");
  const remote = join(workDir, "prepared-remote");
  const traceA = join(workDir, "prepared-project-a-acks.ndjson");
  const firstMarker = join(workDir, "prepared-owner-first.marker");
  const firstRelease = join(workDir, "prepared-owner-first.release");
  const secondMarker = join(workDir, "prepared-owner-second.marker");
  const secondRelease = join(workDir, "prepared-owner-second.release");
  await mkdir(join(remote, "target"), { recursive: true });
  await writeFile(join(remote, "target", "team.txt"), "remote-before", "utf8");

  const session = new SidecarSession(config, limits);
  let first;
  let second;
  let third;
  let projectB;
  let moved;
  let moveSession;
  try {
    const configPatch = (batchId, patch) =>
      session.request("prefs.patch", {
        ...patch,
        config_transaction_app_instance: "prepared-mixed-setup",
        config_transaction_batch_id: batchId,
        config_transaction_owner_process_id: session.child.pid,
      });
    await createProjectInSession(session, rootA, "prepared mixed source a");
    await session.request("team.mapping", {
      repositories: [{
        repo_type: "file",
        url: remote,
        branch: null,
        mappings: [{
          local: "/target/team.txt",
          repository: "/target/team.txt",
          includes: [],
          excludes: [],
        }],
      }],
    });
    await session.request("team.sync");
    await configPatch("prepared-config-0", { theme: "prepared-before-close" });

    const close = await session.request("project.close", {
      transaction_project_root: rootA,
      transaction_generation: 181,
      transaction_batch_id: "prepared-close",
    });
    assert.equal(close.receipt.payload.operation, "project.close");
    await session.request("project.open", { root: rootA });
    await writeFile(join(rootA, "target", "team.txt"), "prepared-team-once", "utf8");
    const team = await session.request("team.commit", {
      which: "target",
      transaction_project_root: rootA,
      transaction_generation: 181,
      transaction_batch_id: "prepared-team",
    });
    assert.equal(team.receipt.payload.operation, "commit-target");
    await configPatch("prepared-config-1", { locale: "it" });
    const save = await session.request("project.save", {
      transaction_project_root: rootA,
      transaction_generation: 181,
      transaction_batch_id: "prepared-save",
    });
    assert.equal(save.receipt.payload.operation, "project.save");
    await writeFile(join(rootA, "source", "source.txt"), "prepared refresh source", "utf8");
    const refresh = await session.request("project.refresh.enqueue", {
      root: rootA,
      app_instance: "prepared-refresh-owner",
      generation: 181,
      paths: [join(rootA, "source", "source.txt")],
      fingerprints: { "source/source.txt": "prepared-refresh" },
      sources: ["native", "sidecar"],
    });
    assert.equal(refresh.batch.status, "pending");

    await createProjectInSession(session, rootB, "prepared mixed source b");
    const saveB = await session.request("project.save", {
      transaction_project_root: rootB,
      transaction_generation: 191,
      transaction_batch_id: "prepared-save-b",
    });
    assert.equal(saveB.receipt.batch_id, "prepared-save-b");
    const refreshB = await session.request("project.refresh.enqueue", {
      root: rootB,
      app_instance: "prepared-refresh-owner-b",
      generation: 191,
      paths: [join(rootB, "source", "source.txt")],
      fingerprints: { "source/source.txt": "prepared-refresh-b" },
      sources: ["native"],
    });
    assert.equal(refreshB.batch.status, "pending");
    await configPatch("prepared-config-2", { theme: "prepared-after-b" });
    await session.close();

    // Seed the two former project active shapes and the former single-copy
    // renderer owner. The first real packaged claimant must restartably
    // migrate them before dispatching any product.
    for (const root of [rootA, rootB]) {
      const paths = projectPaths(root);
      const current = JSON.parse(await readFile(paths.active, "utf8"));
      const legacy = {
        version: 2,
        project_root: root,
        batches: current.batches,
      };
      await Promise.all([
        writeJson(paths.active, legacy),
        writeJson(paths.activeRecovery, legacy),
      ]);
    }
    await writeJson(projectPaths(rootA).owner, {
      version: 1,
      project_root: rootA,
      app_instance: "legacy-dead-renderer",
      process_id: 4_000_000,
      generation: 180,
      claim_id: "legacy-dead-claim",
      updated_unix_ms: Date.now() - 10_000,
    });

    const expectedA = [
      ["prepared-close", "sidecar_committed"],
      ["prepared-team", "sidecar_committed"],
      ["prepared-save", "sidecar_committed"],
      [refresh.batch.batch_id, "pending"],
    ];
    assert.deepEqual(
      JSON.parse(await readFile(projectPaths(rootA).active, "utf8"))
        .batches.map((row) => [row.batch_id, row.status]),
      expectedA,
    );
    assert.deepEqual(
      JSON.parse(await readFile(projectPaths(rootB).active, "utf8"))
        .batches.map((row) => [row.batch_id, row.status]),
      [
        ["prepared-save-b", "sidecar_committed"],
        [refreshB.batch.batch_id, "pending"],
      ],
    );

    first = await launchPackagedRenderer(display, config, rootA, {
      ...limits,
      OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_FOR: "project.close",
      OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_MARKER: firstMarker,
      OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_RELEASE: firstRelease,
    });
    const firstClaim = await waitFor("first prepared owner claim", async () =>
      await pathExists(firstMarker)
        ? JSON.parse(await readFile(firstMarker, "utf8"))
        : undefined
    );
    assert.equal(firstClaim.batch_id, "prepared-close");
    const firstKilled = await killPackaged(first);
    first = undefined;

    second = await launchPackagedRenderer(display, config, rootA, {
      ...limits,
      OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_FOR: "project.close",
      OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_MARKER: secondMarker,
      OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_RELEASE: secondRelease,
    });
    const secondClaim = await waitFor("second prepared owner claim", async () =>
      await pathExists(secondMarker)
        ? JSON.parse(await readFile(secondMarker, "utf8"))
        : undefined
    );
    assert.equal(secondClaim.batch_id, "prepared-close");
    assert.notEqual(secondClaim.owner_process_id, firstClaim.owner_process_id);
    const secondKilled = await killPackaged(second);
    second = undefined;

    // A recovered close receipt must be discovered as a detached scope.
    // Starting with OMEGAT_PROJECT would attach a watcher before the close is
    // delivered, so renderer unwatch would follow the ordinary close path and
    // cancel the queued refresh instead of draining the detached FIFO tail.
    third = await launchPackagedRenderer(display, config, null, {
      ...limits,
      OMEGAT_TEST_TRANSACTION_ACK_TRACE: traceA,
    });
    await waitFor("prepared cross-root FIFO drain", async () =>
      !await pathExists(projectPaths(rootA).active)
        && !await pathExists(projectPaths(rootB).active)
        ? true
        : undefined
    );
    const acknowledged = parseNdjson(await readFile(traceA, "utf8"))
      .filter((row) => row.result === "acknowledged");
    const expectedB = [
      ["prepared-save-b", "project.save"],
      [refreshB.batch.batch_id, "project.external-refresh"],
    ];
    assert.deepEqual(
      acknowledged.map((row) => row.batch_id),
      [
        expectedA[0][0],
        expectedB[0][0],
        expectedA[1][0],
        expectedB[1][0],
        expectedA[2][0],
        expectedA[3][0],
      ],
    );
    const acknowledgedA = acknowledged.filter((row) =>
      expectedA.some(([batchId]) => batchId === row.batch_id)
    );
    const operationsA = acknowledgedA.map((row) => row.operation);
    assert.deepEqual(operationsA, [
      "project.close",
      "commit-target",
      "project.save",
      "project.external-refresh",
    ]);
    assert.deepEqual(
      acknowledgedA.map((row) => row.batch_id),
      expectedA.map(([batchId]) => batchId),
    );
    await terminatePackaged(third);
    third = undefined;

    projectB = await launchPackagedRenderer(display, config, rootB, {
      ...limits,
    });
    const acknowledgedB = acknowledged.filter((row) =>
      expectedB.some(([batchId]) => batchId === row.batch_id)
    );
    assert.deepEqual(
      acknowledgedB.map((row) => [row.batch_id, row.operation]),
      expectedB,
    );
    assert.equal(
      await readFile(join(remote, "target", "team.txt"), "utf8"),
      "prepared-team-once",
    );
    assert.equal((await rpc(projectB.client, "prefs.get")).theme, "prepared-after-b");

    const configState = await segmentedRows(configPaths(config));
    const configIds = configState.rows.map((row) => row.batch_id);
    const configOrder = [
      "prepared-config-0",
      "prepared-config-1",
      "prepared-config-2",
    ];
    const positions = configOrder.map((batchId) => configIds.indexOf(batchId));
    assert(positions.every((position) => position >= 0));
    assert(positions[0] < positions[1] && positions[1] < positions[2]);

    const relocationLimits = {
      ...limits,
      OMEGAT_TEST_PRODUCT_HISTORY_COMPACTION_SEGMENTS: "1024",
    };
    moveSession = new SidecarSession(config, relocationLimits);
    await moveSession.request("project.open", { root: rootA });
    const moveSave = await moveSession.request("project.save", {
      transaction_project_root: rootA,
      transaction_generation: 201,
      transaction_batch_id: "prepared-move-save",
    });
    assert.equal(moveSave.receipt.status, "sidecar_committed");
    await moveSession.close();

    const immutableBefore = {};
    for (const file of await readdir(projectPaths(rootA).archive)) {
      immutableBefore[file] = (await readFile(
        join(projectPaths(rootA).archive, file),
      )).toString("base64");
    }
    await terminatePackaged(projectB);
    projectB = undefined;
    await rename(rootA, movedA);
    moved = await launchPackaged(display, config, movedA, relocationLimits);
    await waitFor("moved project receipt drain", async () =>
      !await pathExists(projectPaths(movedA).active) ? true : undefined
    );
    assert.equal((await rpc(moved.client, "project.props")).root, movedA);
    const movedManifest = JSON.parse(
      await readFile(projectPaths(movedA).manifest, "utf8"),
    );
    assert.equal(movedManifest.scope, movedA);
    assert.deepEqual(
      movedManifest,
      JSON.parse(await readFile(projectPaths(movedA).manifestRecovery, "utf8")),
    );
    for (const [file, bytes] of Object.entries(immutableBefore)) {
      assert.equal(
        (await readFile(join(projectPaths(movedA).archive, file))).toString("base64"),
        bytes,
      );
    }
    await assertNoTemporaryFiles(projectPaths(movedA).directory);
    return {
      roots: [movedA, rootB],
      projectAOrder: operationsA,
      projectBOrder: acknowledgedB.map((row) => row.operation),
      crossRootDispatchOrder: acknowledged.map((row) => row.batch_id),
      configOrder,
      consecutiveOwnerTakeovers: [firstKilled, secondKilled],
      projectMoveRebasedMutableMetadata: true,
      immutableProjectSegmentsRetained: Object.keys(immutableBefore).length,
      globalConfigProjectIsolation: true,
      ownerReplicasEqual: await Promise.all([movedA, rootB].map(async (root) =>
        (await readFile(projectPaths(root).owner)).equals(
          await readFile(projectPaths(root).ownerRecovery),
        )
      )),
    };
  } finally {
    if (session.child.exitCode === null) {
      session.child.kill("SIGKILL");
    }
    if (moveSession?.child.exitCode === null) {
      moveSession.child.kill("SIGKILL");
    }
    await Promise.all([
      terminatePackaged(moved),
      terminatePackaged(projectB),
      terminatePackaged(third),
      terminatePackaged(second),
      terminatePackaged(first),
    ]);
  }
}

async function runDeletedRootReplacement(display, workDir, config) {
  const deleted = join(workDir, "deleted-project");
  const replacement = join(workDir, "replacement-project");
  let launched = await launchPackagedRenderer(display, config, null, limits);
  try {
    await createProject(launched.client, deleted, "deleted source");
    await rpc(launched.client, "project.refresh.enqueue", {
      root: deleted,
      app_instance: "deleted-root-owner",
      generation: 101,
      paths: [join(deleted, "source", "source.txt")],
      fingerprints: { "source/source.txt": "deleted" },
      sources: ["native"],
    });
    await terminatePackaged(launched);
    launched = undefined;
    await rm(deleted, { recursive: true, force: true });

    launched = await launchPackagedRenderer(display, config, null, limits);
    await createProject(launched.client, replacement, "replacement source");
    const queued = await rpc(launched.client, "project.refresh.enqueue", {
      root: replacement,
      app_instance: "deleted-root-owner",
      generation: 102,
      paths: [join(replacement, "source", "source.txt")],
      fingerprints: { "source/source.txt": "replacement" },
      sources: ["native"],
    });
    assert.equal(queued.batch.status, "pending");
    assert.equal(queued.batch.project_root, replacement);
    await rpc(launched.client, "project.refresh.discard", {
      root: replacement,
      app_instance: "deleted-root-owner",
      generation: 102,
    });
    return {
      staleRootDeleted: true,
      replacementRootSelected: true,
      replacement,
    };
  } finally {
    await terminatePackaged(launched);
  }
}

async function prepareFaultFixture(display, workDir, name) {
  const config = join(workDir, name);
  let setup = await launchPackagedRenderer(display, config, null, limits);
  try {
    for (let index = 0; index < 4; index += 1) {
      await rpc(setup.client, "prefs.patch", {
        theme: `${name}-seed-${index}`,
        config_transaction_retry_batch_id: `${name}-seed-${index}`,
      });
    }
  } finally {
    await terminatePackaged(setup);
    setup = undefined;
  }
  return { config, paths: configPaths(config) };
}

async function runHistoryCrashBoundary(display, workDir, point, sequence) {
  const fixture = await prepareFaultFixture(
    display,
    workDir,
    `history-boundary-${sequence}`,
  );
  const marker = join(workDir, `history-boundary-${sequence}.marker`);
  const batchId = `history-boundary-batch-${sequence}`;
  let owner = await launchPackagedRenderer(display, fixture.config, null, {
    ...limits,
    OMEGAT_TEST_CONFIG_TRANSACTION_OPERATION: "prefs.patch",
    OMEGAT_TEST_CONFIG_TRANSACTION_POINT: point,
    OMEGAT_TEST_CONFIG_TRANSACTION_MARKER: marker,
  });
  let recovered;
  try {
    await startRpc(owner.client, "prefs.patch", {
      locale: `boundary-${sequence}`,
      config_transaction_retry_batch_id: batchId,
    });
    await waitFor(`config history ${point}`, () => pathExists(marker));
    const productAtKill = await snapshot(join(fixture.config, "omegat.prefs.json"));
    const killed = await killPackaged(owner);
    owner = undefined;
    recovered = await launchPackagedRenderer(
      display,
      fixture.config,
      null,
      limits,
    );
    assert.equal((await rpc(recovered.client, "prefs.get")).locale, `boundary-${sequence}`);
    assert.deepEqual(
      await snapshot(join(fixture.config, "omegat.prefs.json")),
      productAtKill,
    );
    await rpc(recovered.client, "prefs.patch", {
      locale: `boundary-${sequence}`,
      config_transaction_retry_batch_id: batchId,
    });
    assert.deepEqual(
      await snapshot(join(fixture.config, "omegat.prefs.json")),
      productAtKill,
    );
    const state = await segmentedRows(fixture.paths);
    assert.equal(
      state.rows.filter((row) => row.batch_id === batchId).length,
      1,
    );
    await assertNoTemporaryFiles(fixture.paths.directory);
    return { point, killed, exactTerminalRows: 1, productReplayed: false };
  } finally {
    await Promise.all([
      terminatePackaged(recovered),
      terminatePackaged(owner),
    ]);
  }
}

async function runReplicaCrashBoundary(
  display,
  workDir,
  file,
  point,
  sequence,
) {
  const fixture = await prepareFaultFixture(
    display,
    workDir,
    `replica-boundary-${sequence}`,
  );
  const marker = join(workDir, `replica-boundary-${sequence}.marker`);
  const batchId = `replica-boundary-batch-${sequence}`;
  let owner = await launchPackagedRenderer(display, fixture.config, null, {
    ...limits,
    OMEGAT_TEST_DURABLE_FILE_NAME: file,
    OMEGAT_TEST_DURABLE_FILE_POINT: point,
    OMEGAT_TEST_DURABLE_FILE_MARKER: marker,
  });
  let recovered;
  try {
    await startRpc(owner.client, "prefs.patch", {
      theme: `replica-${sequence}`,
      config_transaction_retry_batch_id: batchId,
    });
    await waitFor(`${file} ${point}`, () => pathExists(marker));
    const productAtKill = await snapshot(join(fixture.config, "omegat.prefs.json"));
    const killed = await killPackaged(owner);
    owner = undefined;
    recovered = await launchPackagedRenderer(
      display,
      fixture.config,
      null,
      limits,
    );
    assert.equal((await rpc(recovered.client, "prefs.get")).theme, `replica-${sequence}`);
    assert.deepEqual(
      await snapshot(join(fixture.config, "omegat.prefs.json")),
      productAtKill,
    );
    const state = await segmentedRows(fixture.paths);
    assert.equal(
      state.rows.filter((row) => row.batch_id === batchId).length,
      1,
    );
    return { file, point, killed, exactTerminalRows: 1, productReplayed: false };
  } finally {
    await Promise.all([
      terminatePackaged(recovered),
      terminatePackaged(owner),
    ]);
  }
}

async function runActiveReplicaCrashBoundary(
  display,
  workDir,
  file,
  point,
  sequence,
) {
  const fixture = await prepareFaultFixture(
    display,
    workDir,
    `active-boundary-${sequence}`,
  );
  const marker = join(workDir, `active-boundary-${sequence}.marker`);
  const batchId = `active-boundary-batch-${sequence}`;
  const requestedTheme = `active-boundary-${sequence}`;
  const beforeTheme = `active-boundary-${sequence}-seed-3`;
  let owner = await launchPackagedRenderer(display, fixture.config, null, {
    ...limits,
    OMEGAT_TEST_DURABLE_FILE_NAME: file,
    OMEGAT_TEST_DURABLE_FILE_POINT: point,
    OMEGAT_TEST_DURABLE_FILE_MARKER: marker,
  });
  let recovered;
  try {
    await startRpc(owner.client, "prefs.patch", {
      theme: requestedTheme,
      config_transaction_retry_batch_id: batchId,
    });
    await waitFor(`config active ${file} ${point}`, () => pathExists(marker));
    const killed = await killPackaged(owner);
    owner = undefined;
    recovered = await launchPackagedRenderer(
      display,
      fixture.config,
      null,
      limits,
    );
    const queueWasDurable = file === "active.json"
      || point === "after_rename"
      || point === "after_parent_fsync";
    const preferences = await rpc(recovered.client, "prefs.get");
    assert.equal(
      preferences.theme,
      queueWasDurable ? requestedTheme : beforeTheme,
    );
    const state = await segmentedRows(fixture.paths);
    assert.equal(
      state.rows.filter((row) => row.batch_id === batchId).length,
      queueWasDurable ? 1 : 0,
    );
    await assertNoTemporaryFiles(fixture.paths.directory);
    return {
      file,
      point,
      killed,
      queueWasDurable,
      exactTerminalRows: queueWasDurable ? 1 : 0,
    };
  } finally {
    await Promise.all([
      terminatePackaged(recovered),
      terminatePackaged(owner),
    ]);
  }
}

async function runDualReplicaCorruption(display, workDir, kind) {
  const fixture = await prepareFaultFixture(
    display,
    workDir,
    `dual-corrupt-${kind}`,
  );
  const productBefore = await snapshot(join(fixture.config, "omegat.prefs.json"));
  const files = kind === "hot"
    ? [fixture.paths.hot, fixture.paths.hotRecovery]
    : [fixture.paths.manifest, fixture.paths.manifestRecovery];
  await Promise.all(files.map((file, index) =>
    writeFile(file, index === 0 ? "{" : "not-json", "utf8")
  ));
  let launched = await launchPackagedRenderer(
    display,
    fixture.config,
    null,
    limits,
  );
  try {
    const failed = await rpcOutcome(launched.client, "prefs.patch", {
      locale: "must-not-publish",
      config_transaction_retry_batch_id: `dual-corrupt-${kind}-batch`,
    });
    assert.equal(failed.resolved, false);
    assert.match(
      failed.error,
      kind === "hot"
        ? /both segmented history hot replicas are invalid/
        : /both segmented history manifest replicas are invalid/,
    );
    assert.deepEqual(
      await snapshot(join(fixture.config, "omegat.prefs.json")),
      productBefore,
    );
    return { kind, failedClosed: true, productMutation: false };
  } finally {
    await terminatePackaged(launched);
    launched = undefined;
  }
}

const workDir = await mkdtemp(join(tmpdir(), "omegat-unified-persistence-e2e-"));
const display = await startPackagedDisplay();
try {
  const migration = await verifyLegacyMigrationAndPrefixCollision(
    display.display,
    workDir,
  );
  const mixedQueues = await runMixedQueueTakeovers(
    display.display,
    workDir,
    migration.config,
  );
  const deletedRoot = await runDeletedRootReplacement(
    display.display,
    workDir,
    migration.config,
  );

  const historyBoundaries = [];
  let sequence = 0;
  for (const point of [
    "after_recent_append",
    "after_hot_append",
    "after_segment_candidate_write",
    "after_segment_candidate_fsync",
    "after_segment_rename",
    "after_segment_parent_fsync",
    "after_manifest_publish",
    "after_hot_prune",
    "after_generation_manifest_publish",
    "after_gc_delete",
  ]) {
    historyBoundaries.push(
      await runHistoryCrashBoundary(
        display.display,
        workDir,
        point,
        sequence++,
      ),
    );
  }

  const replicaBoundaries = [];
  for (const file of [
    ".history-hot.recovery.json",
    "history-hot.json",
    ".history-manifest.recovery.json",
    "history-manifest.json",
  ]) {
    for (const point of [
      "after_candidate_write",
      "after_candidate_fsync",
      "after_rename",
      "after_parent_fsync",
    ]) {
      replicaBoundaries.push(
        await runReplicaCrashBoundary(
          display.display,
          workDir,
          file,
          point,
          sequence++,
        ),
      );
    }
  }

  const activeReplicaBoundaries = [];
  for (const file of ["active.recovery.json", "active.json"]) {
    for (const point of [
      "after_candidate_write",
      "after_candidate_fsync",
      "after_rename",
      "after_parent_fsync",
    ]) {
      activeReplicaBoundaries.push(
        await runActiveReplicaCrashBoundary(
          display.display,
          workDir,
          file,
          point,
          sequence++,
        ),
      );
    }
  }

  const dualReplicaCorruption = [
    await runDualReplicaCorruption(display.display, workDir, "hot"),
    await runDualReplicaCorruption(display.display, workDir, "manifest"),
  ];
  console.log(JSON.stringify({
    result: "passed",
    driver: "packaged-unified-persistence-mixed-linux",
    package: executable,
    platform: process.platform,
    migration,
    mixedQueues,
    deletedRoot,
    historyBoundaries,
    replicaBoundaries,
    activeReplicaBoundaries,
    dualReplicaCorruption,
    platformsNotRun: ["windows", "macos"],
  }, null, 2));
} finally {
  await stopPackagedDisplay(display);
  if (process.env.OMEGAT_KEEP_E2E !== "1") {
    await rm(workDir, { recursive: true, force: true });
  } else {
    console.error(`kept unified persistence workdir: ${workDir}`);
  }
}
