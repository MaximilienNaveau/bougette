// The notifications inside the app (doc/scope.md: Notifications internes):
// what other members did that matters to this one, latest first. Opening
// the list marks them read. What notifies is decided in Rust.

import 'package:flutter/material.dart';

import 'look.dart';
import 'members.dart';
import 'src/rust/api.dart';
import 'src/rust/api/members.dart';

/// The bell of the home screen: a counter of what wasn't read yet.
Widget bell(BuildContext context, int unread, VoidCallback onPressed) {
  return IconButton(
    onPressed: onPressed,
    tooltip: context.words.notifications,
    // `Badge`: the counter on the icon, hidden at zero.
    icon: Badge(
      label: Text('$unread'),
      isLabelVisible: unread > 0,
      child: const Icon(Icons.notifications_outlined),
    ),
  );
}

class NoticesScreen extends StatefulWidget {
  const NoticesScreen({super.key, required this.app});

  final App app;

  @override
  State<NoticesScreen> createState() => _NoticesScreenState();
}

class _NoticesScreenState extends State<NoticesScreen> {
  // Read, then marked read: the unread ones still show as such here.
  late final Future<List<NoticeRow>> _notices = () async {
    final notices = await widget.app.notifications();
    await widget.app.markNotificationsRead();
    return notices;
  }();

  /// A notification in words.
  String _text(NoticeRow notice) {
    final words = context.words;
    final budget = notice.budget.isEmpty ? words.aBudget : notice.budget;
    return switch (notice.kind) {
      NoticeKind.amountChanged => words.noticeAmount(
        notice.who,
        notice.subject,
        budget,
        money(notice.before, notice.currency),
        money(notice.after, notice.currency),
      ),
      NoticeKind.joined => words.noticeJoined(notice.who, budget),
      NoticeKind.left => words.noticeLeft(notice.who, budget),
      NoticeKind.roleChanged => words.noticeRole(
        budget,
        roleLabel(context, notice.role ?? MemberRole.reader),
      ),
      NoticeKind.removed => words.noticeRemoved(budget),
    };
  }

  @override
  Widget build(BuildContext context) {
    final words = context.words;
    return Scaffold(
      appBar: AppBar(title: Text(words.notifications)),
      body: FutureBuilder(
        future: _notices,
        builder: (context, snapshot) {
          final notices = snapshot.data;
          if (notices == null) {
            return Center(
              child: snapshot.hasError
                  ? Text('${snapshot.error}')
                  : const CircularProgressIndicator(),
            );
          }
          if (notices.isEmpty) {
            return Center(child: Text(words.noNotifications));
          }
          return ListView(
            children: [
              for (final notice in notices)
                ListTile(
                  leading: Icon(switch (notice.kind) {
                    NoticeKind.amountChanged => Icons.edit_note,
                    NoticeKind.joined => Icons.person_add,
                    NoticeKind.left ||
                    NoticeKind.removed => Icons.person_remove,
                    NoticeKind.roleChanged => Icons.badge,
                  }),
                  title: Text(
                    _text(notice),
                    // What wasn't read before opening, in bold.
                    style: notice.read
                        ? null
                        : const TextStyle(fontWeight: FontWeight.bold),
                  ),
                  subtitle: Text(timeText(notice.time)),
                ),
            ],
          );
        },
      ),
    );
  }
}
