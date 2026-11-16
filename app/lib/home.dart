// The home screen (doc/scope.md): my budgets, each with its monthly
// balance and running balance, without an aggregate total.

import 'package:flutter/material.dart';

import 'calendar.dart';
import 'forms.dart';
import 'look.dart';
import 'src/rust/api.dart';

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
  }

  Future<void> _create() async {
    final name = await askText(context, 'New budget', 'Name');
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
      appBar: AppBar(title: const Text('My budgets')),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: _create,
        icon: const Icon(Icons.add),
        label: const Text('Budget'),
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
            return const Center(child: Text('No budget yet: add one.'));
          }
          return ListView(
            children: [
              // A `for` inside a list literal: one tile per budget.
              for (final budget in budgets)
                ListTile(
                  leading: Icon(budget.personal ? Icons.person : Icons.group),
                  title: Text(budget.name),
                  subtitle: Row(
                    children: [
                      const Text('Monthly '),
                      Amount(budget.monthlyBalance, budget.currency),
                      const Text('   Running '),
                      Amount(budget.runningBalance, budget.currency),
                    ],
                  ),
                  onTap: () async {
                    await Navigator.of(context).push(
                      MaterialPageRoute(
                        builder: (_) =>
                            CalendarScreen(app: widget.app, budget: budget),
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
