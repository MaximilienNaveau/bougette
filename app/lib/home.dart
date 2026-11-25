// The home screen (doc/scope.md): my budgets, each with its monthly
// balance and running balance, without an aggregate total.

import 'dart:io';

import 'package:flutter/material.dart';

import 'budget.dart';
import 'forms.dart';
import 'look.dart';
import 'notices.dart';
import 'settings.dart';
import 'src/rust/api.dart';
import 'src/rust/api/members.dart';

class HomeScreen extends StatefulWidget {
  const HomeScreen({super.key, required this.app});

  final App app;

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

// A `StatefulWidget` keeps state between rebuilds in its `State` object:
// here, the budgets being loaded. Names starting with `_` are private to
// the file.
class _HomeScreenState extends State<HomeScreen> {
  // `late`: set before first use (in `initState`), not at construction.
  late Future<List<BudgetSummary>> _budgets;
  // The bell's counter: notifications not read yet.
  int _unread = 0;

  @override
  void initState() {
    super.initState();
    _reload();
  }

  void _reload() {
    // `setState` tells Flutter to rebuild with the new value. A block, not
    // `=> ...`: an arrow function would return the Future, which
    // `setState` refuses.
    setState(() {
      _budgets = widget.app.budgets();
    });
    _countUnread();
  }

  Future<void> _countUnread() async {
    final unread = await widget.app.unreadNotifications();
    if (mounted) setState(() => _unread = unread);
  }

  /// The notifications; opening them marks them read.
  Future<void> _notices() async {
    await Navigator.of(
      context,
    ).push(MaterialPageRoute(builder: (_) => NoticesScreen(app: widget.app)));
    _reload();
  }

  /// Syncs with the server, and says what came and went.
  Future<void> _sync() async {
    SyncSummary? summary;
    final ok = await guard(
      context,
      () async => summary = await widget.app.syncNow(),
    );
    if (ok && mounted) {
      final text = context.words.synced(
        '${summary!.sent}',
        '${summary!.received}',
      );
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(text)));
    }
    _reload();
  }

  /// Exports every budget (JSON) and the transactions (CSV) into the
  /// Documents folder, or the home folder without one, and says where.
  Future<void> _export() async {
    final home = Platform.environment['HOME'] ?? '.';
    final documents = Directory('$home/Documents');
    final folder = documents.existsSync() ? documents.path : home;
    final ok = await guard(
      context,
      () => widget.app.exportFiles(folder: folder),
    );
    if (ok && mounted) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(context.words.exportedTo(folder))));
    }
  }

  Future<void> _create() async {
    final name = await askText(
      context,
      context.words.newBudget,
      context.words.name,
    );
    // The screen may have closed while the dialog was open.
    if (name == null || name.isEmpty || !mounted) return;
    await guard(
      context,
      () => widget.app.createBudget(name: name, currency: 'EUR'),
    );
    _reload();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(context.words.myBudgets),
        actions: [
          bell(context, _unread, _notices),
          IconButton(
            onPressed: _export,
            icon: const Icon(Icons.download),
            tooltip: context.words.export,
          ),
          IconButton(
            onPressed: _sync,
            icon: const Icon(Icons.sync),
            tooltip: context.words.syncNow,
          ),
          IconButton(
            onPressed: () async {
              await Navigator.of(context).push(
                MaterialPageRoute(
                  builder: (_) => SettingsScreen(app: widget.app),
                ),
              );
              _reload();
            },
            icon: const Icon(Icons.settings),
            tooltip: context.words.settings,
          ),
        ],
      ),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: _create,
        icon: const Icon(Icons.add),
        label: Text(context.words.budgetButton),
      ),
      // A `FutureBuilder` shows the result of a Future once it arrives,
      // and something else meanwhile.
      body: FutureBuilder(
        future: _budgets,
        builder: (context, snapshot) {
          if (snapshot.hasError) {
            return Center(child: Text('${snapshot.error}'));
          }
          final budgets = snapshot.data;
          if (budgets == null) {
            return const Center(child: CircularProgressIndicator());
          }
          if (budgets.isEmpty) {
            return Center(child: Text(context.words.noBudget));
          }
          return ListView(
            children: [
              // A `for` inside a list literal: one tile per budget.
              for (final budget in budgets)
                ListTile(
                  leading: Icon(budget.personal ? Icons.person : Icons.group),
                  title: Text(budget.name),
                  subtitle: Text(
                    '${context.words.monthlyAmount(money(budget.monthlyBalance, budget.currency))}'
                    '    '
                    '${context.words.runningAmount(money(budget.runningBalance, budget.currency))}',
                  ),
                  onTap: () async {
                    await Navigator.of(context).push(
                      MaterialPageRoute(
                        builder: (_) =>
                            BudgetScreen(app: widget.app, budget: budget),
                      ),
                    );
                    _reload(); // something may have changed
                  },
                ),
            ],
          );
        },
      ),
    );
  }
}
