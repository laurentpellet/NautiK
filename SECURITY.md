
# NautiK — Sécurité et fiabilité

## 1. Objectifs

Garantir :
- Confidentialité des données.
- Intégrité des configurations.
- Disponibilité du moteur.
- Fiabilité des alarmes.
- Protection des interfaces distantes.

## 2. Sécurité réseau

- API liée à 127.0.0.1 par défaut.
- Authentification obligatoire à distance.
- TLS pour les connexions distantes.
- Pas d'exposition Internet automatique.
- Validation des origines WebSocket.
- Limitation des requêtes.
- Gestion des sessions.

## 3. Secrets

- Ne jamais stocker de secrets dans Git.
- Protéger les jetons Signal K.
- Utiliser le coffre du système si disponible.
- Ne jamais écrire de jeton dans les logs.
- Prévoir la révocation des identifiants.

## 4. Tauri

- Permissions minimales.
- Validation des commandes.
- Pas d'exécution arbitraire de commandes.
- Pas d'accès direct au système depuis Svelte.
- Politique CSP restrictive.

## 5. Acquisition

- Vérification des checksums NMEA.
- Validation des données AIS.
- Limites de taille des messages.
- Gestion des messages incomplets.
- Protection contre les flux excessifs.
- Reconnexion avec backoff.

## 6. Navigation

- Identifier les données obsolètes.
- Afficher l'état des sources.
- Conserver les horodatages.
- Ne pas extrapoler une position sans indication.
- Ne pas considérer CPA/TCPA comme une garantie.
- Conserver les limites de précision.

## 7. Alarmes

Les alarmes fonctionnent dans le moteur Rust.

États :
- Inactive.
- Active.
- Acquittée.
- Résolue.

Fonctions :
- Hystérésis.
- Temporisation.
- Historique.
- Journalisation.
- Notification sonore locale.

Une fermeture de l'interface ne doit pas
désactiver le service d'alarme.

## 8. Embarqué

- Service systemd.
- Redémarrage automatique.
- Watchdog.
- Limitation des ressources.
- Rotation des journaux.
- Surveillance du disque.
- Gestion des coupures réseau.

## 9. Sauvegardes

Sauvegarder :
- Base SQLite.
- Configuration.
- Routes.
- Waypoints.
- Traces.

Ne pas copier directement une base SQLite
active sans méthode de sauvegarde cohérente.

## 10. Mise à jour

- Versions identifiées.
- Paquets signés.
- Contrôle des dépendances.
- Journal des changements.
- Possibilité de retour arrière.

## 11. Sûreté maritime

NautiK est une aide à la navigation.

Il ne doit pas être présenté comme un
système ECDIS certifié.

Les alarmes et calculs doivent être validés
par des tests et essais en conditions réelles.

Le logiciel ne remplace pas :
- Les cartes officielles requises.
- La veille humaine.
- Les règles COLREG.
- Les équipements de sécurité réglementaires.
