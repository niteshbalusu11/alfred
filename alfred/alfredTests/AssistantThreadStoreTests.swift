import AlfredAPIClient
import Foundation
import XCTest
@testable import alfred

@MainActor
final class AssistantThreadStoreTests: XCTestCase {
    func testLoadReturnsEmptySnapshotWhenNoSavedThreadsExist() async throws {
        let store = AssistantThreadStore(storageDirectoryURL: makeTemporaryDirectoryURL())

        let snapshot = try await store.load(for: "user_123")

        XCTAssertEqual(snapshot, .empty)
    }

    func testSaveAndLoadRoundTripsThreadsAndActiveThread() async throws {
        let store = AssistantThreadStore(storageDirectoryURL: makeTemporaryDirectoryURL())
        let firstSessionID = UUID()
        let secondSessionID = UUID()

        var firstThread = AssistantConversationThread(
            id: UUID(),
            sessionID: firstSessionID,
            createdAt: Date(timeIntervalSince1970: 100),
            updatedAt: Date(timeIntervalSince1970: 101),
            messages: [
                AssistantConversationMessage(
                    id: UUID(),
                    role: .user,
                    text: "Plan an NYC weekend",
                    capability: nil,
                    toolSummaries: [],
                    createdAt: Date(timeIntervalSince1970: 100)
                ),
                AssistantConversationMessage(
                    id: UUID(),
                    role: .assistant,
                    text: "Start with Friday dinner in Soho.",
                    capability: .generalChat,
                    toolSummaries: [],
                    createdAt: Date(timeIntervalSince1970: 101)
                ),
            ]
        )
        firstThread.refreshMetadata()

        var secondThread = AssistantConversationThread(
            id: UUID(),
            sessionID: secondSessionID,
            createdAt: Date(timeIntervalSince1970: 200),
            updatedAt: Date(timeIntervalSince1970: 201),
            messages: [
                AssistantConversationMessage(
                    id: UUID(),
                    role: .user,
                    text: "Review this week's calendar",
                    capability: nil,
                    toolSummaries: [],
                    createdAt: Date(timeIntervalSince1970: 200)
                ),
                AssistantConversationMessage(
                    id: UUID(),
                    role: .assistant,
                    text: "You have six meetings before Thursday.",
                    capability: .meetingsToday,
                    toolSummaries: [],
                    createdAt: Date(timeIntervalSince1970: 201)
                ),
            ]
        )
        secondThread.refreshMetadata()

        let snapshot = AssistantThreadStoreSnapshot(
            activeThreadID: firstThread.id,
            threads: [firstThread, secondThread]
        )

        try await store.save(snapshot, for: "user_123")
        let loadedSnapshot = try await store.load(for: "user_123")

        XCTAssertEqual(loadedSnapshot.activeThreadID, firstThread.id)
        XCTAssertEqual(loadedSnapshot.threads.count, 2)
        XCTAssertEqual(loadedSnapshot.threads[0].id, secondThread.id)
        XCTAssertEqual(loadedSnapshot.threads[1].id, firstThread.id)
    }

    func testClearRemovesSavedSnapshot() async throws {
        let store = AssistantThreadStore(storageDirectoryURL: makeTemporaryDirectoryURL())
        let thread = AssistantConversationThread(
            sessionID: UUID(),
            messages: [
                AssistantConversationMessage(
                    id: UUID(),
                    role: .user,
                    text: "hello",
                    capability: nil,
                    toolSummaries: [],
                    createdAt: Date(timeIntervalSince1970: 10)
                ),
            ]
        )
        try await store.save(
            AssistantThreadStoreSnapshot(activeThreadID: thread.id, threads: [thread]),
            for: "user_123"
        )

        try await store.clear(for: "user_123")
        let loadedSnapshot = try await store.load(for: "user_123")

        XCTAssertEqual(loadedSnapshot, .empty)
    }

    private func makeTemporaryDirectoryURL() -> URL {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("assistant-thread-store-tests-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }
}
