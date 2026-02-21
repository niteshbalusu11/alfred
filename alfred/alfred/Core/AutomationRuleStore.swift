import AlfredAPIClient
import Foundation

struct AutomationRuleCacheEntry: Codable, Sendable {
    var rule: AutomationRuleSummary
    var prompt: String?
}

struct AutomationRuleCacheSnapshot: Codable, Sendable {
    var entries: [AutomationRuleCacheEntry]

    static let empty = AutomationRuleCacheSnapshot(entries: [])

    var promptByRuleID: [UUID: String] {
        var output: [UUID: String] = [:]
        for entry in entries {
            guard let prompt = entry.prompt else { continue }
            let trimmed = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
            if !trimmed.isEmpty {
                output[entry.rule.ruleId] = trimmed
            }
        }
        return output
    }
}

actor AutomationRuleStore {
    private let fileManager: FileManager
    private let storageDirectoryURL: URL
    private let encoder: JSONEncoder
    private let decoder: JSONDecoder

    init(fileManager: FileManager = .default, storageDirectoryURL: URL? = nil) {
        self.fileManager = fileManager
        self.storageDirectoryURL = Self.resolveStorageDirectoryURL(
            fileManager: fileManager,
            override: storageDirectoryURL
        )

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.sortedKeys]
        self.encoder = encoder

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        self.decoder = decoder
    }

    func load(for userID: String) throws -> AutomationRuleCacheSnapshot {
        let fileURL = try fileURL(for: userID)
        guard fileManager.fileExists(atPath: fileURL.path) else {
            return .empty
        }

        let data = try Data(contentsOf: fileURL)
        guard !data.isEmpty else {
            return .empty
        }

        let snapshot = try decoder.decode(AutomationRuleCacheSnapshot.self, from: data)
        return AutomationRuleCacheSnapshot(entries: normalizedEntries(snapshot.entries))
    }

    func save(_ snapshot: AutomationRuleCacheSnapshot, for userID: String) throws {
        try ensureStorageDirectoryExists()
        let normalized = AutomationRuleCacheSnapshot(entries: normalizedEntries(snapshot.entries))
        let data = try encoder.encode(normalized)
        try data.write(to: try fileURL(for: userID), options: [.atomic])
    }

    func clear(for userID: String) throws {
        let fileURL = try fileURL(for: userID)
        guard fileManager.fileExists(atPath: fileURL.path) else { return }
        try fileManager.removeItem(at: fileURL)
    }

    private func normalizedEntries(_ entries: [AutomationRuleCacheEntry]) -> [AutomationRuleCacheEntry] {
        var byRuleID: [UUID: AutomationRuleCacheEntry] = [:]
        for entry in entries {
            var normalized = entry
            if let prompt = normalized.prompt {
                let trimmed = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
                normalized.prompt = trimmed.isEmpty ? nil : trimmed
            }

            if let existing = byRuleID[normalized.rule.ruleId] {
                if normalized.rule.updatedAt >= existing.rule.updatedAt {
                    byRuleID[normalized.rule.ruleId] = normalized
                }
            } else {
                byRuleID[normalized.rule.ruleId] = normalized
            }
        }

        return byRuleID.values.sorted { lhs, rhs in
            if lhs.rule.nextRunAt == rhs.rule.nextRunAt {
                return lhs.rule.ruleId.uuidString < rhs.rule.ruleId.uuidString
            }
            return lhs.rule.nextRunAt < rhs.rule.nextRunAt
        }
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

    private func fileURL(for userID: String) throws -> URL {
        let trimmed = userID.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw AutomationRuleStoreError.invalidUserID
        }

        let sanitized = sanitizedUserComponent(trimmed)
        return storageDirectoryURL
            .appendingPathComponent("automation_rules_\(sanitized)")
            .appendingPathExtension("json")
    }

    private func sanitizedUserComponent(_ userID: String) -> String {
        let base64 = Data(userID.utf8).base64EncodedString()
        let normalized = base64
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
        return normalized.isEmpty ? "unknown_user" : normalized
    }

    private static func resolveStorageDirectoryURL(
        fileManager: FileManager,
        override: URL?
    ) -> URL {
        if let override {
            return override
        }

        let applicationSupportURL = fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? fileManager.temporaryDirectory
        return applicationSupportURL
            .appendingPathComponent("alfred", isDirectory: true)
            .appendingPathComponent("automation_rules", isDirectory: true)
    }
}

enum AutomationRuleStoreError: Error {
    case invalidUserID
}
