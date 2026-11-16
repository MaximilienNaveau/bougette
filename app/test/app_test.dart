// The app's screens, driven like a user would, on the real Rust library
// and a fresh database. Run with the library's path:
//   BOUGETTE_LIB=../target/release/libbougette_api.so flutter test

import 'dart:io';

import 'package:bougette/main.dart';
import 'package:bougette/src/rust/api.dart';
import 'package:bougette/src/rust/frb_generated.dart';
import 'package:flutter/material.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  setUpAll(() async {
    final library = Platform.environment['BOUGETTE_LIB'];
    if (library == null) {
      throw StateError('set BOUGETTE_LIB to libbougette_api.so');
    }
    await RustLib.init(externalLibrary: ExternalLibrary.open(library));
  });

  // Widget tests run on a fake clock, so real work (here, the Rust calls)
  // only progresses inside `runAsync`. Alternate: let it run, then move
  // the fake clock on (page transitions take 300 ms). Not `pumpAndSettle`:
  // a loading spinner never settles.
  Future<void> settle(WidgetTester tester) async {
    for (var i = 0; i < 10; i++) {
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 50)),
      );
      await tester.pump(const Duration(milliseconds: 100));
    }
  }

  testWidgets('from the budgets to a month', (tester) async {
    // A wide window, so the tables fit.
    tester.view.physicalSize = const Size(1600, 1200);
    tester.view.devicePixelRatio = 1;
    final year = DateTime.now().year;
    // The data, entered through the same API the screens use.
    final app = (await tester.runAsync(() async {
      final folder = Directory.systemTemp.createTempSync('bougette-test');
      final app = await App.open(path: '${folder.path}/test.sqlite');
      final id = await app.createBudget(name: 'Maison', currency: 'EUR');
      await app.addLine(
        budget: id,
        name: 'Loyer',
        category: 'Logement',
        income: false,
        amount: 110000,
        repeat: Repeat.monthly,
        first: '$year-01-05',
      );
      return app;
    }))!;

    await tester.pumpWidget(BougetteApp(app: app));
    await settle(tester);
    expect(find.text('Maison'), findsOneWidget);
    // No income, 1,100 of rent a month; and the running balance is the
    // same, past months having no transaction (their actual is 0).
    expect(
      find.text('Monthly -€1,100.00    Running -€1,100.00'),
      findsOneWidget,
    );

    await tester.tap(find.text('Maison'));
    await settle(tester);
    expect(find.text('January'), findsOneWidget);
    expect(find.text('December'), findsOneWidget);
    expect(find.text('forecast'), findsOneWidget);

    await tester.tap(find.text('December'));
    await settle(tester);
    expect(find.text('Loyer (Logement)'), findsOneWidget);

    // A pot, through its form.
    await tester.tap(find.text('Add a pot'));
    await settle(tester);
    await tester.enterText(find.widgetWithText(TextField, 'Name'), 'Vélo');
    await tester.enterText(
      find.widgetWithText(TextField, 'Goal (optional)'),
      '1200',
    );
    await tester.tap(find.text('OK'));
    await settle(tester);
    expect(find.text('Vélo'), findsOneWidget);
    expect(find.text('goal €1,200.00'), findsOneWidget);

    // The rent has no transaction: it can be removed.
    await tester.tap(find.text('Loyer (Logement)'));
    await settle(tester);
    await tester.tap(find.text('Remove it'));
    await settle(tester);
    await tester.tap(find.text('Yes'));
    await settle(tester);
    expect(find.text('Loyer (Logement)'), findsNothing);
  });

  testWidgets('in French', (tester) async {
    tester.view.physicalSize = const Size(1600, 1200);
    tester.view.devicePixelRatio = 1;
    // The system's language, as the app sees it at start.
    tester.platformDispatcher.localesTestValue = const [Locale('fr', 'FR')];
    final app = (await tester.runAsync(() async {
      final folder = Directory.systemTemp.createTempSync('bougette-test');
      final app = await App.open(path: '${folder.path}/test.sqlite');
      final id = await app.createBudget(name: 'Maison', currency: 'EUR');
      await app.addLine(
        budget: id,
        name: 'Loyer',
        category: 'Logement',
        income: false,
        amount: 123456,
        repeat: Repeat.monthly,
        first: '${DateTime.now().year}-01-05',
      );
      return app;
    }))!;
    await tester.pumpWidget(BougetteApp(app: app));
    await settle(tester);
    expect(find.text('Mes budgets'), findsOneWidget);
    // A decimal comma, a narrow no-break space between thousands, and the
    // symbol after the amount.
    expect(find.textContaining('1\u202f234,56\u00a0€'), findsOneWidget);
    await tester.tap(find.text('Maison'));
    await settle(tester);
    expect(find.text('janvier'), findsOneWidget);
    expect(find.text('projection'), findsOneWidget);
  });
}
