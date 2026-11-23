// A budget's year by category (doc/scope.md: Bilan de l'année): for each
// category, what was planned for the year, what actually happened so far,
// and the forecast for the end of the year, then the totals. Costs show
// negative, so each column adds up to the balance. The figures are the
// Rust core's.

import 'package:flutter/material.dart';

import 'look.dart';
import 'src/rust/api.dart';

class YearScreen extends StatefulWidget {
  const YearScreen({
    super.key,
    required this.app,
    required this.budget,
    required this.currency,
    required this.year,
  });

  final App app;
  final String budget;
  final String currency;
  final int year;

  @override
  State<YearScreen> createState() => _YearScreenState();
}

class _YearScreenState extends State<YearScreen> {
  late final Future<YearView> _view = widget.app.year(
    budget: widget.budget,
    year: widget.year,
  );

  @override
  Widget build(BuildContext context) {
    final words = context.words;
    return Scaffold(
      appBar: AppBar(title: Text(words.yearByCategory(widget.year))),
      body: FutureBuilder(
        future: _view,
        builder: (context, snapshot) {
          final view = snapshot.data;
          if (view == null) {
            return Center(
              child: snapshot.hasError
                  ? Text('${snapshot.error}')
                  : const CircularProgressIndicator(),
            );
          }
          final bold = const TextStyle(fontWeight: FontWeight.bold);
          // An amount cell; `style` for the totals.
          DataCell cell(int cents, [TextStyle? style]) =>
              DataCell(Amount(cents, widget.currency, style: style));
          return ListView(
            padding: const EdgeInsets.all(16),
            children: [
              // Wide tables scroll sideways on a phone.
              SingleChildScrollView(
                scrollDirection: Axis.horizontal,
                child: DataTable(
                  columns: [
                    DataColumn(label: Text(words.category)),
                    DataColumn(label: Text(words.planned), numeric: true),
                    DataColumn(label: Text(words.actual), numeric: true),
                    DataColumn(label: Text(words.forecast), numeric: true),
                  ],
                  rows: [
                    for (final row in view.categories)
                      DataRow(
                        cells: [
                          DataCell(
                            Row(
                              children: [
                                Icon(
                                  row.income
                                      ? Icons.south_west
                                      : Icons.north_east,
                                  size: 16,
                                ),
                                const SizedBox(width: 6),
                                Text(
                                  row.category.isEmpty
                                      ? words.noCategory
                                      : row.category,
                                ),
                              ],
                            ),
                          ),
                          // A cost shown negative.
                          for (final cents in [
                            row.planned,
                            row.actual,
                            row.forecast,
                          ])
                            cell(row.income ? cents : -cents),
                        ],
                      ),
                    DataRow(
                      cells: [
                        DataCell(Text(words.balance, style: bold)),
                        cell(view.planned.balance, bold),
                        cell(view.actual.balance, bold),
                        cell(view.forecast.balance, bold),
                      ],
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 16),
              Text(
                words.yearForecastHint,
                style: Theme.of(context).textTheme.bodySmall,
              ),
            ],
          );
        },
      ),
    );
  }
}
