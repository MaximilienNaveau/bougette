// One month of a budget: each line planned and actual, the transactions,
// the totals, the running balance and the pots. Tapping a line or a
// transaction offers what can be done to it.

import 'package:flutter/material.dart';

import 'forms.dart';
import 'look.dart';
import 'src/rust/api.dart';

class MonthScreen extends StatefulWidget {
  const MonthScreen({
    super.key,
    required this.app,
    required this.budget,
    required this.month,
  });

  final App app;
  final String budget;
  final String month;

  @override
  State<MonthScreen> createState() => _MonthScreenState();
}

class _MonthScreenState extends State<MonthScreen> {
  late Future<MonthView> _view;

  @override
  void initState() {
    super.initState();
    _load();
  }

  void _load() {
    setState(() {
      _view = widget.app.month(budget: widget.budget, month: widget.month);
    });
  }

  /// Runs a form, and reloads the month if it changed something.
  Future<void> _then(Future<bool> changed) async {
    if (await changed) _load();
  }

  @override
  Widget build(BuildContext context) {
    final app = widget.app;
    final budget = widget.budget;
    return FutureBuilder(
      future: _view,
      builder: (context, snapshot) {
        final view = snapshot.data;
        return Scaffold(
          appBar: AppBar(
            title: Text(
              view == null ? widget.month : '${view.budget}, ${view.month}',
            ),
          ),
          floatingActionButton: view == null
              ? null
              : Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    FloatingActionButton.small(
                      heroTag: 'line',
                      tooltip: 'Add a line',
                      onPressed: () =>
                          _then(addLine(context, app, budget, view.pots)),
                      child: const Icon(Icons.playlist_add),
                    ),
                    const SizedBox(height: 8),
                    FloatingActionButton.extended(
                      heroTag: 'transaction',
                      onPressed: () =>
                          _then(addTransaction(context, app, budget, view)),
                      icon: const Icon(Icons.add),
                      label: const Text('Transaction'),
                    ),
                  ],
                ),
          body: snapshot.hasError
              ? Center(child: Text('${snapshot.error}'))
              : view == null
              ? const Center(child: CircularProgressIndicator())
              : _MonthBody(
                  view: view,
                  onLine: (line) => _then(editLine(context, app, budget, line)),
                  onTransaction: (transaction) => _then(
                    _removeTransaction(context, app, budget, transaction),
                  ),
                  onAddPot: () => _then(addPot(context, app, budget)),
                ),
        );
      },
    );
  }
}

/// Asks, then removes a transaction; a reflection can't be removed here.
Future<bool> _removeTransaction(
  BuildContext context,
  App app,
  String budget,
  TransactionRow transaction,
) async {
  if (transaction.reflected) {
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(
        content: Text('It comes from a shared budget: change it there.'),
      ),
    );
    return false;
  }
  if (!await confirm(context, 'Remove this transaction?') || !context.mounted) {
    return false;
  }
  return guard(
    context,
    () => app.removeTransaction(budget: budget, transaction: transaction.id),
  );
}

class _MonthBody extends StatelessWidget {
  const _MonthBody({
    required this.view,
    required this.onLine,
    required this.onTransaction,
    required this.onAddPot,
  });

  final MonthView view;
  // Callbacks: what the screen does when something is tapped.
  final void Function(LineRow) onLine;
  final void Function(TransactionRow) onTransaction;
  final VoidCallback onAddPot;

  @override
  Widget build(BuildContext context) {
    final currency = view.currency;
    final heading = Theme.of(context).textTheme.titleMedium;
    // A cell of the tables: an amount, right-aligned.
    DataCell cell(int cents, [Status status = Status.none]) =>
        DataCell(Amount(cents, currency, status: status));
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 8, 16, 120),
      children: [
        Text('Lines (${sourceLabel(view.source)})', style: heading),
        DataTable(
          showCheckboxColumn: false,
          columns: const [
            DataColumn(label: Text('Line')),
            DataColumn(label: Text('Due'), numeric: true),
            DataColumn(label: Text('Smoothed'), numeric: true),
            DataColumn(label: Text('Actual'), numeric: true),
          ],
          rows: [
            for (final line in view.lines)
              DataRow(
                // Tapping the row opens what can be done to the line.
                onSelectChanged: (_) => onLine(line),
                cells: [
                  DataCell(
                    Row(
                      children: [
                        Icon(
                          line.income ? Icons.south_west : Icons.north_east,
                          size: 16,
                        ),
                        const SizedBox(width: 6),
                        Text('${line.name} (${line.category})'),
                        if (line.endsOn != null)
                          Text(
                            '  until ${line.endsOn}',
                            style: Theme.of(context).textTheme.bodySmall,
                          ),
                      ],
                    ),
                  ),
                  cell(line.due),
                  cell(line.smoothed),
                  cell(line.actual, line.status),
                ],
              ),
          ],
        ),
        const SizedBox(height: 16),
        Text('Totals', style: heading),
        DataTable(
          columns: const [
            DataColumn(label: Text('')),
            DataColumn(label: Text('Due'), numeric: true),
            DataColumn(label: Text('Smoothed'), numeric: true),
            DataColumn(label: Text('Actual'), numeric: true),
            DataColumn(label: Text('Forecast'), numeric: true),
          ],
          rows: [
            // A list of records `(label, pick)`: each row's name and the
            // function picking its figure from a set of totals.
            for (final (label, pick) in <(String, int Function(Totals))>[
              ('Incomes', (totals) => totals.incomes),
              ('Costs', (totals) => totals.costs),
              ('Balance', (totals) => totals.balance),
            ])
              DataRow(
                cells: [
                  DataCell(Text(label)),
                  cell(pick(view.due)),
                  cell(pick(view.smoothed)),
                  cell(
                    pick(view.actual),
                    label == 'Costs' ? view.status : Status.none,
                  ),
                  cell(pick(view.forecast)),
                ],
              ),
          ],
        ),
        ListTile(
          title: const Text('Running balance at the end of the month'),
          trailing: Amount(view.running, currency),
        ),
        if (view.transactions.isNotEmpty) ...[
          const SizedBox(height: 16),
          Text('Transactions', style: heading),
          for (final transaction in view.transactions)
            ListTile(
              dense: true,
              onTap: () => onTransaction(transaction),
              leading: Text(transaction.date),
              title: Text(
                [
                  transaction.category,
                  ?transaction.line,
                  if (transaction.note.isNotEmpty) transaction.note,
                ].join(' · '),
              ),
              subtitle: transaction.reflected
                  ? const Text('from a shared budget')
                  : null,
              trailing: Amount(
                transaction.income ? transaction.amount : -transaction.amount,
                currency,
              ),
            ),
        ],
        const SizedBox(height: 16),
        Row(
          children: [
            Text('Pots', style: heading),
            const Spacer(),
            TextButton.icon(
              onPressed: onAddPot,
              icon: const Icon(Icons.savings),
              label: const Text('Add a pot'),
            ),
          ],
        ),
        for (final pot in view.pots)
          ListTile(
            dense: true,
            title: Text(pot.name),
            subtitle: pot.goal == null
                ? null
                : Text(
                    'goal ${money(pot.goal!, currency)}'
                    '${pot.goalBy == null ? '' : ' by ${pot.goalBy}'}',
                  ),
            trailing: Amount(pot.balance, currency),
          ),
      ],
    );
  }
}
