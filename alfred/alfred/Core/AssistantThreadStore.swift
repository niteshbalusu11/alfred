import Foundation

actor AssistantThreadStore {
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

    func load(for userID: String) throws -> AssistantThreadStoreSnapshot {
        let fileURL = try fileURL(for: userID)
        guard fileManager.fileExists(atPath: fileURL.path) else {
            return .empty
        }

        let data = try Data(contentsOf: fileURL)
        guard !data.isEmpty else { return .empty }

        var snapshot = try decoder.decode(AssistantThreadStoreSnapshot.self, from: data)
        snapshot.threads = snapshot.threads
            .map { thread in
                var normalized = thread
                normalized.refreshMetadata()
                return normalized
            }
            .sorted(by: Self.updatedAtDescending)

        if let activeThreadID = snapshot.activeThreadID,
           snapshot.threads.contains(where: { $0.id == activeThreadID })
        {
            return snapshot
        }

        snapshot.activeThreadID = snapshot.threads.first?.id
        return snapshot
    }

    func save(_ snapshot: AssistantThreadStoreSnapshot, for userID: String) throws {
        try ensureStorageDirectoryExists()

        let normalizedThreads = snapshot.threads
            .map { thread in
                var normalized = thread
                normalized.refreshMetadata()
                return normalized
            }
            .sorted(by: Self.updatedAtDescending)
        let normalizedActiveID = {
            guard let activeID = snapshot.activeThreadID else { return Optional<UUID>.none }
            return normalizedThreads.contains(where: { $0.id == activeID }) ? activeID : nil
        }()
        let normalizedSnapshot = AssistantThreadStoreSnapshot(
            activeThreadID: normalizedActiveID,
            threads: normalizedThreads
        )
        let data = try encoder.encode(normalizedSnapshot)
        try data.write(to: try fileURL(for: userID), options: [.atomic])
    }

    func clear(for userID: String) throws {
        let fileURL = try fileURL(for: userID)
        guard fileManager.fileExists(atPath: fileURL.path) else { return }
        try fileManager.removeItem(at: fileURL)
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
        guard !userID.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw AssistantThreadStoreError.invalidUserID
        }

        let sanitized = sanitizedUserComponent(userID)
        return storageDirectoryURL
            .appendingPathComponent("assistant_threads_\(sanitized)")
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
            .appendingPathComponent("assistant_threads", isDirectory: true)
    }

    private static func updatedAtDescending(
        _ lhs: AssistantConversationThread,
        _ rhs: AssistantConversationThread
    ) -> Bool {
        if lhs.updatedAt == rhs.updatedAt {
            return lhs.createdAt > rhs.createdAt
        }
        return lhs.updatedAt > rhs.updatedAt
    }
}

enum AssistantThreadStoreError: Error {
    case invalidUserID
}
