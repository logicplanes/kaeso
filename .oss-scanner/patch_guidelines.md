# How we would like patches made

`AGENTS.md` at the repository root, and the `AGENTS.md` of the area a patch changes, hold the rules every change follows. The points below are the ones that matter most for security fixes.

## Size and scope of a fix

- Fix the cause, in the place every caller goes through: the right check, the extractor, the guard, the codec or the store query. Do not add a second check at one call site and leave the others open.
- Keep the patch as small as that allows. No refactoring, renaming or formatting of unrelated code.
- Refuse bad input; never drop data silently. A request KPP can't carry goes back to the app as an error in its own format.
- Keep the gateway's invariants: no `unwrap`, `expect`, `panic!` or panicking indexing outside tests; no `unsafe`; secrets stay in their redacting types; money stays `Decimal`; every query on an organization's data filters by the organization from the authenticated context.
- A fix to a Platform API endpoint keeps exactly one right per endpoint. When a fix changes the API's shape, it also updates `openapi.json` (`cargo run -q -p kaeso -- openapi > openapi.json`) and the console.

## Code style

- Format with `cargo fmt` and, for TypeScript, Prettier through `pnpm format`.
- Comments only where the code can't say why, such as a security invariant the types can't hold. No `TODO`, `FIXME`, `XXX` or `HACK`, and no comment that describes the fix's history.
- An error message people see is one short sentence ending with a period, at most 90 characters as written, that names no internal component. A new message people see in the console or an email needs German and Simplified Chinese as well; when a patch can't provide them, say so in the report.

## Tests

- Add a regression test that fails without the fix and passes with it, asserting on what an attacker or a user sees: the status code, the error body, the data that must not come back.
- Gateway fixes: an integration test in the nearest file in `crates/kaeso-server/tests/server`, using `tests/server/common`. KPP fixes: a unit test next to the codec and, for a decoder crash, the crashing input as a fixture.
- Tests are deterministic: no sleeps, injected clocks.
- Run the affected crate's tests with `cargo nextest run -p <crate>` after `.oss-scanner/services start`, and `cargo clippy -p <crate> --all-targets --all-features -- -D warnings`.

## Branch and commit message

- Against the `dev` branch.
- A commit message whose first line says what changed for people who run Kaeso, as a sentence, and whose body says why. Do not edit `CHANGELOG.md` or any version number.
