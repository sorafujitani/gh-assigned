# gh-assigned

Find the pull requests that need your attention without leaving your terminal. A [GitHub CLI](https://cli.github.com/) extension with fuzzy search, CI status, and keyboard navigation.

- **Mine** — your open PRs, with dependent PRs nested under their base PR.
- **Review requested** — open PRs waiting for your review.
- **Assigned** — open PRs assigned to you.

The picker opens inline below your shell prompt and keeps your scrollback visible. Cached results appear immediately while fresh data loads; CI status arrives separately.

```text
all·fuzzy >
  3/3  mine 3 · review requested 2 · assigned 7
▌ gh-assigned  Add fuzzy search modes                     ✓ approved
  └ gh-assigned  Highlight matching text                  ✓ approved
  ccsession    Preserve metadata when resuming a session  ✓ review
```

## Quick start

Install [GitHub CLI](https://cli.github.com/), then authenticate and install the extension:

```sh
gh auth login
gh extension install sorafujitani/gh-assigned
gh assigned
```

If you are already authenticated, skip `gh auth login`. Type to filter the current list, press `tab` to switch lists, and press `enter` to open a PR.

Release builds target macOS and Linux on Intel/AMD 64-bit and ARM64, and Windows on Intel/AMD 64-bit. Interactive use requires a terminal; use `--json` in scripts.

To update an installed release:

```sh
gh extension upgrade gh-assigned
```

## Keyboard controls

| Key | Action |
| --- | --- |
| type | Filter the current list; matching text is highlighted |
| `ctrl-f` | Cycle search field: all / repo / title / author |
| `ctrl-t` | Cycle match type: fuzzy / substring / exact |
| `tab` / `shift-tab` | Switch list |
| `up` / `down` | Move selection (also `ctrl-p` / `ctrl-n` or `ctrl-k` / `ctrl-j`) |
| `pgup` / `pgdn` | Move ten rows |
| `ctrl-w` / `ctrl-u` / `ctrl-h` | Delete the previous word / entire query / previous character |
| `enter` | Open the selected PR in the browser and exit |
| `O` (shift-o) | Open the selected PR and stay in the picker |
| `Y` (shift-y) | Copy the selected PR URL |
| `N` (shift-n) | Copy the selected PR number |
| `ctrl-r` | Fetch fresh data |
| `F1` / `?` | Show keyboard help (`?` works only with an empty query) |
| `esc` / `ctrl-c` | Quit; `esc` closes help first if it is open |

`O`, `Y`, and `N` are shortcuts, so those uppercase letters are not entered into the search query. Cancelling with `esc` exits with status 130, like fzf.

Run `gh assigned --help` to see the command options and keyboard controls.

## Search examples

The prompt shows the active field and match type, such as `all·fuzzy >`. Searches ignore case and filter the currently selected list.

| To find… | Set the prompt to… | Type… |
| --- | --- | --- |
| PRs matching a shortened repository name | `all·fuzzy >` | `ghas` |
| Titles containing a phrase | `title·substring >` | `fuzzy search` |
| PRs from one repository | `repo·exact >` | `gh-assigned` |

Use `ctrl-f` to choose the field and `ctrl-t` to choose the match type. Exact matching compares the whole field and is available only for a single field; `all` cycles between fuzzy and substring.

The repository field uses the short name (`gh-assigned`, not `sorafujitani/gh-assigned`). The author field is available on the **Review requested** and **Assigned** lists, where authors are displayed.

## Read the list

- Indented rows show PRs whose base branch matches another listed PR's head branch.
- `✓`, `✗`, and `●` indicate successful, failed, and pending CI checks for the latest commit. The review decision follows the CI symbol.
- Draft PRs are dimmed and prefixed with `[draft]`.

The picker uses at most 40% of the terminal and clears its area on exit. The previous result is stored as `gh-assigned/snapshot.json` under your system's user cache directory. Cached data may be stale until the background fetch finishes; press `ctrl-r` to refresh again.

Each list fetches up to **100 open PRs**, ordered by most recently updated, and excludes archived repositories. Lists are fetched across repositories rather than being limited to your current directory.

## Use in scripts

Fetch fresh data once and print all three lists as JSON:

```sh
gh assigned --json
```

The output is an object with a `lists` array in this fixed order:

| Index | List |
| --- | --- |
| `0` | Mine |
| `1` | Review requested |
| `2` | Assigned |

Each PR includes `repo` (owner/name), `number`, `title`, `url`, `author`, `is_draft`, `base_ref`, `head_ref`, `checks`, and `review`.

For example, with [jq](https://jqlang.org/) installed, print the URLs of PRs waiting for your review:

```sh
gh assigned --json | jq -r '.lists[1][].url'
```

`checks` is `Success`, `Failure`, `Pending`, or `None`; `review` is `Approved`, `ChangesRequested`, `Pending`, or `None`. JSON mode also updates the local cache and has the same 100-PR limit per list.

## Linux requirements

Linux binaries support **amd64 (x86-64)** and **arm64 (AArch64)**, with Debian 12
as the minimum tested environment. They require **glibc 2.36+** and
**`libgcc_s.so.1`** (`libc6` and `libgcc-s1` on Debian).

Releases are built on Debian 12 and must pass help-command checks on a clean
Debian 12 image for both architectures before publishing. See the
[build and verification script](scripts/build-linux-release.sh).

Older glibc versions, musl-only systems such as Alpine, and 32-bit Linux are
unsupported. This guarantee applies to releases after v0.2.0.

## Browser and clipboard setup

Opening PRs delegates to GitHub CLI and respects its browser preferences. To troubleshoot launch failures, check `GH_BROWSER`, `BROWSER`, or `gh config get browser`.

Copying uses `pbcopy` on macOS and `clip` on Windows. On Linux, install `wl-copy` (usually provided by `wl-clipboard`) for Wayland, or `xclip` / `xsel` for X11. These commands need access to your desktop session; copying from a remote shell may require additional setup.

## Build from source

With Git, GitHub CLI, and a current stable Rust toolchain installed:

```sh
git clone https://github.com/sorafujitani/gh-assigned.git
cd gh-assigned
cargo build --release --locked
```

On macOS or Linux, link the built executable and install the local extension:

```sh
# Skip this command if the repository already contains this symlink.
ln -s target/release/gh-assigned gh-assigned
gh extension install .
```

On Windows, run the built executable directly:

```powershell
.\target\release\gh-assigned.exe
```

Rebuild after pulling source updates. For local development checks:

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
```

## License

[MIT](LICENSE)
