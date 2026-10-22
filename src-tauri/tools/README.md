# Worker binaries

These helper executables are bundled with Velocity desktop releases so media and
torrent downloads work without separate user setup. Generated binaries are
intentionally excluded from Git: some exceed GitHub's file-size limit, and
committing them would make security review and upgrades harder.

Run `scripts/prepare-windows-tools.ps1` on Windows. The script downloads pinned
versions and verifies both archives and final executables against known SHA-256
values. The GitHub release workflow uses the same script. Linux release runners
use `scripts/prepare-linux-tools.sh`.

`SHA256SUMS` records the Windows helper executables used for the current build.
