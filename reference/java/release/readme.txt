# Java OmegaT reference tree

This directory is a checkout of OmegaT 6.2 from `origin/master` (`05b98cf05`).

It is **not** the default product. Do not run `./gradlew` as the shipping build.

Use it to:

- read the historic implementation (`RealProject`, filters, aligner, team)
- copy golden fixtures from `src/test/resources`
- generate tokenizer / compile goldens when a JDK is available

The shipping application is the Rust sidecar + Electron tree at the repository root.
