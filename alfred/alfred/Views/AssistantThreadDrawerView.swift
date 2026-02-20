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
        VStack(spacing: 12) {
            header

            if let message = model.assistantThreadSyncState.lastSyncErrorMessage {
                syncStateBanner(
                    message: message,
                    showsProgress: false,
                    actionTitle: "Retry",
                    action: onRetrySync
                )
            } else if model.assistantThreadSyncState.syncInFlight {
                syncStateBanner(
                    message: "Syncing thread deletions...",
                    showsProgress: true,
                    actionTitle: nil,
                    action: nil
                )
            }

            threadListContent

            deleteAllButton
        }
        .padding(14)
        .frame(maxHeight: .infinity, alignment: .top)
        .background(AppTheme.Colors.surfaceElevated)
        .overlay(
            RoundedRectangle(cornerRadius: 24, style: .continuous)
                .stroke(AppTheme.Colors.paper.opacity(0.2), lineWidth: 2)
        )
        .clipShape(RoundedRectangle(cornerRadius: 24, style: .continuous))
        .shadow(color: AppTheme.Colors.shadow.opacity(0.9), radius: 0, x: 0, y: 6)
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
                .font(.title3.weight(.black))
                .foregroundStyle(AppTheme.Colors.textPrimary)

            Spacer(minLength: 0)

            Button(action: onClose) {
                Image(systemName: "xmark")
                    .font(.system(size: 13, weight: .black))
                    .foregroundStyle(AppTheme.Colors.textPrimary)
                    .frame(width: 32, height: 32)
                    .background(AppTheme.Colors.surface.opacity(0.8), in: Circle())
            }
            .buttonStyle(.plain)
        }
    }

    @ViewBuilder
    private var threadListContent: some View {
        if model.assistantThreads.isEmpty {
            VStack(spacing: 8) {
                Text("No threads yet")
                    .font(.headline.weight(.bold))
                    .foregroundStyle(AppTheme.Colors.textPrimary)
                Text("Start chatting and your recent threads will appear here.")
                    .font(.footnote.weight(.semibold))
                    .multilineTextAlignment(.center)
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            }
            .frame(maxWidth: .infinity, minHeight: 180)
            .padding(.horizontal, 10)
            .background(AppTheme.Colors.surface.opacity(0.35), in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        } else {
            ScrollView {
                LazyVStack(spacing: 8) {
                    ForEach(model.assistantThreads) { thread in
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
                    }
                }
            }
            .frame(maxHeight: .infinity)
        }
    }

    private var deleteAllButton: some View {
        Button(role: .destructive) {
            showDeleteAllConfirmation = true
        } label: {
            Text("Delete All Threads")
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.appSecondary)
        .disabled(model.assistantThreads.isEmpty)
        .opacity(model.assistantThreads.isEmpty ? 0.4 : 1)
    }

    @ViewBuilder
    private func syncStateBanner(
        message: String,
        showsProgress: Bool,
        actionTitle: String?,
        action: (() -> Void)?
    ) -> some View {
        HStack(spacing: 8) {
            if showsProgress {
                ProgressView()
                    .progressViewStyle(.circular)
                    .tint(AppTheme.Colors.paper)
            }

            Text(message)
                .font(.caption.weight(.semibold))
                .foregroundStyle(AppTheme.Colors.textPrimary)
                .frame(maxWidth: .infinity, alignment: .leading)

            if let actionTitle, let action {
                Button(actionTitle, action: action)
                    .font(.caption.weight(.black))
                    .foregroundStyle(AppTheme.Colors.ink)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 6)
                    .background(AppTheme.Colors.paper, in: Capsule(style: .continuous))
                    .buttonStyle(.plain)
            }
        }
        .padding(10)
        .background(AppTheme.Colors.surface.opacity(0.7), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
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
                    .fill(isActive ? AppTheme.Colors.paper : AppTheme.Colors.smoke.opacity(0.25))
                    .frame(width: 8, height: 8)
                    .padding(.top, 6)

                VStack(alignment: .leading, spacing: 3) {
                    Text(thread.title)
                        .font(.subheadline.weight(.bold))
                        .foregroundStyle(AppTheme.Colors.textPrimary)
                        .lineLimit(1)

                    Text(thread.lastMessagePreview.isEmpty ? "No messages yet" : thread.lastMessagePreview)
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(AppTheme.Colors.textSecondary)
                        .lineLimit(2)

                    Text("Updated \(Self.relativeFormatter.localizedString(for: thread.updatedAt, relativeTo: Date()))")
                        .font(.caption2.weight(.semibold))
                        .foregroundStyle(AppTheme.Colors.textSecondary.opacity(0.85))
                }

                Spacer(minLength: 0)
            }
            .padding(10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .fill(isActive ? AppTheme.Colors.surface.opacity(0.95) : AppTheme.Colors.surface.opacity(0.6))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .stroke(
                        isActive ? AppTheme.Colors.paper.opacity(0.35) : AppTheme.Colors.paper.opacity(0.08),
                        lineWidth: isActive ? 2 : 1
                    )
            )
        }
        .buttonStyle(.plain)
        .contextMenu {
            Button(role: .destructive, action: onDelete) {
                Label("Delete Thread", systemImage: "trash")
            }
        }
    }

    private static let relativeFormatter: RelativeDateTimeFormatter = {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter
    }()
}
