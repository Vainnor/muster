# Privacy

This is the technical privacy statement for the Muster desktop app and website. The website's
[privacy policy](https://muster.vainnor.com/privacy) says the same in plain language.

## The desktop app

- **No telemetry.** Muster collects no usage data, crash reports or analytics, and has no
  account system.
- **No network access by default.** The app works fully offline. The only network request it can
  make is an update check against GitHub Releases, and only when you press "Check for updates"
  or turn on the check yourself. *(The update check is not built yet; see
  [docs/RELEASING.md](docs/RELEASING.md).)*
- **Local storage only.** Your apps, groups, settings and themes are stored in your OS's
  app-config folder:
  - Linux: `$XDG_CONFIG_HOME/muster` (usually `~/.config/muster`)
  - macOS: `~/Library/Application Support/com.vainnor.muster`
  - Windows: `%APPDATA%\Vainnor\Muster`
  Nothing is uploaded. No passwords, tokens or keys are stored.
- **App detection reads app metadata only**: names, icons and launch commands from the standard
  app locations. It never reads your documents, recent files or browser data.
- **Logs stay on your machine**, can be turned off, and do not record launch targets, arguments
  or file paths.

## The website (muster.vainnor.com)

- No cookies, analytics, trackers or third-party scripts, fonts or images.
- The download button links to GitHub Releases; GitHub's own privacy policy applies there.
- The hosting reverse proxy may keep standard access logs (IP address, time, requested page)
  for operating the server. They are not used for tracking or shared.

## Changes

A change to any of the above is called out in the release notes and updated here and on the
website at the same time.
