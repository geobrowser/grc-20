# Support ISO 8601 Expanded Years For DATE And DATETIME

This ExecPlan is a living document. The sections `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work proceeds.

This document must be maintained in accordance with `.agent/PLANS.md`.

## Purpose / Big Picture

After this change, GRC-20 `DATE` and `DATETIME` values will support BCE dates and years outside `0001..9999` in the public Rust and TypeScript APIs. Users will be able to encode and decode values like `0000-01-01Z`, `-0001-12-31Z`, and `+12024-03-15T10:30:00Z` without leaving the standard value types. The observable result is that the spec will describe `DATE` and `DATETIME` strings as ISO 8601 extended forms with expanded years, while the Rust and TypeScript test suites will round-trip BCE and expanded-year values successfully.

## Progress

- [x] (2026-04-21 01:36Z) Confirmed the wire types already support BCE and far-future dates because `DATE` stores signed epoch days and `DATETIME` stores signed epoch microseconds.
- [x] (2026-04-21 01:36Z) Confirmed the current Rust and TypeScript public string parsers reject BCE and expanded years because they assume unsigned 4-digit years and, in TypeScript, depend on `Date`.
- [ ] Update the spec and user-facing docs to describe `DATE` and `DATETIME` strings as ISO 8601 extended forms with expanded years while keeping `TIME` on the existing RFC 3339-compatible shape.
- [ ] Refactor Rust datetime parsing/formatting to support signed and expanded years for `DATE` and `DATETIME`.
- [ ] Refactor TypeScript datetime parsing/formatting to support signed and expanded years without relying on `Date`/`Date.UTC`.
- [ ] Add BCE and expanded-year roundtrip tests in Rust and TypeScript and run both test suites.

## Surprises & Discoveries

- Observation: The wire model was already correct for BCE dates; the bug sits entirely in the string API layer.
  Evidence: `DATE` uses signed `int32 days` and `DATETIME` uses signed `int64 epoch_micros` in `spec.md`.

- Observation: TypeScript’s current implementation is weaker than it first appears because it relies on `Date.UTC` and `Date`, which are poor foundations for expanded-year support and already risky for years near `0000`.
  Evidence: `typescript/src/util/datetime.ts` calculates days and datetimes with `Date.UTC(...)` and `new Date(...)`.

## Decision Log

- Decision: Keep the exported helper function names (`parseDateRfc3339`, `formatDateRfc3339`, and their datetime counterparts) for compatibility, but change their documented behavior to the new ISO 8601-expanded contract for `DATE` and `DATETIME`.
  Rationale: The real user-facing contract is the value-string format, not the helper symbol name. Renaming the helpers would create avoidable API churn on top of the behavioral fix.
  Date/Author: 2026-04-21 / Codex

- Decision: `TIME` remains on the existing RFC 3339-compatible shape, while `DATE` and `DATETIME` move to ISO 8601 extended years with astronomical year numbering.
  Rationale: BCE support matters for dates and datetimes, not standalone times. Keeping TIME unchanged minimizes scope while fixing the real protocol gap.
  Date/Author: 2026-04-21 / Codex

## Outcomes & Retrospective

Implementation has not finished yet. The intended result is a spec and SDK surface where DATE and DATETIME support expanded years, BCE values round-trip, and the public docs no longer incorrectly promise RFC 3339 for those two types.

## Context and Orientation

The normative data model lives in `spec.md`. The binary value encoding and decode/encode logic live in `rust/crates/grc-20/src/codec/value.rs` and `typescript/src/codec/value.ts`. The public string parsing and formatting helpers live in `rust/crates/grc-20/src/util/datetime.rs` and `typescript/src/util/datetime.ts`. The TypeScript type comments are in `typescript/src/types/value.ts`. The Rust builder doc comments are in `rust/crates/grc-20/src/model/builder.rs`. User-facing package docs live in `rust/README.md` and `typescript/readme.md`.

Today, DATE and DATETIME strings are described as RFC 3339 in several places, but the wire types are numeric. That means the protocol can store BCE values while the current SDK parsers reject them. This plan fixes that mismatch by changing the textual contract to ISO 8601 extended strings with astronomical year numbering:

- `0000` means 1 BCE
- `-0001` means 2 BCE
- `+12024` means year 12024 CE

The formatter should emit canonical strings with:

- exactly 4 digits and no sign for years `0000..9999`
- a sign plus at least 4 digits for negative years
- a leading `+` plus at least 5 digits for positive years greater than `9999`

## Plan of Work

First, update `spec.md` so that the DATE and DATETIME sections explicitly define the textual API form as ISO 8601 extended strings with expanded years and astronomical year numbering. Update the README files and TypeScript value comments so the public API examples match. Keep TIME described as RFC 3339-compatible because BCE is irrelevant there.

Next, rework `rust/crates/grc-20/src/util/datetime.rs`. Add a helper that parses a date prefix with either an unsigned four-digit year or a signed expanded year, then reuse that helper in `parse_date_rfc3339` and `parse_datetime_rfc3339`. Add a formatter helper that emits canonical years for both date and datetime formatting. Avoid changing the binary encoding or the existing day/microsecond arithmetic, because those parts are already correct.

Then update `typescript/src/util/datetime.ts`. Port the pure calendar math from Rust (`date_to_days`, `days_to_date`, and floor-division handling for negative epochs) so DATE and DATETIME formatting and parsing no longer depend on `Date.UTC` or `Date`. Extend the parsers to accept the same signed/expanded year grammar as Rust and emit canonical strings with the same rules.

Finally, update codec and public-facing comments that still say “RFC 3339 date/datetime”, then add roundtrip tests in `rust/crates/grc-20/src/util/datetime.rs`, `rust/crates/grc-20/src/codec/value.rs`, and `typescript/src/test/basic.test.ts` for BCE and expanded-year values. Run `cargo test` and `npm test`.

## Concrete Steps

1. Edit `spec.md`, `rust/README.md`, `typescript/readme.md`, `typescript/src/types/value.ts`, and `rust/crates/grc-20/src/model/builder.rs` so DATE and DATETIME mention ISO 8601 expanded years.
2. Edit `rust/crates/grc-20/src/util/datetime.rs` to add expanded-year parsing and canonical formatting.
3. Edit `typescript/src/util/datetime.ts` to add the same support without using `Date`.
4. Adjust codec comments and error strings in `rust/crates/grc-20/src/codec/value.rs` and `typescript/src/codec/value.ts`.
5. Add tests for BCE and expanded years.
6. Run:

    cd /Users/nico/Development/geo/grc-20/rust
    cargo test

7. Run:

    cd /Users/nico/Development/geo/grc-20/typescript
    npm test

## Validation and Acceptance

Acceptance means:

1. `DATE` values like `0000-01-01Z` and `-0001-12-31Z` encode and decode successfully.
2. `DATETIME` values like `0000-01-01T00:00:00Z` and `+12024-03-15T10:30:00Z` encode and decode successfully.
3. TypeScript no longer depends on `Date` or `Date.UTC` for DATE/DATETIME conversions.
4. The docs no longer describe DATE and DATETIME strings as RFC 3339.
5. `cargo test` and `npm test` both pass.

## Idempotence and Recovery

These edits are safe to repeat. If tests fail during the transition, the likely causes are mismatched canonical formatting or stale RFC 3339 assertions in docs/tests. Re-run repository-wide searches for `RFC 3339 date`, `RFC 3339 datetime`, `Date.UTC`, and `new Date(` before considering the change complete.

## Artifacts and Notes

Expected evidence after completion:

    cargo test
    npm test

If formatting changes canonicalize an input differently than before, update the roundtrip expectations to the new canonical string rather than preserving the older, narrower format.

## Interfaces and Dependencies

At the end of this change:

- `rust/crates/grc-20/src/util/datetime.rs` must accept signed and expanded years for DATE and DATETIME.
- `typescript/src/util/datetime.ts` must implement date arithmetic directly and must not call `Date.UTC` or `new Date` for DATE/DATETIME parsing or formatting.
- `DATE` and `DATETIME` docs must state ISO 8601 extended strings with expanded years and astronomical year numbering.
- `TIME` remains on the existing RFC 3339-compatible contract.

Revision note: Created this plan to record the DATE/DATETIME expanded-year change and the choice to keep helper symbol names stable while changing their documented format contract.
