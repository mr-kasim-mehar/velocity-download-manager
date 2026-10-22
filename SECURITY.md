# Security policy

## Supported versions

Security fixes are provided for the latest published Velocity release.

## Reporting a vulnerability

Please do not open a public issue for a suspected vulnerability. Use GitHub's
**Security → Report a vulnerability** feature for this repository. Include the
affected version, reproduction steps, impact, and any suggested remediation.

Muhammad Qasim will acknowledge a complete report as soon as practical. Please
allow reasonable time for investigation and remediation before public
disclosure.

## Release verification

Published release assets include a `SHA256SUMS.txt` file. Windows Authenticode
signing is performed only when the repository's SignPath integration is
configured. Unsigned Windows packages are explicitly named `*-unsigned.*`.
