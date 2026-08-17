# Honesty gates

`check.sh` is the structural gate for the 6.2 rewrite. Failure is a CI red.

P0 commits the script **red** on the current tree. Later waves may turn only
their own items green. Do not mark a STATUS row `parity` while the matching
item here is red.

Run from the repository root:

```
bash tools/honesty/check.sh
```

The script always runs every check and prints a summary. Exit status is
nonzero if any check failed.
