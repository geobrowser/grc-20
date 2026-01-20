# Repository Guidelines

## Project Structure & Module Organization
`spec.md` holds the GRC-20 specification, `docs/` contains design rationale and requirements, and `data/` stores sample datasets. The Rust workspace lives under `rust/` with crates for the core library (`crates/grc-20`), benchmarks (`crates/grc-20-bench`), comparison tooling (`crates/grc-20-compare`), and a protobuf baseline (`crates/grc-20-proto-bench`). The TypeScript implementation is under `typescript/` with `src/` split into `builder/`, `codec/`, `types/`, `genesis/`, and `util/`, plus `examples/` and `scripts/`.

## Build, Test, and Development Commands
- `cd rust && cargo build --release` builds all Rust crates.
- `cd rust && cargo test` runs Rust unit/integration tests in the workspace.
- `cd rust/crates/grc-20-compare && cargo run --release` runs the format comparison benchmark.
- `cd typescript && npm install` installs Node.js dependencies.
- `cd typescript && npm run build` compiles TypeScript to `dist/`.
- `cd typescript && npm test` runs Vitest in Node; `npm run test:browser` runs the browser suite; `npm run test:all` runs both.
- `cd typescript && npm run benchmark` runs the JS benchmark; `npm run demo` serves `examples/` via `http://localhost:3000/examples/browser-demo.html`.

## Coding Style & Naming Conventions
Rust uses edition 2024; follow standard Rust naming (`snake_case` functions/modules, `CamelCase` types) and rustfmt defaults. TypeScript uses 2-space indentation, `camelCase` for values, `PascalCase` for types, and kebab-case filenames (e.g., `update-relation.ts`). Keep exports grouped and commented in `typescript/src/index.ts` to match existing sections.

## Testing Guidelines
TypeScript tests use Vitest and live in `typescript/src/test/*.test.ts`. Rust tests should be colocated in crates with `#[cfg(test)]` or in `tests/` for integration coverage. For codec changes, add encode/decode round-trip coverage and run `npm run test:all` plus `cargo test`.

## Commit & Pull Request Guidelines
Commit messages are short, imperative sentences without scope prefixes (e.g., “Update Rust implementation to latest spec”). PRs should describe behavior changes, mention spec updates (`spec.md`) when relevant, and list test commands run. Include benchmarks or demo screenshots only when performance or UI behavior changes.

# ExecPlans

When writing complex features or significant refactors, use an ExecPlan (as described in .agent/PLANS.md) from design to implementation.

## Security & Configuration Notes
The TypeScript codec lazily loads zstd WASM; call `preloadCompression()` when you need predictable startup. Treat `data/` as large sample datasets; avoid modifying it unless the change is intentional.
