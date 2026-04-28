;; family.lisp — canonical example AccessPolicy for the saguão fleet.
;;
;; Edit this form to grant or revoke access for family members. Render
;; via `cracha render examples/family.lisp` (Phase 5+) into a typed
;; AccessPolicy CRD; commit the rendered YAML to
;; k8s/clusters/<control-plane-cluster>/access-policies/.

(defcrachá
  :name family
  :members [drzln cousin wife mom dad]
  :grants
  [;; Operator — full access fleet-wide.
   (grant :user drzln
          :locations [* ]
          :clusters  [* ]
          :services  [* ]
          :verbs     [* ])

   ;; Wife — read+write to photos / jellyfin / notes / paperless on
   ;; rio (Bristol) and mar (Parnamirim) once mar exists.
   (grant :user wife
          :locations [bristol parnamirim]
          :clusters  [rio mar]
          :services  [photos jellyfin notes paperless]
          :verbs     [read write])

   ;; Cousin — read+write to chat / photos / jellyfin on rio only.
   (grant :user cousin
          :locations [bristol]
          :clusters  [rio]
          :services  [chat photos jellyfin]
          :verbs     [read write])

   ;; Parents — read-only photos at parnamirim.
   (grant :user mom
          :locations [parnamirim]
          :clusters  [mar]
          :services  [photos]
          :verbs     [read])

   (grant :user dad
          :locations [parnamirim]
          :clusters  [mar]
          :services  [photos]
          :verbs     [read])])
