# Chiffrement, signatures et accès aux budgets

Ce qui met en œuvre l'[ADR 0003](0003-identite-et-recuperation.md), avec des primitives standard, en Rust pur (`ed25519-dalek` et `x25519-dalek` sous BSD-3-Clause ; `chacha20poly1305`, `argon2`, `sha2`, `hmac` sous MIT ou Apache-2.0). Le code est dans `crates/bougette-store/src/{crypto,message,access,members,backup}.rs`.

## Décisions

- **Identité d'un membre** : une graine secrète de 32 octets, d'où viennent une clé de signature Ed25519 (sa partie publique est l'identifiant du membre) et une clé X25519 pour recevoir les clés des budgets. C'est la graine qui passe d'un appareil à l'autre (`device code` / `device link`) et dans la sauvegarde.
- **Messages** : chaque changement d'un champ est signé par son auteur (Ed25519, sur tous les champs du message). Son contenu est chiffré avec une clé du budget (XChaCha20-Poly1305, nonce aléatoire de 24 octets), lié à son horodatage et à son budget : recopié dans un autre message, il ne se déchiffre plus.
- **Changements d'accès en clair** : création, rôles, clés scellées, demandes d'adhésion. Ils sont signés, mais pas chiffrés : le serveur doit les lire pour savoir à qui transmettre chaque budget.
- **Créateur prouvé** : l'identifiant d'un budget est un UUID v8 tiré de SHA-256(clé du créateur, nonce). Personne d'autre ne peut prétendre l'avoir créé ; le créateur en est le premier Admin.
- **Rôles évalués au moment du changement** : chaque appareil rejoue les messages dans l'ordre des horodatages. Un changement compte si son auteur avait le droit de le faire à ce moment-là (Admin ou Éditeur pour le contenu ; un Lecteur ne peut changer que son propre nom affiché). Un changement non autorisé est **ignoré, pas refusé** : le message reste dans le journal, sinon les préfixes de l'[ADR 0004](0004-vecteur-de-versions-plutot-que-merkle.md) seraient cassés.
- **Clés de budget** : numérotées au hasard. Chacune est scellée pour chaque membre : X25519 avec une paire de clés à usage unique, puis SHA-256, puis XChaCha20-Poly1305. On chiffre avec la dernière clé annoncée qu'on détient.
  - Au départ d'un membre, un Admin tire une nouvelle clé que le partant ne reçoit pas : il garde ce qu'il avait, rien de plus (périmètre).
  - Si deux Admins le font en même temps, il y a deux clés, distribuées toutes les deux.
  - Un nouveau membre reçoit toutes les clés, donc tout l'historique.
- **Invitations** : un secret aléatoire, dont l'appareil de l'Admin garde la trace (budget, rôle, expiration, usage unique). Deux clés en sont dérivées :
  - un **jeton**, que le serveur reçoit pour laisser entrer l'appareil invité ;
  - une **clé de preuve**, qui sert à la demande d'adhésion (HMAC de l'identifiant et de la clé de réception du nouveau membre).

  Le serveur ne voit jamais la clé de preuve : il ne peut pas fabriquer une demande valable pour un membre de son choix. L'appareil de l'Admin qui a créé l'invitation accepte seul la demande, à sa prochaine synchronisation.
- **Chaîne par appareil contre l'antidatage** : l'horodatage est choisi par l'appareil qui écrit, donc un membre retiré à midi pourrait dater un changement de 11 h 59.
  - Chaque message nomme le hachage du message précédent du même appareil dans le même budget. Ce champ est signé : personne ne peut insérer un message dans le passé d'une chaîne.
  - Un changement de rôle (attribution, changement, retrait) nomme les hachages des derniers messages du membre que son auteur avait reçus, appareil par appareil.
  - Au rejeu, un message de ce membre daté d'avant le changement ne compte que s'il fait partie de ces chaînes ; daté d'après, il est jugé avec le nouveau rôle. Un message antidaté n'y est pas : il est ignoré (`access::cut_off`).
- **Appareils** : un appareil prouve qu'il appartient à un membre avec la signature, par ce membre, de son identifiant iroh. iroh authentifie déjà cet identifiant pendant la poignée de main TLS.
- **Serveur** : il garde les messages et les sauvegardes, et transmet à chaque appareil les budgets dont son membre fait partie. L'opérateur gère les noms d'utilisateur et les appareils (`invite`, `revoke`, `list`).
- **Sauvegarde** : la graine, le nom affiché et toutes les clés de budget, chiffrés avec une clé tirée de la phrase de récupération par Argon2id (64 Mio, 3 passes, sel aléatoire ; paramètres stockés avec la sauvegarde).
  - La phrase est générée (160 bits, 8 groupes de 4 symboles de Crockford) ou choisie ; une phrase choisie doit avoir au moins 4 mots et 20 lettres.
  - L'appareil garde la clé tirée de la phrase, pour mettre la sauvegarde à jour à chaque synchronisation sans redemander la phrase.

## Ce que voit le serveur

Les identifiants de budget, les horodatages, les auteurs (clés publiques), les membres et rôles de chaque budget, la taille des messages, les noms d'utilisateur et le nombre d'appareils. **Jamais** : les noms de budgets, de postes ou de catégories, les montants, les notes, les noms affichés. Le test de bout en bout le vérifie sur le fichier du serveur.

## Limites connues

- **Changements d'un membre retiré encore en route** : ceux qu'il a faits juste avant son retrait, mais que l'Admin n'avait pas encore reçus, sont écartés avec les messages antidatés. On perd des changements légitimes plutôt que d'en accepter de faux.
- **Appareil inconnu de l'Admin** : la coupure ne nomme que les appareils du membre dont l'Admin a reçu des messages. Ceux d'un autre appareil, datés d'avant le changement, sont jugés avec le nouveau rôle : écartés après un retrait, acceptés après une promotion.
- **Coupures elles-mêmes** : les changements de rôle valides sont déterminés par un premier rejeu, qui ne tient pas encore compte des coupures. Un Admin retiré qui antidaterait un changement de rôle pourrait donc influencer ce premier rejeu, sans que son changement compte au final. C'est un cas étroit : il faudrait un point fixe pour le fermer.
- **Pas de confidentialité persistante** (*forward secrecy*) dans un budget : une clé couvre tout jusqu'au prochain départ d'un membre.
- **Clés en clair sur l'appareil**, dans sa base SQLite : leur protection est celle du système (chiffrement du disque, verrouillage).
- **Seul l'appareil qui a créé une invitation peut l'accepter** : si cet appareil est perdu avant, il faut en refaire une.
- **Force d'une phrase choisie** : la règle (4 mots, 20 lettres) est grossière ; un estimateur comme zxcvbn ferait mieux.
- **Pas de rotation de la graine d'un membre** : si elle fuit, il faut créer une nouvelle identité et se faire réinviter.

## Options écartées

- **Matrix / Megolm (vodozemac)** : voir l'[étude](../research/matrix-as-sync.md). Leur modèle de sessions est lié aux salons ; ce qu'il faut ici tient en quelques primitives.
- **Chiffrer aussi l'accès** : le serveur devrait alors envoyer tous les budgets à tous ses utilisateurs, qui stockeraient les budgets chiffrés des autres. Il fuirait autant de métadonnées entre utilisateurs, et prendrait plus de place.
- **Rejeter les messages non autorisés à la réception** : cela casse les préfixes, et le verdict dépendrait de l'ordre d'arrivée des messages.
