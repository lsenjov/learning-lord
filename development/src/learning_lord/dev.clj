(ns learning-lord.dev
  (:require [cider.nrepl :as cider]
            [clojure.main :as main]
            [learning-lord.desktop.main :as desktop]
            [nrepl.server :as nrepl]))

(defn -main [& _]
  (with-open [_ (nrepl/start-server :bind "127.0.0.1" :port 7000
                                    :handler cider/cider-nrepl-handler)]
    (desktop/-main)
    (println "nREPL listening on 127.0.0.1:7000")
    (main/repl :init #(do (in-ns 'user)
                          (apply require main/repl-requires)))
    ;; Editors can launch the app with stdin already closed.
    @(promise)))
