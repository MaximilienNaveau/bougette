// Joining a server, the members of a budget and inviting someone, through
// the screens, with a real server running. Needs the library and the
// server program:
//   BOUGETTE_LIB=.../libbougette_api.so \
//   BOUGETTE_SERVER=.../bougette-server flutter test

import 'dart:convert';
import 'dart:io';

import 'package:bougette/main.dart';
import 'package:bougette/src/rust/api.dart';
import 'package:bougette/src/rust/frb_generated.dart';
import 'package:flutter/material.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  final server = Platform.environment['BOUGETTE_SERVER'];
  late Directory folder;
  Process? serving;
  late String address;
  late String token;

  setUpAll(() async {
    await RustLib.init(
      externalLibrary: ExternalLibrary.open(
        Platform.environment['BOUGETTE_LIB']!,
      ),
    );
    if (server == null) return;
    folder = Directory.systemTemp.createTempSync('bougette-members');
    final database = '${folder.path}/server.sqlite';
    // The operator invites `alice`: the token is the last word printed.
    final invited = await Process.run(server, [
      '--db',
      database,
      'invite',
      'alice',
    ]);
    token = (invited.stdout as String).trim().split(RegExp(r'\s+')).last;
    serving = await Process.start(server, [
      '--db',
      database,
      'serve',
      '--port',
      '47299',
    ]);
    // "Serving at ID@IP:PORT,...": reach it through the loopback.
    final line = await serving!.stdout
        .transform(utf8.decoder)
        .transform(const LineSplitter())
        .firstWhere((line) => line.startsWith('Serving at'));
    final id = line.split(' ').last.split('@').first;
    address = '$id@127.0.0.1:47299';
  });

  tearDownAll(() => serving?.kill());

  Future<void> settle(WidgetTester tester) async {
    for (var i = 0; i < 20; i++) {
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 100)),
      );
      await tester.pump(const Duration(milliseconds: 100));
    }
  }

  testWidgets('join a server, create a budget, invite', (tester) async {
    tester.view.physicalSize = const Size(1600, 1200);
    tester.view.devicePixelRatio = 1;
    final app = (await tester.runAsync(
      () => App.open(path: '${folder.path}/alice.sqlite'),
    ))!;
    await tester.pumpWidget(BougetteApp(app: app));
    await settle(tester);

    // The settings: a name, then the server.
    await tester.tap(find.byTooltip('Settings'));
    await settle(tester);
    await tester.tap(find.text('How the other members see you'));
    await settle(tester);
    await tester.enterText(find.byType(TextField), 'Alice');
    await tester.tap(find.text('OK'));
    await settle(tester);
    expect(find.text('Alice'), findsOneWidget);

    await tester.tap(find.text('Join a server'));
    await settle(tester);
    await tester.enterText(find.widgetWithText(TextField, 'Address'), address);
    await tester.enterText(
      find.widgetWithText(TextField, 'Invitation from the operator'),
      token,
    );
    await tester.enterText(find.widgetWithText(TextField, 'Username'), 'alice');
    await tester.tap(find.text('OK'));
    await settle(tester);
    expect(find.textContaining('alice on'), findsOneWidget);
    expect(find.text('Sync now'), findsOneWidget);

    // A budget, and its members: Alice alone, its Admin.
    await tester.pageBack();
    await settle(tester);
    await tester.tap(find.text('Budget'));
    await settle(tester);
    await tester.enterText(find.byType(TextField), 'Maison');
    await tester.tap(find.text('OK'));
    await settle(tester);
    await tester.tap(find.text('Maison'));
    await settle(tester);
    await tester.tap(find.byType(PopupMenuButton<VoidCallback>));
    await settle(tester);
    await tester.tap(find.text('Members'));
    await settle(tester);
    expect(find.text('Alice (you)'), findsOneWidget);
    expect(find.text('Admin'), findsOneWidget);

    // An invitation, as an Editor for 7 days: its text names the server.
    await tester.tap(find.text('Invite'));
    await settle(tester);
    await tester.tap(find.text('Editor'));
    await settle(tester);
    await tester.tap(find.text('OK'));
    await settle(tester);
    expect(find.textContaining('$address#'), findsOneWidget);
  }, skip: server == null);
}
