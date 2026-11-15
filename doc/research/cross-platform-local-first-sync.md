# Étude : application budget multi-plateforme, local-first, synchronisée

*Recherche du 2026-11-14. Sources primaires (docs officielles, dépôts GitHub, specs). L'activité des dépôts a été relevée le 2026-11-14 via l'API GitHub (`pushed_at`). « non vérifié » = information que je n'ai pas pu confirmer sur une source primaire.*

Contexte : voir [`doc/README.md`](../README.md). Besoins : N utilisateurs, M budgets, chaque budget partagé avec un sous-ensemble d'utilisateurs avec des droits (lecture / écriture / admin) ; base locale sur chaque appareil (hors ligne) ; synchronisation réseau ; gestion des conflits ; auto-hébergement et/ou chiffrement de bout en bout (E2E) ; vue calendrier mensuelle avec extrapolation, catégories, projets avec épargne allouée, budget commun redistribué vers les budgets individuels.

Pondération retenue (consigne utilisateur) : **cœur en Rust** (le code sera écrit par un assistant IA, l'utilisateur veut apprendre Rust en le lisant, donc la courbe d'apprentissage de Rust **ne compte pas comme un coût**). Critères : maturité sur mobile + Linux desktop, une seule base de code UI de préférence, web en bonus, auto-hébergement / E2E, adéquation de la couche de sync avec un cœur Rust.

---

## 1. Résumé et recommandation

### Stack n°1 (recommandée) : cœur Rust + UI Flutter via flutter_rust_bridge + sync maison « à la Actual », chiffrée E2E, serveur relais Rust auto-hébergé

| Couche | Choix | Pourquoi |
|---|---|---|
| UI | Flutter (Dart), une seule base de code | Seul framework UI testé ici qui couvre officiellement Android, iOS, Windows, macOS, **Linux** (Debian/Ubuntu) et web ([plateformes Flutter](https://docs.flutter.dev/reference/supported-platforms)). |
| Pont | [flutter_rust_bridge](https://cjycode.com/flutter_rust_bridge/) (MIT) | Prend en charge « Android, iOS, Windows, Linux, MacOS, and Web (both JavaScript and WebAssembly) », package « Flutter Favorite » ([site](https://cjycode.com/flutter_rust_bridge/)). Dernière version v2.14.0-beta.2 (2026-09-12), dépôt actif ([GitHub](https://github.com/fzyzcjy/flutter_rust_bridge)). |
| Cœur | Crate Rust : modèle métier (budgets, grand livre, règles de récurrence, extrapolation), stockage, sync, crypto | Une seule implémentation testable du domaine, de la fusion des conflits et de la crypto, partagée par toutes les plateformes. |
| Stockage local | SQLite via [rusqlite](https://github.com/rusqlite/rusqlite) (MIT) | Sur `wasm32-unknown-unknown`, rusqlite bascule par défaut sur `sqlite-wasm-rs` (feature `ffi-sqlite-wasm-rs`) et `bundled-sqlcipher` fonctionne aussi en wasm ([README rusqlite](https://github.com/rusqlite/rusqlite)) : la cible web reste possible avec le même code. Persistance navigateur : OPFS ([doc SQLite WASM](https://sqlite.org/wasm/doc/trunk/persistence.md)) ; la VFS OPFS exacte utilisée par `sqlite-wasm-rs` est **non vérifiée**. |
| Sync | Journal de messages CRDT par cellule (LWW + horloge logique hybride), diff par arbre de Merkle, contenu chiffré E2E, un journal par budget | Modèle éprouvé en production par Actual Budget (voir §5) ; simple à écrire en Rust ; compatible E2E car le serveur n'a pas besoin de lire les données. |
| Serveur | Relais Rust (axum + SQLite/Postgres) : stocke des enveloppes chiffrées, gère l'authentification et l'appartenance aux budgets | Auto-hébergeable sur un petit VPS / Raspberry Pi, aucune dépendance SaaS. |

**Avantages** : une seule UI ; couverture plateforme la plus mature, Linux compris ; E2E réel ; aucune dépendance à un éditeur de sync (le paysage a beaucoup bougé en 2025-2026, voir §4) ; cœur Rust réutilisable avec n'importe quelle autre UI plus tard (Tauri, Dioxus, natif via UniFFI).
**Inconvénients** : la sync est à écrire et à tester (fusion, rotation de clés, compaction, migrations de schéma) ; deux langages (Dart + Rust) et une frontière FFI ; le rendu Flutter n'est pas « natif » ; sur le web, Flutter dessine dans un canvas (SEO/accessibilité moins bons, sans importance ici).
**Transport réseau** (Zenoh, iroh, libp2p ou simple relais WebSocket) : voir §10. En bref, le transport passe derrière un trait `Transport` ; on implémente d'abord le relais WebSocket/HTTPS, et iroh plutôt que Zenoh en option pour la sync LAN/P2P.
**Distribution libre** (F-Droid, nixpkgs, audit de licences, choix de licence) : voir §11. En bref, la stack n°1 reste viable ; le principal point d'attention est l'intégration Rust de flutter_rust_bridge (cargokit, dépôt amont archivé).
**Matrix comme alternative au relais, à l'identité et à l'E2E** : voir [`matrix-as-sync.md`](matrix-as-sync.md) (verdict : adéquation partielle, prototype recommandé).

### Stack n°2 : cœur Rust + Tauri 2 (UI web TypeScript)

- [Tauri 2.0](https://v2.tauri.app/blog/tauri-20/) cible desktop + Android/iOS (licence Apache-2.0/MIT, dépôt très actif). Mais l'équipe elle-même ne présentait pas la 2.0 comme la version où le mobile est « first class citizen » et signalait que tous les plugins desktop n'étaient pas encore portés sur mobile ([billet RC](https://v2.tauri.app/blog/tauri-2-0-0-release-candidate/), [annonce 2.0](https://v2.tauri.app/blog/tauri-20/)). Je n'ai pas trouvé d'état de parité plus récent (**non vérifié**).
- Atout : la même UI HTML/TS peut aussi être servie comme **vraie application web**, en compilant le cœur Rust en WASM ; c'est le meilleur scénario web des options Rust.
- Limites : rendu via le WebView système (WebKitGTK sous Linux, WKWebView sous iOS) donc des différences de rendu d'une plateforme à l'autre ; Tauri n'a pas de cible « navigateur » en soi ([doc start](https://v2.tauri.app/start/) : il parle de frontends HTML/JS/CSS dans une webview).
- Côté sync, PowerSync propose un SDK Tauri, mais seulement en **alpha** ([statut PowerSync](https://docs.powersync.com/resources/feature-status)).

### Stack n°3 (pragmatique, sans Rust ni E2E) : Flutter + SQLite (Drift ou SDK PowerSync) + PowerSync Open Edition auto-hébergé + Postgres

- PowerSync : SDK Flutter **GA** ; Sync Streams (GA) pour le partitionnement par utilisateur et par appartenance ; Open Edition auto-hébergeable (GA) ; service passé de la licence FSL à **Apache 2.0** (mise à jour du 2026-11-03) ([statut](https://docs.powersync.com/resources/feature-status), [billet licence](https://powersync.com/blog/new-open-era-for-powersync)).
- Le modèle est **server-authoritative** : les écritures passent par votre API, LWW par champ par défaut ([gestion des conflits](https://docs.powersync.com/handling-writes/handling-update-conflicts)). Il n'y a **pas d'E2E natif**, seulement un chiffrement au repos local ([chiffrement](https://docs.powersync.com/client-sdks/advanced/data-encryption)). L'E2E est possible en chiffrant des colonnes côté app, mais le serveur ne peut alors plus filtrer ni valider sur ces colonnes.
- C'est l'option la plus rapide pour arriver à une v1 avec partage multi-utilisateur. Elle peut aussi servir de **plan B** à la stack n°1 : le SDK Rust PowerSync existe, en bêta ([réf. Rust](https://docs.powersync.com/client-sdks/reference/rust)).

### Variantes écartées comme stack principale (détails §2 et §7)

- **Crux + UniFFI + UI natives** (SwiftUI, Compose, web) : excellent pour la testabilité, mais 3 à 4 bases UI et pas de shell desktop documenté ([Crux](https://redbadger.github.io/crux/)). Contraire à la préférence « une seule UI ».
- **Dioxus (tout Rust)** : une seule base de code Rust pour web, desktop et mobile, avec un support Android/iOS annoncé en 0.7 ([README](https://github.com/DioxusLabs/dioxus)). Mais le mobile est jeune et la page alpha de `dioxus-mobile` le qualifiait de « quite experimental » ([docs.rs](https://docs.rs/crate/dioxus-mobile/0.7.0-alpha.2)). C'est le pari « tout Rust » le plus crédible à réévaluer dans 6 à 12 mois.
- **KMP/Compose Multiplatform + SQLDelight/Room 3** : très mature ([iOS stable depuis 1.8.0](https://blog.jetbrains.com/kotlin/2025/05/compose-multiplatform-1-8-0-released-compose-multiplatform-for-ios-is-stable-and-production-ready/), [Room 3.0 KMP stable](https://developer.android.com/jetpack/androidx/releases/room3)), mais ne correspond pas à l'objectif « cœur Rust » (possible via UniFFI + Gobley, ce qui ajoute une couche).
- **Jazz / Evolu / Zero / Electric** : écosystèmes TypeScript-first. Voir §4.

---

## 2. Tableau comparatif : frameworks UI

Légende : ✅ officiel/stable · 🟡 bêta/preview/expérimental · ❌ non supporté · (tiers) = maintenu hors projet principal.

| Framework | Android | iOS | Linux | Windows | macOS | Web | Licence | Maturité / remarques |
|---|---|---|---|---|---|---|---|---|
| **Flutter** | ✅ API 24-37 | ✅ iOS 15-27 | ✅ Debian 10-13, Ubuntu 20.04-24.04 | ✅ 10/11 | ✅ 12-27 | ✅ (WASM opt-in sur Firefox/Safari) | BSD-3 | Très mature ; [plateformes](https://docs.flutter.dev/reference/supported-platforms). DB : Drift (SQLite, natif + web WASM, [plateformes Drift](https://drift.simonbinder.eu/platforms/)), sqflite, ou SQLite côté Rust via frb. |
| **React Native + Expo** | ✅ | ✅ | ❌ (seulement via React Native Skia, communautaire, Linux/macOS) | (tiers Microsoft) | (tiers Microsoft) | (communautaire react-native-web) | MIT | Mobile très mature ; desktop « out-of-tree » ([liste](https://reactnative.dev/docs/out-of-tree-platforms)). **Pas de Linux officiel**, donc rédhibitoire ici. DB : op-sqlite (MIT, [GitHub](https://github.com/OP-Engineering/op-sqlite)), expo-sqlite. |
| **Kotlin MP / Compose MP** | ✅ | ✅ (stable depuis 1.8.0, mai 2025) | ✅ (JVM desktop) | ✅ | ✅ | 🟡 bêta depuis 1.9.0 (sept. 2025) | Apache-2.0 | [iOS stable](https://blog.jetbrains.com/kotlin/2025/05/compose-multiplatform-1-8-0-released-compose-multiplatform-for-ios-is-stable-and-production-ready/), [web bêta](https://blog.jetbrains.com/kotlin/2025/09/compose-multiplatform-1-9-0-compose-for-web-beta/). DB : SQLDelight (Apache-2.0, actif), **Room 3.0** (stable 2026-07-01, Android/iOS/JVM + JS/WasmJS via `WebWorkerSQLiteDriver` + OPFS) ([notes Room 3](https://developer.android.com/jetpack/androidx/releases/room3)). |
| **Tauri 2** (Rust + webview) | ✅ (parité 🟡) | ✅ (parité 🟡) | ✅ (Debian, Arch, Fedora, etc.) | ✅ 7+ | ✅ 10.15+ | ❌ (mais le frontend est du web) | Apache-2.0/MIT | [Prérequis](https://v2.tauri.app/start/prerequisites/), [2.0](https://v2.tauri.app/blog/tauri-20/). DB : plugin SQL ou SQLite côté Rust. |
| **Dioxus** (Rust) | 🟡 | 🟡 | ✅ | ✅ | ✅ | ✅ (DOM via WASM) | Apache-2.0/MIT | 0.7.x (dernière release GitHub v0.7.10, 2026-07-30) ; mobile « first-class » selon le [README](https://github.com/DioxusLabs/dioxus), « quite experimental » selon la page alpha de `dioxus-mobile` ([docs.rs](https://docs.rs/crate/dioxus-mobile/0.7.0-alpha.2)). |
| **Slint** (Rust/C++) | 🟡 | 🟡 (tech preview depuis 1.12, juin 2025) | ✅ | ✅ | ✅ | 🟡 (non vérifié) | GPLv3 / libre de redevance (hors embarqué) / commercial | [Licences](https://slint.dev/pricing), [1.12 iOS](https://slint.dev/blog/slint-1.12-released), [1.17](https://slint.dev/blog/slint-1.17-released). Statut mobile actuel exact : **non vérifié**. |
| **egui / eframe** (Rust) | ✅ (eframe) | ❌ non listé | ✅ | ✅ | ✅ | ✅ | MIT/Apache-2.0 | « eframe… supports … Web, Linux, Mac, Windows, and Android » ([README](https://github.com/emilk/egui)). UI immédiate, peu adaptée à une app grand public. |
| **iced** (Rust) | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ | MIT | « Iced is currently experimental software » ([README](https://github.com/iced-rs/iced)). |
| **.NET MAUI** | ✅ | ✅ | 🟡 via backend Avalonia (preview) | ✅ | ✅ | 🟡 via Avalonia (preview) | MIT | Linux/web absents de MAUI officiel ; backend Avalonia en preview ([Avalonia](https://avaloniaui.net/blog/net-maui-is-coming-to-linux-and-the-browser-powered-by-avalonia), [The Register](https://www.theregister.com/2025/11/13/dotnet_maui_linux_avalonia/)). |
| **Avalonia** | ✅ | ✅ | ✅ (X11/XWayland, pas Wayland natif selon The Register) | ✅ | ✅ | ✅ (WASM) | MIT | [Comparatif Avalonia](https://avaloniaui.net/maui-compare) (source éditeur). |
| **Qt 6 / QML** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ (WebAssembly) | LGPLv3/GPL/commercial | **Non vérifié** dans cette recherche ([plateformes Qt](https://doc.qt.io/qt-6/supported-platforms.html)). |

**Lecture « cœur Rust »** :

| Appariement | UI unique ? | Mobile | Linux | Web | Liaison Rust | Verdict |
|---|---|---|---|---|---|---|
| Flutter + flutter_rust_bridge | ✅ (Dart) | ✅ mature | ✅ | ✅ (frb supporte le WASM) | Génération de code, async, streams | **Meilleur compromis** |
| Tauri 2 | ✅ (TS/HTML) | 🟡 parité incomplète | ✅ | ✅ indirect (même frontend + cœur en WASM) | Natif (commandes Tauri) | Bonne alternative, la meilleure pour le web |
| Crux + UniFFI + UI natives | ❌ (3-4 UI) | ✅ natif | ❓ pas de shell desktop documenté | ✅ | UniFFI (Kotlin/Swift/Python officiels) | Trop de code UI |
| Dioxus | ✅ (Rust) | 🟡 jeune | ✅ | ✅ | Aucune (tout Rust) | Pari à suivre |
| Slint | ✅ (DSL .slint + Rust) | 🟡 | ✅ | ? | Aucune | Licence à surveiller ; mobile jeune |

---

## 3. Moteurs de base locale

| Moteur / wrapper | Plateformes | Statut nov. 2026 | Remarques |
|---|---|---|---|
| **SQLite** | toutes, y compris le navigateur via WASM | Référence | Navigateur : VFS `opfs` (exige COOP/COEP + Worker, multi-onglets), `opfs-sahpool` (sans en-têtes, plus rapide, pas de connexions concurrentes), `opfs-wl` (3.53+) ; OPFS disponible sur Chromium (mi-2022), Firefox 111+, Safari 16.4+ ([persistance SQLite WASM](https://sqlite.org/wasm/doc/trunk/persistence.md)). |
| **rusqlite** (Rust) | natif + `wasm32-unknown-unknown` via `sqlite-wasm-rs` | actif (MIT) | SQLCipher embarqué possible, y compris en wasm ([README](https://github.com/rusqlite/rusqlite)). |
| **SQLx** (Rust) | natif | actif (Apache-2.0/MIT) | Requêtes vérifiées à la compilation ; Postgres/MySQL/SQLite ([GitHub](https://github.com/launchbadge/sqlx)). Plutôt côté serveur. Le SDK Rust PowerSync ne supporte **pas** sqlx/Diesel ([réf.](https://docs.powersync.com/client-sdks/reference/rust)). |
| **Turso Database** (réécriture de SQLite en Rust) | natif, WASM | « runs in production today » selon le README ; MIT | Remplace libSQL comme direction du projet ; `BEGIN CONCURRENT` (MVCC) ([README](https://github.com/tursodatabase/turso)). |
| **Drift** (Dart) | Android, iOS, macOS, Windows, Linux, web (`WasmDatabase`) | actif (MIT) | [Plateformes](https://drift.simonbinder.eu/platforms/). |
| **sqflite** (Dart) | mobile (+ desktop via ffi) | actif | non vérifié en détail. |
| **op-sqlite** (RN) | iOS/Android | actif (MIT) | SQLCipher, utilisé par PowerSync RN pour le chiffrement ([annonce](https://releases.powersync.com/announcements/ann_iqPKRJ9MAuDr1)). |
| **SQLDelight** (KMP) | Android, iOS, JVM, JS | actif (Apache-2.0) | [GitHub](https://github.com/sqldelight/sqldelight). |
| **Room 3.0** (KMP) | Android, iOS, JVM, JS/WasmJS | **stable 3.0.0 le 2026-07-01**, 3.0.3 le 2026-09-09 | Nouveau package `androidx.room3`, KSP uniquement, coroutines ; Room 2.x en maintenance ([blog](https://developer.android.com/blog/posts/modernizing-the-room), [notes](https://developer.android.com/jetpack/androidx/releases/room3)). |
| **Realm** | — | ⚠️ **Atlas Device Sync et les Device SDKs dépréciés en sept. 2024, fin de vie le 2025-09-30** | La base locale reste open source (communauté) ; `realm-dart` peu actif (dernier push 2025-10-23) ([doc de dépréciation](https://www.mongodb.com/docs/atlas/device-sdks/deprecation)). **À éviter.** |
| **ObjectBox** | mobile/desktop | DB open source ; **Sync commerciale** et auto-hébergée uniquement | [Sync](https://objectbox.io/sync), [FAQ](https://objectbox.io/faq/). |

---

## 4. Tableau comparatif : solutions de synchronisation

Colonnes : **Modèle** (CRDT = fusion sans autorité centrale ; SA = server-authoritative, le serveur décide) · **E2E** = le serveur ne voit pas les données en clair · **SDK** : F = Flutter/Dart, RN = React Native, K = Kotlin/KMP, S = Swift, W = Web/JS, R = Rust.

| Solution | Modèle | Licence | Auto-hébergeable | E2E | SDK | Partition / permissions | Statut au 2026-11-14 |
|---|---|---|---|---|---|---|---|
| **PowerSync** | SA (Postgres/MongoDB GA, MySQL/MSSQL bêta) → SQLite client ; file d'upload vers votre API ; LWW par champ par défaut | Service **Apache-2.0** (depuis la mise à jour du 2026-11-03), SDK Apache/MIT | ✅ Open Edition (GA) | ❌ natif (chiffrement au repos ; E2E applicatif possible sur des colonnes) | F ✅, RN ✅, W ✅, K ✅, S ✅, .NET 🟡, **R 🟡**, Capacitor 🟡, Tauri alpha | **Sync Streams** (GA) : requêtes SQL-like en YAML avec `auth.user_id()`, sous-requêtes d'appartenance, paramètres d'abonnement | Actif. [Statut](https://docs.powersync.com/resources/feature-status), [requêtes](https://docs.powersync.com/sync/streams/queries.md), [conflits](https://docs.powersync.com/handling-writes/handling-update-conflicts) |
| **ElectricSQL** | SA, **lecture seule** (Postgres → « Shapes » via HTTP) ; écritures via votre API | Apache-2.0 | ✅ (service Elixir) | ❌ | W (TS), Elixir ; F/K/S/R officiels : non trouvés | Shapes + auth par proxy ou par « gatekeeper » (jetons limités à une shape) ([auth](https://electric.ax/docs/guides/auth)) | ⚠️ **Réécrit en 2024** (abandon de l'ancienne sync bidirectionnelle SQLite/CRDT, [Electric Next](https://electric-sql.com/blog/2024/07/17/electric-next)) ; 1.0 le 2025-03-17 ([billet](https://electric-sql.com/blog/2025/03/17/electricsql-1.0-released)) ; **racheté par Databricks (Neon) en août 2026, Electric Cloud en cours de fermeture**, l'open source « stays open source » ([billet](https://electric.ax/blog/2026/08/11/electric-joining-databricks)) ; dépôt déplacé vers `neondatabase/electric`. |
| **Zero** (Rocicorp) | SA (Postgres), requêtes + mutators | Apache-2.0 | ✅ | ❌ | W, RN (expo-sqlite) ; « only supports TypeScript clients » | Logique dans les queries/mutators ; pas de système de permissions dédié depuis 0.25 | 1.0 en 2026 ([notes 1.0](https://zero.rocicorp.dev/docs/release-notes/1.0)). ⚠️ **Pas d'écritures hors ligne** ([offline](https://zero.rocicorp.dev/docs/offline), [when to use](https://zero.rocicorp.dev/docs/when-to-use)), donc **inadapté**. |
| **Replicache** | SA | Fermé, clé de licence (depuis la v10) | — | ❌ | W | — | Mode maintenance (**non vérifié**) ; [notes v10](https://github.com/rocicorp/replicache/releases). Remplacé par Zero. |
| **Triplit** | SA + cache client | AGPL-3.0 | ✅ | ❌ | W, RN | règles de permissions (non vérifié) | ⚠️ **Dormant** : dernier commit le 2025-09-11 ; le cofondateur a rejoint Supabase le 2025-10-08 ([Supabase](https://supabase.com/blog/triplit-joins-supabase)). |
| **Instant** | SA (triple store sur Postgres), cache local | Apache-2.0 | ✅ (VPS ou AWS, JVM) | ❌ | W, RN | règles de permissions | ⚠️ **En fin de vie** : bannière « Instant is sunsetting. Services will continue until August 31st, 2027 » ([doc](https://www.instantdb.com/docs/self-hosting)). **À éviter.** |
| **cr-sqlite** (vlcn) | CRDT dans SQLite (extension chargeable ; tables « CRR », table virtuelle `crsql_changes`) | MIT | transport à fournir | à faire soi-même (chiffrer les changesets) | extension C/Rust chargeable dans rusqlite, wasm, etc. | à faire soi-même | ⚠️ **Faible activité** : dernière release v0.16.3 le **2024-01-17**, seulement quelques commits de build en août 2026 ([GitHub](https://github.com/vlcn-io/cr-sqlite)). Inserts environ 2,5× plus lents ([README](https://github.com/vlcn-io/cr-sqlite)). |
| **Automerge** | CRDT (documents JSON) | MIT | ✅ (`automerge-repo-sync-server`, JS) | non intégré (non vérifié) | **R (cœur)**, W/WASM, S | par document | Actif ; 3.0 divise la mémoire par plus de 10 ([billet](https://automerge.org/blog/automerge-3/)). |
| **Yjs / y-crdt (yrs)** | CRDT (texte/structures) | MIT (non vérifié via l'API GitHub) | ✅ (y-websocket etc.) | non intégré | W, R (yrs) | par document | Actif ; orienté édition collaborative de texte, peu adapté au relationnel. |
| **Loro** | CRDT (Rust) | MIT | transport à fournir | non intégré | **R (cœur)**, W/WASM, S (expérimental), RN, Python ; pas de Kotlin/Dart officiels | par document | 1.0 le 2024-10-23, format stable ([billet](https://loro.dev/blog/v1.0)) ; actif. |
| **Jazz** | Classic : CRDT « CoValues » ; **2.0 (alpha)** : base relationnelle hébergée dans votre backend, moteur Rust sur l'appareil | MIT | ✅ | Classic : ✅ (XSalsa20, Ed25519, BLAKE3) ([chiffrement](https://classic.jazz.tools/docs/react/reference/encryption)) ; 2.0 : **non vérifié** | W, RN/Expo (TS) | Classic : **Groups** (reader/writer/admin, rotation de clé au retrait) ; 2.0 : politiques déclaratives `definePermissions` | 2.0 en **alpha avec une API entièrement nouvelle** ([README](https://github.com/garden-co/jazz)) : risque de churn. |
| **Evolu** | CRDT sur SQLite, relais sans état | MIT | ✅ relais | ✅ **par défaut** ([doc](https://www.evolu.dev/docs)) | W, RN (TS) | « Owners » : `SharedOwner` (écriture), `SharedReadonlyOwner` (lecture), `ShardOwner` (partition) ([API](https://evolu.vercel.app/docs/api-reference/common/local-first/interfaces/Owner)) | Actif. Le modèle d'owners partagés est très proche du besoin, mais l'écosystème est TS uniquement. |
| **Ditto** | Mesh P2P + « Big Peer » cloud, CRDT | Propriétaire (crate Rust ouverte + bibliothèque fermée, licence requise) | « Self-Managed » sur l'offre Enterprise | non vérifié | F, **R**, S, K, W… | abonnements | Commercial ([tarifs](https://www.ditto.com/pricing), [Rust](https://docs.ditto.live/sdk/latest/compatibility/rust.md)). |
| **CouchDB / PouchDB** | Réplication multi-maître, arbres de révisions, conflits conservés | Apache-2.0 | ✅ | ❌ | W (PouchDB) | sécurité par base (modèle « une base par utilisateur ») : non vérifié | Mature, actif ([conflits CouchDB](https://docs.couchdb.org/en/latest/replication/conflicts.html), [PouchDB](https://pouchdb.com/guides/conflicts.html)). |
| **Couchbase Lite + Sync Gateway** | Réplication de documents | Community/Enterprise (non vérifié) | ✅ | ❌ | K, S, .NET, C ; Flutter : tutoriel, SDK Dart officiel non vérifié | **Channels + roles + sync function** ; à la révocation, les documents sont purgés côté client ([modèle d'accès](https://docs.couchbase.com/sync-gateway/3.2/access-control-concepts.html)) | Mature. |
| **Supabase Realtime** | Diffusion de changements Postgres (pas un moteur offline) | Apache-2.0 | ✅ | ❌ | F, W, K, S | RLS Postgres | Pas de persistance ni de file d'écriture hors ligne intégrées (non vérifié) ([doc](https://supabase.com/docs/guides/realtime)). |
| **Firebase / Firestore** | SA, cache local ; « last write wins » | Propriétaire | ❌ | ❌ | F, W, K, S | Security Rules | Persistance hors ligne sur Android, Apple et web uniquement (pas desktop) ([doc](https://firebase.google.com/docs/firestore/manage-data/enable-offline)). |
| **Turso Sync** | push/pull explicites, « last push wins » | MIT (base) | Docs centrées sur **Turso Cloud** ; auto-hébergement du serveur de sync non documenté | ❌ | W (TS), Python, Go, RN | pas de sync partielle documentée | La bêta publique annonçait « no durability guarantees » ([bêta](https://turso.tech/blog/turso-offline-sync-public-beta), [doc sync](https://docs.turso.tech/sync)). Les embedded replicas sont stables mais en lecture seule. |
| **ObjectBox Sync** | réplication propriétaire | Commerciale | ✅ seulement | non vérifié | F, K, S, … | non vérifié | [Sync](https://objectbox.io/sync). |
| **Realm Atlas Device Sync** | — | — | — | — | — | — | ⚠️ **Fin de vie le 2025-09-30.** |
| **Actual Budget (référence)** | CRDT LWW par cellule + HLC + Merkle | MIT | ✅ (`sync-server`) | ✅ optionnel | W/Electron (TS) | un fichier budget = une unité ; accès par utilisateur (multi-user OIDC) | Voir §5. |

**Conclusions §4** :

1. Les SaaS « sync engine » ont beaucoup bougé en 2025-2026 : Realm arrêté, Instant en fin de vie, Triplit dormant, Electric racheté (cloud fermé) et Replicache fermé. **C'est un argument fort pour ne pas dépendre d'un éditeur** et posséder sa couche de sync (stack n°1) ou choisir une solution open source auto-hébergée (PowerSync, désormais Apache-2.0).
2. Les solutions E2E natives (Jazz Classic, Evolu, Actual) sont **toutes en TypeScript**. Côté Rust, les seules briques CRDT matures sont Automerge et Loro, à combiner avec un relais et une crypto maison.
3. Les solutions SA (PowerSync, Electric, Zero) ont les meilleurs outils de permissions, mais elles exigent que le serveur **lise** les données, ce qui est incompatible avec l'E2E sur les champs filtrés.

---

## 5. Référence existante : comment Actual Budget synchronise

Source : code de [actualbudget/actual](https://github.com/actualbudget/actual) (MIT, très actif, dernier push 2026-11-14) et [doc sync](https://actualbudget.org/docs/getting-started/sync/).

- **Unité de changement = une cellule** : chaque modification est un message `{dataset (table), row (id), column, value}` horodaté ([`sync.proto`](https://github.com/actualbudget/actual/blob/master/packages/crdt/src/proto/sync.proto)), stocké localement dans la table SQLite `messages_crdt(timestamp UNIQUE, dataset, row, column, value)` ([`init.sql`](https://github.com/actualbudget/actual/blob/master/packages/loot-core/src/server/sql/init.sql)).
- **Horloge** : « Hybrid Unique Logical Clock » (HULC), sérialisée en chaîne triable de 46 caractères `ISO-date-compteur-nodeId`, basée sur la spec HLC de Kulkarni et al. ([`timestamp.ts`](https://github.com/actualbudget/actual/blob/master/packages/crdt/src/crdt/timestamp.ts), [papier HLC](http://www.cse.buffalo.edu/tech-reports/2014-04.pdf)).
- **Fusion** : LWW par cellule. Un message reçu n'est appliqué que si aucun message plus récent n'existe pour le même `(dataset,row,column)` ; les messages anciens ne sont pas appliqués mais « still go into the merkle trie » ([`sync/index.ts`](https://github.com/actualbudget/actual/blob/master/packages/loot-core/src/server/sync/index.ts)).
- **Détection de divergence** : arbre de Merkle (trie) des timestamps ; le serveur renvoie son Merkle, le client compare pour savoir depuis quand resynchroniser ([`merkle.ts`](https://github.com/actualbudget/actual/blob/master/packages/crdt/src/crdt/merkle.ts)).
- **E2E** : `MessageEnvelope{timestamp, isEncrypted, content}` : le **timestamp reste en clair** (le serveur peut ordonner et calculer le Merkle), le **contenu est chiffré** (`EncryptedData{iv, authTag, data}`, donc AEAD de type GCM). Clé dérivée d'un second mot de passe ; la perte du mot de passe signifie la perte des données ; changer de clé force un « sync reset » ; la base locale n'est **pas** chiffrée ([doc](https://actualbudget.org/docs/getting-started/sync/)).
- **Limites** : le journal grossit, et le « reset sync » compacte en rendant un appareil autoritaire (les changements non synchronisés des autres appareils sont perdus) ; la doc déconseille l'usage simultané d'un même fichier ; partage = **fichier entier**, sans droits par ligne. Le mode multi-utilisateur (OIDC, rôles Basic/Admin, accès par fichier, versions 25.1.0+) ([doc](https://actualbudget.org/docs/config/multi-user)) a eu une faille : avant **26.2.1**, les endpoints `/sync` ne vérifiaient pas l'accès au fichier (**CVE-2026-27638**, [OSV](https://osv.dev/vulnerability/GHSA-qmjj-p7m9-wjrv)). **Leçon : l'autorisation doit être vérifiée sur chaque endpoint de sync et testée.**

**À reprendre pour notre app** : le modèle de message par cellule, l'HLC, le Merkle et l'enveloppe chiffrée avec timestamp en clair. **À améliorer** : un journal par *budget* (et non par fichier global), des clés par budget partagées entre membres, la signature des messages, des droits lecture/écriture appliqués par le serveur, et une compaction par snapshot chiffré plutôt qu'un reset destructif.

**Autres apps budget open source** : **Firefly III** (PHP, serveur auto-hébergé, pas local-first) : multi-utilisateur désactivé par défaut et partage d'une « administration » non disponible (« you cannot share an administration »), le chantier des « financial administrations / user groups » étant en cours ([multi-user](https://docs.firefly-iii.org/how-to/firefly-iii/features/multi-user/), [administrations](https://docs.firefly-iii.org/explanation/financial-concepts/administrations/index.md)). « Budget Zero » : **non vérifié**, rien de fiable trouvé.

---

## 6. Modélisation financière et conflits

1. **Montants en entiers** (`i64`) dans l'unité mineure de la devise (centimes), jamais en flottant. Toujours stocker le code devise ISO 4217 avec le montant (le nombre de décimales dépend de la devise : JPY 0, EUR 2, etc.) ([ISO 4217](https://www.iso.org/iso-4217-currency-codes.html), [Money pattern, Fowler](https://martinfowler.com/eaaCatalog/money.html)). En Rust : un type `Money { minor: i64, currency: Currency }` avec une arithmétique vérifiée (`checked_add`).
2. **Transactions en ajout seul** (append-only) : une opération = un événement immuable avec un id unique généré sur l'appareil (UUIDv7, triable par date, [RFC 9562](https://www.rfc-editor.org/rfc/rfc9562)). Une correction = une écriture d'annulation + une nouvelle écriture, ou une modification LWW de champs **non monétaires** (libellé, catégorie, note). Deux appareils qui ajoutent chacun une dépense hors ligne n'entrent alors **jamais** en conflit.
3. **Jamais de solde stocké en LWW** : un solde ou un « montant restant » mis à jour par LWW perd des mises à jour concurrentes. Les soldes sont **dérivés** (somme des écritures), éventuellement mis en cache localement et recalculables.
4. **LWW par champ, et non par ligne** : si A recatégorise une dépense pendant que B corrige son libellé, les deux changements survivent. C'est l'intérêt du grain « cellule » d'Actual et de cr-sqlite. Les cas restants de conflit réel (même champ, deux valeurs) sont rares et visibles : on peut conserver la valeur perdante dans un journal pour l'afficher dans l'UI.
5. **Suppressions** : tombstones (`deleted = true` en LWW). Attention : chez PowerSync, « deletes always win » ([doc](https://docs.powersync.com/handling-writes/handling-update-conflicts)) ; décidez votre sémantique (une modification après suppression ressuscite-t-elle la ligne ?).
6. **Partie double pour la redistribution** : modéliser chaque budget, enveloppe, projet et poste d'épargne comme un **compte** ; toute opération est une *écriture* (journal entry) dont les *lignes* (postings) somment à zéro par devise. Exemples :
   - virement du budget commun vers l'épargne individuelle de Parent1 : `commun −300 / P1:épargne +300` ;
   - allocation à un projet partagé : `commun:dispo −100 / projet_vacances +100` ;
   - dépense commune payée par P1 : `P1:compte −80 / commun:dépenses:courses +80` et une créance `commun doit 80 à P1` (pattern « split »).
   L'invariant « somme = 0 » se vérifie localement **et** à la fusion. Il détecte les bugs et les fusions incohérentes.
7. **Écritures inter-budgets avec des journaux séparés** (un journal chiffré par budget) : une écriture qui touche deux budgets est écrite dans **chaque** journal (une ligne par côté, même `entry_id`). L'auteur doit avoir le droit d'écriture sur les deux. Sinon on émet une « demande » dans le budget cible, qu'un membre accepte.
8. **Redistribution automatique (règles)** : stocker la **règle** (« 50/50 », « au prorata des revenus ») comme une donnée versionnée, et générer les écritures de manière **déterministe** à partir d'un id de règle + d'une période, avec un id d'écriture dérivé (hash de `règle+période`). Deux appareils qui génèrent la même redistribution hors ligne produisent alors le **même** id, donc pas de doublon.
9. **Récurrence et extrapolation du calendrier** : stocker les postes récurrents sous forme de règles (RRULE, [RFC 5545](https://www.rfc-editor.org/rfc/rfc5545)) et calculer les projections **à la lecture**, sans les persister ; marquer *réel / prévu / extrapolé* (les couleurs du README). Une occurrence matérialisée (payée) devient une écriture réelle qui référence `(règle, date d'occurrence)`.
10. **Multi-devise** : une écriture est équilibrée **par devise** ; un change crée deux lignes dans deux devises avec un compte de change (« trading account »), plus le taux utilisé stocké avec l'écriture (immuable, jamais recalculé).
11. **Horloges** : HLC (comme Actual) plutôt que l'heure murale, pour supporter la dérive d'horloge entre téléphones ; la date *comptable* de l'opération est un champ métier distinct du timestamp de sync.

Base théorique : [Local-first software (Kleppmann et al., 2019)](https://www.inkandswitch.com/local-first/).

---

## 7. Option Rust (détail)

### 7.1 Architecture « cœur Rust partagé + UI fine »

```
┌──────────── UI (Flutter/Dart │ Tauri/TS │ SwiftUI+Compose) ───────────┐
│  écrans, navigation, état d'affichage                                  │
└──────────────▲───────────────────────────────┬─────────────────────────┘
               │ flutter_rust_bridge / commandes Tauri / UniFFI / Crux
┌──────────────┴───────────────────────────────▼─────────────────────────┐
│ crate `budget-core`                                                    │
│  domain/  (Money, Account, Entry, Posting, Rule, Projection)           │
│  store/   (rusqlite, migrations, vues matérialisées)                   │
│  sync/    (HLC, messages par cellule, Merkle, file d'envoi)            │
│  crypto/  (clés par budget, wrap X25519, AEAD, signatures Ed25519)     │
│  net/     (client HTTP/WebSocket vers le relais)                       │
└────────────────────────────────────────────────────────────────────────┘
          ▲ même crate (sous-ensemble) réutilisée par
┌─────────┴──────────── crate `budget-relay` (axum) ─────────────────────┐
│ auth, appartenance aux budgets, rôles, stockage des enveloppes         │
│ chiffrées, Merkle par budget, snapshots chiffrés                       │
└────────────────────────────────────────────────────────────────────────┘
```

| Brique de liaison | Rôle | Plateformes | Licence | Maturité |
|---|---|---|---|---|
| **flutter_rust_bridge** | Génère les bindings Dart ↔ Rust | Android, iOS, Win, Linux, macOS, Web (JS & WASM) | MIT | « Flutter Favorite », v2.14 bêta (2026-09) ([site](https://cjycode.com/flutter_rust_bridge/)) |
| **UniFFI** (Mozilla) | Génère les bindings depuis une crate Rust | Officiels : **Kotlin, Swift, Python** (Ruby maintenu sans nouveautés) ; tiers : C#, Go, Dart, KMP (Gobley), React Native/WASM… | MPL-2.0 | actif (push 2026-11-12) ; utilisé dans Firefox (bindings Gecko JS listés dans le README) ([manuel](https://mozilla.github.io/uniffi-rs/latest/), [README](https://github.com/mozilla/uniffi-rs)) |
| **Crux** (Red Badger) | Architecture type Elm : cœur Rust sans effets de bord, « shell » natif qui exécute les effets | SwiftUI, Jetpack Compose, React/Svelte/WASM (desktop non mentionné) | Apache-2.0 | « we consider it production ready », mais des breaking changes avant 1.0 ; crux_core v0.20.0 (2026-08) ([docs](https://redbadger.github.io/crux/)) |

**Compromis honnêtes** (la courbe d'apprentissage de Rust n'est pas comptée, conformément à la consigne) :

- **Pour** : un seul cœur pour le domaine, la sync et la crypto, le code le plus critique (argent, fusion, clés). Typage fort, tests de propriétés (proptest) sur la fusion. Le même cœur peut tourner dans le relais serveur pour la validation de format, et en WASM pour le web.
- **Contre** : la chaîne d'outils mobile se complique (cibles `aarch64-linux-android`, `aarch64-apple-ios`, NDK, xcframework, page 16 KB Android : cr-sqlite a dû s'y adapter en août 2026, [commit](https://github.com/vlcn-io/cr-sqlite/commits/main)). Le débogage traverse la frontière FFI. La taille des binaires augmente. Les builds iOS nécessitent macOS. Le cœur doit exposer une API « grossière » (commandes et requêtes), pas des objets fins, pour limiter le bavardage FFI.

### 7.2 Frameworks UI 100 % Rust

| Framework | Android | iOS | Linux | Win | macOS | Web | Licence | Statut |
|---|---|---|---|---|---|---|---|---|
| Tauri 2 (UI web + Rust) | ✅/🟡 | ✅/🟡 | ✅ | ✅ | ✅ | indirect | Apache/MIT | 2.x stable, parité mobile incomplète ([2.0](https://v2.tauri.app/blog/tauri-20/)) |
| Dioxus | 🟡 | 🟡 | ✅ | ✅ | ✅ | ✅ | Apache/MIT | 0.7.x, 0.8 en alpha (non vérifié) ([README](https://github.com/DioxusLabs/dioxus)) |
| Slint | 🟡 | 🟡 (preview) | ✅ | ✅ | ✅ | ? | GPLv3 / libre de redevance / commercial | 1.17 (juin 2026) ([licences](https://slint.dev/pricing)) |
| egui/eframe | ✅ | ❌ | ✅ | ✅ | ✅ | ✅ | MIT/Apache | outil plutôt que grand public ([README](https://github.com/emilk/egui)) |
| iced | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ | MIT | « experimental » ([README](https://github.com/iced-rs/iced)) |

### 7.3 Briques données/sync en Rust

| Brique | Usage | Plateformes | Licence | Statut |
|---|---|---|---|---|
| rusqlite | SQLite local (+ SQLCipher) | natif + wasm32 | MIT | actif ([GitHub](https://github.com/rusqlite/rusqlite)) |
| SQLx | serveur relais (Postgres/SQLite) | natif | Apache/MIT | actif |
| Loro | CRDT (si un document par budget est préféré) | natif, WASM, Swift (exp.) | MIT | 1.x, actif ([1.0](https://loro.dev/blog/v1.0)) |
| Automerge | CRDT documents | natif, WASM, Swift | MIT | 3.x, actif ([3.0](https://automerge.org/blog/automerge-3/)) |
| cr-sqlite | CRDT dans SQLite | extension chargeable (natif, wasm) | MIT | ⚠️ faible activité (release 2024-01) |
| Turso DB / libSQL | SQLite réécrit en Rust / fork | natif, WASM | MIT | Turso DB en prod selon le README ; sync liée à Turso Cloud ([README](https://github.com/tursodatabase/turso), [sync](https://docs.turso.tech/sync)) |
| PowerSync Rust SDK + `powersync-sqlite-core` (Rust, Apache-2.0) | client PowerSync en Rust (rusqlite) | plateformes : page dédiée **non vérifiée** | Apache-2.0 | SDK **bêta** ; pas de sqlx/Diesel ; tokio ou smol ([réf.](https://docs.powersync.com/client-sdks/reference/rust), [core](https://github.com/powersync-ja/powersync-sqlite-core)) |
| Ditto Rust SDK | sync P2P/cloud | Linux x64/arm64, etc. | propriétaire (licence) | commercial ([compat](https://docs.ditto.live/sdk/latest/compatibility/rust.md)) |
| Crypto | AEAD (XChaCha20-Poly1305), X25519, Ed25519, Argon2 | toutes, wasm compris | MIT/Apache (RustCrypto, dalek) | non vérifié en détail ici |
| openmls | groupes E2E standardisés (MLS, [RFC 9420](https://www.rfc-editor.org/rfc/rfc9420)) | natif/wasm | MIT | option lourde si les groupes deviennent grands (non vérifié) |

**Pourquoi un CRDT « par cellule » maison plutôt qu'Automerge ou Loro** pour un budget : les données sont **relationnelles** (requêtes par mois, catégorie, budget, agrégats), et l'on veut que SQLite soit la source d'interrogation. Avec Automerge/Loro, il faut maintenir un document CRDT **et** une projection SQLite (double stockage, reconstruction). Le modèle d'Actual écrit directement dans les tables SQLite et garde le journal à côté, ce qui est plus simple pour ce domaine. Automerge/Loro restent pertinents pour des champs de texte riche collaboratif (notes de projet).

### 7.4 Esquisse du protocole (stack n°1)

- **Identité** : chaque utilisateur a une paire de clés X25519 (chiffrement) et une paire Ed25519 (signature), générées sur l'appareil. Clé privée protégée par un mot de passe (Argon2id) et/ou par le trousseau OS ; sauvegarde par phrase de récupération.
- **Budget** : une clé symétrique `K_budget` (avec `epoch`) ; pour chaque membre, `wrap(K_budget, pk_membre)` stocké sur le relais. Retrait d'un membre : nouvelle époque et nouvelle clé (comme les Groups de Jazz : les anciens membres gardent ce qu'ils avaient déjà lu).
- **Message** : `{budget_id, hlc, author, epoch, nonce, AEAD(K_budget, {table,row,col,value}), sig_Ed25519}`. Le relais voit `budget_id`, `hlc`, `author` et la taille, jamais le contenu (même compromis de métadonnées qu'Actual).
- **Permissions** : le relais tient `membership(budget_id, user_id, role)` et refuse un push d'un `reader`. Les clients vérifient la signature et le rôle **à l'époque du message** (défense contre un relais compromis). Lecture seule = la clé est fournie, mais aucun droit de push.
- **Sync partielle** : naturelle, un client ne tire que les journaux des budgets dont il est membre.
- **Compaction** : un membre `writer` publie périodiquement un snapshot chiffré `(état SQLite du budget, Merkle, hlc_max)`. Les nouveaux appareils partent du snapshot, puis rejouent les messages postérieurs.

---

## 8. Permissions et partage : comment chaque candidat exprime « budget X partagé avec A et B, lecture seule pour C »

| Solution | Mécanisme | Exemple / remarque |
|---|---|---|
| **PowerSync** | Sync Streams (YAML, SQL-like), `auth.user_id()` issu du `sub` du JWT, sous-requête d'appartenance | `SELECT * FROM entries WHERE budget_id IN (SELECT budget_id FROM budget_members WHERE user_id = auth.user_id())` ([requêtes](https://docs.powersync.com/sync/streams/queries.md)). La **lecture seule** est appliquée par votre API d'upload (la sync ne gère que la lecture). |
| **ElectricSQL** | Shapes autorisées par un proxy ou des jetons « gatekeeper » | Le proxy fixe le `where budget_id = …` côté serveur ; les écritures passent par votre API ([auth](https://electric.ax/docs/guides/auth)). |
| **Zero** | Logique d'autorisation dans les queries (lecture) et les mutators (écriture) | Pas de système de permissions dédié depuis 0.25 ([0.25](https://zero.rocicorp.dev/docs/release-notes/0.25)). |
| **Jazz Classic** | Group avec rôles `reader`/`writer`/`admin`, appliqués **cryptographiquement** | A, B = writer ; C = reader ; clé tournée au retrait ([chiffrement](https://classic.jazz.tools/docs/react/reference/encryption)). |
| **Jazz 2.0** | `definePermissions` : `allowRead/Insert/Update/Delete.where(...)` côté backend | ([README](https://github.com/garden-co/jazz)). |
| **Evolu** | `SharedOwner` (écriture) pour A et B ; `SharedReadonlyOwner` (id + clé de chiffrement seulement) pour C | Repose sur le partage de secrets ([API](https://evolu.vercel.app/docs/api-reference/common/local-first/interfaces/Owner)). |
| **Couchbase** | channel `budget:X` ; `access()` en lecture pour A, B, C ; `requireAccess`/`requireRole` pour l'écriture dans la sync function | Purge automatique à la révocation ([modèle](https://docs.couchbase.com/sync-gateway/3.2/access-control-concepts.html)). |
| **CouchDB** | une base par budget, `_security` membres/admins ; l'écriture en lecture seule nécessite une `validate_doc_update` | non vérifié en détail. |
| **Firestore** | Security Rules sur `budgets/{id}` avec une map `members` | non auto-hébergeable. |
| **Actual** | accès par fichier (utilisateur coché ou non) ; pas de lecture seule documentée | [multi-user](https://actualbudget.org/docs/config/multi-user). |
| **Stack n°1 (maison)** | table `membership(budget, user, role)` sur le relais + clés enveloppées par membre + signatures | Lecture seule = clé fournie, push refusé ; vérification aussi côté client. |

---

## 9. Points ouverts : questions à trancher par l'utilisateur

1. **E2E obligatoire ou souhaitable ?** Si l'auto-hébergement chez soi suffit (le serveur est « de confiance »), la stack n°3 (PowerSync auto-hébergé) devient très compétitive et évite d'écrire la sync. Si l'E2E est non négociable (serveur partagé entre amis, VPS chez un tiers), la stack n°1 s'impose.
2. **Web : nécessaire en v1 ?** Si oui, et si l'aspect « vraie page web » compte, Tauri 2 + cœur WASM (stack n°2) est plus naturel que Flutter web.
3. **Granularité du partage** : partage par budget entier suffit-il (un journal par budget) ou faut-il des droits plus fins (par catégorie ou par ligne) ? Le partage fin complique beaucoup l'E2E.
4. **Modèle comptable** : adoptez-vous la partie double (comptes/postings, recommandé §6) ou un modèle plus simple « recette/dépense par budget » ? La redistribution commun → individuel est bien plus robuste en partie double.
5. **Règle de redistribution** du budget commun : fixe (50/50), au prorata des revenus, manuelle ? Faut-il des remboursements entre membres (type Tricount) ?
6. **Lecture seule réelle** : faut-il qu'un « reader » ne voie qu'une partie (ex. totaux sans le détail) ? Si oui, il faut des clés distinctes par sous-ensemble de données.
7. **Récupération de compte** : que se passe-t-il si un parent perd son téléphone et son mot de passe ? (Phrase de récupération, ou clé de secours confiée à un autre membre.)
8. **Import bancaire** (OFX/CSV, agrégateurs) : prévu ? Cela crée des doublons potentiels à dédupliquer (id déterministe dérivé de la ligne bancaire).
9. **Multi-devise** dès la v1 ?
10. **Hébergement du relais** : Raspberry Pi à la maison, VPS, ou les deux ? (Impacte TLS, sauvegardes et notifications push.)
11. **Tolérance au risque d'un framework jeune** : accepter Dioxus (100 % Rust) pour éviter Dart, ou rester sur Flutter (mature) ?

---

## 10. Zenoh (et alternatives de transport : iroh, libp2p)

*Question : Eclipse Zenoh (connu via ROS 2 / `rmw_zenoh`) est-il utile pour la sync de la stack n°1 ? Contraintes ajoutées par l'utilisateur : **cibles prioritaires Android + Linux + Windows** (iOS/macOS = bonus) ; **tout doit être libre**, car l'app sera empaquetée dans **nixpkgs** et **F-Droid**. F-Droid compile depuis les sources, refuse les dépendances propriétaires et signale des « anti-features » comme `NonFreeNet` (« promote or depend entirely on a proprietary network service ») ou `NonFreeDep` ([Anti-Features F-Droid](https://f-droid.org/docs/Anti-Features/)). Sources consultées le 2026-11-14.*

### 10.1 Zenoh : état du projet

- **Version et cadence** : dernière release **1.10.2 (2026-10-28)** ; 1.0.0 le 2024-10-21, puis environ une mineure tous les 1 à 3 mois (1.7.0 2025-12, 1.8.0 2026-03, 1.9.0 2026-04, 1.10.0 2026-08). Toutes les bindings officielles sortent en même temps (zenoh-c, zenoh-cpp, zenoh-kotlin, zenoh-ts, zenoh-pico, zenoh-python, zenoh-java : 1.10.2 le même jour) ([releases zenoh](https://github.com/eclipse-zenoh/zenoh/releases), API GitHub).
- **Licence** : `EPL-2.0 OR Apache-2.0` (champ `license` du [Cargo.toml](https://github.com/eclipse-zenoh/zenoh/blob/main/Cargo.toml), [README](https://github.com/eclipse-zenoh/zenoh)). Libre et compatible F-Droid/nixpkgs. Le routeur `zenohd`, zenoh-c, zenoh-cpp et les backends filesystem/RocksDB/InfluxDB sont **déjà empaquetés dans nixpkgs** (zenoh 1.10.2) ([nixpkgs `pkgs/by-name/ze`](https://github.com/NixOS/nixpkgs/tree/master/pkgs/by-name/ze)).
- **Gouvernance** : projet de la Fondation Eclipse sous Eclipse IoT, état « Incubating » ([projects.eclipse.org](https://projects.eclipse.org/projects/iot.zenoh)). Le contributeur principal (ZettaScale) n'est pas nommé sur cette page (**non vérifié** ici). Roadmap publique sur GitHub ([roadmap](https://github.com/eclipse-zenoh/roadmap)).

### 10.2 Zenoh : bindings et plateformes

| Langage / cible | État | Source |
|---|---|---|
| **Rust** (`zenoh`, `zenoh-ext`) | implémentation de référence ; les autres bindings l'enveloppent (sauf zenoh-pico) | [README](https://github.com/eclipse-zenoh/zenoh) |
| C (zenoh-c), C++ (zenoh-cpp) | officiels ; zenoh-c se cross-compile via CMake + `ZENOHC_CUSTOM_TARGET` | [zenoh-c](https://github.com/eclipse-zenoh/zenoh-c) |
| **Kotlin / Android** | officiel. AAR `org.eclipse.zenoh:zenoh-kotlin-android`, ABI x86, x86_64, arm, arm64, **minSdk 30**. JVM : Linux x86_64/aarch64, macOS, **Windows x86_64/aarch64**. Le code natif Rust est publié à part (`zenoh-flat-jni`) sous forme de binaires sur Maven Central | [README zenoh-kotlin](https://github.com/eclipse-zenoh/zenoh-kotlin) |
| Swift / iOS | **pas de binding officiel** (aucun dépôt `zenoh-swift` dans l'organisation). Une issue de 2024 montrait des échecs de cross-compilation de zenoh-c pour Android/iOS (mémoire partagée POSIX), corrigés côté Android par un commit ([zenoh-c#242](https://github.com/eclipse-zenoh/zenoh-c/issues/242)). Build iOS actuel : **non vérifié** (mineur vu les priorités). | [org eclipse-zenoh](https://github.com/eclipse-zenoh) |
| Dart / Flutter | **pas de binding officiel** ; seulement des projets communautaires (`bluecorn/zenoh_dart`, `rk-zenoh`… sur zenoh-c). **Sans objet pour nous** : Zenoh serait une dépendance du cœur Rust, exposé à Dart par flutter_rust_bridge comme le reste. | recherche GitHub, 2026-11-14 |
| **TypeScript / web** | `zenoh-ts` = client **WebSocket** vers le plugin `zenoh-plugin-remote-api` d'un routeur `zenohd`. Le README dit : « it is not possible to compile Zenoh (Rust) to target WASM ». Donc **pas de Zenoh embarqué dans le cœur Rust en WASM** : le web exigerait un routeur. | [README zenoh-ts](https://github.com/eclipse-zenoh/zenoh-ts) |
| zenoh-pico | implémentation C pure pour microcontrôleurs | [zenoh-pico](https://github.com/eclipse-zenoh/zenoh-pico) |
| Autres | Python, Java, Go officiels ; dépôts `zenoh-csharp`, `zenoh-nostd` | [README](https://github.com/eclipse-zenoh/zenoh) |

**Pour nous** (crate `zenoh` dans `budget-core`) :
- **Linux et Windows** : cibles de première classe. Il existe des binaires officiels `x86_64-pc-windows-msvc`/`-gnu` et Linux gnu/musl ([assets 1.10.2](https://github.com/eclipse-zenoh/zenoh/releases/tag/1.10.2)).
- **Android** : le crate compile pour Android, puisque zenoh-kotlin embarque la même base Rust pour 4 ABI ([README zenoh-kotlin](https://github.com/eclipse-zenoh/zenoh-kotlin)). Le minSdk effectif d'un build direct depuis notre crate est **non vérifié** ; zenoh-kotlin impose 30.
- **F-Droid** : on compile le crate depuis les sources avec cargo, donc pas de binaire précompilé. Il faut **éviter** l'AAR zenoh-kotlin, qui télécharge les `.so` précompilés de `zenoh-flat-jni` depuis Maven Central ([README zenoh-kotlin](https://github.com/eclipse-zenoh/zenoh-kotlin)), ce qui est incompatible avec la politique « build from source ». La crypto TLS passe par `rustls` avec `ring` ([Cargo.toml](https://github.com/eclipse-zenoh/zenoh/blob/main/Cargo.toml)), du code libre.

### 10.3 Zenoh : topologies, découverte, NAT

- **Modes** : `peer` (par défaut, sessions directes entre pairs), `client` (une seule session vers un nœud, typiquement `zenohd`) et `router` (route pour les autres). Les routeurs ne se connectent pas tout seuls : ils « must be configured with the endpoints of the other router nodes » ([déploiement](https://zenoh.io/docs/getting-started/deployment/)).
- **Découverte** : *scouting* multicast UDP `224.0.0.224:7446` (pairs et clients) et *gossip* (les pairs se transmettent les nœuds connus, « useful when multicast communications are not available »), activés par défaut ([déploiement](https://zenoh.io/docs/getting-started/deployment/), [`DEFAULT_CONFIG.json5`](https://github.com/eclipse-zenoh/zenoh/blob/main/DEFAULT_CONFIG.json5)).
- **Android** : le Wi-Fi filtre le multicast par défaut. Il faut un `WifiManager.MulticastLock`, qui cause un « noticeable battery drain » ([doc Android](https://developer.android.com/reference/android/net/wifi/WifiManager.MulticastLock)). Il faut donc l'acquérir seulement pendant une fenêtre de sync LAN. iOS : entitlement `com.apple.developer.networking.multicast` soumis à approbation (**non vérifié**, la page Apple n'a pas pu être lue ; mineur ici).
- **NAT / Internet** : la doc ne décrit **aucune traversée de NAT** (pas de hole punching). La connectivité Internet passe par des `connect.endpoints` explicites, typiquement un routeur `zenohd` joignable publiquement ([déploiement](https://zenoh.io/docs/getting-started/deployment/)). Aucune issue « NAT traversal » ou « hole punching » n'a été trouvée dans le dépôt (recherche GitHub). Pour des téléphones derrière du CGNAT, il faut donc **de toute façon un serveur**, ce qui ne change rien par rapport à notre relais.
- **Mobile (arrière-plan, batterie, sessions persistantes)** : **rien de documenté** sur zenoh.io ; une app Android de démo existe ([zenoh-demos](https://github.com/eclipse-zenoh/zenoh-demos)). Une issue ouverte signale que la liste des interfaces n'est pas mise à jour quand le réseau change ([zenoh#2086](https://github.com/eclipse-zenoh/zenoh/issues/2086)), ce qui est pertinent pour un téléphone qui passe du Wi-Fi à la 4G (impact exact **non vérifié**).

### 10.4 Zenoh : storages et « alignment » (réplication)

- Le **storage manager** est un plugin de `zenohd`. Backends : `memory` (intégré), filesystem, RocksDB, InfluxDB, S3. Un *storage* associe une key expression à un *volume* ([README storage-manager](https://github.com/eclipse-zenoh/zenoh/tree/main/plugins/zenoh-plugin-storage-manager), [manuel](https://zenoh.io/docs/manual/plugin-storage-manager/)). Une requête `get` sur un storage renvoie « the latest values for each matching key » ([abstractions](https://zenoh.io/docs/manual/abstractions/)).
- **Réplication** ([`replication/mod.rs`, `digest.rs`, `core.rs`](https://github.com/eclipse-zenoh/zenoh/tree/main/plugins/zenoh-plugin-storage-manager/src/replication)) :
  - elle ne fonctionne que pour les storages `History::Latest` : **dernière valeur par clé, sélectionnée par timestamp HLC**. C'est du LWW par clé ;
  - le temps est découpé en *intervals* et *sub-intervals*, regroupés en eras **Hot / Warm / Cold**. Chaque événement `(clé, timestamp)` a une empreinte xxhash 64 bits, et l'empreinte d'un intervalle est le XOR de ses sous-intervalles. Le *Digest* est publié périodiquement (`interval`, 10 s par défaut ; `sub_intervals` 5, `hot` 6, `warm` 30, `propagation_delay` 250 ms, valeurs « THE SAME FOR ALL THE REPLICAS ») ;
  - un réplica qui détecte une différence interroge l'*Aligner* distant. Un storage vide fait un alignement initial complet.
  - Ce mécanisme est **conceptuellement très proche de notre Merkle par timestamps** (§5), mais l'empreinte porte sur `(clé, timestamp)` seulement, pas sur le contenu.
- **Exige des timestamps** : le `timestamping` est activé par défaut pour les routeurs mais pas pour les pairs/clients ([`DEFAULT_CONFIG.json5`](https://github.com/eclipse-zenoh/zenoh/blob/main/DEFAULT_CONFIG.json5)).
- **Embarqué sur un téléphone ?** Le crate `zenoh` a des features `plugins`/`runtime_plugins` ([zenoh/Cargo.toml](https://github.com/eclipse-zenoh/zenoh/blob/main/zenoh/Cargo.toml)) et les backends sont chargés comme bibliothèques dynamiques. Faire tourner le storage manager dans une app Android avec un backend persistant n'est documenté nulle part (**non vérifié**, a priori hors des usages prévus).
- **Pourquoi ne pas l'utiliser à la place de notre Merkle** :
  1. Le grain est une clé Zenoh, alors que notre domaine est relationnel dans SQLite. Il faudrait mapper chaque cellule `(table,row,col)` sur une clé et maintenir un deuxième stockage (RocksDB/fichiers) en plus de SQLite, ce qui reproduit le problème du « double stockage » d'Automerge/Loro (§7.3).
  2. Les valeurs seraient des blobs chiffrés opaques : ça marche, mais l'alignement ne connaît ni les budgets, ni les membres, ni les époques de clés.
  3. Il n'y a **aucune notion d'autorisation par utilisateur** (voir §10.5) : un réplica s'aligne avec tous les réplicas qui partagent la key expression.
  4. La réplication est un détail d'implémentation d'un plugin `zenohd`, pas une API stable et documentée ; les crates `commons` sont explicitement instables ([README](https://github.com/eclipse-zenoh/zenoh)).
  
  **Conclusion** : bonne source d'inspiration (eras hot/warm/cold pour limiter le coût du digest), mais **pas un remplaçant**.

### 10.5 Zenoh : sécurité

- **Transport** : TLS, QUIC (quinn), mTLS (`enable_mtls`), coupure des liens à l'expiration du certificat (`close_link_on_expiration`), WebSocket (`ws`) ([`DEFAULT_CONFIG.json5`](https://github.com/eclipse-zenoh/zenoh/blob/main/DEFAULT_CONFIG.json5)). Authentification user/password par `dictionary_file` (`user:password` par ligne) ([doc usrpwd](https://zenoh.io/docs/manual/user-password/)) ; feature `auth_pubkey` présente ([zenoh/Cargo.toml](https://github.com/eclipse-zenoh/zenoh/blob/main/zenoh/Cargo.toml)).
- **ACL** :
  - règles `allow`/`deny` par type de message (`put`, `delete`, `declare_subscriber`, `query`, `reply`, `declare_queryable`, `liveliness_*`), par flux (`ingress`/`egress`) et par `key_exprs` ;
  - sujets : `interfaces`, `cert_common_names`, `usernames`, `link_protocols`, `zids`. Les ZID ne sont « not backed by an authentication mechanism… should not be used in production » ;
  - priorité : deny explicite > allow explicite > défaut ([doc ACL](https://zenoh.io/docs/manual/access-control/), [config](https://github.com/eclipse-zenoh/zenoh/blob/main/DEFAULT_CONFIG.json5)) ;
  - **« ACL config cannot be updated at runtime, and requires a restart of the instance »** ([doc ACL](https://zenoh.io/docs/manual/access-control/)). Ajouter un membre à un budget voudrait donc dire réécrire la config et redémarrer le routeur : **inadapté** à un partage dynamique entre utilisateurs.
- **E2E / chiffrement de la charge utile** : **aucune fonctionnalité trouvée** (doc et issues GitHub). Le chiffrement est par lien (hop-by-hop), donc un routeur intermédiaire voit les données en clair. Notre enveloppe AEAD + signature (§7.4) reste indispensable.

### 10.6 Zenoh : requêtes et liveliness (utilité pour nous)

- **`get` / queryable** : un pair peut déclarer un queryable `budget/<id>/msgs` et répondre à `get("budget/<id>/msgs?since=<hlc>")`, car les sélecteurs portent des paramètres de type URL ([abstractions](https://zenoh.io/docs/manual/abstractions/)). C'est un mécanisme naturel pour « donne-moi les messages manquants depuis X » **entre pairs du LAN**.
- **Liveliness tokens** : un nœud déclare un token, les autres s'abonnent (`Put` à l'apparition, `Delete` à la disparition) ou l'interrogent par `get` ([docs.rs zenoh::liveliness, 1.10.2](https://docs.rs/zenoh/latest/zenoh/liveliness/index.html)). C'est utile pour la **présence** (« l'autre parent est sur le Wi-Fi de la maison : on synchronise en direct »).
- **AdvancedPublisher/Subscriber** (`zenoh-ext`) : historique, détection et récupération d'échantillons manqués. L'API est marquée **unstable** ([docs.rs](https://docs.rs/zenoh-ext/latest/zenoh_ext/struct.AdvancedSubscriber.html)). Notre Merkle rend cette fonction redondante.

### 10.7 Horloge : crate `uhlc` (indépendant de Zenoh)

- `uhlc` 0.9.0 (2026-01-12), licence `EPL-2.0 OR Apache-2.0`, `no_std` possible ([dépôt](https://github.com/atolab/uhlc-rs)). C'est l'HLC utilisée par Zenoh pour horodater (temps NTP64 + identifiant unique) ([abstractions](https://zenoh.io/docs/manual/abstractions/)).
- API : `HLC::default()`, `new_timestamp()`, `update_with_timestamp(&ts)`, `HLCBuilder::with_id/with_clock/with_max_delta` ; `Timestamp` est `Ord`, `Hash` et `serde` ([README](https://github.com/atolab/uhlc-rs), [`timestamp.rs`](https://github.com/atolab/uhlc-rs/blob/master/src/timestamp.rs)).
- **Points d'attention pour notre usage** (source : [`lib.rs`](https://github.com/atolab/uhlc-rs/blob/master/src/lib.rs)) :
  - le compteur logique n'occupe que **4 bits** (`CSIZE = 4`) dans les bits de poids faible de la fraction NTP64 ;
  - `update_with_timestamp` **rejette** un timestamp en avance de plus de `max_delta` (**500 ms par défaut**). Or un téléphone avec une horloge dérivée, qui se synchronise après des jours hors ligne, enverra parfois des messages « dans le futur ». Il faut donc régler `max_delta` largement (minutes) et décider explicitement de la politique : accepter et journaliser, ou refuser ;
  - la forme `Display` (`<u64 décimal>/<id hex>`) n'est pas triable lexicographiquement. Pour le Merkle, il faut sérialiser en binaire big-endian (u64 puis id).
- **Verdict** : utilisable tel quel dans `budget-core` (petit, libre, éprouvé par Zenoh), **à condition** de configurer `max_delta`. Écrire notre propre HLC à la Actual (~100 lignes, §5) reste une alternative valable ; le choix n'engage pas le transport.

### 10.8 Alternatives : iroh, libp2p, relais WebSocket

**iroh** (n0 / number0) :
- « dial keys instead » : connexions **QUIC** adressées par clé publique, hole punching, repli sur des relais ([README](https://github.com/n0-computer/iroh)).
- **1.x stable** : v1.4.1 le 2026-10-22, `main` en 1.5.0. Licence **`MIT OR Apache-2.0`** pour `iroh` et `iroh-relay` (Cargo.toml) ; les dossiers contiennent aussi un `LICENSE-BSD3` (libre).
- **E2E** : « Relay servers cannot read any of the traffic they handle; it's encrypted end-to-end » ([relays](https://docs.iroh.computer/concepts/relays)). C'est une authentification mutuelle **entre clés d'appareils** (TLS de QUIC), et non entre utilisateurs ou budgets : notre crypto applicative reste nécessaire pour les données au repos sur le relais de stockage.
- **Relais auto-hébergé** : binaire libre `iroh-relay`, avec contrôle d'accès par allowlist/denylist d'endpoint IDs ([README iroh-relay](https://github.com/n0-computer/iroh/tree/main/iroh-relay)). Ce relais est **sans état** : il ne stocke pas de messages pour un destinataire hors ligne.
- **Défauts à neutraliser pour F-Droid** :
  - le preset `N0` utilise les relais `*.relay.n0.iroh.link` et la publication pkarr/DNS sur `dns.iroh.link`, exploités par n0 ([`defaults.rs`](https://github.com/n0-computer/iroh/blob/main/iroh/src/defaults.rs), [découverte](https://docs.iroh.computer/concepts/discovery)). n0 les présente comme réservés au développement ([relays](https://docs.iroh.computer/concepts/relays)) ;
  - il faut construire l'endpoint avec `presets::Minimal`/`Empty` + `RelayMode::Custom` vers notre relais ([`presets.rs`](https://github.com/n0-computer/iroh/blob/main/iroh/src/endpoint/presets.rs), [infra dédiée](https://docs.iroh.computer/deployment/dedicated-infrastructure)), et auto-héberger `iroh-dns-server` si l'on veut la découverte DNS ;
  - les serveurs n0 tournent avec du code libre, donc un flag `NonFreeNet` est peu probable, mais contacter un tiers par défaut est à éviter (**non vérifié** côté politique F-Droid). `iroh-relay` n'est pas dans nixpkgs (seul `iroh-ssh` y figure), c'est donc à empaqueter nous-mêmes.
- **Plateformes** (support officiel) : Rust ✓ sur **Android (aarch64, armv7), Windows x86_64/aarch64, Linux glibc/musl**, iOS ([autres langages](https://docs.iroh.computer/deployment/other-languages)). **WASM navigateur** possible mais **relais uniquement** (« All connections from browsers… need to flow via a relay server ») ([WASM](https://docs.iroh.computer/deployment/wasm-browser-support)). FFI officielles Kotlin/Swift/Python/JS ; Dart/Flutter communautaires ([iroh-ffi](https://github.com/n0-computer/iroh-ffi)), ce qui est sans objet pour nous (cœur Rust).
- **LAN** : la découverte mDNS est **optionnelle** en 1.x, via les crates `iroh-mdns-address-lookup` ou `iroh-mdns-peer-lookup`. La connexion peut alors se faire « directly over the local network without needing a relay » ([local lookup](https://docs.iroh.computer/connecting/local-address-lookup)). Le fonctionnement 100 % hors Internet sans aucun service n0 n'est pas confirmé explicitement (**non vérifié**). Sur Android, mDNS demande aussi un MulticastLock (voir §10.3).
- **Protocoles au-dessus** : `iroh-gossip` (v0.102), `iroh-blobs` (v0.104), `iroh-docs` (v0.100, KV à réconciliation, dernier push 2026-08-19). Ils sont **pré-1.0** : « higher-level protocols not yet at 1.0 » ([iroh-ffi](https://github.com/n0-computer/iroh-ffi)). `iroh-docs` serait un concurrent de notre Merkle, avec les mêmes objections que les storages Zenoh (grain clé/valeur, double stockage).

**rust-libp2p** :
- licence MIT ; v0.57.1 le 2026-10-15 (toujours en 0.x) ([dépôt](https://github.com/libp2p/rust-libp2p)) ;
- briques : transports TCP/QUIC/WebSocket/WebRTC, chiffrement Noise/TLS, `mdns`, `relay`, `dcutr` (hole punching), `autonat`, `gossipsub`, `kad`, `request-response`, et des transports navigateur `websocket-websys`/`webrtc-websys`/`webtransport-websys` ([workspace Cargo.toml](https://github.com/libp2p/rust-libp2p/blob/master/Cargo.toml)) ;
- c'est le plus complet, mais aussi le plus « kit à assembler » (NetworkBehaviour, swarm). Le support Android/Windows est plausible mais **non vérifié** sur une page officielle.

**Relais WebSocket/HTTPS maison** (axum + tokio-tungstenite, déjà prévu au §7.1) : il suit TLS de bout en bout jusqu'au serveur et marche partout, y compris dans le navigateur. Surtout, c'est **le seul des quatre qui stocke et réachemine** les messages pour un appareil hors ligne (« store-and-forward »), et **le seul qui porte l'appartenance aux budgets et les rôles** (§7.4, §8).

### 10.9 Tableau comparatif (transport)

Légende : ✅ oui/officiel · 🟡 partiel/optionnel/pré-1.0 · ❌ non · ❓ non vérifié.

| Critère | **Zenoh** | **iroh** | **rust-libp2p** | **Relais WebSocket maison** |
|---|---|---|---|---|
| Licence bibliothèque | EPL-2.0 OR Apache-2.0 ✅ | MIT OR Apache-2.0 ✅ | MIT ✅ | celle de notre code (axum MIT) ✅ |
| Licence serveur | `zenohd` : EPL-2.0/Apache-2.0, **dans nixpkgs** ✅ | `iroh-relay`, `iroh-dns-server` : MIT/Apache ✅ (absents de nixpkgs) | relais = nœud libp2p ✅ | notre code ✅ |
| Point F-Droid/nixpkgs | éviter l'AAR zenoh-kotlin (`.so` précompilés de Maven) ; crate Rust compilé depuis les sources : OK | **désactiver les défauts n0** (relais + DNS) ; sinon OK | OK | OK |
| Natif Rust | ✅ (référence) | ✅ | ✅ | ✅ |
| Android | ✅ (base Rust de zenoh-kotlin, 4 ABI) | ✅ officiel (aarch64, armv7) | ❓ | ✅ |
| Linux / Windows | ✅ / ✅ | ✅ / ✅ | ✅ / ❓ | ✅ / ✅ |
| iOS (bonus) | ❓ pas de binding officiel | ✅ | ❓ | ✅ |
| Web (cœur Rust en WASM) | ❌ (seulement zenoh-ts via routeur + remote-api) | 🟡 relais uniquement | 🟡 transports `*-websys` | ✅ |
| Découverte LAN | ✅ scouting multicast/gossip intégré | 🟡 crates mDNS optionnelles | ✅ `mdns` | ❌ |
| Traversée de NAT | ❌ (routeur joignable requis) | ✅ hole punching + relais | ✅ `dcutr` + `relay` + `autonat` | sans objet (client → serveur) |
| Relais auto-hébergé | ✅ `zenohd` | ✅ `iroh-relay` (sans état) | ✅ | ✅ (avec stockage) |
| Stockage pour appareil hors ligne | 🟡 storages `zenohd` (LWW par clé, sans authz par utilisateur) | ❌ (relais sans état ; iroh-docs 🟡) | ❌ | ✅ |
| Chiffrement | TLS/QUIC par lien (hop-by-hop) | E2E entre clés d'appareils (QUIC) | Noise/TLS par connexion | TLS client↔serveur |
| E2E applicatif (données au repos sur le serveur) | ❌ à faire | ❌ à faire | ❌ à faire | ❌ à faire (§7.4) |
| Autorisation par utilisateur / budget | ❌ ACL statique, redémarrage requis | 🟡 allowlist d'endpoints sur le relais | ❌ à faire | ✅ (c'est son rôle) |
| Sémantique de fusion | LWW par clé (storages) | LWW par clé (iroh-docs, pré-1.0) | aucune | aucune (notre CRDT) |
| Maturité | 1.x, cadence 1-3 mois, Eclipse « Incubating » | 1.x depuis 2026, protocoles annexes pré-1.0 | 0.57, très utilisé (non chiffré ici) | dépend de nous |

### 10.10 Verdict pour la stack n°1

1. **Zenoh ne résout aucun des problèmes difficiles** de l'app : la sémantique de fusion (notre CRDT par cellule + partie double, §6), l'E2E (chiffrement seulement par lien), le partage dynamique entre utilisateurs (ACL statique, redémarrage requis), et la livraison à un appareil hors ligne (pas de store-and-forward authentifié par membre). Ses atouts (latence, multicast, robotique, SHM) ne servent pas une app budget qui synchronise quelques kilo-octets par jour.
2. **Architecture** : un trait `Transport` dans `budget-core/net` qui transporte des **enveloppes déjà chiffrées et signées** (§7.4). Opérations : `push(envelopes)`, `pull(budget_id, merkle_diff | since_hlc)`, `subscribe(budget_id)` pour les notifications, et `presence()` en option. Le CRDT, le Merkle et la crypto restent **au-dessus** et sont identiques quel que soit le transport.
3. **À implémenter en premier** : le **relais WebSocket/HTTPS maison** (axum). Il est indispensable de toute façon pour l'appartenance, les rôles, le stockage des enveloppes et la livraison différée entre téléphones qui ne sont presque jamais en ligne en même temps. Il couvre Android, Linux, Windows (et web) sans dépendance tierce, et s'empaquette sans difficulté dans nixpkgs et F-Droid.
4. **En option, pour la sync LAN/P2P sans serveur** : **iroh plutôt que Zenoh**. iroh fournit l'identité par clé publique (qu'on peut lier à la clé Ed25519 de l'appareil), l'E2E jusqu'au pair, le hole punching, un relais libre auto-hébergeable et un WASM (relais uniquement) ; il est officiellement supporté sur Android, Linux et Windows. Condition F-Droid : **désactiver les relais et le DNS n0 par défaut** et pointer vers notre relais. Zenoh reste défendable **uniquement** pour une sync LAN pure (scouting multicast intégré, `get` + liveliness pratiques), avec un Wi-Fi qui laisse passer le multicast et un MulticastLock sur Android. Ce serait alors un second `Transport` « LAN », sans exposition Internet. Vu le surcoût (deux piles réseau), ce n'est pas recommandé en v1.
5. **Ce qu'on peut reprendre de Zenoh sans l'adopter** : le crate **`uhlc`** pour l'HLC (en réglant `max_delta`), et l'idée des **eras hot/warm/cold** de son alignement pour rendre le diff Merkle moins coûteux sur les messages récents.

---

## 11. Distribution open source : F-Droid et nixpkgs

*Question : la stack n°1 (Flutter + flutter_rust_bridge + cœur Rust + relais Rust) peut-elle entrer dans le **dépôt principal F-Droid** et dans **nixpkgs** ? Sources consultées le 2026-11-14 : docs F-Droid, dépôt [fdroiddata](https://gitlab.com/fdroid/fdroiddata) (clone de `master`, commit `b2aa8dac`, 2026-11-14), [fdroidserver](https://gitlab.com/fdroid/fdroidserver) (commit `fc2c8647`, 2026-11-12), [nixpkgs](https://github.com/NixOS/nixpkgs) (commit `88263860`, 2026-11-14), dépôts amont, crates.io et pub.dev. Les décomptes viennent de `grep` sur ces clones.*

### 11.1 Règles F-Droid qui nous concernent

- Tout doit être libre et **recompilé depuis les sources** par F-Droid : « All applications in the repository must be Free, Libre and Open Source Software (FLOSS) » ; « Library dependencies must be built from source or provided in a trusted Maven repo » ; « Prebuilt binaries should only come from authorized trusted sources » ; Google Play Services, Firebase et Crashlytics sont cités parmi les outils interdits ([Inclusion Policy](https://f-droid.org/docs/Inclusion_Policy/)).
- Le scanner de fdroidserver signale un `Cargo.toml` sans `Cargo.lock` et un `pubspec.yaml` sans `pubspec.lock` (« dependency file without lock »), et signale aussi tout `.so` ou exécutable binaire trouvé dans les sources (table `DEPFILE` et contrôles de binaires dans [`scanner.py`](https://gitlab.com/fdroid/fdroidserver/-/blob/master/fdroidserver/scanner.py)). Il embarque aussi des signatures de bibliothèques non libres (Firebase Crashlytics, Play Billing, Play Install Referrer…) (même fichier, `SUSS_DEFAULT`).
- Les recettes peuvent exécuter des commandes root dans `sudo:` (« run with full root privileges, but the state will be reset after each build »). `prebuild:` ne doit rien compiler (« nothing should be built during this prebuild phase »). `srclibs:` déclare des sources tierces (référencées par `$$nom$$`), `scanignore:`/`scandelete:` excluent ou suppriment des chemins du scan, et `ndk:` fixe la version du NDK ([Build Metadata Reference](https://f-droid.org/docs/Build_Metadata_Reference/)).
- **Builds reproductibles** : si la recette a un champ `binary:`, F-Droid compare son APK à celui de l'amont et publie l'APK signé par l'amont s'il est identique (« F-Droid will use the upstream binary if the verification succeeded ») ([Build Metadata Reference](https://f-droid.org/docs/Build_Metadata_Reference/)). La page [Reproducible Builds](https://f-droid.org/docs/Reproducible_Builds/) cite explicitement Flutter (« If the SDK path ends up embedded in Flutter »), Rust (« The Rust toolchain should be pinned to the same version as upstream », `--remap-path-prefix`) et le NDK (« Different NDK versions also produce different binaries »).

### 11.2 F-Droid et Flutter

- **Pas de « méthode de build » Flutter dédiée.** fdroidserver ne connaît que `maven`, `gradle`, `raw` (dès que `output:` est défini) et `ant` (`Build.build_method()` dans [`metadata.py`](https://gitlab.com/fdroid/fdroidserver/-/blob/master/fdroidserver/metadata.py)). Une app Flutter est donc un build « raw » : la recette appelle elle-même `flutter build apk` et indique l'APK dans `output:`. `fdroid import` génère ce squelette quand il trouve un `pubspec.yaml` : `srclibs: flutter@stable`, `flutter config --no-analytics`, `flutter packages pub get`, `scandelete: .pub-cache`, `flutter build apk` ([`import_subcommand.py`](https://gitlab.com/fdroid/fdroidserver/-/blob/master/fdroidserver/import_subcommand.py)).
- **SDK Flutter** : c'est soit le srclib [`flutter`](https://gitlab.com/fdroid/fdroiddata/-/blob/master/srclibs/flutter.yml) (le dépôt git `flutter/flutter`), soit un **sous-module git** de l'app. Le [modèle officiel `build-flutter.yml`](https://gitlab.com/fdroid/fdroiddata/-/blob/master/templates/build-flutter.yml) propose les deux. **Épinglage** : on déclare `flutter@stable`, puis on fait en `prebuild` un `git -C $$flutter$$ checkout -f <version>`, avec la version lue dans un fichier de l'amont (workflow CI, `.fvmrc`, `pubspec.yaml`). Le modèle conseille aussi `PUB_CACHE=$(pwd)/.pub-cache` (« so that the dart packages are scanned by the scanner »), `flutter pub get --enforce-lockfile`, et de construire au même chemin absolu que la CI amont pour la reproductibilité. Dans fdroiddata, **503 fichiers de métadonnées** référencent le srclib `flutter@…`, et la très grande majorité des blocs de build utilisent `flutter@stable` + checkout.
- **Exemples réels** :
  - [Feeel `com.enjoyingfoss.feeel.yml`](https://gitlab.com/fdroid/fdroiddata/-/blob/master/metadata/com.enjoyingfoss.feeel.yml) (AGPL-3.0) : recette minimale, `srclibs: flutter@3.3.7`, `flutter build apk` ;
  - [Blood Pressure Monitor `com.derdilla.bloodPressureApp.yml`](https://gitlab.com/fdroid/fdroiddata/-/blob/master/metadata/com.derdilla.bloodPressureApp.yml) (GPL-3.0+) : Flutter en sous-module, `scanignore: flutter/bin/cache`, flavor `fdroid`, un APK par ABI ;
  - [FluffyChat `chat.fluffy.fluffychat.yml`](https://gitlab.com/fdroid/fdroiddata/-/blob/master/metadata/chat.fluffy.fluffychat.yml) (AGPL-3.0) : Flutter **+ Rust via cargokit** (voir 11.4) ; version de Flutter lue dans `.fvmrc`.
- **Problèmes connus** :
  1. **Plugins tirant Google Play Services / Firebase / ML Kit / achats in-app.** Les recettes les retirent à la main : FluffyChat supprime les lignes `gms` de `geolocator_android` et du code `FusedLocationClient` dans `.pub-cache` ; LocalSend retire `in_app_purchase` du `pubspec.lock` et lance un script amont `remove_proprietary_dependencies.sh` ; words.hk retire `mlkit` et passe sur un flavor `foss` ([fluffychat](https://gitlab.com/fdroid/fdroiddata/-/blob/master/metadata/chat.fluffy.fluffychat.yml), [localsend](https://gitlab.com/fdroid/fdroiddata/-/blob/master/metadata/org.localsend.localsend_app.yml), [wordshk](https://gitlab.com/fdroid/fdroiddata/-/blob/master/metadata/hk.words.wordshk.yml)). **Pour nous** : ne jamais dépendre d'un plugin Firebase/GMS (notifications push comprises) ; s'il le faut un jour, isoler ces plugins dans un flavor non-F-Droid.
  2. **Binaires précompilés du moteur Flutter.** Le SDK télécharge le Dart SDK et les artefacts du moteur dans `bin/cache`, et le modèle F-Droid les exclut du scan (`scanignore: .flutter/bin/cache`, « Ignore the downloaded dart tools ») ([modèle](https://gitlab.com/fdroid/fdroiddata/-/blob/master/templates/build-flutter.yml)). F-Droid tolère donc ces binaires Google en pratique. Je n'ai pas trouvé de texte de politique qui les autorise explicitement (**non vérifié**). Ce n'est pas un risque spécifique à notre app.
  3. **Autres binaires embarqués** : un `.so` précompilé bloque la mise à jour (ex. `libpdfium.so` de Saber, [fdroiddata#3926](https://gitlab.com/fdroid/fdroiddata/-/work_items/3926)).
  4. **Reproductibilité** : chemins absolus du SDK et du code natif, version du NDK, horodatages ([Reproducible Builds](https://f-droid.org/docs/Reproducible_Builds/)). Les recettes reproductibles construisent au chemin de la CI amont et produisent **un APK par ABI** (`--split-per-abi --target-platform=…`, `VercodeOperation`), comme le modèle.

### 11.3 F-Droid et Rust (NDK)

- **Installer Rust** : il n'y a pas de toolchain Rust dans l'image de build. Deux façons de faire coexistent dans fdroiddata :
  - le **paquet Debian `rustup`** installé en `sudo:` (`apt-get install -y … rustup`), suivi de `rustup default <version>` et `rustup target add …` (**123** fichiers de métadonnées) ;
  - le **srclib [`rustup`](https://gitlab.com/fdroid/fdroiddata/-/blob/master/srclibs/rustup.yml)** (dépôt `rust-lang/rustup` épinglé à un tag), avec `$$rustup$$/rustup-init.sh -y --default-toolchain <version> --target …` puis `source $HOME/.cargo/env` (**77** fichiers).
  
  Dans les deux cas, la **version de rustc est épinglée** (souvent lue dans `rust-toolchain.toml`), comme le demande la page Reproducible Builds.
- **cargo-ndk** : `cargo install --locked cargo-ndk@<version>` puis `cargo ndk -t aarch64-linux-android … -o …/jniLibs build --release` (51 fichiers mentionnent cargo-ndk). Exemple : [Cfait `com.cfait.yml`](https://gitlab.com/fdroid/fdroiddata/-/blob/master/metadata/com.cfait.yml) (GPL-3.0, Rust + Gradle), avec `RUSTFLAGS="--remap-path-prefix … -C link-arg=-Wl,--build-id=none"` et `ndk: r27d`.
- **Crates** : dans les recettes observées, cargo **télécharge les crates depuis crates.io pendant le build**, et la cohérence est assurée par `Cargo.lock` (exigé par le scanner) et `--locked`. Seule **1** recette fait `cargo vendor` et 5 font `cargo fetch`. Je n'ai trouvé aucun texte de la politique F-Droid qui nomme crates.io comme dépôt de confiance : l'Inclusion Policy ne parle que de dépôts Maven (**non vérifié** côté politique ; accepté en pratique).

### 11.4 F-Droid et Flutter + Rust (flutter_rust_bridge, cargokit)

- **Ça existe et c'est courant.** Dans fdroiddata, **37** fichiers combinent le srclib `flutter` et rustup ; **16** mentionnent **cargokit** (dont LocalSend, FluffyChat, Saber, Ente Photos/Locker, Venera, Polycule, Wormhole…) ; **4** mentionnent explicitement **flutter_rust_bridge** :
  - [GitSync `com.viscouspot.gitsync.yml`](https://gitlab.com/fdroid/fdroiddata/-/blob/master/metadata/com.viscouspot.gitsync.yml) (GPL-3.0, **build reproductible** : `binary:` + `AllowedAPKSigningKeys`). `apt-get install … rustup`, `rustup default 1.88.0`, `cargo install flutter_rust_bridge_codegen --version 2.12.0 --locked` et `cargo-expand`, puis `flutter_rust_bridge_codegen generate` et `flutter build apk --split-per-abi`. Il ajoute `CARGO_ENCODED_RUSTFLAGS` (`--remap-path-prefix`, `-Ccodegen-units=1`), `SOURCE_DATE_EPOCH` et un `rm -rf rust/target` après la génération du code. Les `MaintainerNotes` expliquent chaque contournement (chemin `/home/runner/work/...` identique à la CI amont, un bloc par ABI). C'est **le modèle le plus proche de notre cas** ;
  - [words.hk `hk.words.wordshk.yml`](https://gitlab.com/fdroid/fdroiddata/-/blob/master/metadata/hk.words.wordshk.yml) (MIT) : `srclibs: rustup@1.27.0`. Il **neutralise les appels `rustup` de cargokit** (`sed 's/"rustup"/"echo"/' rust_builder/cargokit/build_tool/lib/src/builder.dart`) ;
  - [Ente Photos `io.ente.photos.fdroid.yml`](https://gitlab.com/fdroid/fdroiddata/-/blob/master/metadata/io.ente.photos.fdroid.yml) et Ente Locker : ils **forcent la toolchain de cargokit** (`sed` de `_toolchain` → `'1.97.0'` dans `cargokit/build_tool/lib/src/builder.dart`) ;
  - [LocalSend `org.localsend.localsend_app.yml`](https://gitlab.com/fdroid/fdroiddata/-/blob/master/metadata/org.localsend.localsend_app.yml) (Apache-2.0, reproductible) : Flutter en sous-module, `rustup` de Debian, `cargo-ndk@4.1.2`, cargokit appelé par `flutter build apk`.
- **Comment cargokit construit** : flutter_rust_bridge génère par défaut un paquet `rust_builder/` qui **embarque une copie de cargokit** (`--integration-backend`, valeur par défaut `cargokit`, dans [`commands.rs`](https://github.com/fzyzcjy/flutter_rust_bridge/blob/master/frb_codegen/src/binary/commands.rs) ; modèle `integration_template/cargokit/app/rust_builder`). Pendant le build Gradle/CMake, le `build_tool` Dart de cargokit appelle `rustup toolchain install`, puis `rustup target add` sur la toolchain `stable` par défaut (`_toolchain => … ?? 'stable'`, [`builder.dart`](https://github.com/irondash/cargokit/blob/main/build_tool/lib/src/builder.dart), [`rustup.dart`](https://github.com/irondash/cargokit/blob/main/build_tool/lib/src/rustup.dart)). La compilation passe par `rustup run <toolchain> cargo build`, et l'option `toolchain` de cargokit n'accepte que `stable`, `beta` ou `nightly` (enum `Toolchain`, [`options.dart`](https://github.com/irondash/cargokit/blob/main/build_tool/lib/src/options.dart)). Conséquences pour F-Droid : il faut rustup dans l'environnement, et on **ne peut pas épingler une version précise de rustc** par la configuration. D'où les `sed` dans `builder.dart` : Ente remplace `_toolchain` par `'1.97.0'` et FluffyChat remplace `'stable'` par `'1.98.0'`. Comme la copie de cargokit vit dans **notre** dépôt (`rust_builder/cargokit`), nous pouvons la modifier directement pour respecter `rust-toolchain.toml`.
- **Binaires précompilés de cargokit** : la fonction est **opt-in côté crate**. Elle n'agit que si un `cargokit.yaml` à côté de `Cargo.toml` contient une section `precompiled_binaries` (URL + clé publique). Les binaires sont alors téléchargés et vérifiés par signature. Pour désactiver côté app : `use_precompiled_binaries: false` dans `cargokit_options.yaml` (cargokit « will insist on building from source »). Sans ce fichier, cargokit compile depuis les sources si rustup est installé ([precompiled_binaries.md](https://github.com/irondash/cargokit/blob/main/docs/precompiled_binaries.md)). **Pour nous** : ne pas publier de `precompiled_binaries` et ajouter `cargokit_options.yaml` avec `use_precompiled_binaries: false` dès le départ.
- **Bloqueur potentiel : cargokit n'est plus maintenu en amont.** Le dépôt [`irondash/cargokit`](https://github.com/irondash/cargokit) est **archivé** (API GitHub : `archived: true`, dernier push 2025-06-07) et se décrit comme « Experimental repository ». flutter_rust_bridge en maintient une copie dans son modèle, et propose désormais un second backend, **Native Assets** : `--integration-backend native-assets`, un hook `hook/build.dart` qui appelle `flutter_rust_bridge_hooks` (`FlutterRustBridgeNativeAssetsBuilder`) ([modèle](https://github.com/fzyzcjy/flutter_rust_bridge/tree/master/frb_codegen/assets/integration_template/native_assets)). Le paquet [`flutter_rust_bridge_hooks`](https://pub.dev/packages/flutter_rust_bridge_hooks) est en 2.13.0 (2026-08-23). La maturité de ce backend et son comportement sous F-Droid et Nix (appels rustup, téléchargements) sont **non vérifiés** : aucune recette fdroiddata ni aucun paquet nixpkgs ne l'utilise encore.
- **Autre piège** : un SDK qui télécharge un binaire pendant le build. C'est le cas du SDK Dart **PowerSync** (plan B) : son hook `hook/build.dart` télécharge `libpowersync_*.so` depuis les releases GitHub de `powersync-sqlite-core` ; l'option de checkout local est une constante `_localCoreExtensionCheckout = null` à patcher ([build.dart](https://github.com/powersync-ja/powersync.dart/blob/main/packages/powersync/hook/build.dart)). Pour F-Droid, il faudrait donc patcher le SDK et compiler `powersync-sqlite-core` (Rust) dans la recette. C'est faisable mais fragile (pas de recette existante trouvée, **non vérifié**).

### 11.5 nixpkgs

- **Applications Flutter** : `flutterXYZ.buildFlutterApplication` (une version épinglée, par ex. `flutter347`, et non `flutter`, recommandation du [manuel](https://github.com/NixOS/nixpkgs/blob/master/doc/languages-frameworks/dart.section.md)). Les dépendances pub viennent de `pubspecLock = lib.importJSON ./pubspec.lock.json`, obtenu par conversion avec « something like `yq . pubspec.lock` ». `autoPubspecLock` (IFD) n'est « not permitted in Nixpkgs ». Les dépendances git exigent `gitHashes`, et les paquets à patcher passent par `customSourceBuilders` (même manuel). Cibles : Linux et web (`targetFlutterPlatform`). Le SDK Flutter de nixpkgs utilise les **artefacts précompilés du moteur** (`sourceProvenance = [ lib.sourceTypes.binaryNativeCode ]`, licence `bsd3`, plateformes `x86_64-linux`, `aarch64-linux`, `aarch64-darwin`) ([`flutter.nix`](https://github.com/NixOS/nixpkgs/blob/master/pkgs/development/compilers/flutter/flutter.nix)). Versions empaquetées : 3.29 à 3.47 ([`versions/`](https://github.com/NixOS/nixpkgs/tree/master/pkgs/development/compilers/flutter/versions)). Au moins 43 paquets `pkgs/by-name` + `pkgs/applications` utilisent `buildFlutterApplication`.
- **Flutter + crate Rust (flutter_rust_bridge/cargokit)** : le schéma établi **contourne cargokit**. Le crate est construit à part par `rustPlatform.buildRustPackage` (avec `cargoHash`), puis un `customSourceBuilders.<rust_lib_…>` patche le `CMakeLists.txt` du plugin pour pointer vers la `.so` produite par Nix :
  - [LocalSend `linux.nix`](https://github.com/NixOS/nixpkgs/blob/master/pkgs/by-name/lo/localsend/linux.nix) + [`no-cargokit.patch`](https://github.com/NixOS/nixpkgs/blob/master/pkgs/by-name/lo/localsend/no-cargokit.patch) remplace `apply_cargokit(...)` par `@rustLib@/lib/librust_lib_localsend_app.so` ;
  - [Mangayomi `package.nix`](https://github.com/NixOS/nixpkgs/blob/master/pkgs/by-name/ma/mangayomi/package.nix) remplace `cargokit.cmake` par une fonction `apply_cargokit` factice qui renvoie le chemin de la lib construite par `buildRustPackage` ;
  - d'autres paquets combinent aussi `rustPlatform` et `buildFlutterApplication` : `intiface-central`, `rustdesk-flutter`. Des paquets dont le `pubspec.lock` contient flutter_rust_bridge ou cargokit : fluffychat, ente-auth, commet-chat, bluebubbles, finamp, fladder… `flutter_rust_bridge_codegen` est empaqueté (2.13.0, [`package.nix`](https://github.com/NixOS/nixpkgs/blob/master/pkgs/by-name/fl/flutter_rust_bridge_codegen/package.nix)).
  
  **Pour nous** : le dépôt peut fournir dès le départ un `flake.nix` qui reproduit ce schéma : `budget-core` construit par `buildRustPackage`, et un `CMakeLists.txt` du plugin qui accepte un chemin de lib précompilée (variable CMake) en plus de `apply_cargokit`. Ça évite le patch côté nixpkgs.
- **Serveur Rust** : `rustPlatform.buildRustPackage` avec `cargoHash`, « computed over all crate sources », ou `cargoLock.lockFile` pour un projet en développement, plus pratique parce que le hash ne change pas à chaque modification du `Cargo.lock` ([manuel Rust](https://github.com/NixOS/nixpkgs/blob/master/doc/languages-frameworks/rust.section.md)). Un module NixOS (`services.<relais>`) serait un plus pour l'auto-hébergement. La convention de modules NixOS n'a pas été relue ici (**non vérifié**).

### 11.6 Tauri 2 et Dioxus sur F-Droid et nixpkgs

- **Tauri 2** : **13** recettes fdroiddata construisent un projet `src-tauri`, par ex. [Bosun `com.bosun.app.yml`](https://gitlab.com/fdroid/fdroiddata/-/blob/master/metadata/com.bosun.app.yml) (GPL-3.0+). Le build fait `rustup` (Debian) + `nodejs`/`npm` en `sudo`, `npm ci`, `npx tauri android init --ci`, `npx tauri android build --apk --ci -- --locked`, avec un wrapper `gradlew` vers `gradlew-fdroid`. Côté nixpkgs, **70** paquets utilisent `cargo-tauri.hook` ([`hook.nix`](https://github.com/NixOS/nixpkgs/blob/master/pkgs/by-name/ca/cargo-tauri/hook.nix)). Difficulté : **moyenne**. On ajoute la chaîne npm (lockfile exigé par le scanner pour `package.json`, `scanner.py`) ; c'est un chemin bien rodé.
- **Dioxus** : **1** recette fdroiddata, [Counted `fr.counted.app.yml`](https://gitlab.com/fdroid/fdroiddata/-/blob/master/metadata/fr.counted.app.yml) (AGPL-3.0) : `cargo install dioxus-cli --version 0.7.10 --locked`, `cargo fetch --locked`, puis un Makefile amont. nixpkgs empaquette `dioxus-cli` et au moins une app desktop, [`kopuz`](https://github.com/NixOS/nixpkgs/blob/master/pkgs/by-name/ko/kopuz/package.nix) (`dx build --release --platform desktop --offline --frozen`). Difficulté : **faible côté toolchain** (tout est Rust + cargo), mais il y a très peu de précédents, donc peu de recettes à copier.

### 11.7 Audit de licences de la stack n°1

Légende F-Droid/nixpkgs : ✅ accepté, avec des précédents cités ; 🟡 accepté avec un travail de recette.

| Composant | Licence (source) | F-Droid OK ? | nixpkgs OK ? | Remarques |
|---|---|---|---|---|
| Flutter SDK / moteur | BSD-3-Clause ([LICENSE](https://github.com/flutter/flutter/blob/master/LICENSE)) ; notices tierces du moteur ([`sky_engine/LICENSE`](https://github.com/flutter/flutter/blob/master/engine/src/flutter/sky/packages/sky_engine/LICENSE), 1,3 Mo) | ✅ (503 apps ; `bin/cache` exclu du scan) | ✅ (`bsd3`, `binaryNativeCode`) | Aucune licence GPL, LGPL ou MPL trouvée dans les notices du moteur (seules occurrences « GNU » : listes de diffusion FreeType et Autoconf). Le fichier `NOTICES` est généré automatiquement ([LicenseRegistry](https://api.flutter.dev/flutter/foundation/LicenseRegistry-class.html)) |
| flutter_rust_bridge (runtime Dart + crate + codegen) | MIT (GitHub, crates.io 2.13.0) | ✅ (GitSync, words.hk, Ente) | ✅ (codegen 2.13.0 empaqueté) | — |
| cargokit (copié dans `rust_builder/`) | MIT **et** Apache-2.0 dans un même `LICENSE` ([LICENSE](https://github.com/irondash/cargokit/blob/main/LICENSE)) ; GitHub : `NOASSERTION` | 🟡 (rustup requis, toolchain à épingler) | 🟡 (contourné, cf. 11.5) | **Dépôt archivé** ; précompilés à laisser désactivés |
| rusqlite / libsqlite3-sys | MIT (crates.io 0.42.0 / 0.40.0) | ✅ | ✅ | SQLite bundlé : domaine public ([sqlite.org/copyright](https://sqlite.org/copyright.html), page non relue ici : **non vérifié**) ; SQLCipher (si `bundled-sqlcipher`) : BSD-style (**non vérifié**) |
| uhlc | `EPL-2.0 OR Apache-2.0` (crates.io 0.9.0) | ✅ | ✅ | Prendre la branche **Apache-2.0** ; EPL (copyleft faible) non subi car alternative |
| iroh / iroh-relay | `MIT OR Apache-2.0` (crates.io 1.4.1) | ✅ (si défauts n0 désactivés, §10.8) | ✅ (iroh-relay à empaqueter) | Son [`deny.toml`](https://github.com/n0-computer/iroh/blob/main/deny.toml) autorise **MPL-2.0**, BSL-1.0, CDLA-Permissive-2.0, Unicode-3.0, Zlib : des dépendances transitives **MPL-2.0** sont donc possibles |
| axum | MIT (0.8.9) | sans objet (serveur) | ✅ | Son `deny.toml` n'autorise que Apache-2.0, BSD-3, MIT, Unicode-3.0 |
| tokio / tokio-tungstenite | MIT | ✅ | ✅ | — |
| rustls / ring / webpki-roots | `Apache-2.0 OR ISC OR MIT` / `Apache-2.0 AND ISC` / `CDLA-Permissive-2.0` | ✅ | ✅ | ring : **Apache-2.0 obligatoire** (pas d'alternative MIT) ; LocalSend dans nixpkgs remplace `webpki-roots` par le magasin système (`native-roots.patch`) |
| Crypto (choix proposé) : `chacha20poly1305` (RustCrypto), `x25519-dalek`, `ed25519-dalek`, `hkdf`, `blake3` | `Apache-2.0 OR MIT` ; BSD-3-Clause (dalek 3.0.0) ; `MIT OR Apache-2.0` ; `CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception` | ✅ | ✅ | Tout permissif, pur Rust. **Choix recommandé** : primitives RustCrypto/dalek, que l'on compose nous-mêmes selon §7.4 |
| Alternatives crypto : `age` / `orion` | `MIT OR Apache-2.0` (0.12.1) / MIT (0.18.1) | ✅ | ✅ | `age` (chiffrement de fichiers/flux vers des destinataires X25519) pourrait servir à emballer les clés de budget ; `orion` offre une API de haut niveau. Ni l'un ni l'autre n'est nécessaire si l'on compose les primitives ci-dessus (descriptions non relues dans leurs README : **non vérifié**) |
| Dépendance transitive typique `option-ext` (via `dirs`) | **MPL-2.0** (crates.io 0.2.0) | ✅ | ✅ | Exemple de copyleft faible **au niveau fichier** qui arrive par transitivité. À détecter avec `cargo-deny` |
| PowerSync (plan B) : SDK Dart `powersync`, `powersync-sqlite-core`, crate `powersync`, service | Apache-2.0 ([LICENSE Dart](https://github.com/powersync-ja/powersync.dart/blob/main/packages/powersync/LICENSE), [core](https://github.com/powersync-ja/powersync-sqlite-core), crates.io 0.2.0, [service](https://github.com/powersync-ja/powersync-service/blob/main/LICENSE)) | 🟡 (le hook télécharge une `.so`, cf. 11.4) | 🟡 (même problème : à patcher, **non vérifié**) | Licences OK. Le blocage potentiel est technique, pas juridique |

**Copyleft dans l'arbre** : aucune dépendance **GPL/LGPL/AGPL** identifiée parmi les composants directs. Le seul copyleft probable est du **MPL-2.0 transitif** (copyleft au niveau du fichier). Il est compatible avec une app MIT/Apache, GPL ou AGPL : il suffit de rendre disponibles les sources des fichiers MPL, qui le sont déjà sur crates.io. **Il ne gêne pas** une éventuelle version propriétaire tant que ces fichiers ne sont pas modifiés de façon fermée. L'utilisateur ne prévoit d'ailleurs pas de fermer le code (cf. 11.9). L'arbre transitif complet n'a **pas** été résolu ici (**non vérifié**) : c'est le rôle du job `cargo deny check licenses` (11.10).
**Attribution** : MIT, BSD et Apache exigent de reproduire les notices dans les binaires distribués. Le `NOTICES` de Flutter ne couvre que les paquets pub et le moteur. Les crates Rust **n'y sont pas** : il faut générer leurs notices (par ex. `cargo about`, **non vérifié**) et les enregistrer via `LicenseRegistry.addLicense` pour qu'elles apparaissent dans `showLicensePage`.

### 11.8 Anti-Features F-Droid et URL de serveur par défaut

- **NonFreeNet** s'applique aux apps qui « promote or depend entirely on a **proprietary** network service ». **TetheredNet** s'applique à une dépendance à un service « impossible (or not easy) to replace », mais pas s'il existe une option simple pour pointer vers une instance alternative d'un serveur libre et auto-hébergeable. **Tracking** couvre l'envoi d'activité « without your permission or by default », sauf en opt-in désactivé par défaut. **NonFreeDep** s'applique quand l'app « require[s] things that are not Free Software in order to run » ([Anti-Features](https://f-droid.org/docs/Anti-Features/)).
- **Pour nous** :
  - notre relais est libre, donc **NonFreeNet ne s'applique pas** ;
  - l'app fonctionne **sans réseau** (local-first) et l'URL du relais est configurable, donc **TetheredNet ne s'applique pas** non plus, même si l'app propose une instance par défaut ;
  - aucun SDK d'analytics ou de crash reporting, donc pas de **Tracking**. `flutter config --no-analytics` dans les recettes concerne l'outil de build, pas l'app ;
  - pas de GMS/Firebase (ni FCM pour les notifications), donc pas de **NonFreeDep**.
- **Recommandation** : **aucune URL de serveur par défaut en v1**. L'app est 100 % locale tant que l'utilisateur n'a pas saisi l'URL de son relais, ou scanné un QR code d'invitation qui la contient. Ça évite aussi de contacter un tiers sans consentement (§10.8 pour iroh). Si un service hébergé payant existe plus tard, on peut le **proposer comme choix explicite** dans l'écran de configuration, à côté de « mon serveur ». C'est compatible avec la définition de TetheredNet ci-dessus (interprétation, **non vérifiée** auprès des mainteneurs F-Droid).

### 11.9 Choix de licence pour notre projet

*Consigne utilisateur : projet entièrement ouvert, avec une communauté active, **sans fermeture ultérieure du code** ; le volet commercial éventuel = services (sync hébergée, support).*

- **F-Droid et nixpkgs acceptent toutes les licences en discussion** : MIT, `MIT OR Apache-2.0`, BSD-3, MPL-2.0, GPL-3.0 et AGPL-3.0 sont des licences FLOSS au sens de l'[Inclusion Policy](https://f-droid.org/docs/Inclusion_Policy/). On trouve dans fdroiddata des apps sous MIT (words.hk), Apache-2.0 (LocalSend), GPL-3.0 (GitSync) et AGPL-3.0 (FluffyChat, Counted). nixpkgs utilise des identifiants `lib.licenses.*` (par ex. `bsd3` pour Flutter, [`flutter.nix`](https://github.com/NixOS/nixpkgs/blob/master/pkgs/development/compilers/flutter/flutter.nix)) ; les noms exacts `mit`, `asl20`, `gpl3Only`, `agpl3Only` n'ont pas été relus dans `lib/licenses.nix` (**non vérifié**, mais ces licences y sont courantes).
- **MIT seul ou `MIT OR Apache-2.0` ?** La convention Rust est le double licenciement : « dual-licensed, under either the MIT or Apache 2.0 licenses », `license = "MIT OR Apache-2.0"` ([Rust API Guidelines, C-PERMISSIVE](https://rust-lang.github.io/api-guidelines/necessities.html)). C'est le cas de la plupart de nos dépendances. Apache-2.0 ajoute une **licence de brevets explicite** de chaque contributeur, résiliée pour qui engage un procès en brevets (§3), et dit qu'une contribution est réputée faite sous la licence du projet (« inbound = outbound », §5) ([Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0)). Mais Apache-2.0 seule n'est pas compatible GPLv2 (« the FSF has never considered the Apache License to be compatible with GPL version 2 ») ([ASF](https://www.apache.org/licenses/GPL-compatibility.html)). Le « OR MIT » règle ce point. **Recommandation : `MIT OR Apache-2.0`** pour le cœur, l'app et le relais. Ça donne la même permissivité que MIT, la protection brevets d'Apache en option, la cohérence avec l'écosystème Rust (et la réutilisation facile du crate `budget-core` par d'autres), et c'est sans effet sur F-Droid et nixpkgs. MIT seul reste acceptable si l'on veut un seul fichier de licence le plus simple possible.
- **DCO ou rien ?** Le [DCO 1.1](https://developercertificate.org/) est une attestation par commit (`Signed-off-by`) : le contributeur certifie avoir le droit de soumettre sa contribution sous la licence du projet. Il n'y a ni transfert de droits ni CLA. Avec `MIT OR Apache-2.0`, la clause §5 d'Apache couvre déjà l'« inbound = outbound ». **Recommandation : pas de CLA ; DCO facultatif mais conseillé**, peu coûteux (`git commit -s`, vérifié par un bot) et utile pour la traçabilité de provenance. Le copyright reste réparti entre les contributeurs. Comme on ne prévoit pas de relicencier en propriétaire, ça ne pose pas de problème.
- **Mentions secondaires** :
  - **GPL-3.0 / AGPL-3.0 sur le relais** : l'AGPL obligerait un hébergeur qui modifie le relais à publier ses modifications. C'est un frein à la réappropriation commerciale par un tiers, mais aussi pour certains contributeurs. Le client et le serveur ne communiquent que par le réseau, donc on pourrait combiner un client permissif et un relais AGPL sans contamination du client ;
  - l'**open core / double licence AGPL + commerciale** suppose de détenir tous les droits, donc un CLA, et contredit l'objectif « communauté » ;
  - **MPL-2.0** : copyleft par fichier, compromis possible mais atypique dans l'écosystème Rust/Flutter.
  
  Aucune de ces options n'est nécessaire pour F-Droid ou nixpkgs.

### 11.10 Verdict

**La stack n°1 reste viable pour F-Droid (dépôt principal) et nixpkgs.** Toutes les briques sont sous licence libre permissive. Flutter + Rust via cargokit/flutter_rust_bridge a déjà des précédents publiés, dont des builds **reproductibles** (GitSync, LocalSend) dans fdroiddata, et un schéma établi dans nixpkgs (LocalSend, Mangayomi).

**Risques concrets et parades** :

| Risque | Gravité | Parade |
|---|---|---|
| cargokit archivé en amont ; il appelle `rustup run stable cargo` (version non épinglable par config) | moyenne | Patcher notre copie de `rust_builder/cargokit` pour utiliser la version de `rust-toolchain.toml` ; `cargokit_options.yaml` avec `use_precompiled_binaries: false` ; surveiller le backend **Native Assets** de flutter_rust_bridge et migrer quand il aura des précédents F-Droid/Nix ; garder `budget-core` construisible seul par `cargo` (le pont n'est qu'une fine couche) |
| Plugin Flutter qui tire GMS/Firebase (géoloc, push, ML Kit, IAP) | moyenne | Règle de dépendances : aucun plugin GMS ; contrôle en CI (`grep` de `com.google.android.gms`/`firebase` dans `pubspec.lock` et Gradle) ; flavor `fdroid` si un jour nécessaire |
| SDK qui télécharge des binaires au build (PowerSync, futurs plugins à « hooks ») | moyenne (plan B surtout) | Refuser toute dépendance qui télécharge un binaire, ou fournir le patch et le build source dans la recette |
| Build non reproductible (chemins, NDK, horodatages, codegen FRB) | faible à moyenne | Reprendre les recettes GitSync et LocalSend : chemin absolu fixe, `--remap-path-prefix`, `-Ccodegen-units=1`, `SOURCE_DATE_EPOCH`, NDK épinglé, un APK par ABI, versions épinglées de `flutter_rust_bridge_codegen` et de rustc ; générer le code FRB et le **committer** pour éviter `cargo expand` dans la recette |
| Notices de licence Rust absentes de l'app | faible | Générer les notices des crates, les enregistrer dans `LicenseRegistry` ; `cargo deny check licenses` avec une liste blanche (MIT, Apache-2.0, BSD-2/3, ISC, Zlib, Unicode-3.0, CDLA-Permissive-2.0, MPL-2.0, CC0) |
| Défauts réseau de tiers (relais/DNS n0 d'iroh) | faible | Presets `Minimal`/`Empty` (§10.8) ; aucune URL par défaut (11.8) |
| Flutter : moteur précompilé (F-Droid comme Nix) | faible (toléré partout) | Rien à faire ; c'est le cas de toutes les apps Flutter |

**À mettre en place dès le jour 1** :
1. **Job CI « comme F-Droid »** : un conteneur Debian stable, `rustup` de Debian, rustc et NDK épinglés, Flutter épinglé par `.fvmrc` ou checkout de tag, `PUB_CACHE` dans le dépôt, `flutter pub get --enforce-lockfile`, puis `fdroid build` / `fdroid scanner` (fdroidserver) sur une recette `metadata/<appid>.yml` versionnée dans notre dépôt. On peut partir du [modèle Flutter](https://gitlab.com/fdroid/fdroiddata/-/blob/master/templates/build-flutter.yml) et de la recette GitSync. Ajouter une étape qui compare l'APK au build de release, pour viser `binary:` et la reproductibilité.
2. **`flake.nix`** : `budget-core` et le relais par `rustPlatform.buildRustPackage` (`cargoLock.lockFile`) ; l'app Linux par `buildFlutterApplication` (`pubspec.lock.json` régénéré en CI avec `yq`), avec la lib Rust injectée comme dans LocalSend ; un `devShell` ; un module NixOS pour le relais.
3. **Hygiène des dépendances** : `Cargo.lock` et `pubspec.lock` committés ; `cargo deny` (licences + advisories) ; liste noire GMS/Firebase ; aucune dépendance qui télécharge des binaires au build.
4. **Licence et contributions** : `LICENSE-MIT` + `LICENSE-APACHE`, `license = "MIT OR Apache-2.0"` dans chaque `Cargo.toml` et dans `pubspec.yaml`, DCO (`Signed-off-by`) vérifié en CI, pas de CLA.
5. **Produit** : aucune URL de serveur par défaut ; aucun SDK d'analytics ni de crash reporting ; flavor unique tant que rien n'impose un flavor non libre.

---

## Annexe : activité des dépôts (API GitHub, 2026-11-14)

| Dépôt | Dernier push | Licence (GitHub) | ★ |
|---|---|---|---|
| actualbudget/actual | 2026-11-14 | MIT | 30k |
| powersync-ja/powersync-service | 2026-11-13 | Apache-2.0 | 405 |
| neondatabase/electric (ex electric-sql) | 2026-11-13 | Apache-2.0 | 10.4k |
| rocicorp/mono (Zero) | 2026-11-14 | Apache-2.0 | 3.4k |
| aspen-cloud/triplit | 2026-01-19 (dernier commit sur main le 2025-09-11) | AGPL-3.0 | 3.1k |
| instantdb/instant | 2026-11-05 | Apache-2.0 | 10.6k |
| vlcn-io/cr-sqlite | 2026-08-10 | MIT | 3.8k |
| automerge/automerge | 2026-11-13 | MIT | 6.7k |
| loro-dev/loro | 2026-11-13 | MIT | 6.3k |
| garden-co/jazz | 2026-11-14 | MIT | 244 (chiffre renvoyé par l'API, probablement après un transfert/renommage du dépôt) |
| evoluhq/evolu | 2026-11-14 | MIT | 1.9k |
| tursodatabase/turso | 2026-11-14 | MIT | 25k |
| realm/realm-dart | 2025-10-23 | Apache-2.0 | 0.8k |
| fzyzcjy/flutter_rust_bridge | 2026-11-08 | MIT | 5.5k |
| mozilla/uniffi-rs | 2026-11-12 | MPL-2.0 | 5.1k |
| redbadger/crux | 2026-11-12 | Apache-2.0 | 2.8k |
| DioxusLabs/dioxus | 2026-11-13 | Apache-2.0 (+MIT) | 39.8k |
| slint-ui/slint | 2026-11-14 | multi-licence | 24.6k |
| tauri-apps/tauri | 2026-11-13 | Apache-2.0 (+MIT) | 112.9k |
