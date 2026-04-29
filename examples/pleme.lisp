;; pleme.lisp — the canonical Fleet declaration.
;;
;; Render: cracha render fleet examples/pleme.lisp --out out/
;; Emits one subdir per cluster + a fleet-summary.md at the root.
;;
;; Note: nested structs (Cluster) are authored as kwargs sublists,
;; not as standalone (defcluster …) calls — that's how
;; #[derive(TataraDomain)] composes Vec<Nested> per the canonical
;; pattern.

(deffleet
  :name "pleme"
  :tld  "quero.cloud"
  :passaporte (:host "auth.quero.cloud"   :on-cluster "rio")
  :cracha     (:host "cracha.quero.cloud" :on-cluster "rio")
  :clusters
  ((:name     "rio"
    :location "bristol"
    :label    "Bristol home edge"
    :country  "TN, USA"
    :role     control-plane
    :saguao   (:vigia #t :varanda #t :passaporte #t :cracha #t))
   (:name     "mar"
    :location "parnamirim"
    :label    "Parnamirim home edge"
    :country  "RN, Brazil"
    :role     consumer
    :saguao   (:vigia #t :varanda #t :passaporte #f :cracha #f))))
