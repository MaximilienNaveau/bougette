# Transport : iroh, avec le serveur comme pair toujours en ligne

Les appareils se synchronisent via [iroh](https://github.com/n0-computer/iroh) (QUIC adressé par clé publique) : directement entre eux sur le réseau local (mDNS) ou à travers Internet (hole punching, sinon relais `iroh-relay` auto-hébergé). Le relais iroh ne stocke rien. `bougette-server` est donc lui-même un **pair iroh toujours en ligne** qui garde les messages chiffrés pour les appareils hors ligne et vérifie l'appartenance aux budgets par clé d'appareil. Il ne voit jamais les données en clair. La logique de synchronisation (horloge HLC, fusion LWW, Merkle, chiffrement par budget) ne dépend pas du transport : elle passe par un trait `Transport`.

## Options considérées

- **Relais WebSocket/HTTPS maison (axum)** : le plus simple, et le seul qui marche nativement dans un navigateur. Mais pas de synchronisation directe sur le réseau local, et l'authentification des appareils est à écrire. Il reste le repli, notamment pour une future version web.
- **Zenoh** : ne traverse pas les NAT, ses ACL sont statiques et le routeur voit les données en clair. Rejeté (étude §10).
- **Matrix** : il apporte identité, E2EE et stockage, mais les événements d'état ne sont pas chiffrés, les droits sont grossiers en salle chiffrée, l'historique n'est jamais compacté, et une absence de déchiffrement fausse un solde sans prévenir. Rejeté pour l'instant ; un prototype jetable reste possible plus tard derrière le même trait (`doc/research/matrix-as-sync.md`).

## Conséquences

- Les relais et le DNS de n0 sont désactivés (`presets::Minimal` + `RelayMode::Custom`) : l'app ne contacte aucun tiers par défaut (F-Droid, vie privée).
- On n'utilise que le cœur `iroh` 1.x, pas `iroh-docs` ni `iroh-gossip` (pré-1.0, et en concurrence avec notre propre fusion).
- `iroh-relay` n'est pas dans nixpkgs : on devra l'empaqueter quand on sortira du laptop.
- Le web ne passera que par un relais : acceptable, le web n'est pas prioritaire.
