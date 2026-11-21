# Development REPL

Connect to the running game at `127.0.0.1:7000` using nREPL. In Emacs, use `M-x cider-connect-clj`. If the development session is stopped, run `bin/dev.sh` or `clojure -M:dev` from the repository root. Both start Swing, a terminal REPL, and CIDER middleware in the same JVM.

Inspect the initial town from either REPL.

```clojure
(require '[learning-lord.town.interface :as town])
(town/initial-town)
```

Closing the window or sending Ctrl+D leaves nREPL running. Ctrl+C in the launching terminal stops the session.

## Formatting

Enable the pre-commit hook in a fresh clone with `git config --local core.hooksPath .githooks`. The hook uses `~/bin/zprint` for staged `.clj` and `.cljc` files.

When the hook reformats a file, review and stage the result, then retry the commit. The hook only rewrites files whose working copy matches the staged bytes. Partial staging or Git line-ending conversion can require formatting and staging the file yourself.
