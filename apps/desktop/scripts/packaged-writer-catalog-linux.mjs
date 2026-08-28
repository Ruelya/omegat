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
import { isAbsolute, join, relative } from "node:path";
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
  throw new Error("The generated writer catalog matrix requires a Linux runner");
}
await Promise.all([stat(executable), stat(sidecar)]);

const sourceRegistry = JSON.parse(
  await readFile(
    new URL("../../../crates/omegat-ipc/rpc-methods.json", import.meta.url),
    "utf8",
  ),
);
const sourceCatalog = sourceRegistry.methods.flatMap(({ method, writer }) =>
  writer ? [{ method, ...writer }] : []
);
assert.equal(sourceCatalog.length, 26);
assert.equal(new Set(sourceCatalog.map(({ method }) => method)).size, 26);

const limits = {
  OMEGAT_TEST_CONFIG_HISTORY_LIMIT: "1",
  OMEGAT_TEST_CONFIG_DEDUPE_HOT_LIMIT: "1",
  OMEGAT_TEST_CONFIG_ARCHIVE_SEGMENT_LIMIT: "1",
  OMEGAT_TEST_CONFIG_ARCHIVE_COMPACTION_SEGMENT_LIMIT: "2",
  OMEGAT_TEST_CONFIG_ARCHIVE_COMPACTION_BATCH_LIMIT: "8",
  OMEGAT_TEST_CONFIG_ARCHIVE_BATCH_PREFIX_HEX: "1",
  OMEGAT_TEST_PRODUCT_HISTORY_RECENT_LIMIT: "1",
  OMEGAT_TEST_PRODUCT_HISTORY_HOT_LIMIT: "1",
  OMEGAT_TEST_PRODUCT_HISTORY_SEGMENT_LIMIT: "1",
  OMEGAT_TEST_PRODUCT_HISTORY_COMPACTION_SEGMENTS: "2",
  OMEGAT_TEST_PRODUCT_HISTORY_COMPACTION_RECORDS: "8",
  OMEGAT_TEST_PRODUCT_HISTORY_PREFIX_HEX: "1",
};

const rpc = (client, method, params = {}) =>
  client.evaluate(
    `window.omegat.rpc(${JSON.stringify(method)}, ${JSON.stringify(params)})`,
    true,
  );

const rpcWithReceipt = (client, method, params = {}, requestId = undefined) =>
  client.evaluate(
    `window.omegat.rpcWithTransactionReceipt(
      ${JSON.stringify(method)},
      ${JSON.stringify(params)},
      ${JSON.stringify(requestId)}
    )`,
    true,
  );

function parseNdjson(raw) {
  return raw
    .trim()
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

async function snapshotFile(path) {
  try {
    const info = await stat(path, { bigint: true });
    assert(info.isFile(), `${path} is not a regular file`);
    return {
      exists: true,
      bytes: (await readFile(path)).toString("base64"),
      mtimeNs: info.mtimeNs.toString(),
      size: info.size.toString(),
    };
  } catch (error) {
    if (error.code === "ENOENT") return { exists: false };
    throw error;
  }
}

async function snapshotFiles(paths) {
  return Object.fromEntries(
    await Promise.all(
      [...new Set(paths)].map(async (path) => [path, await snapshotFile(path)]),
    ),
  );
}

async function assertExactSnapshots(expected, label) {
  for (const [path, snapshot] of Object.entries(expected)) {
    assert.deepEqual(await snapshotFile(path), snapshot, `${label}: ${path}`);
  }
}

function snapshotsChanged(before, after) {
  return Object.keys(after).some((path) =>
    JSON.stringify(before[path]) !== JSON.stringify(after[path])
  );
}

async function treeBytes(root) {
  if (!await pathExists(root)) return {};
  const files = (await readdir(root, { recursive: true }))
    .map((name) => join(root, name));
  const rows = [];
  for (const path of files) {
    const info = await stat(path);
    if (!info.isFile()) continue;
    rows.push([
      relative(root, path),
      (await readFile(path)).toString("base64"),
    ]);
  }
  rows.sort(([left], [right]) => left.localeCompare(right));
  return Object.fromEntries(rows);
}

function historyPaths(directory) {
  return {
    directory,
    recent: join(directory, "history.ndjson"),
    hot: join(directory, "history-hot.json"),
    hotRecovery: join(directory, ".history-hot.recovery.json"),
    manifest: join(directory, "history-manifest.json"),
    manifestRecovery: join(directory, ".history-manifest.recovery.json"),
    archive: join(directory, "history-archive"),
  };
}

function projectHistoryPaths(project) {
  return historyPaths(join(project, ".repositories", "transactions"));
}

function configHistoryPaths(config) {
  return historyPaths(join(config, "transactions", "shared-config"));
}

async function segmentedRows(paths) {
  if (!await pathExists(paths.manifest)) {
    return parseNdjson(await readFile(paths.recent, "utf8"));
  }
  const [manifest, manifestRecovery, hot, hotRecovery] = await Promise.all([
    readFile(paths.manifest, "utf8").then(JSON.parse),
    readFile(paths.manifestRecovery, "utf8").then(JSON.parse),
    readFile(paths.hot, "utf8").then(JSON.parse),
    readFile(paths.hotRecovery, "utf8").then(JSON.parse),
  ]);
  assert.deepEqual(manifestRecovery, manifest, "manifest replicas diverged");
  assert.deepEqual(hotRecovery, hot, "hot replicas diverged");
  const rows = [];
  for (const descriptor of [...manifest.segments].sort((a, b) => a.id - b.id)) {
    const segment = JSON.parse(
      await readFile(join(paths.archive, descriptor.file), "utf8"),
    );
    assert.equal(segment.id, descriptor.id);
    assert.equal(segment.records.length, descriptor.record_count);
    rows.push(...segment.records);
  }
  rows.push(...hot.records);
  return rows;
}

const terminalRows = (rows) =>
  rows.filter(({ status }) => status !== "pending");

async function assertOneTerminal(paths, batchId, operation, status = "completed") {
  const rows = await segmentedRows(paths);
  const exact = terminalRows(rows)
    .filter(({ batch_id }) => batch_id === batchId);
  assert.equal(exact.length, 1, `${batchId} must have one terminal row`);
  assert.equal(exact[0].status, status, batchId);
  assert.equal(
    exact[0].operation ?? exact[0].payload?.operation,
    operation,
    batchId,
  );
  return exact[0];
}

async function assertNoCandidates(directory) {
  const names = await readdir(directory, { recursive: true });
  assert.deepEqual(
    names.filter((name) => name.endsWith(".tmp") || name.endsWith(".candidate")),
    [],
    `temporary transaction files remain under ${directory}`,
  );
}

function resolvedOperation(writer, params) {
  return writer.journal_operation.replace(
    "{which}",
    typeof params.which === "string" ? params.which : "target",
  );
}

async function pauseWatcherForWrite(launched, project, write) {
  const state = await workspaceState(launched.client);
  assert(state.generation > 0);
  await launched.client.evaluate("window.omegat.unwatchProject()", true);
  await write();
  await launched.client.evaluate(
    `window.omegat.watchProject(
      ${JSON.stringify(project)},
      ${JSON.stringify(state.generation)}
    )`,
    true,
  );
}

async function prepareFixture(display, workDir) {
  const config = join(workDir, "config");
  const project = join(workDir, "project");
  const remote = join(workDir, "remote");
  const alignSource = join(workDir, "align-source.txt");
  const alignTarget = join(workDir, "align-target.txt");
  const wikiSource = join(workDir, "catalog-wiki.txt");
  const importSource = join(workDir, "catalog-import.txt");
  const scriptDir = join(config, "scripts");
  await Promise.all([
    mkdir(join(remote, "source"), { recursive: true }),
    mkdir(join(remote, "target"), { recursive: true }),
    mkdir(scriptDir, { recursive: true }),
  ]);
  await Promise.all([
    writeFile(join(remote, "source", "team-sync.txt"), "catalog remote sync", "utf8"),
    writeFile(join(remote, "target", "team-commit.txt"), "catalog remote before", "utf8"),
    writeFile(alignSource, "One line\nTwo line\n", "utf8"),
    writeFile(alignTarget, "Une ligne\nDeux lignes\n", "utf8"),
    writeFile(wikiSource, "catalog wiki product", "utf8"),
    writeFile(importSource, "catalog imported product", "utf8"),
    writeFile(
      join(scriptDir, "slot01.js"),
      "editor.setTranslation('catalog slot product'); project.save();",
      "utf8",
    ),
  ]);

  let setup = await launchPackagedRenderer(display, config, null, limits);
  try {
    await rpc(setup.client, "project.create", {
      root: project,
      source_lang: "en",
      target_lang: "fr",
      sentence_seg: false,
    });
    await writeFile(
      join(project, "source", "source.txt"),
      "catalog initial source",
      "utf8",
    );
    await rpc(setup.client, "project.reload");
    const entry = await rpc(setup.client, "entry.get", { index: 0 });
    await rpc(setup.client, "entry.set", {
      index: 0,
      key: entry.key,
      translation: "catalog initial translation",
      note: "",
      revision: entry.revision,
      default_translation: true,
    });
    await rpc(setup.client, "team.mapping", {
      repositories: [{
        repo_type: "file",
        url: remote,
        branch: null,
        mappings: [
          {
            local: "/source/team-sync.txt",
            repository: "/source/team-sync.txt",
            includes: [],
            excludes: [],
          },
          {
            local: "/target/team-commit.txt",
            repository: "/target/team-commit.txt",
            includes: [],
            excludes: [],
          },
        ],
      }],
    });
    await rpc(setup.client, "prefs.patch", {
      script_dir: scriptDir,
      config_transaction_retry_batch_id: "catalog-setup-script-dir",
    });
  } finally {
    await terminatePackaged(setup);
    setup = undefined;
  }

  return {
    workDir,
    config,
    project,
    remote,
    scriptDir,
    alignSource,
    alignTarget,
    wikiSource,
    importSource,
    sourceMain: join(project, "source", "source.txt"),
    saveTmx: join(project, "omegat", "project_save.tmx"),
    projectFile: join(project, "omegat.project"),
    glossaryFile: join(project, "glossary", "glossary.txt"),
    learnedWords: join(project, "omegat", "learned_words.txt"),
    ignoredWords: join(project, "omegat", "ignored_words.txt"),
    prefsFile: join(config, "omegat.prefs.json"),
    traceFile: join(workDir, "catalog-acks.ndjson"),
    projectProducts: new Set(),
  };
}

const projectDrivers = {
  "project.close": {
    prepare: async (launched) =>
      rpc(launched.client, "script.run", {
        index: 0,
        source: "editor.setTranslation('catalog close product');",
      }),
    params: async () => ({}),
    products: (fixture) => [fixture.saveTmx],
    verify: async (client) => {
      const entry = await rpc(client, "entry.get", { index: 0 });
      assert.equal(entry.translation, "catalog close product");
    },
  },
  "project.save": {
    prepare: async (launched) =>
      rpc(launched.client, "script.run", {
        index: 0,
        source: "editor.setTranslation('catalog save product');",
      }),
    params: async () => ({}),
    products: (fixture) => [fixture.saveTmx],
    verify: async (client) => {
      assert.equal(
        (await rpc(client, "entry.get", { index: 0 })).translation,
        "catalog save product",
      );
    },
  },
  "project.compile": {
    prepare: async (launched) =>
      rpc(launched.client, "script.run", {
        index: 0,
        source: "editor.setTranslation('catalog compiled product');",
      }),
    params: async () => ({}),
    products: (fixture) => [join(fixture.project, "target", "source.txt")],
    verify: async (_client, fixture) => {
      assert.match(
        await readFile(join(fixture.project, "target", "source.txt"), "utf8"),
        /catalog compiled product/,
      );
    },
  },
  "project.reload": {
    prepare: async (launched, fixture) =>
      pauseWatcherForWrite(launched, fixture.project, () =>
        writeFile(fixture.sourceMain, "catalog reload source", "utf8")
      ),
    params: async () => ({}),
    products: (fixture) => [fixture.sourceMain],
    stableInput: true,
    verify: async (client) => {
      assert.equal(
        (await rpc(client, "entry.get", { index: 0 })).source,
        "catalog reload source",
      );
    },
  },
  "project.external-refresh": {
    prepare: async (launched, fixture) =>
      pauseWatcherForWrite(launched, fixture.project, () =>
        writeFile(fixture.sourceMain, "catalog external refresh source", "utf8")
      ),
    params: async () => ({}),
    products: (fixture) => [fixture.sourceMain],
    stableInput: true,
    verify: async (client) => {
      assert.equal(
        (await rpc(client, "entry.get", { index: 0 })).source,
        "catalog external refresh source",
      );
    },
  },
  "entry.set": {
    params: async (client) => {
      const entry = await rpc(client, "entry.get", { index: 0 });
      return {
        index: 0,
        key: entry.key,
        translation: "catalog entry product",
        note: "catalog entry note",
        revision: entry.revision,
        default_translation: true,
      };
    },
    products: (fixture) => [fixture.saveTmx],
    verify: async (client) => {
      const entry = await rpc(client, "entry.get", { index: 0 });
      assert.equal(entry.translation, "catalog entry product");
      assert.equal(entry.note, "catalog entry note");
    },
  },
  "glossary.add": {
    params: async () => ({
      source: "catalog glossary source",
      target: "catalog glossary target",
      comment: "catalog glossary comment",
    }),
    products: (fixture) => [fixture.glossaryFile],
    verify: async (_client, fixture) => {
      assert.match(
        await readFile(fixture.glossaryFile, "utf8"),
        /catalog glossary source\tcatalog glossary target\tcatalog glossary comment/,
      );
    },
  },
  "search.replace": {
    prepare: async (launched) =>
      rpc(launched.client, "script.run", {
        index: 0,
        source: "editor.setTranslation('catalog replace alpha');",
      }),
    params: async () => ({
      query: "alpha",
      replace: "omega",
      source: false,
      translation: true,
    }),
    products: (fixture) => [fixture.saveTmx],
    verify: async (client) => {
      assert.equal(
        (await rpc(client, "entry.get", { index: 0 })).translation,
        "catalog replace omega",
      );
    },
  },
  "spell.learn": {
    params: async () => ({ word: "cataloglearned" }),
    products: (fixture) => [fixture.learnedWords],
    verify: async (_client, fixture) => {
      assert.match(await readFile(fixture.learnedWords, "utf8"), /cataloglearned/);
    },
  },
  "spell.ignore": {
    params: async () => ({ word: "catalogignored" }),
    products: (fixture) => [fixture.ignoredWords],
    verify: async (_client, fixture) => {
      assert.match(await readFile(fixture.ignoredWords, "utf8"), /catalogignored/);
    },
  },
  "tmx.export": {
    params: async (_client, fixture) => ({
      dest: join(fixture.workDir, "catalog-export.tmx"),
      level: "level2",
    }),
    products: (fixture) => [join(fixture.workDir, "catalog-export.tmx")],
    verify: async (_client, fixture) => {
      assert.match(
        await readFile(join(fixture.workDir, "catalog-export.tmx"), "utf8"),
        /<tmx version=/,
      );
    },
  },
  "team.sync": {
    params: async () => ({}),
    products: (fixture) => [
      join(fixture.project, "source", "team-sync.txt"),
      join(fixture.project, "target", "team-commit.txt"),
    ],
    verify: async (_client, fixture) => {
      assert.equal(
        await readFile(join(fixture.project, "source", "team-sync.txt"), "utf8"),
        "catalog remote sync",
      );
    },
  },
  "team.commit": {
    prepare: async (launched, fixture) =>
      pauseWatcherForWrite(launched, fixture.project, () =>
        writeFile(
          join(fixture.project, "target", "team-commit.txt"),
          "catalog committed target",
          "utf8",
        )
      ),
    params: async () => ({ which: "target" }),
    products: (fixture) => [join(fixture.remote, "target", "team-commit.txt")],
    verify: async (_client, fixture) => {
      assert.equal(
        await readFile(join(fixture.remote, "target", "team-commit.txt"), "utf8"),
        "catalog committed target",
      );
    },
  },
  "team.resolve": {
    prepare: async (launched, fixture) => {
      const entry = await rpc(launched.client, "entry.get", { index: 0 });
      const conflicts = [{
        kind: "tmx",
        source: entry.source,
        ours: entry.translation,
        theirs: "catalog resolved theirs",
        message: `TMX conflict on ${entry.source}`,
      }];
      const conflictsPath = join(
        fixture.project,
        ".repositories",
        "prep",
        "conflicts.json",
      );
      await mkdir(join(fixture.project, ".repositories", "prep"), {
        recursive: true,
      });
      await writeFile(conflictsPath, JSON.stringify(conflicts, null, 2), "utf8");
    },
    params: async (client) => ({
      source: (await rpc(client, "entry.get", { index: 0 })).source,
      side: "theirs",
    }),
    products: (fixture) => [
      fixture.saveTmx,
      join(fixture.project, ".repositories", "prep", "conflicts.json"),
      join(fixture.project, ".repositories", "prep", "resolved.json"),
    ],
    verify: async (_client, fixture) => {
      assert.match(await readFile(fixture.saveTmx, "utf8"), /catalog resolved theirs/);
      assert.deepEqual(
        JSON.parse(
          await readFile(
            join(fixture.project, ".repositories", "prep", "conflicts.json"),
            "utf8",
          ),
        ),
        [],
      );
    },
  },
  "team.mapping": {
    params: async (_client, fixture) => ({
      repositories: [{
        repo_type: "file",
        url: fixture.remote,
        branch: null,
        mappings: [{
          local: "/source/team-sync.txt",
          repository: "/source/team-sync.txt",
          includes: ["**/*.txt"],
          excludes: ["**/.catalog-ignore/**"],
        }],
      }],
    }),
    products: (fixture) => [fixture.projectFile],
    verify: async (client) => {
      assert.equal((await rpc(client, "project.props")).has_repositories, true);
    },
  },
  "project.update": {
    params: async () => ({ external_command: "printf catalog-writer" }),
    products: (fixture) => [fixture.projectFile],
    verify: async (client) => {
      assert.equal(
        (await rpc(client, "project.props")).external_command,
        "printf catalog-writer",
      );
    },
  },
  "script.run": {
    params: async () => ({
      index: 0,
      source:
        "editor.setTranslation('catalog script product'); project.save(); glossary.addEntry('catalog script term','catalog script target','catalog script comment');",
    }),
    products: (fixture) => [fixture.saveTmx, fixture.glossaryFile],
    verify: async (client) => {
      assert.equal(
        (await rpc(client, "entry.get", { index: 0 })).translation,
        "catalog script product",
      );
    },
  },
  "align.run": {
    params: async (_client, fixture) => ({
      source: fixture.alignSource,
      target: fixture.alignTarget,
      dest: join(fixture.workDir, "catalog-align-run.tmx"),
      mode: "parsewise",
      algo: "viterbi",
      counter: "word",
      calculator: "normal",
      segment: false,
      source_lang: "en",
      target_lang: "fr",
    }),
    products: (fixture) => [join(fixture.workDir, "catalog-align-run.tmx")],
    verify: async (_client, fixture) => {
      assert.match(
        await readFile(join(fixture.workDir, "catalog-align-run.tmx"), "utf8"),
        /<tmx version=/,
      );
    },
  },
  "align.write": {
    params: async (_client, fixture) => ({
      dest: join(fixture.workDir, "catalog-align-write.tmx"),
      source_lang: "en",
      target_lang: "fr",
      pairs: [
        { source: "Catalog aligned source", target: "Catalog aligned target" },
      ],
    }),
    products: (fixture) => [join(fixture.workDir, "catalog-align-write.tmx")],
    verify: async (_client, fixture) => {
      assert.match(
        await readFile(join(fixture.workDir, "catalog-align-write.tmx"), "utf8"),
        /Catalog aligned target/,
      );
    },
  },
  "wiki.import": {
    params: async (_client, fixture) => ({ source: fixture.wikiSource }),
    products: (fixture) => [join(fixture.project, "source", "catalog-wiki.txt")],
    verify: async (_client, fixture) => {
      assert.equal(
        await readFile(join(fixture.project, "source", "catalog-wiki.txt"), "utf8"),
        "catalog wiki product",
      );
    },
  },
  "project.import": {
    params: async (_client, fixture) => ({ files: [fixture.importSource] }),
    products: (fixture) => [join(fixture.project, "source", "catalog-import.txt")],
    verify: async (_client, fixture) => {
      assert.equal(
        await readFile(join(fixture.project, "source", "catalog-import.txt"), "utf8"),
        "catalog imported product",
      );
    },
  },
  "script.slot": {
    params: async () => ({ slot: 1, index: 0 }),
    products: (fixture) => [fixture.saveTmx],
    verify: async (client) => {
      assert.equal(
        (await rpc(client, "entry.get", { index: 0 })).translation,
        "catalog slot product",
      );
    },
  },
};

const configDrivers = {
  "prefs.set": {
    params: async (client) => ({
      ...await rpc(client, "prefs.get"),
      locale: "fr",
    }),
    products: (fixture) => [fixture.prefsFile],
    verify: async (client) => {
      assert.equal((await rpc(client, "prefs.get")).locale, "fr");
    },
  },
  "prefs.patch": {
    params: async () => ({ theme: "dark" }),
    products: (fixture) => [fixture.prefsFile],
    verify: async (client) => {
      assert.equal((await rpc(client, "prefs.get")).theme, "dark");
    },
  },
  "spell.install": {
    params: async () => ({ lang: "en" }),
    products: (fixture) => [
      join(fixture.config, "spell", "hunspell", "en.aff"),
      join(fixture.config, "spell", "hunspell", "en.dic"),
    ],
    verify: async (_client, fixture) => {
      assert.equal(
        await pathExists(join(fixture.config, "spell", "hunspell", "en.aff")),
        true,
      );
      assert.equal(
        await pathExists(join(fixture.config, "spell", "hunspell", "en.dic")),
        true,
      );
    },
  },
  "aligner.configure": {
    params: async () => ({
      persist: true,
      algo: "forward-backward",
      calculator: "poisson",
      counter: "char",
      source_lang: "en-US",
      target_lang: "fr-FR",
    }),
    products: (fixture) => [fixture.prefsFile],
    verify: async (client) => {
      const prefs = await rpc(client, "prefs.get");
      assert.equal(prefs.aligner_algorithm, "forward-backward");
      assert.equal(prefs.aligner_calculator, "poisson");
    },
  },
};

const driverMethods = [
  ...Object.keys(projectDrivers),
  ...Object.keys(configDrivers),
].sort();
assert.deepEqual(
  driverMethods,
  sourceCatalog.map(({ method }) => method).sort(),
  "packaged driver keys drifted from the canonical writer catalog",
);

async function waitForAck(traceFile, batchId) {
  return waitFor(`acknowledgement for ${batchId}`, async () => {
    if (!await pathExists(traceFile)) return undefined;
    const matches = parseNdjson(await readFile(traceFile, "utf8"))
      .filter((row) =>
        row.batch_id === batchId && row.result === "acknowledged"
      );
    return matches.length === 1 ? matches[0] : undefined;
  });
}

async function recoverProjectReceipt(
  launched,
  fixture,
  writer,
  receipt,
  committedSnapshots,
) {
  const killed = await killPackaged(launched);
  let recovered = writer.method === "project.close"
    ? await launchPackagedRenderer(
      fixture.display,
      fixture.config,
      fixture.project,
      fixture.launchEnv,
    )
    : await launchPackaged(
      fixture.display,
      fixture.config,
      fixture.project,
      fixture.launchEnv,
    );
  await waitForAck(fixture.traceFile, receipt.batch_id);
  await waitFor(`${writer.method} active queue cleanup`, async () =>
    !await pathExists(
      join(fixture.project, ".repositories", "transactions", "active.json"),
    )
  );
  await assertExactSnapshots(
    committedSnapshots,
    `${writer.method} recovery replayed product`,
  );
  await assertOneTerminal(
    projectHistoryPaths(fixture.project),
    receipt.batch_id,
    receipt.payload.operation,
  );
  if (writer.method === "project.close") {
    await waitFor("recovered close visible state", async () => {
      const state = await workspaceState(recovered.client);
      return state.welcome && state.project === null ? state : undefined;
    });
    await terminatePackaged(recovered);
    recovered = await launchPackaged(
      fixture.display,
      fixture.config,
      fixture.project,
      fixture.launchEnv,
    );
  }
  return { launched: recovered, killed };
}

async function runProjectWriter(launched, fixture, writer) {
  const driver = projectDrivers[writer.method];
  await driver.prepare?.(launched, fixture);
  const params = await driver.params(launched.client, fixture);
  const productPaths = driver.products(fixture, params);
  productPaths
    .filter((path) => relative(fixture.project, path) !== ".."
      && !relative(fixture.project, path).startsWith(`..${process.platform === "win32" ? "\\" : "/"}`)
      && !isAbsolute(relative(fixture.project, path)))
    .forEach((path) => fixture.projectProducts.add(path));
  const before = await snapshotFiles(productPaths);
  let result;
  let receipt;
  if (writer.method === "project.external-refresh") {
    const state = await workspaceState(launched.client);
    const queued = await rpc(launched.client, "project.refresh.enqueue", {
      root: fixture.project,
      app_instance: "catalog-external-refresh",
      generation: state.generation,
      paths: [fixture.sourceMain],
      fingerprints: { "source/source.txt": "catalog-external-refresh" },
      sources: ["native"],
    });
    result = await rpc(launched.client, writer.method, {
      transaction_project_root: fixture.project,
      transaction_generation: state.generation,
      transaction_batch_id: queued.batch.batch_id,
    });
    const pending = await rpc(launched.client, "transaction.receipt.pending", {
      root: fixture.project,
      app_instance: "catalog-external-refresh",
      generation: state.generation,
    });
    assert.equal(pending.envelopes.length, 1);
    receipt = pending.envelopes[0];
  } else {
    result = await rpcWithReceipt(launched.client, writer.method, params);
    receipt = result.receipt;
  }
  assert(receipt, `${writer.method} did not publish a receipt`);
  assert.equal(receipt.status, "sidecar_committed");
  assert.equal(receipt.payload.operation, resolvedOperation(writer, params));
  const committed = await snapshotFiles(productPaths);
  if (driver.stableInput) {
    assert.deepEqual(committed, before, `${writer.method} rewrote its input`);
  } else {
    assert(
      snapshotsChanged(before, committed),
      `${writer.method} did not mutate its declared product`,
    );
  }
  const recovery = await recoverProjectReceipt(
    launched,
    fixture,
    writer,
    receipt,
    committed,
  );
  await driver.verify(recovery.launched.client, fixture, result);
  return {
    launched: recovery.launched,
    evidence: {
      method: writer.method,
      operation: receipt.payload.operation,
      batchId: receipt.batch_id,
      products: productPaths,
      killedBeforeAck: recovery.killed,
      productBytesAndMtimeStable: true,
      terminalRows: 1,
      replayCount: 0,
    },
  };
}

async function runConfigWriter(launched, fixture, writer) {
  const driver = configDrivers[writer.method];
  const batchId = `catalog-${writer.method.replaceAll(".", "-")}`;
  const params = {
    ...await driver.params(launched.client, fixture),
    config_transaction_retry_batch_id: batchId,
  };
  const productPaths = driver.products(fixture, params);
  const before = await snapshotFiles(productPaths);
  const first = await rpc(launched.client, writer.method, params);
  await assertOneTerminal(
    configHistoryPaths(fixture.config),
    batchId,
    writer.journal_operation,
  );
  const committed = await snapshotFiles(productPaths);
  assert(
    snapshotsChanged(before, committed),
    `${writer.method} did not mutate its declared config product`,
  );
  const killed = await killPackaged(launched);
  const recovered = await launchPackaged(
    fixture.display,
    fixture.config,
    fixture.project,
    fixture.launchEnv,
  );
  const exactRetry = await rpc(recovered.client, writer.method, params);
  assert.deepEqual(exactRetry, first, `${writer.method} exact retry result changed`);
  await assertExactSnapshots(
    committed,
    `${writer.method} exact retry replayed its product`,
  );
  await assertOneTerminal(
    configHistoryPaths(fixture.config),
    batchId,
    writer.journal_operation,
  );
  await driver.verify(recovered.client, fixture, exactRetry);
  return {
    launched: recovered,
    evidence: {
      method: writer.method,
      operation: writer.journal_operation,
      batchId,
      products: productPaths,
      killedAfterTerminal: killed,
      exactRetry: true,
      productBytesAndMtimeStable: true,
      terminalRows: 1,
      replayCount: 0,
    },
  };
}

async function runCancellation(launched, fixture) {
  const importDir = join(fixture.workDir, "cancel-import");
  await mkdir(importDir, { recursive: true });
  const payload = Buffer.alloc(2 * 1024 * 1024, 0x63);
  const files = [];
  for (let index = 0; index < 96; index += 1) {
    const path = join(importDir, `cancel-${index.toString().padStart(3, "0")}.bin`);
    await writeFile(path, payload);
    files.push(path);
  }
  const sourceBefore = await treeBytes(join(fixture.project, "source"));
  const historyBefore = await segmentedRows(projectHistoryPaths(fixture.project));
  const requestId = "catalog-cancel-project-import";
  await launched.client.evaluate(`(() => {
    window.__catalogCancellationTrace = [];
    window.__catalogCancellationDispose?.();
    window.__catalogCancellationDispose = window.omegat.onRpcOperation((event) => {
      if (event.requestId === ${JSON.stringify(requestId)}) {
        window.__catalogCancellationTrace.push(event);
      }
    });
    window.__catalogCancellationResult = window.omegat.rpcWithTransactionReceipt(
      "project.import",
      ${JSON.stringify({ files })},
      ${JSON.stringify(requestId)}
    ).then(
      (value) => ({ resolved: true, value }),
      (error) => ({ resolved: false, error: String(error) })
    );
    return true;
  })()`);
  await waitFor("project.import cancellable copy checkpoint", async () => {
    const trace = await launched.client.evaluate(
      "window.__catalogCancellationTrace",
    );
    return trace.some(({ phase, stage }) =>
        phase === "progress" && stage === "project.import.copy"
      )
      ? trace
      : undefined;
  });
  assert.equal(
    await launched.client.evaluate(
      `window.omegat.cancelRpc(${JSON.stringify(requestId)})`,
      true,
    ),
    true,
  );
  const outcome = await launched.client.evaluate(
    "window.__catalogCancellationResult",
    true,
  );
  assert.equal(outcome.resolved, false);
  assert.match(outcome.error, /request cancelled/i);
  const trace = await launched.client.evaluate(
    "window.__catalogCancellationTrace",
  );
  const cancelling = trace.findIndex(({ phase }) => phase === "cancelling");
  const cancelled = trace.findIndex(({ phase, errorCode }) =>
    phase === "cancelled" && errorCode === -32800
  );
  assert(cancelling >= 0, JSON.stringify(trace));
  assert(cancelled > cancelling, JSON.stringify(trace));
  assert.deepEqual(
    await treeBytes(join(fixture.project, "source")),
    sourceBefore,
    "cancelled project.import changed product bytes",
  );
  const active = join(
    fixture.project,
    ".repositories",
    "transactions",
    "active.json",
  );
  await waitFor("cancelled project.import queue cleanup", async () =>
    !await pathExists(active)
  );
  const historyAfter = await segmentedRows(projectHistoryPaths(fixture.project));
  const previous = new Set(historyBefore.map(({ batch_id }) => batch_id));
  const cancellationRows = terminalRows(historyAfter)
    .filter(({ batch_id }) => !previous.has(batch_id));
  assert.equal(cancellationRows.length, 1);
  assert.equal(cancellationRows[0].status, "request_cancelled");
  assert.equal(cancellationRows[0].payload.operation, "project.import");

  const killed = await killPackaged(launched);
  const recovered = await launchPackaged(
    fixture.display,
    fixture.config,
    fixture.project,
    fixture.launchEnv,
  );
  assert.deepEqual(
    await treeBytes(join(fixture.project, "source")),
    sourceBefore,
    "cancelled project.import replayed after restart",
  );
  await assertOneTerminal(
    projectHistoryPaths(fixture.project),
    cancellationRows[0].batch_id,
    "project.import",
    "request_cancelled",
  );
  return {
    launched: recovered,
    evidence: {
      method: "project.import",
      batchId: cancellationRows[0].batch_id,
      errorCode: -32800,
      phases: trace.map(({ phase, stage }) => ({ phase, stage: stage ?? null })),
      killed,
      productBytesStable: true,
      terminalRows: 1,
      replayCount: 0,
    },
  };
}

async function verifyMoveAndGc(launched, fixture, evidence) {
  await terminatePackaged(launched);
  const oldProject = fixture.project;
  const movedProject = join(fixture.workDir, "project-renamed");
  const snapshots = await snapshotFiles([...fixture.projectProducts]);
  await rename(oldProject, movedProject);
  fixture.project = movedProject;
  fixture.sourceMain = join(movedProject, "source", "source.txt");
  fixture.saveTmx = join(movedProject, "omegat", "project_save.tmx");
  fixture.projectFile = join(movedProject, "omegat.project");
  fixture.glossaryFile = join(movedProject, "glossary", "glossary.txt");
  fixture.learnedWords = join(movedProject, "omegat", "learned_words.txt");
  fixture.ignoredWords = join(movedProject, "omegat", "ignored_words.txt");
  const moved = await launchPackaged(
    fixture.display,
    fixture.config,
    movedProject,
    fixture.launchEnv,
  );
  assert.equal((await rpc(moved.client, "project.props")).root, movedProject);
  for (const [oldPath, snapshot] of Object.entries(snapshots)) {
    const movedPath = join(movedProject, relative(oldProject, oldPath));
    assert.deepEqual(
      await snapshotFile(movedPath),
      snapshot,
      `project move changed product ${oldPath}`,
    );
  }
  const projectRows = await segmentedRows(projectHistoryPaths(movedProject));
  for (const row of evidence.filter(({ scope }) => row.scope === "project")) {
    assert.equal(
      terminalRows(projectRows)
        .filter(({ batch_id }) => batch_id === row.batchId).length,
      1,
      `${row.method} terminal did not survive project rename`,
    );
  }
  const projectManifest = JSON.parse(
    await readFile(projectHistoryPaths(movedProject).manifest, "utf8"),
  );
  const configManifest = JSON.parse(
    await readFile(configHistoryPaths(fixture.config).manifest, "utf8"),
  );
  assert(projectManifest.generation > 0, "project writer matrix did not force GC");
  assert(configManifest.generation > 0, "config writer matrix did not force GC");
  await Promise.all([
    assertNoCandidates(projectHistoryPaths(movedProject).directory),
    assertNoCandidates(configHistoryPaths(fixture.config).directory),
  ]);
  return {
    launched: moved,
    evidence: {
      from: oldProject,
      to: movedProject,
      productFiles: Object.keys(snapshots).length,
      projectGcGeneration: projectManifest.generation,
      configGcGeneration: configManifest.generation,
      terminalRowsPreserved: evidence.length,
      replayCount: 0,
    },
  };
}

const workDir = await mkdtemp(join(tmpdir(), "omegat-writer-catalog-e2e-"));
const display = await startPackagedDisplay();
let launched;
try {
  const fixture = await prepareFixture(display.display, workDir);
  fixture.display = display.display;
  fixture.launchEnv = {
    ...limits,
    OMEGAT_TEST_TRANSACTION_ACK_TRACE: fixture.traceFile,
  };
  launched = await launchPackaged(
    display.display,
    fixture.config,
    fixture.project,
    fixture.launchEnv,
  );
  const runtimeCatalog = await rpc(launched.client, "sys.writer-catalog");
  assert.equal(runtimeCatalog.version, sourceRegistry.version);
  assert.deepEqual(runtimeCatalog.writers, sourceCatalog);

  const evidence = [];
  for (const writer of runtimeCatalog.writers) {
    const result = writer.scope === "project"
      ? await runProjectWriter(launched, fixture, writer)
      : await runConfigWriter(launched, fixture, writer);
    launched = result.launched;
    evidence.push({ scope: writer.scope, ...result.evidence });
  }
  assert.equal(evidence.length, 26);
  assert.deepEqual(
    evidence.map(({ method }) => method),
    runtimeCatalog.writers.map(({ method }) => method),
  );
  assert(evidence.every(({ terminalRows }) => terminalRows === 1));
  assert(evidence.every(({ replayCount }) => replayCount === 0));

  const cancellation = await runCancellation(launched, fixture);
  launched = cancellation.launched;
  const moved = await verifyMoveAndGc(launched, fixture, evidence);
  launched = moved.launched;

  console.log(JSON.stringify({
    result: "passed",
    driver: "packaged-writer-catalog-linux",
    package: executable,
    registryVersion: runtimeCatalog.version,
    catalogRows: evidence.length,
    projectWriters: evidence.filter(({ scope }) => scope === "project").length,
    configWriters: evidence.filter(({ scope }) => scope === "config").length,
    evidence,
    cancellation: cancellation.evidence,
    renameReopenAndGc: moved.evidence,
    productBytesAndMtimeCheckedPerWriter: true,
    oneTerminalPerWriter: true,
    zeroReplayPerWriter: true,
    platformsNotRun: ["windows", "macos"],
  }, null, 2));
} finally {
  await terminatePackaged(launched);
  await stopPackagedDisplay(display);
  if (process.env.OMEGAT_KEEP_E2E !== "1") {
    await rm(workDir, {
      recursive: true,
      force: true,
      maxRetries: 10,
      retryDelay: 100,
    });
  } else {
    console.error(`kept writer catalog workdir: ${workDir}`);
  }
}
