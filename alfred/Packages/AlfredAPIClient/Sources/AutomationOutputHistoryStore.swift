import Foundation

public struct AutomationOutputHistoryEntry: Codable, Equatable, Sendable, Identifiable {
    public let requestID: String
    public let title: String
    public let body: String
    public let receivedAt: Date
    public let openedAt: Date?

    public var id: String { requestID }

    enum CodingKeys: String, CodingKey {
        case requestID = "request_id"
        case title
        case body
        case receivedAt = "received_at"
        case openedAt = "opened_at"
    }

    public init(
        requestID: String,
        title: String,
        body: String,
        receivedAt: Date,
        openedAt: Date? = nil
    ) {
        self.requestID = requestID
        self.title = title
        self.body = body
        self.receivedAt = receivedAt
        self.openedAt = openedAt
    }
}

public enum AutomationOutputHistoryStoreError: Error {
    case invalidRequestID
    case invalidContent
}

public actor AutomationOutputHistoryStore {
    public static let appGroupIdentifier = "group.com.prodata.alfred.shared"

    private struct Snapshot: Codable {
        var pendingOpenRequestID: String?
        var entries: [AutomationOutputHistoryEntry]

        enum CodingKeys: String, CodingKey {
            case pendingOpenRequestID = "pending_open_request_id"
            case entries
        }
    }

    private let fileManager: FileManager
    private let storageDirectoryURL: URL
    private let historyFileURL: URL
    private let maxEntries: Int
    private let encoder: JSONEncoder
    private let decoder: JSONDecoder

    public init(
        fileManager: FileManager = .default,
        storageDirectoryURL: URL? = nil,
        maxEntries: Int = 200
    ) {
        self.fileManager = fileManager
        self.storageDirectoryURL = Self.resolveStorageDirectoryURL(
            fileManager: fileManager,
            override: storageDirectoryURL
        )
        self.historyFileURL = self.storageDirectoryURL.appendingPathComponent("automation_output_history.json")
        self.maxEntries = max(1, maxEntries)

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.sortedKeys]
        self.encoder = encoder

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        self.decoder = decoder
    }

    public func list() throws -> [AutomationOutputHistoryEntry] {
        try loadSnapshot().entries
    }

    @discardableResult
    public func upsertDelivered(
        requestID: String,
        title: String,
        body: String,
        receivedAt: Date = Date()
    ) throws -> AutomationOutputHistoryEntry {
        var snapshot = try loadSnapshot()
        let normalizedRequestID = try normalizedRequestID(from: requestID)
        let normalizedTitle = try normalizedContent(title)
        let normalizedBody = try normalizedContent(body)

        let existingIndex = snapshot.entries.firstIndex { $0.requestID == normalizedRequestID }
        let entry = AutomationOutputHistoryEntry(
            requestID: normalizedRequestID,
            title: normalizedTitle,
            body: normalizedBody,
            receivedAt: receivedAt,
            openedAt: existingIndex.flatMap { snapshot.entries[$0].openedAt }
        )

        if let existingIndex {
            snapshot.entries[existingIndex] = entry
        } else {
            snapshot.entries.append(entry)
        }

        snapshot.entries = normalizedEntries(snapshot.entries)
        try saveSnapshot(snapshot)
        return entry
    }

    @discardableResult
    public func upsertOpenedFromNotificationTap(
        requestID: String,
        title: String,
        body: String,
        openedAt: Date = Date()
    ) throws -> AutomationOutputHistoryEntry {
        var snapshot = try loadSnapshot()
        let normalizedRequestID = try normalizedRequestID(from: requestID)
        let normalizedTitle = try normalizedContent(title)
        let normalizedBody = try normalizedContent(body)

        let existing = snapshot.entries.first { $0.requestID == normalizedRequestID }
        let entry = AutomationOutputHistoryEntry(
            requestID: normalizedRequestID,
            title: existing?.title ?? normalizedTitle,
            body: existing?.body ?? normalizedBody,
            receivedAt: existing?.receivedAt ?? openedAt,
            openedAt: openedAt
        )

        if let existingIndex = snapshot.entries.firstIndex(where: { $0.requestID == normalizedRequestID }) {
            snapshot.entries[existingIndex] = entry
        } else {
            snapshot.entries.append(entry)
        }
        snapshot.pendingOpenRequestID = normalizedRequestID
        snapshot.entries = normalizedEntries(snapshot.entries)

        try saveSnapshot(snapshot)
        return entry
    }

    public func markOpened(
        requestID: String,
        openedAt: Date = Date()
    ) throws -> AutomationOutputHistoryEntry? {
        var snapshot = try loadSnapshot()
        let normalizedRequestID = try normalizedRequestID(from: requestID)
        guard let existingIndex = snapshot.entries.firstIndex(where: { $0.requestID == normalizedRequestID }) else {
            return nil
        }

        let existing = snapshot.entries[existingIndex]
        let updated = AutomationOutputHistoryEntry(
            requestID: existing.requestID,
            title: existing.title,
            body: existing.body,
            receivedAt: existing.receivedAt,
            openedAt: openedAt
        )

        snapshot.entries[existingIndex] = updated
        snapshot.entries = normalizedEntries(snapshot.entries)
        try saveSnapshot(snapshot)
        return updated
    }

    public func peekPendingOpenRequestID() throws -> String? {
        try loadSnapshot().pendingOpenRequestID
    }

    public func consumePendingOpenRequestID() throws -> String? {
        var snapshot = try loadSnapshot()
        let pending = snapshot.pendingOpenRequestID
        snapshot.pendingOpenRequestID = nil
        try saveSnapshot(snapshot)
        return pending
    }

    public func clear() throws {
        if fileManager.fileExists(atPath: historyFileURL.path) {
            try fileManager.removeItem(at: historyFileURL)
        }
    }

    private func loadSnapshot() throws -> Snapshot {
        guard fileManager.fileExists(atPath: historyFileURL.path) else {
            return Snapshot(pendingOpenRequestID: nil, entries: [])
        }

        let data = try Data(contentsOf: historyFileURL)
        guard !data.isEmpty else {
            return Snapshot(pendingOpenRequestID: nil, entries: [])
        }

        let decoded = try decoder.decode(Snapshot.self, from: data)
        let pendingRequestID = decoded.pendingOpenRequestID.map {
            $0.trimmingCharacters(in: .whitespacesAndNewlines)
        }
        return Snapshot(
            pendingOpenRequestID: pendingRequestID?.isEmpty == false ? pendingRequestID : nil,
            entries: normalizedEntries(decoded.entries)
        )
    }

    private func saveSnapshot(_ snapshot: Snapshot) throws {
        try ensureStorageDirectoryExists()
        let normalized = Snapshot(
            pendingOpenRequestID: snapshot.pendingOpenRequestID,
            entries: normalizedEntries(snapshot.entries)
        )
        let data = try encoder.encode(normalized)
        try data.write(to: historyFileURL, options: [.atomic])
    }

    private func normalizedEntries(_ entries: [AutomationOutputHistoryEntry]) -> [AutomationOutputHistoryEntry] {
        var byRequestID: [String: AutomationOutputHistoryEntry] = [:]

        for entry in entries {
            let requestID = entry.requestID.trimmingCharacters(in: .whitespacesAndNewlines)
            let title = entry.title.trimmingCharacters(in: .whitespacesAndNewlines)
            let body = entry.body.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !requestID.isEmpty, !title.isEmpty, !body.isEmpty else {
                continue
            }

            let normalized = AutomationOutputHistoryEntry(
                requestID: requestID,
                title: title,
                body: body,
                receivedAt: entry.receivedAt,
                openedAt: entry.openedAt
            )

            if let existing = byRequestID[requestID] {
                let preferred = preferredEntry(lhs: existing, rhs: normalized)
                byRequestID[requestID] = preferred
            } else {
                byRequestID[requestID] = normalized
            }
        }

        return byRequestID.values
            .sorted(by: Self.receivedAtDescending)
            .prefix(maxEntries)
            .map { $0 }
    }

    private func preferredEntry(
        lhs: AutomationOutputHistoryEntry,
        rhs: AutomationOutputHistoryEntry
    ) -> AutomationOutputHistoryEntry {
        let pickedReceivedAt = max(lhs.receivedAt, rhs.receivedAt)
        let pickedOpenedAt: Date?
        switch (lhs.openedAt, rhs.openedAt) {
        case let (left?, right?):
            pickedOpenedAt = max(left, right)
        case let (left?, nil):
            pickedOpenedAt = left
        case let (nil, right?):
            pickedOpenedAt = right
        case (nil, nil):
            pickedOpenedAt = nil
        }
        let title = rhs.receivedAt >= lhs.receivedAt ? rhs.title : lhs.title
        let body = rhs.receivedAt >= lhs.receivedAt ? rhs.body : lhs.body

        return AutomationOutputHistoryEntry(
            requestID: lhs.requestID,
            title: title,
            body: body,
            receivedAt: pickedReceivedAt,
            openedAt: pickedOpenedAt
        )
    }

    private func normalizedRequestID(from value: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw AutomationOutputHistoryStoreError.invalidRequestID
        }
        return trimmed
    }

    private func normalizedContent(_ value: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw AutomationOutputHistoryStoreError.invalidContent
        }
        return trimmed
    }

    private func ensureStorageDirectoryExists() throws {
        var isDirectory: ObjCBool = false
        let directoryPath = storageDirectoryURL.path

        if fileManager.fileExists(atPath: directoryPath, isDirectory: &isDirectory) {
            if isDirectory.boolValue {
                return
            }
            try fileManager.removeItem(at: storageDirectoryURL)
        }

        try fileManager.createDirectory(
            at: storageDirectoryURL,
            withIntermediateDirectories: true
        )
    }

    private static func resolveStorageDirectoryURL(
        fileManager: FileManager,
        override: URL?
    ) -> URL {
        if let override {
            return override
        }

        if let appGroupURL = fileManager.containerURL(
            forSecurityApplicationGroupIdentifier: appGroupIdentifier
        ) {
            return appGroupURL
                .appendingPathComponent("automation_outputs", isDirectory: true)
        }

        let applicationSupportURL = fileManager.urls(
            for: .applicationSupportDirectory,
            in: .userDomainMask
        ).first ?? fileManager.temporaryDirectory
        return applicationSupportURL
            .appendingPathComponent("alfred", isDirectory: true)
            .appendingPathComponent("automation_outputs", isDirectory: true)
    }

    private static func receivedAtDescending(
        _ lhs: AutomationOutputHistoryEntry,
        _ rhs: AutomationOutputHistoryEntry
    ) -> Bool {
        if lhs.receivedAt == rhs.receivedAt {
            return lhs.requestID > rhs.requestID
        }
        return lhs.receivedAt > rhs.receivedAt
    }
}
