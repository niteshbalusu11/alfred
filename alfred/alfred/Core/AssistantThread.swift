import Foundation

nonisolated struct AssistantConversationThread: Identifiable, Equatable, Codable, Sendable {
    let id: UUID
    var sessionID: UUID?
    var title: String
    var createdAt: Date
    var updatedAt: Date
    var lastMessagePreview: String
    var messages: [AssistantConversationMessage]

    enum CodingKeys: String, CodingKey {
        case id = "thread_id"
        case sessionID = "session_id"
        case title
        case createdAt = "created_at"
        case updatedAt = "updated_at"
        case lastMessagePreview = "last_message_preview"
        case messages
    }

    init(
        id: UUID = UUID(),
        sessionID: UUID? = nil,
        title: String? = nil,
        createdAt: Date = Date(),
        updatedAt: Date = Date(),
        lastMessagePreview: String? = nil,
        messages: [AssistantConversationMessage] = []
    ) {
        self.id = id
        self.sessionID = sessionID
        self.createdAt = createdAt
        self.updatedAt = updatedAt
        self.messages = messages
        self.title = title ?? Self.derivedTitle(from: messages)
        self.lastMessagePreview = lastMessagePreview ?? Self.derivedLastMessagePreview(from: messages)
    }

    mutating func append(_ newMessages: [AssistantConversationMessage], sessionID: UUID?) {
        messages.append(contentsOf: newMessages)
        self.sessionID = sessionID
        if let lastMessageDate = messages.last?.createdAt {
            updatedAt = lastMessageDate
        } else {
            updatedAt = Date()
        }
        title = Self.derivedTitle(from: messages)
        lastMessagePreview = Self.derivedLastMessagePreview(from: messages)
    }

    mutating func refreshMetadata() {
        title = Self.derivedTitle(from: messages)
        lastMessagePreview = Self.derivedLastMessagePreview(from: messages)
        if let lastMessageDate = messages.last?.createdAt {
            updatedAt = lastMessageDate
        }
    }

    private static func derivedTitle(from messages: [AssistantConversationMessage]) -> String {
        let preferredSource = messages.first(where: { $0.role == .user })?.text
            ?? messages.first?.text
            ?? "New Chat"
        return snippet(from: preferredSource, limit: 48, fallback: "New Chat")
    }

    private static func derivedLastMessagePreview(from messages: [AssistantConversationMessage]) -> String {
        let source = messages.last?.text ?? ""
        return snippet(from: source, limit: 84, fallback: "")
    }

    private static func snippet(from value: String, limit: Int, fallback: String) -> String {
        let normalized = value
            .replacingOccurrences(of: "\n", with: " ")
            .split(whereSeparator: \.isWhitespace)
            .joined(separator: " ")

        guard !normalized.isEmpty else { return fallback }
        guard normalized.count > limit else { return normalized }

        let prefix = normalized.prefix(limit)
        return "\(prefix)..."
    }
}

nonisolated struct AssistantThreadStoreSnapshot: Equatable, Codable, Sendable {
    var activeThreadID: UUID?
    var threads: [AssistantConversationThread]
    var pendingSessionDeletionIDs: [UUID]
    var pendingDeleteAll: Bool

    enum CodingKeys: String, CodingKey {
        case activeThreadID = "active_thread_id"
        case threads
        case pendingSessionDeletionIDs = "pending_session_deletion_ids"
        case pendingDeleteAll = "pending_delete_all"
    }

    init(
        activeThreadID: UUID?,
        threads: [AssistantConversationThread],
        pendingSessionDeletionIDs: [UUID] = [],
        pendingDeleteAll: Bool = false
    ) {
        self.activeThreadID = activeThreadID
        self.threads = threads
        self.pendingSessionDeletionIDs = pendingSessionDeletionIDs
        self.pendingDeleteAll = pendingDeleteAll
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        activeThreadID = try container.decodeIfPresent(UUID.self, forKey: .activeThreadID)
        threads = try container.decode([AssistantConversationThread].self, forKey: .threads)
        pendingSessionDeletionIDs =
            try container.decodeIfPresent([UUID].self, forKey: .pendingSessionDeletionIDs) ?? []
        pendingDeleteAll = try container.decodeIfPresent(Bool.self, forKey: .pendingDeleteAll) ?? false
    }

    static let empty = AssistantThreadStoreSnapshot(
        activeThreadID: nil,
        threads: [],
        pendingSessionDeletionIDs: [],
        pendingDeleteAll: false
    )
}
