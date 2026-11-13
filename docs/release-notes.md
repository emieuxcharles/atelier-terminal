Première version expérimentale d'Atelier, terminal macOS organisé par projets.

- ⌘B affiche ou masque la barre des projets.
- Projets persistants, panneaux multiples, palette de commandes et thèmes clair/sombre.
- Repères de sessions SSH/Docker et commandes Docker Compose à la demande.
- Installation : télécharger le DMG arm64, l'ouvrir et glisser Atelier dans Applications.
- Mac Apple Silicon (M1 et suivants). Minimum de compilation : macOS 11 ; fonctionnement non validé sur toutes les versions.
- Signature ad hoc, sans notarisation Apple. Si macOS bloque l'ouverture : Réglages Système → Confidentialité et sécurité → Ouvrir quand même, après une première tentative de lancement et seulement si vous faites confiance au téléchargement.
- La fermeture termine les shells locaux. Le redémarrage restaure la disposition, pas les processus.

Le fichier `.sha256` fournit la somme de contrôle du DMG.
