# Privacy Rules (always loaded)

`PRIVACY.md` and the website's privacy page are the promise. Code that would make either untrue
does not ship until both are changed and the user has agreed.

- **No telemetry, analytics, crash reporting or usage pings**, opt-in or not, in the app or the
  site. No Sentry, no PostHog, no "anonymous" counters.
- **No network calls by default.** The app works fully offline. The only planned call is the
  manual update check, which runs only when the user presses it, or after they turn on the check
  in settings. It sends nothing but the request itself.
- **No third-party requests.** No CDN fonts, scripts, images or icons; no remote theme gallery;
  the site does not call the GitHub API from the visitor's browser. Assets ship in the bundle.
  The one exception is the edge: Cloudflare fronts the site and adds `NEL`/`Report-To`, so
  browsers may send it network-error reports. That is documented on the privacy page; add no
  other.
- **No accounts, no cookies on the site**, no fingerprinting, no IP logging beyond what
  Cloudflare and the homelab ingress do by default (documented on the privacy page).
- **Logs stay local** and the user can turn them off. Never log launch targets, arguments,
  group names, file paths under the user's home, or environment variables at `info` or above.
  `debug` may log ids; never full paths.
- **Detection reads only what it needs**: app metadata (name, icon path, exec). It never reads
  document history, recent files or browser data.
- **Fixtures are synthetic.** No real home directories, usernames or installed-app lists from
  your machine in tests, docs or issues.
