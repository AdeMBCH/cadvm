# cadvm

**CAD Version Manager** — local-first version control, geometric diff,
verification and an MCP server for **STEP / STP / STL / OBJ** files. Built to sit
under AI-generated CAD: snapshot each iteration, diff it, and `verify` it
(pass/fail) — from the CLI, in CI, or as MCP tools an agent calls.

```bash
cargo install cadvm
cadvm --help
cadvm mcp        # run the MCP server (stdio) for AI agents
```

The geometric diff is pure Rust for STL/OBJ (no dependencies) and uses an
Open CASCADE helper for STEP/STP. See the docs and source for details.

## Links

- Documentation: <https://adembch.github.io/cadvm/>
- Repository: <https://github.com/AdeMBCH/cadvm>
- cadvm for AI agents: <https://adembch.github.io/cadvm/ai.html>
- MCP Registry name: `mcp-name: io.github.AdeMBCH/cadvm`

## License

Prosperity Public License 3.0.0 — free for noncommercial use, 30-day commercial
trial. See the repository for details.
