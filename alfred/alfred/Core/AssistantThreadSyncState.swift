import Foundation

struct AssistantThreadSyncState: Equatable {
    var pendingSessionDeletionIDs: Set<UUID> = []
    var pendingDeleteAll = false
    var syncInFlight = false
    var lastSyncErrorMessage: String?

    var hasPendingSync: Bool {
        pendingDeleteAll || !pendingSessionDeletionIDs.isEmpty
    }

    var pendingSessionIDsInStableOrder: [UUID] {
        pendingSessionDeletionIDs.sorted { $0.uuidString < $1.uuidString }
    }

    static let empty = AssistantThreadSyncState()
}
