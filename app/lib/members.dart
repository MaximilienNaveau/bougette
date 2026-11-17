// The members of a budget, their roles; inviting someone, changing a
// role, removing someone, leaving. What is allowed is decided in Rust: an
// action refused shows why.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'forms.dart';
import 'look.dart';
import 'src/rust/api.dart';
import 'src/rust/api/members.dart';

/// A role, in words.
String roleLabel(BuildContext context, MemberRole role) => switch (role) {
  MemberRole.admin => context.words.roleAdmin,
  MemberRole.editor => context.words.roleEditor,
  MemberRole.reader => context.words.roleReader,
};

class MembersScreen extends StatefulWidget {
  const MembersScreen({super.key, required this.app, required this.budget});

  final App app;
  final String budget;

  @override
  State<MembersScreen> createState() => _MembersScreenState();
}

class _MembersScreenState extends State<MembersScreen> {
  late Future<List<MemberRow>> _members;

  @override
  void initState() {
    super.initState();
    _load();
  }

  void _load() {
    setState(() {
      _members = widget.app.members(budget: widget.budget);
    });
  }

  /// Chooses a role, starting from `current`; null if cancelled.
  Future<MemberRole?> _chooseRole(String title, MemberRole current) {
    return showDialog<MemberRole>(
      context: context,
      builder: (context) => SimpleDialog(
        title: Text(title),
        children: [
          for (final role in MemberRole.values)
            SimpleDialogOption(
              onPressed: () => Navigator.of(context).pop(role),
              child: Text(
                roleLabel(context, role),
                style: role == current
                    ? const TextStyle(fontWeight: FontWeight.bold)
                    : null,
              ),
            ),
        ],
      ),
    );
  }

  Future<void> _invite() async {
    final role = await _chooseRole(context.words.invite, MemberRole.editor);
    if (role == null || !mounted) return;
    final days = TextEditingController(text: '7');
    String? invitation;
    final done = await form(
      context,
      '${context.words.invite} (${roleLabel(context, role)})',
      (setState) => [field(days, context.words.validDays)],
      (context) => guard(context, () async {
        invitation = await widget.app.invite(
          budget: widget.budget,
          role: role,
          days: int.tryParse(days.text) ?? 7,
        );
      }),
    );
    if (!done || invitation == null || !mounted) return;
    await showDialog<void>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(context.words.invitation),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            SelectableText(
              invitation!,
              style: const TextStyle(fontFamily: 'monospace'),
            ),
            const SizedBox(height: 12),
            Text(context.words.giveInvitation),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () =>
                Clipboard.setData(ClipboardData(text: invitation!)),
            child: Text(context.words.copy),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(),
            child: Text(context.words.close),
          ),
        ],
      ),
    );
  }

  Future<void> _member(MemberRow member) async {
    final name = member.name ?? context.words.noName;
    final choice = await showDialog<String>(
      context: context,
      builder: (context) => SimpleDialog(
        title: Text(name),
        children: [
          SimpleDialogOption(
            onPressed: () => Navigator.of(context).pop('role'),
            child: Text(context.words.changeRole),
          ),
          SimpleDialogOption(
            onPressed: () => Navigator.of(context).pop('remove'),
            child: Text(context.words.removeMember),
          ),
        ],
      ),
    );
    if (choice == null || !mounted) return;
    if (choice == 'role') {
      final role = await _chooseRole(context.words.changeRole, member.role);
      if (role == null || !mounted) return;
      await guard(
        context,
        () => widget.app.setRole(
          budget: widget.budget,
          member: member.id,
          role: role,
        ),
      );
    } else {
      if (!await confirm(context, context.words.removeMemberQuestion(name)) ||
          !mounted) {
        return;
      }
      await guard(
        context,
        () => widget.app.removeMember(budget: widget.budget, member: member.id),
      );
    }
    _load();
  }

  Future<void> _leave() async {
    if (!await confirm(context, context.words.leaveQuestion) || !mounted) {
      return;
    }
    final ok = await guard(
      context,
      () => widget.app.leave(budget: widget.budget),
    );
    if (ok && mounted) Navigator.of(context).pop();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(context.words.members),
        actions: [
          IconButton(
            onPressed: _leave,
            icon: const Icon(Icons.logout),
            tooltip: context.words.leave,
          ),
        ],
      ),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: _invite,
        icon: const Icon(Icons.person_add),
        label: Text(context.words.invite),
      ),
      body: FutureBuilder(
        future: _members,
        builder: (context, snapshot) {
          if (snapshot.hasError) {
            return Center(child: Text('${snapshot.error}'));
          }
          final members = snapshot.data;
          if (members == null) {
            return const Center(child: CircularProgressIndicator());
          }
          return ListView(
            children: [
              for (final member in members)
                ListTile(
                  leading: Icon(
                    member.me ? Icons.person : Icons.person_outline,
                  ),
                  title: Text(
                    member.me
                        ? '${member.name ?? context.words.noName} (${context.words.you})'
                        : member.name ?? context.words.noName,
                  ),
                  subtitle: Text(roleLabel(context, member.role)),
                  onTap: member.me ? null : () => _member(member),
                ),
            ],
          );
        },
      ),
    );
  }
}
