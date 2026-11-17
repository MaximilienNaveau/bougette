// The calendar (doc/scope.md): one year of a budget, January to
// December, the current year by default, scrolling without limit. Each
// month shows its expected balance, coloured by where it comes from
// (actual, forecast, plan) and by any overspending.

import 'package:flutter/material.dart';

import 'forms.dart';
import 'look.dart';
import 'members.dart';
import 'month.dart';
import 'src/rust/api.dart';

class CalendarScreen extends StatefulWidget {
  const CalendarScreen({super.key, required this.app, required this.budget});

  final App app;
  final BudgetSummary budget;

  @override
  State<CalendarScreen> createState() => _CalendarScreenState();
}

class _CalendarScreenState extends State<CalendarScreen> {
  int _year = DateTime.now().year;
  late Future<List<MonthCell>> _months;
  // The name and flag as changed here (the summary given is a snapshot).
  late String _name = widget.budget.name;
  late bool _personal = widget.budget.personal;

  @override
  void initState() {
    super.initState();
    _load();
  }

  void _load() {
    setState(() {
      _months = widget.app.calendar(budget: widget.budget.id, year: _year);
    });
  }

  Future<void> _rename() async {
    final name = await askText(
      context,
      context.words.rename,
      context.words.name,
      initial: _name,
    );
    if (name == null || name.isEmpty || !mounted) return;
    final ok = await guard(
      context,
      () => widget.app.renameBudget(budget: widget.budget.id, name: name),
    );
    if (ok) setState(() => _name = name);
  }

  Future<void> _members() async {
    await Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) =>
            MembersScreen(app: widget.app, budget: widget.budget.id),
      ),
    );
    _load();
  }

  Future<void> _togglePersonal() async {
    final ok = await guard(
      context,
      () => widget.app.setPersonal(
        budget: widget.budget.id,
        personal: !_personal,
      ),
    );
    if (ok) setState(() => _personal = !_personal);
  }

  void _moveYear(int by) {
    _year += by;
    _load();
  }

  @override
  Widget build(BuildContext context) {
    final currency = widget.budget.currency;
    return Scaffold(
      appBar: AppBar(
        title: Text(_name),
        actions: [
          IconButton(
            onPressed: () => _moveYear(-1),
            icon: const Icon(Icons.chevron_left),
            tooltip: context.words.previousYear,
          ),
          Center(child: Text('$_year')),
          IconButton(
            onPressed: () => _moveYear(1),
            icon: const Icon(Icons.chevron_right),
            tooltip: context.words.nextYear,
          ),
          // A menu of the budget's settings.
          PopupMenuButton<VoidCallback>(
            onSelected: (action) => action(),
            itemBuilder: (context) => [
              PopupMenuItem(
                value: _members,
                child: Text(context.words.members),
              ),
              PopupMenuItem(value: _rename, child: Text(context.words.rename)),
              CheckedPopupMenuItem(
                value: _togglePersonal,
                checked: _personal,
                child: Text(context.words.personalBudget),
              ),
            ],
          ),
        ],
      ),
      body: FutureBuilder(
        future: _months,
        builder: (context, snapshot) {
          if (snapshot.hasError) {
            return Center(child: Text('${snapshot.error}'));
          }
          final months = snapshot.data;
          if (months == null) {
            return const Center(child: CircularProgressIndicator());
          }
          // Four columns of three months; as many rows as fit.
          return GridView.count(
            crossAxisCount: 4,
            padding: const EdgeInsets.all(12),
            mainAxisSpacing: 8,
            crossAxisSpacing: 8,
            childAspectRatio: 1.6,
            children: [
              for (final cell in months)
                _MonthTile(
                  cell: cell,
                  currency: currency,
                  onTap: () async {
                    await Navigator.of(context).push(
                      MaterialPageRoute(
                        builder: (_) => MonthScreen(
                          app: widget.app,
                          budget: widget.budget.id,
                          month: cell.month,
                        ),
                      ),
                    );
                    _load();
                  },
                ),
            ],
          );
        },
      ),
    );
  }
}

/// One month of the calendar.
class _MonthTile extends StatelessWidget {
  const _MonthTile({
    required this.cell,
    required this.currency,
    required this.onTap,
  });

  final MonthCell cell;
  final String currency;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final overspent = statusColour(cell.status);
    return Card(
      color: sourceColour(context, cell.source),
      // An overspent month gets a border in its status colour.
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: overspent == null
            ? BorderSide.none
            : BorderSide(color: overspent, width: 3),
      ),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(12),
        child: Padding(
          padding: const EdgeInsets.all(10),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                monthTitle(cell.month, withYear: false),
                style: Theme.of(context).textTheme.titleMedium,
              ),
              Text(
                sourceLabel(context, cell.source),
                style: Theme.of(context).textTheme.labelSmall,
              ),
              const Spacer(),
              Amount(cell.expected, currency, status: cell.status),
              Text(
                context.words.runningAmount(money(cell.running, currency)),
                textAlign: TextAlign.end,
                style: Theme.of(context).textTheme.bodySmall,
              ),
            ],
          ),
        ),
      ),
    );
  }
}
