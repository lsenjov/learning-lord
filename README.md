# Learning Lord

Learning Lord is a Clojure desktop game scaffold. The Swing window displays the initial town and its one peasant.

## Run the desktop app

Use the Clojure CLI with JDK 17 or newer and a graphical desktop.

```sh
cd projects/learning-lord
clojure -M:run
```

## Development REPL

From the repository root, run `clojure -M:dev`. Inspect the initial town in the REPL.

```clojure
(require '[learning-lord.town.interface :as town])
(town/initial-town)
```

Run the Polylith workspace check from the repository root with `clojure -M:poly check`.
