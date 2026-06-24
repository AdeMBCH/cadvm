# Publishing to the MCP Registry

cadvm ships a [`server.json`](https://github.com/AdeMBCH/cadvm/blob/main/server.json)
describing it for the official [MCP Registry](https://registry.modelcontextprotocol.io)
— the catalog MCP clients use to discover and install servers. This page is the
maintainer checklist to (re)publish it.

The registry hosts **metadata only**; the runnable artifact must live in a public
package registry. cadvm uses the **cargo** path (crates.io).

## Prerequisites

- A crates.io account + API token (`cargo login`).
- A GitHub account (namespace `io.github.AdeMBCH/*`).
- The publisher CLI: `brew install mcp-publisher`.

## 1. Publish the crates to crates.io

cadvm is a workspace; publish bottom-up so dependencies resolve:

```bash
cargo publish -p cadvm-store
cargo publish -p cadvm-core     # after cadvm-store is live
cargo publish -p cadvm          # the binary crate (the MCP server)
```

Ownership verification: the `cadvm` crate's README already carries the visible
marker `mcp-name: io.github.AdeMBCH/cadvm`, which crates.io serves and the
registry checks. Keep it intact.

## 2. Keep versions in sync

The release tag, the workspace `version`, the crate on crates.io, and the two
`version` fields in `server.json` must all match (e.g. `0.1.1`).

## 3. Publish to the registry

```bash
mcp-publisher login github      # browser OAuth → grants io.github.AdeMBCH/*
mcp-publisher publish           # reads ./server.json
```

Verify it is listed:

```bash
curl -s "https://registry.modelcontextprotocol.io/v0/servers?search=cadvm" | jq .
```

## Optional: publish from CI (OIDC, no browser)

In a release workflow, authenticate with GitHub OIDC instead of a browser:

```yaml
permissions:
  id-token: write
  contents: read
steps:
  - run: |
      curl -fsSL https://github.com/modelcontextprotocol/registry/releases/latest/download/mcp-publisher_linux_amd64.tar.gz | tar -xz
      ./mcp-publisher login github-oidc
      ./mcp-publisher publish
```

(Run this after the crates are published to crates.io for that version.)

## Notes

- Once a version is published it cannot be re-published; bump the version.
- The registry currently restricts cargo packages to `https://crates.io`.
- A no-toolchain **MCPB** path (prebuilt binary from GitHub Releases) is an
  alternative for end users without Rust — a possible future addition.
