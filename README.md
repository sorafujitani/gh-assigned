# gh-assigned

A fast fuzzy viewer for the pull requests that need you. Runs as a GitHub CLI extension.

- **Mine**: your open PRs, with stacked PRs nested under the PR they are based on
- **Review requested**: PRs waiting for your review
- **Assigned**: PRs assigned to you

The last result is cached and shown instantly on start while a fresh fetch runs in the background. PR lists arrive first; CI status fills in a few seconds later.

## Install

```sh
gh extension install sorafujitani/gh-assigned
```

Or build from source:

```sh
cargo build --release
ln -s target/release/gh-assigned gh-assigned
gh extension install .
```

## Usage

```sh
gh assigned
```

| Key | Action |
| --- | --- |
| type | filter on what is shown (repo, title, author); hits are highlighted |
| `ctrl-f` | cycle search field: all / repo / title / author |
| `ctrl-t` | cycle match type: fuzzy / substring / exact (exact matches the whole field, so it needs a single field) |
| `tab` / `shift-tab` | switch list |
| `up` / `down`, `ctrl-p` / `ctrl-n`, `ctrl-k` / `ctrl-j` | move |
| `enter` | open the selected PR in the browser and exit |
| `O` (shift-o) | open the selected PR in the browser and stay |
| `Y` (shift-y) | copy the selected PR URL to the clipboard |
| `N` (shift-n) | copy the selected PR number to the clipboard |
| `ctrl-r` | refetch |
| `F1` / `?` | show the key list (`?` only on an empty prompt); `esc` closes |
| `esc` / `ctrl-c` | quit |

The prompt is prefixed with the current search mode, for example `all·fuzzy >` or `repo·exact >`.

Cancelling with `esc` exits with status 130, like fzf.

`gh assigned --json` fetches once and prints all three lists as JSON.

## Layout

Drawn inline below your prompt like fzf, using at most 40% of the terminal. The shell scrollback stays visible and the area is cleared on exit.

```
all·fuzzy > query
  28/28  mine 28 · review requested 2 · assigned 7
▌ gh-assigned  Add fuzzy search modes                 ✓ approved
  └ gh-assigned  Highlight matching text                 ✓ approved
  ccsession    Preserve metadata when resuming a session  ✓ review
```

`✓` `✗` `●` show the CI rollup of the latest commit, followed by the review decision. Drafts are dimmed and prefixed with `[draft]`. The author column is shown only on the review-requested and assigned lists.

## Requirements

- [GitHub CLI](https://cli.github.com/) authenticated with `gh auth login`

### Linux

Linux binaries support **amd64 (x86-64)** and **arm64 (AArch64)**, with Debian 12
as the minimum tested environment. They require **glibc 2.36+** and
**`libgcc_s.so.1`** (`libc6` and `libgcc-s1` on Debian).

Releases are built on Debian 12 and must pass help-command checks on a clean
Debian 12 image for both architectures before publishing. See the
[build and verification script](scripts/build-linux-release.sh).

Older glibc versions, musl-only systems such as Alpine, and 32-bit Linux are
unsupported. This guarantee applies to releases after v0.2.0.

## License

MIT
