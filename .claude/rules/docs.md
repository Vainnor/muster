# Docs Rules (always loaded)

A doc that disagrees with the code is a bug in the same change. Update in the same commit:

| Change | Update |
| --- | --- |
| New crate, module boundary, dependency or data flow | `docs/ARCHITECTURE.md` (and § Dependencies with the reason) |
| New capability, command, CSP source, plugin, file read/write location, import format, or anything crossing a trust boundary | `docs/THREAT_MODEL.md` |
| Theme schema field, token, preset | `docs/THEMING.md`, `themes/theme.schema.json`, the Rust validator, every preset |
| Anything that sends data off the machine, stores new data, or changes logging | `PRIVACY.md` and `web/src/app/privacy.rs` |
| Build, signing, release workflow | `docs/RELEASING.md` |
| User-visible feature | `README.md` feature list and `web/src/app/features.rs` |
| Security reporting or supported versions | `SECURITY.md` |

- Docs are for maintainers unless they say otherwise; write for one reader.
- Describe what is true now. Plans go in issues, not docs; history goes in commits.
- Word choice: `writing.md`.
