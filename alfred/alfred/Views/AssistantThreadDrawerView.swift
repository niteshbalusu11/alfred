import SwiftUI

struct AssistantThreadDrawerView: View {
    @ObservedObject var model: AppModel

    let onSelectThread: (UUID) -> Void
    let onDeleteThread: (UUID) -> Void
    let onDeleteAll: () -> Void
    let onRetrySync: () -> Void
    let onClose: () -> Void

    @State private var showDeleteAllConfirmation = false

    var body: some View {
        VStack(spacing: 0) {
            header

            if let message = model.assistantThreadSyncState.lastSyncErrorMessage {
                syncStatusLabel(
                    message: message,
                    showsProgress: false,
                    actionTitle: "Retry",
                    action: onRetrySync
                )
            } else if model.assistantThreadSyncState.syncInFlight {
                syncStatusLabel(
                    message: "Syncing thread deletions...",
                    showsProgress: true,
                    actionTitle: nil,
                    action: nil
                )
            }

            Divider()
                .overlay(AppTheme.Colors.outline.opacity(0.2))

            threadListContent

            if !model.assistantThreads.isEmpty {
                footerActions
            }
        }
        .frame(maxHeight: .infinity, alignment: .top)
        .background(
            LinearGradient(
                colors: [
                    AppTheme.Colors.background.opacity(0.985),
                    AppTheme.Colors.surface.opacity(0.98),
                ],
                startPoint: .top,
                endPoint: .bottom
            )
        )
        .overlay(
            Rectangle()
                .fill(AppTheme.Colors.outline.opacity(0.18))
                .frame(width: 1),
            alignment: .trailing
        )
        .shadow(color: AppTheme.Colors.shadow.opacity(0.45), radius: 14, x: 0, y: 0)
        .confirmationDialog("Delete all threads?", isPresented: $showDeleteAllConfirmation) {
            Button("Delete All", role: .destructive) {
                onDeleteAll()
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This removes all local threads and syncs deletion to the server.")
        }
    }

    private var header: some View {
        HStack(spacing: 8) {
            Text("Threads")
                .font(.title2.weight(.semibold))
                .foregroundStyle(AppTheme.Colors.textPrimary)

            Spacer(minLength: 0)

            Button(action: onClose) {
                Image(systemName: "chevron.right")
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(AppTheme.Colors.textPrimary)
                    .frame(width: 34, height: 34)
                    .background(AppTheme.Colors.surfaceElevated.opacity(0.8), in: Circle())
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 16)
        .padding(.top, 18)
        .padding(.bottom, 12)
    }

    @ViewBuilder
    private var threadListContent: some View {
        if model.assistantThreads.isEmpty {
            VStack(spacing: 10) {
                Text("No threads yet")
                    .font(.headline.weight(.semibold))
                    .foregroundStyle(AppTheme.Colors.textPrimary)
                Text("Start chatting and your recent threads will appear here.")
                    .font(.footnote.weight(.regular))
                    .multilineTextAlignment(.center)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            }
            .frame(maxWidth: .infinity, minHeight: 180)
            .padding(.horizontal, 16)
            .frame(maxHeight: .infinity, alignment: .center)
        } else {
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(Array(model.assistantThreads.enumerated()), id: \.element.id) { index, thread in
                        AssistantThreadDrawerRow(
                            thread: thread,
                            isActive: model.activeAssistantThreadID == thread.id,
                            onSelect: {
                                onSelectThread(thread.id)
                            },
                            onDelete: {
                                onDeleteThread(thread.id)
                            }
                        )

                        if index < model.assistantThreads.count - 1 {
                            Divider()
                                .overlay(AppTheme.Colors.outline.opacity(0.12))
                                .padding(.leading, 16)
                        }
                    }
                }
            }
            .frame(maxHeight: .infinity)
        }
    }

    private var footerActions: some View {
        HStack {
            Button(role: .destructive) {
                showDeleteAllConfirmation = true
            } label: {
                Label("Delete All", systemImage: "trash")
                    .font(.subheadline.weight(.medium))
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            }
            .buttonStyle(.plain)

            Spacer(minLength: 0)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .overlay(alignment: .top) {
            Divider()
                .overlay(AppTheme.Colors.outline.opacity(0.12))
        }
    }

    @ViewBuilder
    private func syncStatusLabel(
        message: String,
        showsProgress: Bool,
        actionTitle: String?,
        action: (() -> Void)?
    ) -> some View {
        HStack(spacing: 8) {
            if showsProgress {
                ProgressView()
                    .progressViewStyle(.circular)
                    .tint(AppTheme.Colors.textSecondary)
            }

            Text(message)
                .font(.caption.weight(.medium))
                .foregroundStyle(AppTheme.Colors.textSecondary)
                .frame(maxWidth: .infinity, alignment: .leading)

            if let actionTitle, let action {
                Button(actionTitle, action: action)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(AppTheme.Colors.textPrimary)
                    .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
    }
}

private struct AssistantThreadDrawerRow: View {
    let thread: AssistantConversationThread
    let isActive: Bool
    let onSelect: () -> Void
    let onDelete: () -> Void

    var body: some View {
        Button(action: onSelect) {
            HStack(alignment: .top, spacing: 10) {
                Circle()
                    .fill(isActive ? AppTheme.Colors.textPrimary : AppTheme.Colors.outline.opacity(0.45))
                    .frame(width: isActive ? 8 : 6, height: isActive ? 8 : 6)
                    .padding(.top, 8)

                VStack(alignment: .leading, spacing: 3) {
                    Text(thread.title)
                        .font(.system(size: 19, weight: isActive ? .semibold : .regular))
                        .foregroundStyle(AppTheme.Colors.textPrimary)
                        .lineLimit(1)

                    Text(thread.lastMessagePreview.isEmpty ? "No messages yet" : thread.lastMessagePreview)
                        .font(.subheadline.weight(.regular))
                        .foregroundStyle(AppTheme.Colors.textSecondary)
                        .lineLimit(2)

                    Text(Self.timeLabel(for: thread.updatedAt))
                        .font(.caption.weight(.regular))
                        .foregroundStyle(AppTheme.Colors.textSecondary.opacity(0.85))
                }

                Spacer(minLength: 0)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(isActive ? AppTheme.Colors.surfaceElevated.opacity(0.55) : Color.clear)
        }
        .buttonStyle(.plain)
        .contextMenu {
            Button(role: .destructive, action: onDelete) {
                Label("Delete Thread", systemImage: "trash")
            }
        }
    }

    private static let weekdayFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateFormat = "EEEE"
        return formatter
    }()

    private static let timeFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateFormat = "h:mm a"
        return formatter
    }()

    private static let shortDateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .short
        formatter.timeStyle = .none
        return formatter
    }()

    private static func timeLabel(for date: Date) -> String {
        let calendar = Calendar.current
        if calendar.isDateInToday(date) {
            return timeFormatter.string(from: date)
        }
        if calendar.isDateInYesterday(date) {
            return "Yesterday"
        }
        if let days = calendar.dateComponents([.day], from: date, to: Date()).day, days < 7 {
            return weekdayFormatter.string(from: date)
        }
        return shortDateFormatter.string(from: date)
    }
}
