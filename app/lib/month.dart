// One month of a budget: each line planned and actual, the transactions,
// the totals, the running balance and the pots.

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

  @override
  Widget build(BuildContext context) {
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
                      onPressed: () async {
                        if (await addLine(context, widget.app, widget.budget)) {
                          _load();
                        }
                      },
                      child: const Icon(Icons.playlist_add),
                    ),
                    const SizedBox(height: 8),
                    FloatingActionButton.extended(
                      heroTag: 'transaction',
                      onPressed: () async {
                        final added = await addTransaction(
                          context,
                          widget.app,
                          widget.budget,
                          view,
                        );
                        if (added) {
                          _load();
                        }
                      },
                      icon: const Icon(Icons.add),
                      label: const Text('Transaction'),
                    ),
                  ],
                ),
          body: snapshot.hasError
              ? Center(child: Text('${snapshot.error}'))
              : view == null
              ? const Center(child: CircularProgressIndicator())
              : _MonthBody(view: view),
        );
      },
    );
  }
}

class _MonthBody extends StatelessWidget {
  const _MonthBody({required this.view});

  final MonthView view;

  @override
  Widget build(BuildContext context) {
    final currency = view.currency;
    final heading = Theme.of(context).textTheme.titleMedium;
    // A cell of the tables: an amount, right-aligned.
    DataCell cell(int cents, [Status status = Status.none]) =>
        DataCell(Amount(cents, currency, status: status));
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 8, 16, 96),
      children: [
        Text('Lines (${sourceLabel(view.source)})', style: heading),
        DataTable(
          columns: const [
            DataColumn(label: Text('Line')),
            DataColumn(label: Text('Due'), numeric: true),
            DataColumn(label: Text('Smoothed'), numeric: true),
            DataColumn(label: Text('Actual'), numeric: true),
          ],
          rows: [
            for (final line in view.lines)
              DataRow(
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
            // A local function: the row of one total, from the four views.
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
              leading: Text(transaction.date),
              title: Text(
                [
                  transaction.category,
                  ?transaction.line,
                  if (transaction.note.isNotEmpty) transaction.note,
                ].join(' · '),
              ),
              trailing: Amount(
                transaction.income ? transaction.amount : -transaction.amount,
                currency,
              ),
            ),
        ],
        if (view.pots.isNotEmpty) ...[
          const SizedBox(height: 16),
          Text('Pots', style: heading),
          for (final pot in view.pots)
            ListTile(
              dense: true,
              title: Text(pot.name),
              subtitle: pot.goal == null
                  ? null
                  : Text('goal ${money(pot.goal!, currency)}'),
              trailing: Amount(pot.balance, currency),
            ),
        ],
      ],
    );
  }
}
