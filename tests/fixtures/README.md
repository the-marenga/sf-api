# Parser replay fixtures

These are small, synthetic cached `Response` JSON files. They contain no account
or player data.

- `.login` files initialize a `GameState`.
- `.response` files are applied to every cached login in the directory.

Run them locally with:

```sh
cargo run --example replay_cache -- --strict tests/fixtures
```

The debug CI job runs the same command. Add focused fixtures here when a parser
migration needs a reproducible regression case.
