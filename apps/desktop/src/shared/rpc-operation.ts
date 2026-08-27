// SPDX-License-Identifier: GPL-3.0-or-later

export const LONG_OPERATION_METHODS = {
  reload: "project.reload",
  compile: "project.compile",
  teamSync: "team.sync",
  teamCommit: "team.commit",
  align: "align.run",
} as const;

export type LongOperationKind = keyof typeof LONG_OPERATION_METHODS;
export type LongOperationMethod =
  (typeof LONG_OPERATION_METHODS)[LongOperationKind];

export type RpcOperationPhase =
  | "started"
  | "progress"
  | "cancelling"
  | "cancelled"
  | "succeeded"
  | "failed";

export type RpcOperationEvent = {
  requestId: string;
  method: string;
  phase: RpcOperationPhase;
  stage?: string;
  error?: string;
};

export function longOperationKindForMethod(
  method: string,
): LongOperationKind | null {
  const match = Object.entries(LONG_OPERATION_METHODS).find(
    ([, candidate]) => candidate === method,
  );
  return (match?.[0] as LongOperationKind | undefined) ?? null;
}

export function isLongOperationMethod(
  method: string,
): method is LongOperationMethod {
  return longOperationKindForMethod(method) !== null;
}
