;; mar.lisp — typed Cluster declaration for the Parnamirim home edge.
;;
;; Render: cracha render cluster examples/mar.lisp --out out/mar
;; Then place each emitted file into the repo it names.
;;
;; Convention: strings are quoted; bare symbols are enum variants
;; (e.g. `consumer` is ClusterRole::Consumer).

(defcluster
  :name     "mar"
  :location "parnamirim"
  :label    "Parnamirim home edge"
  :country  "RN, Brazil"
  :role     consumer
  :saguao   (:vigia #t :varanda #t :passaporte #f :cracha #f)
  :ssh-user "luis")
