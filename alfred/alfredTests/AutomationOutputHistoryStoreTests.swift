import AlfredAPIClient
import Foundation
import XCTest

final class AutomationOutputHistoryStoreTests: XCTestCase {
    func testUpsertDeliveredAndListReturnNewestFirst() async throws {
        let store = makeStore()
        let firstDate = Date(timeIntervalSince1970: 100)
        let secondDate = Date(timeIntervalSince1970: 200)

        try await store.upsertDelivered(
            requestID: "req-1",
            title: "Morning Brief",
            body: "First body",
            receivedAt: firstDate
        )
        try await store.upsertDelivered(
            requestID: "req-2",
            title: "Inbox Summary",
            body: "Second body",
            receivedAt: secondDate
        )

        let entries = try await store.list()
        XCTAssertEqual(entries.map(\.requestID), ["req-2", "req-1"])
        XCTAssertEqual(entries.first?.title, "Inbox Summary")
    }

    func testUpsertOpenedFromTapSetsAndConsumesPendingOpenRequest() async throws {
        let store = makeStore()
        try await store.upsertOpenedFromNotificationTap(
            requestID: "req-tap",
            title: "Automation",
            body: "Tap body",
            openedAt: Date(timeIntervalSince1970: 300)
        )

        let pending = try await store.peekPendingOpenRequestID()
        XCTAssertEqual(pending, "req-tap")

        let consumed = try await store.consumePendingOpenRequestID()
        XCTAssertEqual(consumed, "req-tap")

        let afterConsume = try await store.peekPendingOpenRequestID()
        XCTAssertNil(afterConsume)
    }

    func testRetentionCapKeepsMostRecentEntries() async throws {
        let store = makeStore(maxEntries: 2)

        try await store.upsertDelivered(
            requestID: "req-1",
            title: "One",
            body: "Body one",
            receivedAt: Date(timeIntervalSince1970: 10)
        )
        try await store.upsertDelivered(
            requestID: "req-2",
            title: "Two",
            body: "Body two",
            receivedAt: Date(timeIntervalSince1970: 20)
        )
        try await store.upsertDelivered(
            requestID: "req-3",
            title: "Three",
            body: "Body three",
            receivedAt: Date(timeIntervalSince1970: 30)
        )

        let entries = try await store.list()
        XCTAssertEqual(entries.count, 2)
        XCTAssertEqual(entries.map(\.requestID), ["req-3", "req-2"])
    }

    private func makeStore(maxEntries: Int = 200) -> AutomationOutputHistoryStore {
        let directoryURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("automation-output-history-tests-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: directoryURL, withIntermediateDirectories: true)
        return AutomationOutputHistoryStore(storageDirectoryURL: directoryURL, maxEntries: maxEntries)
    }
}
