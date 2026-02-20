import SwiftUI

struct AssistantThreadsView: View {
    @ObservedObject var model: AppModel
    var reservesTrailingOverlaySpace: Bool = false
    @State private var showDeleteAllConfirmation = false

    var body: some View {
        VStack(spacing: 0) {
            header

            if let message = model.assistantThreadSyncState.lastSyncErrorMessage {
                syncStatusLabel(
                    message: message,
                    showsProgress: false,
                    actionTitle: "Retry",
                    action: model.retryAssistantThreadSync
                )
            } else if model.assistantThreadSyncState.syncInFlight {
                syncStatusLabel(
                    message: "Syncing thread deletions...",
                    showsProgress: true,
                    actionTitle: nil,
                    action: nil
                )
            }

            if model.assistantThreads.isEmpty {
                emptyState
            } else {
                threadList
            }
        }
        .appScreenBackground()
        .confirmationDialog("Delete all threads?", isPresented: $showDeleteAllConfirmation) {
            Button("Delete All", role: .destructive) {
                model.deleteAllAssistantThreads()
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This removes all local threads and queues server-side deletion.")
        }
    }

    private var header: some View {
        HStack(spacing: 14) {
            Text("Threads")
                .font(.title2.weight(.semibold))
                .foregroundStyle(AppTheme.Colors.textPrimary)

            Spacer(minLength: 0)

            Button {
                model.clearAssistantConversation()
                model.selectedTab = .home
            } label: {
                Image(systemName: "square.and.pencil")
                    .font(.system(size: 18, weight: .semibold))
                    .foregroundStyle(AppTheme.Colors.textPrimary)
                    .frame(width: 44, height: 44)
                    .background(AppTheme.Colors.surface.opacity(0.7), in: Circle())
            }
            .buttonStyle(.plain)

            Button(role: .destructive) {
                showDeleteAllConfirmation = true
            } label: {
                Image(systemName: "trash")
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(AppTheme.Colors.textSecondary)
                    .frame(width: 34, height: 34)
                    .background(AppTheme.Colors.surface.opacity(0.7), in: Circle())
            }
            .buttonStyle(.plain)
            .disabled(model.assistantThreads.isEmpty)
            .opacity(model.assistantThreads.isEmpty ? 0.45 : 1)
        }
        .padding(.leading, 16)
        .padding(.trailing, reservesTrailingOverlaySpace ? 76 : 16)
        .padding(.top, 8)
        .padding(.bottom, 10)
    }

    private var threadList: some View {
        List {
            ForEach(model.assistantThreads) { thread in
                Button {
                    model.selectAssistantThread(thread.id)
                    model.selectedTab = .home
                } label: {
                    threadRow(for: thread)
                }
                .buttonStyle(.plain)
                .listRowInsets(EdgeInsets(top: 10, leading: 16, bottom: 10, trailing: 16))
                .listRowBackground(
                    model.activeAssistantThreadID == thread.id
                        ? AppTheme.Colors.surfaceElevated.opacity(0.5)
                        : Color.clear
                )
                .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                    Button(role: .destructive) {
                        model.deleteAssistantThread(thread.id)
                    } label: {
                        Label("Delete", systemImage: "trash")
                    }
                }
                .contextMenu {
                    Button("Delete Thread", role: .destructive) {
                        model.deleteAssistantThread(thread.id)
                    }
                }
            }
        }
        .listStyle(.plain)
        .scrollContentBackground(.hidden)
    }

    private var emptyState: some View {
        VStack(spacing: 10) {
            Spacer(minLength: 0)
            Text("No threads yet")
                .font(.headline.weight(.semibold))
                .foregroundStyle(AppTheme.Colors.textPrimary)
            Text("Start chatting in Home and your conversation threads will appear here.")
                .font(.footnote)
                .foregroundStyle(AppTheme.Colors.textSecondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 28)
            Spacer(minLength: 0)
        }
    }

    private func threadRow(for thread: AssistantConversationThread) -> some View {
        HStack(alignment: .top, spacing: 10) {
            Circle()
                .fill(model.activeAssistantThreadID == thread.id ? AppTheme.Colors.textPrimary : AppTheme.Colors.outline.opacity(0.45))
                .frame(width: model.activeAssistantThreadID == thread.id ? 8 : 6, height: model.activeAssistantThreadID == thread.id ? 8 : 6)
                .padding(.top, 8)

            VStack(alignment: .leading, spacing: 4) {
                Text(thread.title)
                    .font(.system(size: 20, weight: model.activeAssistantThreadID == thread.id ? .semibold : .regular))
                    .foregroundStyle(AppTheme.Colors.textPrimary)
                    .lineLimit(1)

                Text(thread.lastMessagePreview.isEmpty ? "No messages yet" : thread.lastMessagePreview)
                    .font(.subheadline.weight(.regular))
                    .foregroundStyle(AppTheme.Colors.textSecondary)
                    .lineLimit(2)

                Text(timeLabel(for: thread.updatedAt))
                    .font(.caption.weight(.regular))
                    .foregroundStyle(AppTheme.Colors.textSecondary.opacity(0.85))
            }

            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
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
        .padding(.bottom, 8)
    }

    private func timeLabel(for date: Date) -> String {
        let calendar = Calendar.current
        if calendar.isDateInToday(date) {
            return Self.timeFormatter.string(from: date)
        }
        if calendar.isDateInYesterday(date) {
            return "Yesterday"
        }
        if let days = calendar.dateComponents([.day], from: date, to: Date()).day, days < 7 {
            return Self.weekdayFormatter.string(from: date)
        }
        return Self.shortDateFormatter.string(from: date)
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
}

#Preview {
    AssistantThreadsView(model: AppModel())
}
