# Testing Rules (always loaded)

## What gets tested
- **All of `muster-core`** has unit tests beside the code (`#[cfg(test)] mod tests`): every
  public function, every validation rule (one accepting and one rejecting case per rule), every
  error variant a caller can see.
- **Integration tests** (`crates/muster-core/tests/`) drive public APIs across modules: store
  round-trips through a temp dir, a group resolving to launch plans, theme import → validate →
  CSS variables.
- **Detection is tested against fixture directories**, never the real machine:
  `crates/muster-core/tests/fixtures/{linux,macos,windows}/` hold synthetic `.desktop` files,
  `.app` bundles (`Info.plist`), and Start Menu shortcuts. Detection functions take the roots to
  scan as parameters so tests pass the fixture path.
- **Launching is tested through the plan, not the process.** `launch::plan()` returns the
  program, arguments and working dir; tests assert on the plan. Only one ignored smoke test may
  spawn a process, and only `true`/`cmd /c exit 0`.
- **The desktop shell** keeps commands thin; their logic is core and tested there. The React UI
  has vitest tests for logic in `desktop/src/lib/` and component behavior that matters.
- **The website** has tests for anything computed (OS detection from the User-Agent, security
  headers present on every response).

## How tests are written
- Name says the behavior: `rejects_url_with_javascript_scheme`, not `test_url_2`.
- Assert the exact value, not a weaker proxy (`is_err()` → `matches!(e, Error::Scheme(_))`).
- No network, no real home dir, no clock dependence: inject paths and time.
- A test for a guard is proven by breaking the guard and watching it go red, at least once,
  before you trust it.

## Reading results
- Read the `test result:` line of every binary, not the exit code. A sharply lower test count
  means a binary failed to build or a filter excluded too much.
- Say which tiers ran. The Windows and macOS CI jobs and any GUI behavior do not run locally on
  Linux; say so rather than implying they passed.
