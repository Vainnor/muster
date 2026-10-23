# Privacy

This is the technical privacy statement for the Muster desktop app and website. The website's
[privacy policy](https://muster.vainnor.com/privacy) says the same in plain language.

## The desktop app

- **No telemetry.** Muster collects no usage data, crash reports or analytics, and has no
  account system.
- **No network access.** The app works fully offline and makes no network requests. The update
  check is compiled out of release builds (`docs/RELEASING.md`); if it ships, it will run only
  when you press "Check for updates" or turn the check on, and this file will say so first.
- **The window is the OS's own web view**: WebKit on macOS, WebView2 on Windows, WebKitGTK on
  Linux. On Windows, Muster starts WebView2 with Microsoft SmartScreen turned off, so the pages
  it shows are not sent to Microsoft for checking. Keeping the web view itself up to date is
  the OS's job (on Windows, Microsoft Edge's updater), not Muster's. On macOS, watching Muster
  and its WebKit helper processes for 30 seconds with the window open showed no network
  connections.
- **Local storage only.** Your apps, groups, settings and themes are stored in your OS's
  app-config folder:
  - Linux: `$XDG_CONFIG_HOME/com.vainnor.muster` (usually `~/.config/com.vainnor.muster`)
  - macOS: `~/Library/Application Support/com.vainnor.muster`
  - Windows: `%APPDATA%\com.vainnor.muster`
  The file holds the apps you added by hand, your groups, a copy of each detected app a group
  uses (name and launch command), the command lines you approved to run, and your settings. Nothing is uploaded. No passwords, tokens
  or keys are stored.
- **App detection reads app metadata only**: names, icons and launch commands from the standard
  app locations (`.desktop` files on Linux, `.app` bundles' `Info.plist` on macOS, Start Menu
  shortcuts on Windows). It runs when you open or refresh the app list, never on a timer, and
  keeps the result in memory. It never reads your documents, recent files or browser data.
- **No log files.** When something fails, Muster prints a short message to its standard error
  output, which is only visible if you started it from a terminal. The message never includes
  launch targets, arguments or file paths.

## The website (muster.vainnor.com)

- No cookies, analytics, trackers or third-party scripts, fonts or images.
- The download button links to GitHub Releases; GitHub's own privacy policy applies there.
- The hosting reverse proxy may keep standard access logs (IP address, time, requested page)
  for operating the server. They are not used for tracking or shared.
- The site is served through Cloudflare, which sees every request, including your IP address,
  under [Cloudflare's privacy policy](https://www.cloudflare.com/privacypolicy/). Cloudflare
  adds Network Error Logging headers (`NEL`, `Report-To`) to each response. They do not change
  the page, but a browser that supports them may send Cloudflare a short report when a request
  to the site fails, such as the page address and the kind of error.

## Changes

A change to any of the above is called out in the release notes and updated here and on the
website at the same time.

## Contact

Questions about privacy: muster@vainnor.com. Security issues go through
[private vulnerability reporting](SECURITY.md) instead.
