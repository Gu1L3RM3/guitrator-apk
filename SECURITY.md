# Security Policy

## Supported versions

Only the latest release receives fixes.

## Reporting a vulnerability

Please do **not** open a public issue. Use [private vulnerability reporting](https://github.com/Gu1L3RM3/guitrator-apk/security/advisories/new). You can expect a first reply within a few days.

## Things worth knowing

- `download` fetches only from `f-droid.org` over HTTPS and checks the file is a valid APK, but it does not verify F-Droid's index hash.
- The binaries are not code-signed.
- On first run the app may download Android platform-tools from `dl.google.com`.
