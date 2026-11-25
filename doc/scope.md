# Périmètre de Bougette

Décisions de périmètre prises pendant la séance de cadrage des 14 et 15 novembre 2026. Le vocabulaire est celui de [GLOSSARY.md](../GLOSSARY.md) ; les décisions structurantes ont leur ADR dans [adr/](adr/).

## v1 : dedans

- **Budgets** individuels et partagés, entre membres quelconques : couple, colocation, amis. Pas de rôle familial ; un enfant est un membre comme un autre.
- **Rôles** par budget : Admin, Éditeur, Lecteur.
- **Le prévu** :
  - postes : catégorie, montant, échéancier, dates de validité ; un changement s'applique à partir d'une date ;
  - revenus de toute sorte, y compris externes (CAF) ;
  - catégories propres à chaque budget, à partir d'une liste par défaut modifiable, sans sous-catégories.
- **Cagnottes** dans un budget, avec objectif facultatif et dotations prévues. Un investissement est une simple cagnotte.
- **Le réel** :
  - saisie manuelle des opérations ; la catégorie est obligatoire, le poste facultatif, et plusieurs opérations peuvent se rattacher à un même poste ;
  - comparaison prévu/réel.
- **Saisi une fois, visible dans deux budgets** : chaque versement, et chaque avance (opération d'un budget partagé payée par un membre), apparaît automatiquement comme sortie dans le budget individuel du membre.
  - Un versement prévu est un seul poste : en l'ajoutant depuis son budget individuel, le membre indique où va l'argent (une cagnotte, ou un autre budget, partagé ou non, comme un appartement en location). Le poste est écrit dans le budget qui reçoit, comme un revenu payé par le membre, et se voit comme une dépense dans son budget individuel ; on le modifie depuis l'un ou l'autre, et l'opération qui le réalise compte une seule fois dans chacun.
  - Où va l'argent d'une dépense prévue (aucune cagnotte, une cagnotte, un autre budget) se change à partir d'une date : le poste est coupé en deux, la partie d'avant garde son passé et ses opérations là où elle était, la partie d'après (mêmes montants, mêmes échéances) va à la nouvelle destination. Refusé si des opérations réalisent déjà le poste à partir de cette date.
- **Soldes** : solde mensuel et solde cumulé, prévus et réels, calculés par l'app (pas de solde bancaire).
- **Vue calendrier mensuelle** : réel pour les mois passés, projection pour le mois en cours, prévu pour les mois futurs, chacun avec sa couleur. Le mois est **toujours** le mois calendaire (du 1er à la fin du mois).
- **Dépassements** : une cagnotte peut passer en négatif, rien n'est bloqué. Tout dépassement est visible, y compris par une couleur dans la vue calendrier.
- **Excédent du mois** : il reste dans le solde cumulé, rien d'automatique ; un Éditeur peut le verser dans une cagnotte à la main.
- **Un poste = un montant par échéancier** : les variations saisonnières passent par un changement daté ou par l'écart.
- **Échéanciers** : mensuel le jour N (le 31 devient le dernier jour du mois), tous les N mois, annuel à une date, hebdomadaire, ponctuel. Un poste ponctuel n'est pas lissé : pour l'anticiper, on passe par une cagnotte (à revoir selon les retours).
- **Dates** : calendaires, sans heure ni fuseau horaire.
- **Dépense depuis une cagnotte** : elle diminue le solde de la cagnotte et ne compte pas comme un dépassement du mois.
- **Équilibrages** : saisis une fois dans le budget partagé, visibles dans le budget individuel du membre concerné.
- **Toujours un Admin** : le dernier Admin ne peut partir qu'après avoir promu quelqu'un d'autre ou archivé le budget.
- **Notifications internes** déclenchées seulement par :
  - un montant écrasé ;
  - l'arrivée ou le départ d'un membre ;
  - l'archivage, la restauration ou le déplacement d'un budget ;
  - un changement de son propre rôle.
- **Serveur administré en ligne de commande** (`invite`, `revoke`, `list`).
- **Traçabilité** : chaque poste et chaque opération affiche son auteur et sa dernière modification, sans écran d'historique complet.
- **Modifications concurrentes** : la plus récente gagne, champ par champ. Quand un montant est écrasé, l'app le signale par une notification interne (cloche avec pastille ou compteur, et historique des notifications).
- **Corbeille** par budget : lignes et opérations supprimées restaurables pendant 30 jours par un Éditeur.
- **Remboursements** : opération négative dans une catégorie de dépense.
- **Couleurs prévu/réel**, par poste et dans le calendrier, pour les dépenses seulement (pas de couleur sur les revenus en v1) :
  - vert : sous le prévu ;
  - jaune ou orange : proche du prévu (au moins 90 %, seuil fixe en v1) ;
  - rouge : dépassement, avec le montant réel affiché.
- **Projection du mois en cours** : pour chaque poste, max(réel à date, prévu), plus les dépenses imprévues.
- **Bilan de l'année par catégorie** : pour chaque catégorie, le prévu de toute l'année, le réel jusqu'ici et la projection de fin d'année (le réel des mois passés, la projection du mois en cours, le prévu des mois à venir) ; elle se corrige à mesure que les opérations arrivent.
- **Calendrier** : par défaut l'année civile en cours, de janvier à décembre ; on peut faire défiler vers le passé jusqu'à l'année de la première entrée du budget (poste ou opération), et sans limite vers l'avenir.
- **Écran d'accueil** : la liste de mes budgets, chacun avec son solde mensuel et son solde cumulé, sans total agrégé.
- **Serveurs** :
  - n'accepte que les appareils invités (par l'opérateur, ou par une invitation à un budget) ; l'opérateur peut révoquer ;
  - un budget passe par un seul serveur, et on peut le déplacer vers un autre ;
  - une même app gère des budgets sur plusieurs serveurs ;
  - un budget individuel peut vivre sans serveur (synchronisation en réseau local seulement, donc sans sauvegarde chiffrée : l'app prévient).
- **Identité sans compte** :
  - un membre est une clé et un nom ; chaque appareil a sa propre clé, liée par QR code depuis un appareil déjà lié ;
  - on rejoint un budget par une invitation à usage unique qui expire ;
  - une phrase de récupération (générée par défaut, ou choisie si elle est assez forte) déverrouille la sauvegarde chiffrée déposée sur le serveur, après la perte de tous ses appareils ([ADR 0003](adr/0003-identite-et-recuperation.md)) ;
  - l'opérateur du serveur rétablit l'accès au serveur (lien et nom d'utilisateur), jamais le contenu.
- **Langues** : français et anglais, chaînes traduisibles dès le départ ; la langue par défaut est celle du système au premier lancement.
- **Export** des données : CSV (opérations) et JSON (tout).
- **Calculateur de répartition**, indépendant du reste : il part d'un budget existant (les dotations comprises, à revoir selon les retours des utilisateurs) ou d'un total saisi, et ne stocke jamais les revenus.
- **Une devise par budget** (EUR par défaut), montants en centimes entiers. La devise est fixée à la création et ne change plus.
- **Un Lecteur voit tout** le contenu du budget ; aucun champ n'est masqué selon le rôle.
- **Le serveur garde l'historique chiffré** sans limite de durée ni quota en v1 ; la compaction par instantanés est un détail interne.
- **Retrait d'un membre** (par un Admin, ou départ volontaire) : son historique reste, il perd l'accès aux changements suivants (nouvelle clé de budget), et son appareil garde la copie qu'il avait déjà.
- **Suppression d'un budget** : d'abord archivé (lecture seule pour tous), puis supprimé définitivement sur demande d'un Admin.

## Jalons sur le laptop (Linux, 1 serveur, clients locaux)

Chaque jalon est utilisable et testé seul.

1. **M0 : domaine et stockage**
   - `bougette-core` : budgets, postes, échéanciers, lissage, opérations, cagnottes, soldes, projection, calculateur de répartition, sur SQLite ;
   - client en ligne de commande `bougette-cli` ;
   - pas de réseau.
2. **M1 : synchronisation**
   - deux appareils d'un même membre convergent : directement par iroh, puis via `bougette-server` qui joue la boîte aux lettres ;
   - HLC, la modification la plus récente gagne, vecteur de versions ([ADR 0004](adr/0004-vecteur-de-versions-plutot-que-merkle.md)) ;
   - l'adresse d'un pair se donne à la main (`ID@IP:PORT`) ; la découverte sur le réseau local (mDNS) et le QR code viendront avec l'interface.
3. **M2 : membres et chiffrement** ([ADR 0005](adr/0005-chiffrement-signatures-et-acces.md))
   - clés par budget, invitations, rôles ;
   - budget partagé avec versements et avances reflétés dans le budget personnel du membre (payeur ou bénéficiaire d'une opération) ;
   - phrase de récupération ;
   - outils en ligne de commande du serveur ;
   - restent pour la v1 : les notifications internes, l'archivage et la suppression d'un budget, les équilibrages directs entre membres, la découverte en réseau local et le QR code.
4. **M3 : interface Linux** : Flutter sur le même cœur, vue calendrier et couleurs.
   - l'app n'appelle que `bougette-api`, qui lui donne des vues prêtes à afficher (budgets et soldes, calendrier de l'année, détail d'un mois) ; elle ne calcule rien ;
   - Nix construit la bibliothèque Rust à part et la place à côté de l'app : pas de compilation Rust dans la compilation Flutter ;
   - l'app est en français et en anglais (la langue du système), montants et dates compris ; on y modifie, termine et supprime les postes, supprime les opérations, crée et alimente les cagnottes ;
   - les réglages de l'app couvrent le nom affiché, le serveur (rejoindre, synchroniser), la récupération (mettre en place, récupérer), les autres appareils (code, liaison) et l'acceptation d'une invitation ; chaque budget a ses membres (inviter, changer un rôle, retirer, quitter) ;
   - reste : la traduction des messages d'erreur venant de Rust (en anglais).

Viennent ensuite : Android, Windows, le NAS puis datcat.fr, et F-Droid/nixpkgs.

## Plus tard

- Import de relevés bancaires : CSV, OFX, CAMT.053, et PDF via un parseur.
- **Biens** (appartement en location, maison habitée), même modèle pour les deux :
  - un bien appartient à un budget ;
  - valeur modifiable à la main ;
  - tableau d'amortissement du prêt, qui produit automatiquement le poste de mensualité ;
  - ce qui entre (loyers, prix de vente) et ce qui sort (apport, frais, mensualités, assurances, taxes, gestion locative, travaux) sont des postes ordinaires rattachés au bien ;
  - date d'équilibre : valeur − capital restant dû + entrées − sorties ≥ 0.
- Export ICS des échéances, pour synchroniser les agendas (pas prioritaire).
- Réimport d'un export JSON (sauvegarde).
- Récupération sociale : une part du secret confiée à une personne de confiance.
- Montant différent par occurrence d'un poste (saisonnalité).

## Hors périmètre

- **Jamais** :
  - connexion directe à une banque ([ADR 0002](adr/0002-planification-pas-comptabilite.md)) ;
  - mois qui ne commence pas le 1er.
- Comptes bancaires et soldes réels, comptabilité.
- Déclarations fiscales ; factures et justificatifs (photos, OCR).
- Gestion de portefeuille boursier (cours en direct).
- Notifications système (push) et rappels ; les notifications internes à l'app, elles, sont dans la v1.
- Versement directement dans une cagnotte (on passe par un versement puis une dotation).
- Usage professionnel ; version web ; conversions entre devises.
