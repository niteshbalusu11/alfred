import AlfredAPIClient
import Foundation

struct AssistantConversationMessage: Identifiable, Equatable {
    enum Role: String, Equatable {
        case user
        case assistant
    }

    let id: UUID
    let role: Role
    let text: String
    let capability: AssistantQueryCapability?
    let toolSummaries: [AssistantToolSummary]
    let createdAt: Date
}

struct AssistantToolSummary: Identifiable, Equatable {
    let id: UUID
    let capability: AssistantQueryCapability
    let title: String
    let summary: String
    let keyPoints: [String]
    let followUps: [String]
}

enum AssistantConversationMapper {
    static func userMessage(from query: String, createdAt: Date = Date()) -> AssistantConversationMessage {
        AssistantConversationMessage(
            id: UUID(),
            role: .user,
            text: query,
            capability: nil,
            toolSummaries: [],
            createdAt: createdAt
        )
    }

    static func assistantMessage(
        from response: AssistantPlaintextQueryResponse,
        createdAt: Date = Date()
    ) -> AssistantConversationMessage {
        let normalizedText = response.responseParts
            .first(where: { $0.type == .chatText })
            .flatMap(\.text)?
            .trimmingCharacters(in: .whitespacesAndNewlines)

        let displayText: String
        if let normalizedText, !normalizedText.isEmpty {
            displayText = normalizedText
        } else if !response.displayText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            displayText = response.displayText
        } else {
            displayText = response.payload.summary
        }
        return AssistantConversationMessage(
            id: UUID(),
            role: .assistant,
            text: displayText,
            capability: response.capability,
            toolSummaries: toolSummaries(from: response),
            createdAt: createdAt
        )
    }

    private static func toolSummaries(from response: AssistantPlaintextQueryResponse) -> [AssistantToolSummary] {
        var toolSummaries = response.responseParts.compactMap { part -> AssistantToolSummary? in
            guard part.type == .toolSummary, let payload = part.payload else {
                return nil
            }

            return AssistantToolSummary(
                id: UUID(),
                capability: part.capability ?? response.capability,
                title: payload.title,
                summary: payload.summary,
                keyPoints: payload.keyPoints,
                followUps: payload.followUps
            )
        }

        if toolSummaries.isEmpty && response.capability != .generalChat {
            toolSummaries = [
                AssistantToolSummary(
                    id: UUID(),
                    capability: response.capability,
                    title: response.payload.title,
                    summary: response.payload.summary,
                    keyPoints: response.payload.keyPoints,
                    followUps: response.payload.followUps
                ),
            ]
        }

        return toolSummaries
    }
}
