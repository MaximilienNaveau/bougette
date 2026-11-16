// Bougette's app (M3): it shows what the Rust core computes and forwards
// what the user does; it computes nothing itself (CLAUDE.md, §4).
//
// About Dart, for a C++/Python reader: classes and types look like Java or
// C#; `final` is a variable set once (like `const` on a local), `?` after
// a type allows null (`String?`), and `async`/`await` work as in Python.
// Flutter builds the screen from *widgets*: small objects describing a
// piece of the interface, rebuilt whenever what they show changes.

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';

import 'package:intl/intl.dart';

import 'home.dart';
import 'l10n/app_localizations.dart';
import 'src/rust/api.dart';
import 'src/rust/frb_generated.dart';

// `Future<void> main() async`: an asynchronous entry point, so it can
// `await` the library loading before showing anything.
Future<void> main() async {
  await RustLib.init(externalLibrary: ExternalLibrary.open(_libraryPath()));
  final app = await App.open(path: _databasePath());
  runApp(BougetteApp(app: app));
}

/// The Rust library: `$BOUGETTE_LIB` while developing, else next to the
/// executable, in `lib/` (where the Nix package puts it).
String _libraryPath() {
  final fromEnvironment = Platform.environment['BOUGETTE_LIB'];
  if (fromEnvironment != null) return fromEnvironment;
  final bundle = File(Platform.resolvedExecutable).parent.path;
  return '$bundle/lib/libbougette_api.so';
}

/// The database, shared with the command-line client: `$BOUGETTE_DB`, or
/// `$XDG_DATA_HOME/bougette/bougette.sqlite` (`~/.local/share` by default).
String _databasePath() {
  final environment = Platform.environment;
  final fromEnvironment = environment['BOUGETTE_DB'];
  if (fromEnvironment != null) return fromEnvironment;
  // `??` gives the right side when the left is null, like Python's `or`.
  final data =
      environment['XDG_DATA_HOME'] ?? '${environment['HOME']}/.local/share';
  final folder = Directory('$data/bougette')..createSync(recursive: true);
  return '${folder.path}/bougette.sqlite';
}

/// The whole app: its theme and its first screen.
// A `StatelessWidget` shows only what it is given: here, the `App` handle.
class BougetteApp extends StatelessWidget {
  // `required this.app`: a named constructor argument stored in the field.
  const BougetteApp({super.key, required this.app});

  final App app;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Bougette',
      debugShowCheckedModeBanner: false,
      // The languages, French and English; the system's is used, English
      // if it is neither (doc/scope.md).
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      // Amounts and dates follow the language chosen.
      builder: (context, child) {
        Intl.defaultLocale = Localizations.localeOf(context).toLanguageTag();
        return child!;
      },
      // Light and dark themes from one seed colour; the system picks.
      theme: ThemeData(colorSchemeSeed: Colors.teal),
      darkTheme: ThemeData(
        colorSchemeSeed: Colors.teal,
        brightness: Brightness.dark,
      ),
      home: HomeScreen(app: app),
    );
  }
}
