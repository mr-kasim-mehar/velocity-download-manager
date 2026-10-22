# Windows code signing with SignPath Foundation

Velocity is prepared for free open-source code signing through SignPath
Foundation. The workflow never stores a private signing key in this repository
or in GitHub Actions. SignPath performs signing in its managed service after it
verifies the GitHub Actions build origin.

Until the project is accepted and configured, Windows release files are clearly
named `*-unsigned.exe` and `*-unsigned.msi`. A checksum does not replace a
trusted Authenticode signature, and a self-signed certificate does not remove
Windows trust warnings for public users.

## One-time setup

1. Keep this repository public, maintain the MIT license, public project
   homepage/README, security policy, and public GitHub Releases download page.
2. Apply at <https://signpath.org/apply> using:
   - Project: `Velocity Download Manager`
   - Repository: `https://github.com/mr-kasim-mehar/velocity-download-manager`
   - Download page: `https://github.com/mr-kasim-mehar/velocity-download-manager/releases`
   - Build system: GitHub Actions
   - Release workflow: `.github/workflows/release.yml`
3. After approval, create or copy the values shown by the SignPath project and
   signing policy into GitHub **Settings → Secrets and variables → Actions**.
4. Add `SIGNPATH_API_TOKEN` as a repository secret.
5. Add these repository variables:
   - `SIGNPATH_ORGANIZATION_ID`
   - `SIGNPATH_PROJECT_SLUG`
   - `SIGNPATH_SIGNING_POLICY_SLUG`
   - `SIGNPATH_ARTIFACT_CONFIGURATION_SLUG`
6. Configure the SignPath artifact configuration to Authenticode-sign every
   `.exe` and `.msi` contained in the uploaded GitHub artifact. Sign both the
   installer and relevant embedded executable files when the configuration
   supports it.

## Release process

1. Update the same semantic version in `package.json`, `src-tauri/Cargo.toml`,
   and `src-tauri/tauri.conf.json`.
2. Commit the release changes.
3. Create and push a matching signed Git tag, such as `v0.4.2`.
4. GitHub Actions builds Windows and Linux packages. If SignPath is configured,
   the Windows artifact is submitted for signing and the workflow waits for the
   required approval. The final GitHub Release contains the signed installers
   and `SHA256SUMS.txt`.
5. Download the release asset on Windows and verify it with:

   ```powershell
   Get-AuthenticodeSignature .\Velocity_*_x64-setup.exe | Format-List
   ```

The status must be `Valid`, and the signer must match the identity issued for
the SignPath Foundation project. Windows SmartScreen reputation is a separate,
gradual system; no legitimate free process can guarantee that every new binary
will immediately avoid all reputation prompts.

Official references: [SignPath Foundation](https://signpath.org/),
[SignPath GitHub integration](https://docs.signpath.io/trusted-build-systems/github),
and [Microsoft code-signing options](https://learn.microsoft.com/windows/apps/package-and-deploy/code-signing-options).
