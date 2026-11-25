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

import 'waiting.dart';

void main() {
  setUpAll(() async {
    final library = Platform.environment['BOUGETTE_LIB'];
    if (library == null) {
      throw StateError('set BOUGETTE_LIB to libbougette_api.so');
    }
    await RustLib.init(externalLibrary: ExternalLibrary.open(library));
  });

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
    await expectSoon(tester, find.text('Maison'), findsOneWidget);
    // No income, 1,100 of rent a month; and the running balance is the
    // same, past months having no transaction (their actual is 0).
    await expectSoon(
      tester,
      find.text('Monthly -€1,100.00    Running -€1,100.00'),
      findsOneWidget,
    );

    await tapSoon(tester, find.text('Maison'));
    await settle(tester);
    await expectSoon(tester, find.text('January'), findsOneWidget);
    await expectSoon(tester, find.text('December'), findsOneWidget);
    await expectSoon(tester, find.text('forecast'), findsOneWidget);

    await tapSoon(tester, find.text('December'));
    await settle(tester);
    await expectSoon(tester, find.text('Loyer (Logement)'), findsOneWidget);

    // A pot, through its form.
    await tapSoon(tester, find.text('Add a pot'));
    await settle(tester);
    await tester.enterText(find.widgetWithText(TextField, 'Name'), 'Vélo');
    await tester.enterText(
      find.widgetWithText(TextField, 'Goal (optional)'),
      '1200',
    );
    await tapSoon(tester, find.text('OK'));
    await settle(tester);
    await expectSoon(tester, find.text('Vélo'), findsOneWidget);
    await expectSoon(tester, find.text('goal €1,200.00'), findsOneWidget);

    // The rent in full: its amounts as a table, a change added to it.
    await tapSoon(tester, find.text('Loyer (Logement)'));
    await settle(tester);
    await expectSoon(tester, find.text('Start'), findsOneWidget);
    await expectSoon(tester, find.text('€1,100.00'), findsOneWidget);
    await tapSoon(tester, find.text('Add a change'));
    await settle(tester);
    await tester.enterText(find.widgetWithText(TextField, 'Amount'), '1200');
    await tapSoon(tester, find.text('OK'));
    await settle(tester);
    await expectSoon(tester, find.text('Change 1'), findsOneWidget);
    await expectSoon(tester, find.text('€1,200.00'), findsOneWidget);
    // Removed again from the table (the start has a button too).
    await tapSoon(tester, find.byTooltip('Remove this change').last);
    await settle(tester);
    await tapSoon(tester, find.text('Yes'));
    await settle(tester);
    await expectSoon(tester, find.text('Change 1'), findsNothing);

    // The rent has no transaction: it can be removed.
    await tapSoon(tester, find.byTooltip('Remove it'));
    await settle(tester);
    await tapSoon(tester, find.text('Yes'));
    await settle(tester);
    await expectSoon(tester, find.text('Loyer (Logement)'), findsNothing);
  });

  testWidgets('a transaction is changed, then removed', (tester) async {
    tester.view.physicalSize = const Size(1600, 1200);
    tester.view.devicePixelRatio = 1;
    final now = DateTime.now();
    final today =
        '${now.year}-${'${now.month}'.padLeft(2, '0')}-${'${now.day}'.padLeft(2, '0')}';
    final app = (await tester.runAsync(() async {
      final folder = Directory.systemTemp.createTempSync('bougette-test');
      final app = await App.open(path: '${folder.path}/test.sqlite');
      final id = await app.createBudget(name: 'Maison', currency: 'EUR');
      await app.addTransaction(
        budget: id,
        dateText: today,
        income: false,
        amount: 4200,
        category: 'Courses',
        note: 'marché',
        fromPot: false,
      );
      return app;
    }))!;
    await tester.pumpWidget(BougetteApp(app: app));
    await settle(tester);
    await tapSoon(tester, find.text('Maison'));
    await settle(tester);

    // Opened with what it holds; 42 becomes 45.
    await tapSoon(tester, find.text('Courses · marché'));
    await settle(tester);
    await expectSoon(
      tester,
      find.text('Change the transaction'),
      findsOneWidget,
    );
    // Who added it and when: this member, just now.
    await expectSoon(
      tester,
      find.textContaining('Added: you, '),
      findsOneWidget,
    );
    await tester.enterText(find.widgetWithText(TextField, '42.00'), '45');
    await tapSoon(tester, find.text('OK'));
    await settle(tester);
    await expectSoon(tester, find.text('-€45.00'), findsWidgets);

    // Opened again, removed, and put back with Undo.
    Future<void> remove() async {
      await tapSoon(tester, find.text('Courses · marché'));
      await settle(tester);
      await tapSoon(tester, find.text('Remove it'));
      await settle(tester);
      await tapSoon(tester, find.text('Yes'));
      await settle(tester);
    }

    await remove();
    await expectSoon(tester, find.text('Courses · marché'), findsNothing);
    await tapSoon(tester, find.text('Undo'));
    await settle(tester);
    await expectSoon(tester, find.text('Courses · marché'), findsOneWidget);

    // Removed again, then put back from the trash.
    await remove();
    await expectSoon(tester, find.text('Courses · marché'), findsNothing);
    await tapSoon(tester, find.byType(PopupMenuButton<VoidCallback>));
    await settle(tester);
    await tapSoon(tester, find.text('Trash'));
    await settle(tester);
    await expectSoon(tester, find.text('Courses · marché'), findsOneWidget);
    await tapSoon(tester, find.byTooltip('Restore'));
    await settle(tester);
    await expectSoon(
      tester,
      find.text('Nothing deleted in the last 30 days.'),
      findsOneWidget,
    );
    await tester.pageBack();
    await settle(tester);
    await expectSoon(tester, find.text('Courses · marché'), findsOneWidget);

    // Removed for good: removed, then the trash emptied.
    await remove();
    await tapSoon(tester, find.byType(PopupMenuButton<VoidCallback>));
    await settle(tester);
    await tapSoon(tester, find.text('Trash'));
    await settle(tester);
    await tapSoon(tester, find.byTooltip('Empty the trash'));
    await settle(tester);
    await tapSoon(tester, find.text('Yes'));
    await settle(tester);
    await expectSoon(
      tester,
      find.text('Nothing deleted in the last 30 days.'),
      findsOneWidget,
    );
  });

  testWidgets('the year by category', (tester) async {
    tester.view.physicalSize = const Size(1600, 1200);
    tester.view.devicePixelRatio = 1;
    final year = DateTime.now().year;
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
    await tapSoon(tester, find.text('Maison'));
    await settle(tester);

    // The year above the months opens it by category.
    await tapSoon(tester, find.text('$year'));
    await settle(tester);
    await expectSoon(tester, find.text('$year by category'), findsOneWidget);
    await expectSoon(tester, find.text('Logement'), findsOneWidget);
    // Twelve rents planned: the category's row and the balance's.
    await expectSoon(tester, find.text('-€13,200.00'), findsNWidgets(2));
  });

  testWidgets('a narrow window shows the whole year', (tester) async {
    final app = (await tester.runAsync(() async {
      final folder = Directory.systemTemp.createTempSync('bougette-test');
      final app = await App.open(path: '${folder.path}/test.sqlite');
      await app.createBudget(name: 'Maison', currency: 'EUR');
      return app;
    }))!;
    // Two rows of 6, then three rows of 4: every month can be tapped.
    for (final width in [800.0, 500.0]) {
      tester.view.physicalSize = Size(width, 1000);
      tester.view.devicePixelRatio = 1;
      await tester.pumpWidget(BougetteApp(app: app));
      await settle(tester);
      await tapSoon(tester, find.text('Maison'));
      await settle(tester);
      for (final month in ['January', 'June', 'December']) {
        await expectSoon(
          tester,
          find.text(month).hitTestable(),
          findsOneWidget,
          reason: month,
        );
      }
      // Empty: no year before this one to go to; the next one, named.
      final year = DateTime.now().year;
      final back = find.ancestor(
        of: find.byIcon(Icons.chevron_left),
        matching: find.byWidgetPredicate((w) => w is ButtonStyleButton),
      );
      expect(tester.widget<ButtonStyleButton>(back).enabled, isFalse);
      await tapSoon(tester, find.text('${year + 1}'));
      await settle(tester);
      await expectSoon(tester, find.text('${year + 2}'), findsOneWidget);
      expect(tester.widget<ButtonStyleButton>(back).enabled, isTrue);
      await tester.pageBack();
      await settle(tester);
    }
  });

  testWidgets('a line going to another budget shows in both', (tester) async {
    tester.view.physicalSize = const Size(1600, 1200);
    tester.view.devicePixelRatio = 1;
    final app = (await tester.runAsync(() async {
      final folder = Directory.systemTemp.createTempSync('bougette-test');
      final app = await App.open(path: '${folder.path}/test.sqlite');
      final home = await app.createBudget(name: 'Maison', currency: 'EUR');
      await app.setPersonal(budget: home, personal: true);
      await app.createBudget(name: 'Appartement', currency: 'EUR');
      return app;
    }))!;
    await tester.pumpWidget(BougetteApp(app: app));
    await settle(tester);

    // From the personal budget, 100 a month to the flat's budget.
    await tapSoon(tester, find.text('Maison'));
    await settle(tester);
    await tapSoon(tester, find.text('Add a line'));
    await settle(tester);
    await tester.enterText(find.widgetWithText(TextField, 'Name'), 'Appart');
    await tester.enterText(
      find.widgetWithText(TextField, 'Category'),
      'Apport',
    );
    await tester.enterText(find.widgetWithText(TextField, 'Amount'), '100');
    await tapSoon(tester, find.text('Feeds no pot'));
    await settle(tester);
    // The open menu draws its items again: tap the last one.
    await tapSoon(tester, find.text('Goes to the Appartement budget').last);
    await settle(tester);
    await tapSoon(tester, find.text('OK'));
    await settle(tester);
    // Here, a cost in the category named after the other budget.
    await expectSoon(tester, find.text('Appart (Appartement)'), findsOneWidget);
    await expectSoon(
      tester,
      find.byTooltip('from another budget'),
      findsOneWidget,
    );

    // There, an income in the category typed.
    await tester.pageBack();
    await settle(tester);
    await tapSoon(tester, find.text('Appartement'));
    await settle(tester);
    await expectSoon(tester, find.text('Appart (Apport)'), findsOneWidget);
    await expectSoon(
      tester,
      find.byTooltip('from another budget'),
      findsNothing,
    );

    // From the personal budget again, redirected from today (its start):
    // all of it stays home, as a plain cost.
    await tester.pageBack();
    await settle(tester);
    await tapSoon(tester, find.text('Maison'));
    await settle(tester);
    await tapSoon(tester, find.text('Appart (Appartement)'));
    await settle(tester);
    await tapSoon(tester, find.text('Goes to the Appartement budget'));
    await settle(tester);
    // The form's choice, drawn over the screen's tile: the last one.
    await tapSoon(tester, find.text('Goes to the Appartement budget').last);
    await settle(tester);
    await tapSoon(tester, find.text('Feeds no pot').last);
    await settle(tester);
    await tapSoon(tester, find.text('OK'));
    await settle(tester);
    // Back on the month: home's own line now, in the category typed.
    await expectSoon(tester, find.text('Appart (Apport)'), findsOneWidget);
    await expectSoon(
      tester,
      find.byTooltip('from another budget'),
      findsNothing,
    );
    await tester.pageBack();
    await settle(tester);
    await tapSoon(tester, find.text('Appartement'));
    await settle(tester);
    await expectSoon(tester, find.text('Appart (Apport)'), findsNothing);
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
    await expectSoon(tester, find.text('Mes budgets'), findsOneWidget);
    // A decimal comma, a narrow no-break space between thousands, and the
    // symbol after the amount.
    await expectSoon(
      tester,
      find.textContaining('1\u202f234,56\u00a0€'),
      findsOneWidget,
    );
    await tapSoon(tester, find.text('Maison'));
    await settle(tester);
    await expectSoon(tester, find.text('janvier'), findsOneWidget);
    await expectSoon(tester, find.text('projection'), findsOneWidget);
  });
}
