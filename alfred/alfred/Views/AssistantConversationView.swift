import AlfredAPIClient
import SwiftUI

struct AssistantConversationView: View {
    let messages: [AssistantConversationMessage]
    let draftMessage: String
    let isLoading: Bool
    let showsHeader: Bool
    let emptyStateText: String

    init(
        messages: [AssistantConversationMessage],
        draftMessage: String,
        isLoading: Bool,
        showsHeader: Bool = true,
        emptyStateText: String = "Tap Ask Alfred to start a conversation."
    ) {
        self.messages = messages
        self.draftMessage = draftMessage
        self.isLoading = isLoading
        self.showsHeader = showsHeader
        self.emptyStateText = emptyStateText
    }

    private var normalizedDraftMessage: String? {
        let trimmed = draftMessage.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    private var scrollTargetID: AnyHashable? {
        if isLoading {
            return "loading-row"
        }
        if normalizedDraftMessage != nil {
            return "draft-row"
        }
        return messages.last?.id
    }

    var body: some View {
        VStack(spacing: 8) {
            if showsHeader {
                HStack {
                    Text("Chat")
                        .font(.caption.weight(.bold))
                        .foregroundStyle(AppTheme.Colors.textSecondary)
                    Spacer(minLength: 0)
                }
            }

            Group {
                if messages.isEmpty && normalizedDraftMessage == nil && !isLoading {
                    Text(emptyStateText)
                        .font(.footnote.weight(.semibold))
                        .foregroundStyle(AppTheme.Colors.textSecondary)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
                } else {
                    ScrollViewReader { proxy in
                        ScrollView(.vertical, showsIndicators: true) {
                            LazyVStack(spacing: 10) {
                                ForEach(messages) { message in
                                    AssistantConversationMessageRow(message: message)
                                        .id(message.id)
                                }

                                if let draftMessage = normalizedDraftMessage {
                                    AssistantDraftMessageRow(text: draftMessage)
                                        .id("draft-row")
                                }

                                if isLoading {
                                    AssistantLoadingRow()
                                        .id("loading-row")
                                }
                            }
                            .padding(.vertical, 8)
                        }
                        .onAppear {
                            scrollToBottom(with: proxy, animated: false)
                        }
                        .onChange(of: messages.count) { _, _ in
                            scrollToBottom(with: proxy, animated: true)
                        }
                        .onChange(of: normalizedDraftMessage) { _, _ in
                            scrollToBottom(with: proxy, animated: true)
                        }
                        .onChange(of: isLoading) { _, _ in
                            scrollToBottom(with: proxy, animated: true)
                        }
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
            .background(AppTheme.Colors.surfaceElevated.opacity(0.65))
            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .stroke(AppTheme.Colors.outline.opacity(0.6), lineWidth: 1)
            )
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(.horizontal, 4)
    }

    private func scrollToBottom(with proxy: ScrollViewProxy, animated: Bool) {
        guard let scrollTargetID else { return }
        if animated {
            withAnimation(.easeOut(duration: 0.2)) {
                proxy.scrollTo(scrollTargetID, anchor: .bottom)
            }
        } else {
            proxy.scrollTo(scrollTargetID, anchor: .bottom)
        }
    }
}

private struct AssistantConversationMessageRow: View {
    let message: AssistantConversationMessage

    private var roleTitle: String {
        switch message.role {
        case .user:
            return "Me"
        case .assistant:
            return "Alfred"
        }
    }

    var body: some View {
        VStack(
            alignment: message.role == .user ? .trailing : .leading,
            spacing: 6
        ) {
            Text("\(roleTitle):")
                .font(.caption2.weight(.bold))
                .foregroundStyle(AppTheme.Colors.textSecondary)

            Text(message.text)
                .font(.footnote.weight(.semibold))
                .foregroundStyle(AppTheme.Colors.textPrimary)
                .multilineTextAlignment(message.role == .user ? .trailing : .leading)
                .frame(maxWidth: .infinity, alignment: message.role == .user ? .trailing : .leading)
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
                .background(
                    message.role == .user
                        ? AppTheme.Colors.surface
                        : AppTheme.Colors.surfaceElevated.opacity(0.9)
                )
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay(
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .stroke(AppTheme.Colors.outline.opacity(0.45), lineWidth: 1)
                )

            if message.role == .assistant {
                ForEach(message.toolSummaries) { summary in
                    AssistantToolSummaryCard(summary: summary)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: message.role == .user ? .trailing : .leading)
    }
}

private struct AssistantDraftMessageRow: View {
    let text: String

    var body: some View {
        VStack(alignment: .trailing, spacing: 6) {
            Text("Me:")
                .font(.caption2.weight(.bold))
                .foregroundStyle(AppTheme.Colors.textSecondary)

            Text(text)
                .font(.footnote.weight(.semibold))
                .foregroundStyle(AppTheme.Colors.textPrimary)
                .multilineTextAlignment(.trailing)
                .frame(maxWidth: .infinity, alignment: .trailing)
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
                .background(AppTheme.Colors.surface.opacity(0.55))
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay(
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .stroke(AppTheme.Colors.outline.opacity(0.3), lineWidth: 1)
                )
        }
        .frame(maxWidth: .infinity, alignment: .trailing)
    }
}

private struct AssistantLoadingRow: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Alfred:")
                .font(.caption2.weight(.bold))
                .foregroundStyle(AppTheme.Colors.textSecondary)

            HStack(spacing: 8) {
                ProgressView()
                    .progressViewStyle(.circular)
                    .tint(AppTheme.Colors.textPrimary)

                Text("Thinking...")
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(AppTheme.Colors.textSecondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(AppTheme.Colors.surfaceElevated.opacity(0.9))
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .stroke(AppTheme.Colors.outline.opacity(0.45), lineWidth: 1)
            )
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct AssistantToolSummaryCard: View {
    let summary: AssistantToolSummary

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Text("Tool")
                    .font(.caption2.weight(.bold))
                    .foregroundStyle(AppTheme.Colors.textSecondary)

                Text(capabilityLabel(for: summary.capability))
                    .font(.caption2.weight(.bold))
                    .foregroundStyle(AppTheme.Colors.textPrimary)
            }

            Text(summary.title)
                .font(.caption.weight(.bold))
                .foregroundStyle(AppTheme.Colors.textPrimary)

            Text(summary.summary)
                .font(.caption)
                .foregroundStyle(AppTheme.Colors.textSecondary)

            if !summary.keyPoints.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(Array(summary.keyPoints.prefix(3).enumerated()), id: \.offset) { _, keyPoint in
                        HStack(alignment: .top, spacing: 6) {
                            Circle()
                                .fill(AppTheme.Colors.textSecondary)
                                .frame(width: 5, height: 5)
                                .padding(.top, 4)

                            Text(keyPoint)
                                .font(.caption2)
                                .foregroundStyle(AppTheme.Colors.textSecondary)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    }
                }
            }

            if !summary.followUps.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Next")
                        .font(.caption2.weight(.bold))
                        .foregroundStyle(AppTheme.Colors.textSecondary)

                    ForEach(Array(summary.followUps.prefix(2).enumerated()), id: \.offset) { _, followUp in
                        HStack(alignment: .top, spacing: 6) {
                            Text("->")
                                .font(.caption2.weight(.bold))
                                .foregroundStyle(AppTheme.Colors.textSecondary)

                            Text(followUp)
                                .font(.caption2)
                                .foregroundStyle(AppTheme.Colors.textSecondary)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(10)
        .background(AppTheme.Colors.surface)
        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .stroke(AppTheme.Colors.outline.opacity(0.4), lineWidth: 1)
        )
    }

    private func capabilityLabel(for capability: AssistantQueryCapability) -> String {
        switch capability {
        case .meetingsToday:
            return "Meetings Today"
        case .calendarLookup:
            return "Calendar"
        case .emailLookup:
            return "Email"
        case .generalChat:
            return "Chat"
        case .mixed:
            return "Mixed"
        }
    }
}
