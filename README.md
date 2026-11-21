# Learning Lord

Learning Lord is a Clojure desktop game scaffold. The Swing window displays the initial town and its one peasant.

## Run the desktop app

Use the Clojure CLI with JDK 17 or newer and a graphical desktop.

```sh
cd projects/learning-lord
clojure -M:run
```

## Development REPL

From the repository root, run `clojure -M:dev` or `bin/dev.sh`. This opens the Swing window and starts a terminal REPL and an nREPL server with CIDER middleware in the same JVM. In Emacs, run `M-x cider-connect-clj` and connect to `127.0.0.1:7000`. Closing the window or sending Ctrl+D leaves nREPL running. Press Ctrl+C in the terminal to stop the development session.

Inspect the initial town from either REPL.

```clojure
(require '[learning-lord.town.interface :as town])
(town/initial-town)
```

Run the Polylith workspace check from the repository root with `clojure -M:poly check`.

## Formatting

The pre-commit hook uses `~/bin/zprint` to format staged `.clj` and `.cljc` files. Enable it once per clone with `git config --local core.hooksPath .githooks`.
