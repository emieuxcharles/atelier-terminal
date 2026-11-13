# Historique de dossiers natif

Demande : remplacer l’import de z par une fonction autonome, livrée dans Atelier, en Rust, légère. Interface conservée : ⌘⇧P, priorité aux projets ouverts et déduplication.

Décisions : moteur Rust (fréquence/récence, 2 000 chemins maximum, persistence dans state.json avec migration). Capturer les ouvertures et changements de dossier du shell zsh initial par un hook événementiel intégré à la configuration temporaire déjà fournie par Atelier. Hook sans processus externe : notification hexadécimale via PTY, jeton propre au panneau. Pas de commande z ni dépendance externe, pas de timer/scanner. Les autres shells conservent l’apprentissage des projets ouverts et des dossiers relevés lors des actions explicites.

1. Tests du moteur (classement, déduplication, plafond, migration/sauvegarde) puis implémentation.
2. Tests d’un vrai zsh sans plugins : cd, espaces/accents, symlinks, pas de répétitions à chaque prompt ; intégrer le hook.
3. Remplacer z_history et son thread par historique mémoire et snapshot à l’ouverture de palette. Sauvegarder sur changement, jamais à chaque rendu inchangé.
4. Vérifier tests, relais SSH, bundle et signature. Documenter ; aucune installation sur le Mac utilisateur.
