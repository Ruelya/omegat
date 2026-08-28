// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmod,
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
  killPackagedProcess,
  launchPackagedRenderer,
  pathExists,
  sidecar,
  sleep,
  spawnPackagedApplication,
  startPackagedDisplay,
  stopPackagedDisplay,
  terminatePackaged,
  waitFor,
} from "./packaged-driver.mjs";

const durablePoints = [
  "after_candidate_write",
  "after_candidate_fsync",
  "after_rename",
  "after_parent_fsync",
];
const archivePoints = [
  "after_archive_candidate_write",
  "after_archive_candidate_fsync",
  "after_archive_rename",
  "after_archive_parent_fsync",
];

function transactionPaths(config) {
  const directory = join(config, "transactions", "shared-config");
  return {
    directory,
    active: join(directory, "active.json"),
    activeRecovery: join(directory, "active.recovery.json"),
    history: join(directory, "history.ndjson"),
    dedupe: join(directory, "dedupe.json"),
    dedupeRecovery: join(directory, "dedupe.recovery.json"),
    manifest: join(directory, "manifest.json"),
    manifestRecovery: join(directory, "manifest.recovery.json"),
    archive: join(directory, "archive"),
  };
}

function terminalEnvelope(config, id, index, version = 2) {
  return {
    version,
    config_dir: config,
    batch_id: id,
    operation: "prefs.patch",
    app_instance: `legacy-electron-${index}`,
    owner_process_id: 10_000 + index,
    status: "completed",
    payload: { theme: `legacy-theme-${index}` },
    result: { legacy_result: index },
    error: null,
    updated_unix_ms: Date.now() + index,
  };
}

function pendingEnvelope(config, id, payload, version = 1) {
  return {
    version,
    config_dir: config,
    batch_id: id,
    operation: "prefs.patch",
    app_instance: "legacy-pending-electron",
    owner_process_id: 20_000,
    status: "pending",
    payload,
    result: null,
    error: null,
    updated_unix_ms: Date.now(),
  };
}

async function writeJson(path, value) {
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

async function seedV2(configPath, count) {
  await mkdir(configPath, { recursive: true });
  const config = await realpath(configPath);
  const paths = transactionPaths(config);
  await mkdir(paths.directory, { recursive: true });
  const rows = Array.from(
    { length: count },
    (_, index) => terminalEnvelope(config, `v2-seed-${index}`, index),
  );
  const dedupe = {
    version: 2,
    config_dir: config,
    revision: 1,
    batches: Object.fromEntries(rows.map((row) => [row.batch_id, row])),
    order: rows.map((row) => row.batch_id),
    updated_unix_ms: Date.now(),
  };
  const manifest = {
    version: 2,
    config_dir: config,
    revision: 1,
    next_segment_id: 1,
    generation: 0,
    segments: [],
    batch_index: {},
    batch_index_complete: true,
    updated_unix_ms: Date.now(),
  };
  await Promise.all([
    writeJson(paths.dedupe, dedupe),
    writeJson(paths.dedupeRecovery, dedupe),
    writeJson(paths.manifest, manifest),
    writeJson(paths.manifestRecovery, manifest),
    writeFile(
      paths.history,
      rows.map((row) => JSON.stringify(row)).join("\n") + (rows.length ? "\n" : ""),
      "utf8",
    ),
  ]);
  return { config, paths, rows };
}

async function seedV1RollingMigration(configPath) {
  await mkdir(configPath, { recursive: true });
  const config = await realpath(configPath);
  const paths = transactionPaths(config);
  await mkdir(paths.directory, { recursive: true });
  const rows = Array.from(
    { length: 3 },
    (_, index) => terminalEnvelope(config, `migration-old-${index}`, index, 1),
  );
  const pending = pendingEnvelope(
    config,
    "migration-pending",
    { locale: "fr" },
  );
  await writeJson(paths.active, {
    version: 1,
    config_dir: config,
    batches: [pending],
    updated_unix_ms: Date.now(),
  });
  await writeFile(
    paths.history,
    `${rows.map((row) => JSON.stringify(row)).join("\n")}\n`,
    "utf8",
  );
  return { config, paths, rows, pending };
}

async function invokeRpcResult(client, method, params) {
  return client.evaluate(`(() => window.omegat.rpc(
    ${JSON.stringify(method)},
    ${JSON.stringify(params)}
  ).then(
    (value) => ({ resolved: true, value }),
    (error) => ({ resolved: false, error: String(error) })
  ))()`, true);
}

async function startRpc(client, method, params) {
  const started = await client.evaluate(`(() => {
    window.__omegatSharedConfigV2Request = window.omegat.rpc(
      ${JSON.stringify(method)},
      ${JSON.stringify(params)}
    );
    window.__omegatSharedConfigV2Request.catch(() => {});
    return true;
  })()`);
  assert.equal(started, true);
}

async function snapshot(path) {
  const info = await stat(path, { bigint: true });
  return {
    bytes: await readFile(path),
    mtimeNs: info.mtimeNs,
    size: info.size,
  };
}

async function assertNoCandidates(directory) {
  const pending = [];
  if (!await pathExists(directory)) return;
  for (const name of await readdir(directory, { recursive: true })) {
    if (
      name.endsWith(".tmp")
      || name.includes(".archive-segment.")
    ) {
      pending.push(name);
    }
  }
  assert.deepEqual(pending, []);
}

async function inspectV2(paths, expectedIds, hotLimit) {
  assert.equal(await pathExists(paths.active), false);
  assert.equal(await pathExists(paths.activeRecovery), false);
  const [dedupeBytes, dedupeRecoveryBytes, manifestBytes, manifestRecoveryBytes] =
    await Promise.all([
      readFile(paths.dedupe),
      readFile(paths.dedupeRecovery),
      readFile(paths.manifest),
      readFile(paths.manifestRecovery),
    ]);
  assert.deepEqual(dedupeBytes, dedupeRecoveryBytes);
  assert.deepEqual(manifestBytes, manifestRecoveryBytes);
  const dedupe = JSON.parse(dedupeBytes);
  const manifest = JSON.parse(manifestBytes);
  assert.equal(dedupe.version, 2);
  assert.equal(manifest.version, 2);
  assert.equal(manifest.batch_index_complete, true);
  assert(Number.isSafeInteger(manifest.generation));
  assert(dedupe.order.length <= hotLimit);
  assert.deepEqual(Object.keys(dedupe.batches).sort(), [...dedupe.order].sort());
  const archived = [];
  const immutable = {};
  for (const descriptor of manifest.segments) {
    const path = join(paths.archive, descriptor.file);
    const bytes = await readFile(path);
    immutable[descriptor.file] = bytes;
    const segment = JSON.parse(bytes);
    assert.equal(segment.version, 2);
    assert.equal(segment.id, descriptor.id);
    assert.equal(segment.generation ?? 0, descriptor.generation ?? 0);
    assert.equal(segment.batches.length, descriptor.batch_count);
    for (const row of segment.batches) {
      const prefix = createHash("sha256")
        .update(row.batch_id)
        .digest("hex")
        .slice(0, 4);
      assert(
        manifest.batch_index[prefix].includes(descriptor.id),
        `manifest index does not locate ${row.batch_id}`,
      );
    }
    archived.push(...segment.batches);
  }
  const all = [
    ...archived.map((row) => row.batch_id),
    ...dedupe.order,
  ];
  assert.deepEqual(all, expectedIds);
  const history = (await readFile(paths.history, "utf8"))
    .trim()
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
  assert(history.every((row) => row.version === 2));
  await assertNoCandidates(paths.directory);
  return { dedupe, manifest, immutable, history };
}

function storageEnv() {
  return {
    OMEGAT_TEST_CONFIG_HISTORY_LIMIT: "2",
    OMEGAT_TEST_CONFIG_DEDUPE_HOT_LIMIT: "2",
    OMEGAT_TEST_CONFIG_ARCHIVE_SEGMENT_LIMIT: "1",
  };
}

async function runRollingMigration(display, workDir) {
  const seeded = await seedV1RollingMigration(join(workDir, "rolling-config"));
  const firstMarker = join(workDir, "rolling-first.marker");
  const secondMarker = join(workDir, "rolling-second.marker");
  const first = spawnPackagedApplication(
    display,
    seeded.config,
    null,
    {
      ...storageEnv(),
      OMEGAT_TEST_CONFIG_TRANSACTION_OPERATION: "prefs.patch",
      OMEGAT_TEST_CONFIG_TRANSACTION_POINT: "after_archive_rename",
      OMEGAT_TEST_CONFIG_TRANSACTION_MARKER: firstMarker,
    },
  );
  await waitFor("first rolling migration archive rename", () =>
    pathExists(firstMarker)
  );
  const second = spawnPackagedApplication(
    display,
    seeded.config,
    null,
    {
      ...storageEnv(),
      OMEGAT_TEST_CONFIG_TRANSACTION_OPERATION: "prefs.patch",
      OMEGAT_TEST_CONFIG_TRANSACTION_POINT: "after_history_compaction",
      OMEGAT_TEST_CONFIG_TRANSACTION_MARKER: secondMarker,
    },
  );
  await sleep(200);
  assert.equal(
    await pathExists(secondMarker),
    false,
    "second Electron bypassed the first process lock",
  );
  const firstKilled = await killPackagedProcess(first);
  await waitFor("second rolling migration compaction", () =>
    pathExists(secondMarker)
  );
  const secondKilled = await killPackagedProcess(second);

  const recovered = await launchPackagedRenderer(
    display,
    seeded.config,
    null,
    storageEnv(),
  );
  try {
    const prefs = await invokeRpcResult(recovered.client, "prefs.get", {});
    assert.equal(prefs.resolved, true, prefs.error);
    assert.equal(prefs.value.locale, "fr");
    const productBeforeRetry = await snapshot(
      join(seeded.config, "omegat.prefs.json"),
    );
    const retry = await invokeRpcResult(recovered.client, "prefs.patch", {
      theme: "legacy-theme-0",
      config_transaction_retry_batch_id: "migration-old-0",
    });
    assert.deepEqual(retry, {
      resolved: true,
      value: { legacy_result: 0 },
    });
    const conflict = await invokeRpcResult(recovered.client, "prefs.patch", {
      theme: "conflict",
      config_transaction_retry_batch_id: "migration-old-0",
    });
    assert.equal(conflict.resolved, false);
    assert.match(conflict.error, /reused for a different operation or payload/);
    const productAfterRetry = await snapshot(
      join(seeded.config, "omegat.prefs.json"),
    );
    assert.deepEqual(productAfterRetry, productBeforeRetry);
    const expectedIds = [
      ...seeded.rows.map((row) => row.batch_id),
      seeded.pending.batch_id,
    ];
    const state = await inspectV2(seeded.paths, expectedIds, 2);
    for (const [file, bytes] of Object.entries(state.immutable)) {
      assert.deepEqual(await readFile(join(seeded.paths.archive, file)), bytes);
    }
    return {
      firstKilled,
      secondKilled,
      expectedIds,
      v1IndexWasMissing: true,
      activeAndHistoryMigratedToV2: true,
      orphanArchiveAdopted: true,
      exactOldRetry: true,
      conflictRejected: true,
      immutableSegments: state.manifest.segments.length,
    };
  } finally {
    await terminatePackaged(recovered);
  }
}

async function runDurableBoundary(
  display,
  workDir,
  file,
  point,
  sequence,
) {
  const isManifest = file.startsWith("manifest");
  const seeded = await seedV2(
    join(workDir, `boundary-${sequence}`),
    isManifest ? 2 : 0,
  );
  const marker = join(workDir, `boundary-${sequence}.marker`);
  let owner = await launchPackagedRenderer(
    display,
    seeded.config,
    null,
    {
      ...storageEnv(),
      OMEGAT_TEST_DURABLE_FILE_NAME: file,
      OMEGAT_TEST_DURABLE_FILE_POINT: point,
      OMEGAT_TEST_DURABLE_FILE_MARKER: marker,
    },
  );
  const batchId = `boundary-batch-${sequence}`;
  let recovery;
  try {
    await startRpc(owner.client, "prefs.patch", {
      locale: `boundary-${sequence}`,
      config_transaction_retry_batch_id: batchId,
    });
    await waitFor(`${file} ${point}`, () => pathExists(marker));
    const killed = await killPackaged(owner);
    owner = undefined;
    const productAtKill = await snapshot(join(seeded.config, "omegat.prefs.json"));
    recovery = await launchPackagedRenderer(
      display,
      seeded.config,
      null,
      storageEnv(),
    );
    const prefs = await invokeRpcResult(recovery.client, "prefs.get", {});
    assert.equal(prefs.resolved, true, prefs.error);
    assert.equal(prefs.value.locale, `boundary-${sequence}`);
    assert.deepEqual(
      await snapshot(join(seeded.config, "omegat.prefs.json")),
      productAtKill,
    );
    const expectedIds = [
      ...seeded.rows.map((row) => row.batch_id),
      batchId,
    ];
    await inspectV2(seeded.paths, expectedIds, 2);
    return { file, point, batchId, killed, exactlyOnce: true };
  } finally {
    await Promise.all([
      terminatePackaged(owner),
      terminatePackaged(recovery),
    ]);
  }
}

async function runArchiveBoundary(display, workDir, point, sequence) {
  const seeded = await seedV2(join(workDir, `archive-${sequence}`), 2);
  const marker = join(workDir, `archive-${sequence}.marker`);
  const batchId = `archive-batch-${sequence}`;
  let owner = await launchPackagedRenderer(
    display,
    seeded.config,
    null,
    {
      ...storageEnv(),
      OMEGAT_TEST_CONFIG_TRANSACTION_OPERATION: "prefs.patch",
      OMEGAT_TEST_CONFIG_TRANSACTION_POINT: point,
      OMEGAT_TEST_CONFIG_TRANSACTION_MARKER: marker,
    },
  );
  let recovery;
  try {
    await startRpc(owner.client, "prefs.patch", {
      locale: `archive-${sequence}`,
      config_transaction_retry_batch_id: batchId,
    });
    await waitFor(point, () => pathExists(marker));
    const killed = await killPackaged(owner);
    owner = undefined;
    const productAtKill = await snapshot(join(seeded.config, "omegat.prefs.json"));
    recovery = await launchPackagedRenderer(
      display,
      seeded.config,
      null,
      storageEnv(),
    );
    assert.deepEqual(
      await snapshot(join(seeded.config, "omegat.prefs.json")),
      productAtKill,
    );
    await inspectV2(
      seeded.paths,
      [...seeded.rows.map((row) => row.batch_id), batchId],
      2,
    );
    return { point, batchId, killed, exactlyOnce: true };
  } finally {
    await Promise.all([
      terminatePackaged(owner),
      terminatePackaged(recovery),
    ]);
  }
}

async function createArchivedConfig(display, workDir, name) {
  const seeded = await seedV2(join(workDir, name), 2);
  const batchId = `${name}-terminal`;
  const owner = await launchPackagedRenderer(
    display,
    seeded.config,
    null,
    storageEnv(),
  );
  try {
    const result = await invokeRpcResult(owner.client, "prefs.patch", {
      locale: name,
      config_transaction_retry_batch_id: batchId,
    });
    assert.equal(result.resolved, true, result.error);
  } finally {
    await terminatePackaged(owner);
  }
  const manifest = JSON.parse(await readFile(seeded.paths.manifest, "utf8"));
  assert(manifest.segments.length > 0);
  return {
    ...seeded,
    batchId,
    expectedIds: [...seeded.rows.map((row) => row.batch_id), batchId],
    manifest,
  };
}

async function runArchiveIntegrityMatrix(display, workDir) {
  const outcomes = [];
  for (const fault of ["missing-segment", "hash-tamper", "manifest-conflict"]) {
    const seeded = await createArchivedConfig(
      display,
      workDir,
      `integrity-${fault}`,
    );
    const productBefore = await snapshot(join(seeded.config, "omegat.prefs.json"));
    if (fault === "missing-segment") {
      await rm(join(
        seeded.paths.archive,
        seeded.manifest.segments[0].file,
      ));
    } else if (fault === "hash-tamper") {
      const segmentPath = join(
        seeded.paths.archive,
        seeded.manifest.segments[0].file,
      );
      await writeFile(
        segmentPath,
        Buffer.concat([await readFile(segmentPath), Buffer.from(" ")]),
      );
    } else {
      const conflicting = structuredClone(seeded.manifest);
      conflicting.updated_unix_ms += 1;
      await writeJson(seeded.paths.manifestRecovery, conflicting);
    }

    const recovery = await launchPackagedRenderer(
      display,
      seeded.config,
      null,
      storageEnv(),
    );
    try {
      const failed = await invokeRpcResult(recovery.client, "prefs.patch", {
        theme: "legacy-theme-0",
        config_transaction_retry_batch_id: "v2-seed-0",
      });
      assert.equal(failed.resolved, false);
      if (fault === "missing-segment") {
        assert.match(failed.error, /missing segment/);
      } else if (fault === "hash-tamper") {
        assert.match(failed.error, /digest mismatch|descriptor disagrees/);
      } else {
        assert.match(failed.error, /replicas disagree at revision/);
      }
      assert.deepEqual(
        await snapshot(join(seeded.config, "omegat.prefs.json")),
        productBefore,
      );
      outcomes.push({ fault, failedClosed: true, productMutation: false });
    } finally {
      await terminatePackaged(recovery);
    }
  }
  return outcomes;
}

async function runConfigRelocation(display, workDir) {
  const seeded = await createArchivedConfig(
    display,
    workDir,
    "relocation-before",
  );
  const immutable = Object.fromEntries(await Promise.all(
    seeded.manifest.segments.map(async (descriptor) => [
      descriptor.file,
      await readFile(join(seeded.paths.archive, descriptor.file)),
    ]),
  ));
  const movedPath = join(workDir, "relocation-after");
  await rename(seeded.config, movedPath);
  const config = await realpath(movedPath);
  const paths = transactionPaths(config);
  const recovery = await launchPackagedRenderer(
    display,
    config,
    null,
    storageEnv(),
  );
  try {
    const relocatedPreferences = await invokeRpcResult(
      recovery.client,
      "prefs.get",
      {},
    );
    assert.equal(
      relocatedPreferences.resolved,
      true,
      relocatedPreferences.error,
    );
    assert.equal(relocatedPreferences.value.config_dir, config);
    const state = await inspectV2(paths, seeded.expectedIds, 2);
    assert.equal(state.dedupe.config_dir, config);
    assert.equal(state.manifest.config_dir, config);
    const retry = await invokeRpcResult(recovery.client, "prefs.patch", {
      theme: "legacy-theme-0",
      config_transaction_retry_batch_id: "v2-seed-0",
    });
    assert.deepEqual(retry, {
      resolved: true,
      value: { legacy_result: 0 },
    });
    for (const [file, bytes] of Object.entries(immutable)) {
      assert.deepEqual(await readFile(join(paths.archive, file)), bytes);
    }
    return {
      exactArchivedRetry: true,
      mutableMetadataRebased: true,
      immutableSegmentsRewritten: false,
    };
  } finally {
    await terminatePackaged(recovery);
  }
}

async function runConsecutiveGcOwnerDeaths(display, workDir) {
  const seeded = await seedV2(join(workDir, "generation-gc"), 5);
  const batchId = "generation-gc-terminal";
  const environment = {
    ...storageEnv(),
    OMEGAT_TEST_CONFIG_ARCHIVE_COMPACTION_SEGMENT_LIMIT: "4",
    OMEGAT_TEST_CONFIG_ARCHIVE_COMPACTION_BATCH_LIMIT: "16",
    OMEGAT_TEST_CONFIG_TRANSACTION_OPERATION: "prefs.patch",
    OMEGAT_TEST_CONFIG_TRANSACTION_POINT: "after_archive_gc_delete",
  };
  const killedOwners = [];

  for (let ownerIndex = 0; ownerIndex < 3; ownerIndex += 1) {
    const marker = join(workDir, `generation-gc-owner-${ownerIndex}.marker`);
    const owner = await launchPackagedRenderer(
      display,
      seeded.config,
      null,
      {
        ...environment,
        OMEGAT_TEST_CONFIG_TRANSACTION_MARKER: marker,
      },
    );
    if (ownerIndex === 0) {
      await startRpc(owner.client, "prefs.patch", {
        locale: "generation-gc",
        config_transaction_retry_batch_id: batchId,
      });
    } else {
      await startRpc(owner.client, "prefs.get", {});
    }
    await waitFor(`generation GC owner ${ownerIndex}`, () => pathExists(marker));
    const manifestBytes = await readFile(seeded.paths.manifest);
    assert.deepEqual(
      manifestBytes,
      await readFile(seeded.paths.manifestRecovery),
      "predecessor GC began before both replacement manifests were durable",
    );
    const manifest = JSON.parse(manifestBytes);
    assert.equal(manifest.generation, 1);
    killedOwners.push(await killPackaged(owner));
  }

  const productAtLastKill = await snapshot(
    join(seeded.config, "omegat.prefs.json"),
  );
  const recovery = await launchPackagedRenderer(
    display,
    seeded.config,
    null,
    {
      ...storageEnv(),
      OMEGAT_TEST_CONFIG_ARCHIVE_COMPACTION_SEGMENT_LIMIT: "4",
      OMEGAT_TEST_CONFIG_ARCHIVE_COMPACTION_BATCH_LIMIT: "16",
    },
  );
  try {
    const prefs = await invokeRpcResult(recovery.client, "prefs.get", {});
    assert.equal(prefs.resolved, true, prefs.error);
    assert.equal(prefs.value.locale, "generation-gc");
    assert.deepEqual(
      await snapshot(join(seeded.config, "omegat.prefs.json")),
      productAtLastKill,
    );
    const state = await inspectV2(
      seeded.paths,
      [...seeded.rows.map((row) => row.batch_id), batchId],
      2,
    );
    assert.equal(state.manifest.generation, 1);
    assert.equal(state.manifest.segments.length, 1);
    assert.deepEqual(
      (await readdir(seeded.paths.archive))
        .filter((name) => name.startsWith("segment-"))
        .sort(),
      state.manifest.segments.map((descriptor) => descriptor.file).sort(),
    );
    return {
      killedOwners,
      replacementGeneration: state.manifest.generation,
      predecessorSegmentsAfterRecovery: 0,
      productReplayed: false,
      exactBatchCount: seeded.rows.length + 1,
    };
  } finally {
    await terminatePackaged(recovery);
  }
}

async function compileIoFaultShim(workDir) {
  const source = join(workDir, "config-v2-io-fault.c");
  const library = join(workDir, "config-v2-io-fault.so");
  await writeFile(
    source,
    String.raw`
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static int selected_fd(int fd) {
  const char *needle = getenv("OMEGAT_TEST_IO_FAULT_PATH");
  char link_path[64];
  char target[PATH_MAX + 1];
  if (!needle) return 0;
  snprintf(link_path, sizeof(link_path), "/proc/self/fd/%d", fd);
  ssize_t length = readlink(link_path, target, PATH_MAX);
  if (length < 0) return 0;
  target[length] = '\0';
  return strstr(target, needle) != NULL;
}

ssize_t write(int fd, const void *buffer, size_t count) {
  static ssize_t (*real_write)(int, const void *, size_t) = NULL;
  if (!real_write) real_write = dlsym(RTLD_NEXT, "write");
  const char *kind = getenv("OMEGAT_TEST_IO_FAULT");
  if (kind && selected_fd(fd)) {
    errno = strcmp(kind, "enospc") == 0 ? ENOSPC : EACCES;
    return -1;
  }
  return real_write(fd, buffer, count);
}
`,
    "utf8",
  );
  const compiler = spawn(
    "cc",
    ["-shared", "-fPIC", "-O2", "-o", library, source, "-ldl"],
    { stdio: ["ignore", "ignore", "pipe"] },
  );
  let stderr = "";
  compiler.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });
  const code = await new Promise((resolveExit, reject) => {
    compiler.once("error", reject);
    compiler.once("exit", resolveExit);
  });
  assert.equal(code, 0, `cannot compile config v2 I/O shim: ${stderr}`);
  return library;
}

async function runInjectedFailure(
  display,
  workDir,
  shim,
  kind,
  file,
  sequence,
) {
  const seeded = await seedV2(join(workDir, `failure-${sequence}`), 2);
  const batchId = `failure-batch-${sequence}`;
  let owner = await launchPackagedRenderer(
    display,
    seeded.config,
    null,
    {
      ...storageEnv(),
      LD_PRELOAD: shim,
      OMEGAT_TEST_IO_FAULT: kind,
      OMEGAT_TEST_IO_FAULT_PATH: file,
    },
  );
  let recovery;
  try {
    const failed = await invokeRpcResult(owner.client, "prefs.patch", {
      locale: `${kind}-${sequence}`,
      config_transaction_retry_batch_id: batchId,
    });
    assert.equal(failed.resolved, false);
    assert.match(
      failed.error,
      kind === "enospc" ? /space|os error 28/i : /permission|os error 13/i,
    );
    const productAtFailure = await snapshot(
      join(seeded.config, "omegat.prefs.json"),
    );
    await terminatePackaged(owner);
    owner = undefined;
    recovery = await launchPackagedRenderer(
      display,
      seeded.config,
      null,
      storageEnv(),
    );
    assert.deepEqual(
      await snapshot(join(seeded.config, "omegat.prefs.json")),
      productAtFailure,
    );
    await inspectV2(
      seeded.paths,
      [...seeded.rows.map((row) => row.batch_id), batchId],
      2,
    );
    return {
      kind,
      file,
      productStableAcrossRecovery: true,
      exactlyOnce: true,
    };
  } finally {
    await Promise.all([
      terminatePackaged(owner),
      terminatePackaged(recovery),
    ]);
  }
}

async function runRealPermissionFailure(display, workDir) {
  const seeded = await seedV2(join(workDir, "permission-real"), 0);
  let owner = await launchPackagedRenderer(
    display,
    seeded.config,
    null,
    storageEnv(),
  );
  try {
    const productBefore = await snapshot(
      join(seeded.config, "omegat.prefs.json"),
    );
    await chmod(seeded.paths.directory, 0o555);
    const failed = await invokeRpcResult(owner.client, "prefs.patch", {
      locale: "must-not-publish",
      config_transaction_retry_batch_id: "permission-real-batch",
    });
    assert.equal(failed.resolved, false);
    assert.match(failed.error, /permission|os error 13/i);
    assert.deepEqual(
      await snapshot(join(seeded.config, "omegat.prefs.json")),
      productBefore,
    );
    return { realDirectoryMode: "0555", productMutation: false };
  } finally {
    await chmod(seeded.paths.directory, 0o755).catch(() => {});
    await terminatePackaged(owner);
  }
}

if (process.platform !== "linux") {
  throw new Error(
    "This packaged shared-config v2 evidence currently runs on Linux; "
      + "the driver also resolves Windows and macOS package layouts",
  );
}
await Promise.all([
  stat(executable),
  stat(sidecar),
]);

const workDir = await mkdtemp(join(tmpdir(), "omegat-shared-config-v2-e2e-"));
const display = await startPackagedDisplay();
try {
  const rollingMigration = await runRollingMigration(display.display, workDir);
  const archiveIntegrity = await runArchiveIntegrityMatrix(
    display.display,
    workDir,
  );
  const configRelocation = await runConfigRelocation(display.display, workDir);
  const consecutiveGcOwnerDeaths = await runConsecutiveGcOwnerDeaths(
    display.display,
    workDir,
  );
  const durableBoundaries = [];
  let sequence = 0;
  for (const file of [
    "dedupe.recovery.json",
    "dedupe.json",
    "manifest.recovery.json",
    "manifest.json",
  ]) {
    for (const point of durablePoints) {
      durableBoundaries.push(
        await runDurableBoundary(
          display.display,
          workDir,
          file,
          point,
          sequence++,
        ),
      );
    }
  }
  const archiveBoundaries = [];
  for (const point of archivePoints) {
    archiveBoundaries.push(
      await runArchiveBoundary(display.display, workDir, point, sequence++),
    );
  }
  const shim = await compileIoFaultShim(workDir);
  const failures = [
    await runInjectedFailure(
      display.display,
      workDir,
      shim,
      "enospc",
      "dedupe.json",
      sequence++,
    ),
    await runInjectedFailure(
      display.display,
      workDir,
      shim,
      "enospc",
      "manifest.json",
      sequence++,
    ),
    await runInjectedFailure(
      display.display,
      workDir,
      shim,
      "eacces",
      "manifest.json",
      sequence++,
    ),
    await runRealPermissionFailure(display.display, workDir),
  ];
  console.log(JSON.stringify({
    result: "passed",
    package: executable,
    platform: process.platform,
    driver: "scripts/packaged-driver.mjs",
    rollingMigration,
    archiveIntegrity,
    configRelocation,
    consecutiveGcOwnerDeaths,
    durableBoundaries,
    archiveBoundaries,
    failures,
    platformsNotRun: ["windows", "macos"],
  }));
} finally {
  await stopPackagedDisplay(display);
  await rm(workDir, { recursive: true, force: true });
}
