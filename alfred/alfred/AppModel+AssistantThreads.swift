import AlfredAPIClient
import Foundation

extension AppModel {
    func queryAssistant(query: String) async {
        let trimmedQuery = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedQuery.isEmpty else {
            errorBanner = ErrorBanner(
                message: "Message is empty. Type or dictate something first.",
                retryAction: nil,
                sourceAction: nil
            )
            return
        }

        let sessionID = assistantSessionIDForActiveThread()
        let targetThreadID = activeAssistantThreadID
        await run(action: .queryAssistant, retryAction: .queryAssistant(query: trimmedQuery)) { [self] in
            let response = try await apiClient.queryAssistantEncrypted(
                query: trimmedQuery,
                sessionId: sessionID,
                attestationConfig: AppConfiguration.assistantAttestationVerificationConfig
            )
            AppLogger.info(
                """
                Assistant response received capability=\(response.capability.rawValue) \
                response_parts=\(response.responseParts.count) \
                payload_key_points=\(response.payload.keyPoints.count) \
                payload_follow_ups=\(response.payload.followUps.count)
                """,
                category: .network
            )

            applySuccessfulAssistantTurn(
                query: trimmedQuery,
                response: response,
                targetThreadID: targetThreadID
            )
            try await persistAssistantThreadsForCurrentUser()
        }
    }

    func clearAssistantConversation() {
        assistantConversation = []
        assistantResponseText = ""
        activeAssistantThreadID = nil
        Task { [weak self] in
            guard let self else { return }
            try? await self.persistAssistantThreadsForCurrentUser()
        }
    }

    func selectAssistantThread(_ threadID: UUID) {
        guard let thread = assistantThreads.first(where: { $0.id == threadID }) else { return }

        activeAssistantThreadID = thread.id
        assistantConversation = thread.messages
        assistantResponseText = thread.messages.last(where: { $0.role == .assistant })?.text ?? ""

        Task { [weak self] in
            guard let self else { return }
            try? await self.persistAssistantThreadsForCurrentUser()
        }
    }

    func deleteAssistantThread(_ threadID: UUID) {
        let deletion = removeAssistantThreadLocally(threadID)
        guard deletion.removed else { return }

        if let sessionID = deletion.sessionID {
            assistantThreadSyncState.pendingSessionDeletionIDs.insert(sessionID)
            assistantThreadSyncState.lastSyncErrorMessage = nil
        }

        Task { [weak self] in
            guard let self else { return }

            await persistAssistantThreadsIgnoringErrors()
            await syncAssistantThreadDeletionsIfNeeded()
        }
    }

    func deleteAllAssistantThreads() {
        let sessionIDs = assistantThreads.compactMap(\.sessionID)
        resetAssistantThreadState()

        assistantThreadSyncState.pendingDeleteAll = true
        assistantThreadSyncState.pendingSessionDeletionIDs = Set(sessionIDs)
        assistantThreadSyncState.lastSyncErrorMessage = nil

        Task { [weak self] in
            guard let self else { return }
            await persistAssistantThreadsIgnoringErrors()
            await syncAssistantThreadDeletionsIfNeeded()
        }
    }

    func retryAssistantThreadSync() {
        Task { [weak self] in
            guard let self else { return }
            await syncAssistantThreadDeletionsIfNeeded()
        }
    }

    func assistantSessionIDForActiveThread() -> UUID? {
        guard let activeAssistantThreadID else { return nil }
        return assistantThreads.first(where: { $0.id == activeAssistantThreadID })?.sessionID
    }

    func applySuccessfulAssistantTurn(
        query: String,
        response: AssistantPlaintextQueryResponse,
        timestamp: Date = Date(),
        targetThreadID: UUID? = nil
    ) {
        if assistantThreadSyncState.pendingDeleteAll {
            assistantThreadSyncState.pendingDeleteAll = false
        }

        let userMessage = AssistantConversationMapper.userMessage(from: query, createdAt: timestamp)
        let assistantMessage = AssistantConversationMapper.assistantMessage(from: response, createdAt: timestamp)
        let newMessages = [userMessage, assistantMessage]

        let resolvedThreadID = targetThreadID ?? activeAssistantThreadID
        let threadID: UUID
        if let resolvedThreadID,
           let existingThreadIndex = assistantThreads.firstIndex(where: { $0.id == resolvedThreadID })
        {
            assistantThreads[existingThreadIndex].append(newMessages, sessionID: response.sessionId)
            threadID = resolvedThreadID
        } else {
            let newThread = AssistantConversationThread(
                sessionID: response.sessionId,
                createdAt: timestamp,
                updatedAt: timestamp,
                messages: newMessages
            )
            assistantThreads.insert(newThread, at: 0)
            threadID = newThread.id
        }

        assistantThreads.sort(by: assistantThreadUpdatedAtDescending)
        if let activeThread = assistantThreads.first(where: { $0.id == threadID }) {
            activeAssistantThreadID = activeThread.id
            assistantConversation = activeThread.messages
        } else {
            activeAssistantThreadID = nil
            assistantConversation = []
        }
        assistantResponseText = assistantMessage.text
    }

    func restoreAssistantThreads(for userID: String) async {
        do {
            let snapshot = try await assistantThreadStore.load(for: userID)
            applyAssistantThreadSnapshot(snapshot)
            await syncAssistantThreadDeletionsIfNeeded()
        } catch {
            AppLogger.error(
                "Failed to restore assistant threads from local store for user \(userID).",
                category: .app
            )
            resetAssistantThreadState()
        }
    }

    func clearPersistedAssistantThreads(for userID: String?) async {
        guard let userID,
              !userID.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else {
            return
        }

        do {
            try await assistantThreadStore.clear(for: userID)
        } catch {
            AppLogger.error(
                "Failed to clear assistant thread store for user \(userID).",
                category: .app
            )
        }
    }

    func resetAssistantThreadState() {
        assistantThreads = []
        activeAssistantThreadID = nil
        assistantConversation = []
        assistantResponseText = ""
        assistantThreadSyncState = .empty
    }

    private func applyAssistantThreadSnapshot(_ snapshot: AssistantThreadStoreSnapshot) {
        let sortedThreads = snapshot.threads.sorted(by: assistantThreadUpdatedAtDescending)
        assistantThreads = sortedThreads
        assistantThreadSyncState.pendingSessionDeletionIDs = Set(snapshot.pendingSessionDeletionIDs)
        assistantThreadSyncState.pendingDeleteAll = snapshot.pendingDeleteAll
        assistantThreadSyncState.syncInFlight = false
        assistantThreadSyncState.lastSyncErrorMessage = nil

        if let activeThreadID = snapshot.activeThreadID,
           let activeThread = sortedThreads.first(where: { $0.id == activeThreadID })
        {
            self.activeAssistantThreadID = activeThread.id
            assistantConversation = activeThread.messages
            assistantResponseText = activeThread.messages.last(where: { $0.role == .assistant })?.text ?? ""
            return
        }

        guard let latestThread = sortedThreads.first else {
            activeAssistantThreadID = nil
            assistantConversation = []
            assistantResponseText = ""
            return
        }

        activeAssistantThreadID = latestThread.id
        assistantConversation = latestThread.messages
        assistantResponseText = latestThread.messages.last(where: { $0.role == .assistant })?.text ?? ""
    }

    private func persistAssistantThreadsForCurrentUser() async throws {
        guard let userID = assistantStorageUserID else { return }

        let snapshot = AssistantThreadStoreSnapshot(
            activeThreadID: activeAssistantThreadID,
            threads: assistantThreads,
            pendingSessionDeletionIDs: assistantThreadSyncState.pendingSessionIDsInStableOrder,
            pendingDeleteAll: assistantThreadSyncState.pendingDeleteAll
        )
        try await assistantThreadStore.save(snapshot, for: userID)
    }

    private func removeAssistantThreadLocally(_ threadID: UUID) -> (removed: Bool, sessionID: UUID?) {
        guard let index = assistantThreads.firstIndex(where: { $0.id == threadID }) else {
            return (false, nil)
        }

        let removedThread = assistantThreads.remove(at: index)
        selectFallbackAssistantThreadAfterLocalDeletion(deletedThreadID: threadID)
        return (true, removedThread.sessionID)
    }

    private func selectFallbackAssistantThreadAfterLocalDeletion(deletedThreadID: UUID) {
        guard activeAssistantThreadID == deletedThreadID else {
            return
        }

        guard let latestThread = assistantThreads.first else {
            activeAssistantThreadID = nil
            assistantConversation = []
            assistantResponseText = ""
            return
        }

        activeAssistantThreadID = latestThread.id
        assistantConversation = latestThread.messages
        assistantResponseText = latestThread.messages.last(where: { $0.role == .assistant })?.text ?? ""
    }

    private func syncAssistantThreadDeletionsIfNeeded() async {
        guard assistantStorageUserID != nil else { return }
        guard !assistantThreadSyncState.syncInFlight, assistantThreadSyncState.hasPendingSync else { return }
        assistantThreadSyncState.syncInFlight = true

        defer {
            assistantThreadSyncState.syncInFlight = false
        }

        if assistantThreadSyncState.pendingDeleteAll {
            do {
                _ = try await apiClient.deleteAllAssistantSessions()
                assistantThreadSyncState.pendingDeleteAll = false
                assistantThreadSyncState.pendingSessionDeletionIDs = []
                assistantThreadSyncState.lastSyncErrorMessage = nil
                await persistAssistantThreadsIgnoringErrors()
                return
            } catch {
                assistantThreadSyncState.lastSyncErrorMessage = assistantThreadSyncErrorMessage(from: error)
                return
            }
        }

        for sessionID in assistantThreadSyncState.pendingSessionIDsInStableOrder {
            do {
                _ = try await apiClient.deleteAssistantSession(sessionID: sessionID)
                assistantThreadSyncState.pendingSessionDeletionIDs.remove(sessionID)
                if assistantThreadSyncState.pendingSessionDeletionIDs.isEmpty {
                    assistantThreadSyncState.lastSyncErrorMessage = nil
                }
            } catch AlfredAPIClientError.serverError(let statusCode, _, _) where statusCode == 404 {
                assistantThreadSyncState.pendingSessionDeletionIDs.remove(sessionID)
                if assistantThreadSyncState.pendingSessionDeletionIDs.isEmpty {
                    assistantThreadSyncState.lastSyncErrorMessage = nil
                }
            } catch {
                assistantThreadSyncState.lastSyncErrorMessage = assistantThreadSyncErrorMessage(from: error)
                await persistAssistantThreadsIgnoringErrors()
                return
            }
        }

        assistantThreadSyncState.lastSyncErrorMessage = nil
        await persistAssistantThreadsIgnoringErrors()
    }

    private func persistAssistantThreadsIgnoringErrors() async {
        do {
            try await persistAssistantThreadsForCurrentUser()
        } catch {
            AppLogger.warning("Failed to persist assistant thread snapshot after local mutation.")
        }
    }

    private func assistantThreadSyncErrorMessage(from error: Error) -> String {
        switch error {
        case let AlfredAPIClientError.serverError(statusCode, _, _):
            if statusCode == 429 || (500...599).contains(statusCode) {
                return "Thread deletion sync failed due to a temporary server issue. Retry to sync."
            }
            if statusCode == 401 {
                return "Thread deletion sync requires a valid sign-in session."
            }
            return "Thread deletion sync failed (\(statusCode)). Retry to sync."
        case is URLError:
            return "Thread deletion sync failed due to a network issue. Retry to sync."
        case AlfredAPIClientError.unauthorized:
            return "Thread deletion sync requires a valid sign-in session."
        default:
            return "Thread deletion sync failed. Retry to sync."
        }
    }

    private func assistantThreadUpdatedAtDescending(
        _ lhs: AssistantConversationThread,
        _ rhs: AssistantConversationThread
    ) -> Bool {
        if lhs.updatedAt == rhs.updatedAt {
            return lhs.createdAt > rhs.createdAt
        }
        return lhs.updatedAt > rhs.updatedAt
    }
}
