import AlfredAPIClient
import Foundation
import XCTest
@testable import alfred

@MainActor
final class AutomationRuleStoreTests: XCTestCase {
    func testLoadReturnsEmptySnapshotWhenNoFileExists() async throws {
        let store = AutomationRuleStore(storageDirectoryURL: makeTemporaryDirectoryURL())

        let snapshot = try await store.load(for: "user_123")

        XCTAssertTrue(snapshot.entries.isEmpty)
        XCTAssertTrue(snapshot.promptByRuleID.isEmpty)
    }

    func testSaveAndLoadRoundTripsRulesAndPrompts() async throws {
        let store = AutomationRuleStore(storageDirectoryURL: makeTemporaryDirectoryURL())
        let firstRule = makeRuleSummary(title: "Morning Brief", localTime: "08:00")
        let secondRule = makeRuleSummary(title: "Calendar Check", localTime: "09:30")

        let snapshot = AutomationRuleCacheSnapshot(
            entries: [
                AutomationRuleCacheEntry(rule: firstRule, prompt: "Summarize my day"),
                AutomationRuleCacheEntry(rule: secondRule, prompt: "Check calendar and conflicts"),
            ]
        )
        try await store.save(snapshot, for: "user_123")

        let loaded = try await store.load(for: "user_123")
        let promptMap = loaded.promptByRuleID

        XCTAssertEqual(loaded.entries.count, 2)
        XCTAssertEqual(promptMap[firstRule.ruleId], "Summarize my day")
        XCTAssertEqual(promptMap[secondRule.ruleId], "Check calendar and conflicts")
    }

    func testClearRemovesPersistedSnapshot() async throws {
        let store = AutomationRuleStore(storageDirectoryURL: makeTemporaryDirectoryURL())
        let rule = makeRuleSummary(title: "One Task", localTime: "10:00")
        try await store.save(
            AutomationRuleCacheSnapshot(
                entries: [AutomationRuleCacheEntry(rule: rule, prompt: "Do the thing")]
            ),
            for: "user_123"
        )

        try await store.clear(for: "user_123")
        let loaded = try await store.load(for: "user_123")

        XCTAssertTrue(loaded.entries.isEmpty)
        XCTAssertTrue(loaded.promptByRuleID.isEmpty)
    }

    private func makeRuleSummary(title: String, localTime: String) -> AutomationRuleSummary {
        let nowISO = "2026-02-21T12:00:00Z"
        let payload = """
        {
          "rule_id": "\(UUID().uuidString.lowercased())",
          "title": "\(title)",
          "status": "ACTIVE",
          "schedule": {
            "schedule_type": "DAILY",
            "time_zone": "UTC",
            "local_time": "\(localTime)"
          },
          "next_run_at": "2026-02-21T13:00:00Z",
          "last_run_at": null,
          "prompt_sha256": "\(String(repeating: "a", count: 64))",
          "created_at": "\(nowISO)",
          "updated_at": "\(nowISO)"
        }
        """
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return try! decoder.decode(AutomationRuleSummary.self, from: Data(payload.utf8))
    }

    private func makeTemporaryDirectoryURL() -> URL {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("automation-rule-store-tests-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }
}
