import type { ProjectPropsDto, RepositoryRow } from "./types";

export type RepositoryEditorRow = {
  repo_type: string;
  url: string;
  branch: string;
  local: string;
  repository: string;
  includes: string;
  excludes: string;
};

export function rootRepositoryMapping(repositories: RepositoryRow[]): RepositoryRow | null {
  return (
    repositories.find(
      (repository) =>
        repository.mappings[0]?.local === "/" && repository.mappings[0]?.repository === "/",
    ) ?? null
  );
}

/** Replace only root repository identity, preserving its mapping rules. */
export function setRootRepositoryMapping(
  repositories: RepositoryRow[],
  replacement: RepositoryRow,
): RepositoryRow[] {
  const root = rootRepositoryMapping(repositories);
  if (!root) return repositories;
  return repositories.map((repository) =>
    repository === root
      ? {
          ...repository,
          repo_type: replacement.repo_type,
          url: replacement.url,
          branch: replacement.branch,
        }
      : repository,
  );
}

/** Java intentionally compares repository identity, not mappings. */
export function repositoryEquals(a: RepositoryRow | null, b: RepositoryRow | null): boolean {
  return (
    a !== null &&
    b !== null &&
    a.repo_type === b.repo_type &&
    a.url === b.url &&
    (a.branch ?? null) === (b.branch ?? null)
  );
}

export function projectPropertiesIdentical(a: ProjectPropsDto, b: ProjectPropsDto): boolean {
  const scalarKeys: (keyof ProjectPropsDto)[] = [
    "root",
    "source_lang",
    "target_lang",
    "sentence_seg",
    "source_dir",
    "target_dir",
    "tm_dir",
    "glossary_dir",
    "glossary_file",
    "dictionary_dir",
    "export_tm_levels",
    "support_default_translations",
    "remove_tags",
    "has_repositories",
  ];
  if (scalarKeys.some((key) => a[key] !== b[key])) return false;
  const left = a.repositories ?? [];
  const right = b.repositories ?? [];
  return (
    left.length === right.length &&
    left.every((repository, index) => {
      const other = right[index];
      return (
        other !== undefined &&
        repositoryEquals(repository, other) &&
        repository.mappings.length === other.mappings.length &&
        repository.mappings.every(
          (mapping, mappingIndex) =>
            mapping.local === other.mappings[mappingIndex]?.local &&
            mapping.repository === other.mappings[mappingIndex]?.repository,
        )
      );
    })
  );
}

export function repositoryEditorRows(
  repositories: RepositoryRow[],
  fallbackRoot: string,
): RepositoryEditorRow[] {
  if (!repositories.length) {
    return [
      {
        repo_type: "git",
        url: fallbackRoot,
        branch: "main",
        local: "/",
        repository: "/",
        includes: "/**",
        excludes: "omegat/**",
      },
    ];
  }
  return repositories.flatMap((repository) =>
    (repository.mappings.length
      ? repository.mappings
      : [{ local: "/", repository: "/", includes: [], excludes: [] }]
    ).map((mapping) => ({
      repo_type: repository.repo_type,
      url: repository.url,
      branch: repository.branch ?? "",
      local: mapping.local,
      repository: mapping.repository,
      includes: mapping.includes.join(","),
      excludes: mapping.excludes.join(","),
    })),
  );
}

export function repositoriesFromEditorRows(rows: RepositoryEditorRow[]): RepositoryRow[] {
  return rows.map((row) => ({
    repo_type: row.repo_type,
    url: row.url,
    branch: row.branch || null,
    mappings: [
      {
        local: row.local,
        repository: row.repository,
        includes: row.includes.split(/[,;]/).map((value) => value.trim()).filter(Boolean),
        excludes: row.excludes.split(/[,;]/).map((value) => value.trim()).filter(Boolean),
      },
    ],
  }));
}
