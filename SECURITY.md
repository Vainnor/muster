# Security Policy

## Supported versions

Muster is pre-1.0. Only the latest release gets security fixes.

## Reporting a vulnerability

Please report privately through GitHub's
[private vulnerability reporting](https://github.com/Vainnor/muster/security/advisories/new).
Do not open a public issue for a security problem.

Include what you found, how to reproduce it, and the version and OS. You can expect an
acknowledgement within 7 days and a fix or a plan within 30 days for confirmed issues. Credit is
given in the release notes unless you ask otherwise.

## Scope

In scope: the desktop app (IPC and capabilities, process launching, config storage, theme and
group import), the release artifacts and workflows, and the website.

The design and the threats we defend against are in
[docs/THREAT_MODEL.md](docs/THREAT_MODEL.md).

## Release integrity

Each release publishes `SHA256SUMS.txt` beside the artifacts. Builds are not code-signed yet;
see [docs/RELEASING.md](docs/RELEASING.md).
