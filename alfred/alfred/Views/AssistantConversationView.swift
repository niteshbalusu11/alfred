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
                        .font(.headline.weight(.semibold))
                        .foregroundStyle(AppTheme.Colors.textSecondary)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
                        .padding(.horizontal, 28)
                } else {
                    ScrollViewReader { proxy in
                        ScrollView(.vertical, showsIndicators: false) {
                            LazyVStack(spacing: 18) {
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
                            .padding(.vertical, 10)
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
            .padding(.horizontal, 2)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
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

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if message.role == .user {
                Text(message.text)
                    .font(.system(size: 33.0 / 2.0, weight: .medium))
                    .foregroundStyle(AppTheme.Colors.textPrimary)
                    .multilineTextAlignment(.leading)
                    .frame(maxWidth: .infinity, alignment: .trailing)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 12)
                    .background(AppTheme.Colors.surfaceElevated.opacity(0.92))
                    .clipShape(Capsule(style: .continuous))
            } else {
                Text(message.text)
                    .font(.system(size: 34.0 / 2.0, weight: .regular))
                    .foregroundStyle(AppTheme.Colors.textPrimary)
                    .multilineTextAlignment(.leading)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .fixedSize(horizontal: false, vertical: true)
            }

            if message.role == .assistant, !message.toolSummaries.isEmpty {
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
        Text(text)
            .font(.system(size: 33.0 / 2.0, weight: .medium))
            .foregroundStyle(AppTheme.Colors.textPrimary.opacity(0.9))
            .multilineTextAlignment(.leading)
            .frame(maxWidth: .infinity, alignment: .trailing)
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .background(AppTheme.Colors.surfaceElevated.opacity(0.65))
            .clipShape(Capsule(style: .continuous))
        .frame(maxWidth: .infinity, alignment: .trailing)
    }
}

private struct AssistantLoadingRow: View {
    var body: some View {
        HStack(spacing: 8) {
            ProgressView()
                .progressViewStyle(.circular)
                .tint(AppTheme.Colors.textPrimary)

            Text("Thinking...")
                .font(.footnote.weight(.semibold))
                .foregroundStyle(AppTheme.Colors.textSecondary)
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
