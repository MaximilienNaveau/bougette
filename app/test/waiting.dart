// Waiting in widget tests for work done outside Flutter: the Rust calls,
// which take real time while the test runs on a fake clock.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

/// One round: real time for the Rust calls (`runAsync`), then the fake
/// clock moved on so Flutter shows their results.
Future<void> _round(WidgetTester tester) async {
  await tester.runAsync(
    () => Future<void>.delayed(const Duration(milliseconds: 50)),
  );
  await tester.pump(const Duration(milliseconds: 100));
}

/// Lets a tap or a page change happen: a few rounds (a page transition
/// takes 300 ms). Not `pumpAndSettle`: a loading spinner never settles.
/// The checks after it wait for the Rust results themselves (`expectSoon`).
Future<void> settle(WidgetTester tester) async {
  for (var i = 0; i < 10; i++) {
    await _round(tester);
  }
}

/// `expect(finder, matcher)`, once it holds: tried again every round for
/// up to about 15 seconds, so a slow or busy machine gets the time it
/// needs and a fast one goes on at once. Fails as `expect` would.
Future<void> expectSoon(
  WidgetTester tester,
  Finder finder,
  Matcher matcher, {
  String? reason,
}) async {
  for (var i = 0; i < 100; i++) {
    // `matches` is what `expect` checks, without failing.
    if (matcher.matches(finder, <dynamic, dynamic>{})) return;
    await _round(tester);
  }
  // Still not: fail, saying what the screen shows, to see why.
  final shown = find
      .byType(Text)
      .evaluate()
      .map((element) => (element.widget as Text).data)
      .whereType<String>()
      .join(' | ');
  expect(finder, matcher, reason: '${reason ?? ''} (shown: $shown)');
}

/// Taps `finder` once it finds something to tap (see `expectSoon`).
Future<void> tapSoon(WidgetTester tester, Finder finder) async {
  await expectSoon(tester, finder, findsWidgets);
  await tester.tap(finder);
}
