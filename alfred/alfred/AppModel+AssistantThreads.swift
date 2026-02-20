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
    }

    private func applyAssistantThreadSnapshot(_ snapshot: AssistantThreadStoreSnapshot) {
        let sortedThreads = snapshot.threads.sorted(by: assistantThreadUpdatedAtDescending)
        assistantThreads = sortedThreads

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
            threads: assistantThreads
        )
        try await assistantThreadStore.save(snapshot, for: userID)
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
