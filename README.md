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
| type | fuzzy filter on repo, PR number, title and author |
| `tab` / `shift-tab` | switch list |
| `up` / `down`, `ctrl-p` / `ctrl-n`, `ctrl-k` / `ctrl-j` | move |
| `enter` | open the selected PR in the browser and exit |
| `O` (shift-o) | open the selected PR in the browser and stay |
| `Y` (shift-y) | copy the selected PR URL to the clipboard |
| `N` (shift-n) | copy the selected PR number to the clipboard |
| `ctrl-r` | refetch |
| `esc` / `ctrl-c` | quit |

Cancelling with `esc` exits with status 130, like fzf.

`gh assigned --json` fetches once and prints all three lists as JSON.

## Layout

Drawn inline below your prompt like fzf, using at most 40% of the terminal. The shell scrollback stays visible and the area is cleared on exit.

```
> query
  28/28  mine 28 · review requested 2 · assigned 7
▌ marketing-backend  TAS-320 pnpm を更新                     ✓ approved
  └ marketing-backend  TAS-319 NestJS 11 へ更新               ✓ approved
  marketing-backend  fix: INVALID_OOB_CODE エラー               ✓ review
```

`✓` `✗` `●` show the CI rollup of the latest commit, followed by the review decision. Drafts are dimmed and prefixed with `[draft]`. The author column is shown only on the review-requested and assigned lists.

## Requirements

- [GitHub CLI](https://cli.github.com/) authenticated with `gh auth login`

## License

MIT
