(ns learning-lord.desktop.main
  (:require [learning-lord.town.interface :as town])
  (:import (java.awt GridLayout)
           (javax.swing JFrame JLabel JPanel SwingUtilities WindowConstants)))

(defn- show-town
  [{:keys [peasants]}]
  (let [panel (JPanel. (GridLayout. 0 1))]
    (.add panel (JLabel. "Town"))
    (doseq [{:keys [id]} peasants] (.add panel (JLabel. (str "Peasant " id))))
    (doto (JFrame. "Learning Lord")
      (.setDefaultCloseOperation WindowConstants/DISPOSE_ON_CLOSE)
      (.setContentPane panel)
      (.setSize 320 200)
      (.setVisible true))))

(defn -main [& _] (SwingUtilities/invokeLater #(show-town (town/initial-town))))
