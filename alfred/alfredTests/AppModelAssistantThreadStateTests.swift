import AlfredAPIClient
import ClerkKit
import XCTest
@testable import alfred

@MainActor
final class AppModelAssistantThreadStateTests: XCTestCase {
    func testClearAssistantConversationKeepsExistingThreads() {
        let model = makeModel()
        let initialResponse = makeAssistantResponse(
            sessionID: UUID(),
            text: "First answer"
        )

        model.applySuccessfulAssistantTurn(
            query: "first question",
            response: initialResponse,
            timestamp: Date(timeIntervalSince1970: 10)
        )
        XCTAssertEqual(model.assistantThreads.count, 1)

        model.clearAssistantConversation()

        XCTAssertEqual(model.assistantThreads.count, 1)
        XCTAssertTrue(model.assistantConversation.isEmpty)
        XCTAssertNil(model.activeAssistantThreadID)
    }

    func testSelectingThreadSwitchesActiveSessionAndConversation() {
        let model = makeModel()
        let firstSessionID = UUID()
        let secondSessionID = UUID()
        let firstResponse = makeAssistantResponse(
            sessionID: firstSessionID,
            text: "First thread answer"
        )
        let secondResponse = makeAssistantResponse(
            sessionID: secondSessionID,
            text: "Second thread answer"
        )

        model.applySuccessfulAssistantTurn(
            query: "first question",
            response: firstResponse,
            timestamp: Date(timeIntervalSince1970: 100)
        )
        let firstThreadID = try! XCTUnwrap(model.assistantThreads.first?.id)

        model.clearAssistantConversation()
        model.applySuccessfulAssistantTurn(
            query: "second question",
            response: secondResponse,
            timestamp: Date(timeIntervalSince1970: 200)
        )
        XCTAssertEqual(model.assistantSessionIDForActiveThread(), secondSessionID)

        model.selectAssistantThread(firstThreadID)

        XCTAssertEqual(model.assistantSessionIDForActiveThread(), firstSessionID)
        XCTAssertEqual(model.assistantConversation.count, 2)
        XCTAssertEqual(model.assistantConversation.first?.text, "first question")
        XCTAssertEqual(model.assistantConversation.last?.text, "First thread answer")
    }

    func testDeleteAssistantThreadRemovesLocalThreadAndPromotesMostRecentThread() {
        let model = makeModel()
        let firstSessionID = UUID()
        let secondSessionID = UUID()

        model.applySuccessfulAssistantTurn(
            query: "first question",
            response: makeAssistantResponse(sessionID: firstSessionID, text: "First thread answer"),
            timestamp: Date(timeIntervalSince1970: 100)
        )
        let firstThreadID = try! XCTUnwrap(model.assistantThreads.first?.id)

        model.clearAssistantConversation()
        model.applySuccessfulAssistantTurn(
            query: "second question",
            response: makeAssistantResponse(sessionID: secondSessionID, text: "Second thread answer"),
            timestamp: Date(timeIntervalSince1970: 200)
        )

        model.deleteAssistantThread(firstThreadID)

        XCTAssertEqual(model.assistantThreads.count, 1)
        XCTAssertEqual(model.assistantThreads.first?.sessionID, secondSessionID)
        XCTAssertEqual(model.activeAssistantThreadID, model.assistantThreads.first?.id)
        XCTAssertEqual(model.assistantConversation.last?.text, "Second thread answer")
        XCTAssertTrue(model.assistantThreadSyncState.pendingSessionDeletionIDs.contains(firstSessionID))
    }

    func testDeleteAllAssistantThreadsClearsConversationAndMarksDeleteAllForSync() {
        let model = makeModel()
        let firstSessionID = UUID()
        let secondSessionID = UUID()

        model.applySuccessfulAssistantTurn(
            query: "first question",
            response: makeAssistantResponse(sessionID: firstSessionID, text: "First thread answer"),
            timestamp: Date(timeIntervalSince1970: 100)
        )
        model.clearAssistantConversation()
        model.applySuccessfulAssistantTurn(
            query: "second question",
            response: makeAssistantResponse(sessionID: secondSessionID, text: "Second thread answer"),
            timestamp: Date(timeIntervalSince1970: 200)
        )

        model.deleteAllAssistantThreads()

        XCTAssertTrue(model.assistantThreads.isEmpty)
        XCTAssertNil(model.activeAssistantThreadID)
        XCTAssertTrue(model.assistantConversation.isEmpty)
        XCTAssertTrue(model.assistantThreadSyncState.pendingDeleteAll)
        XCTAssertEqual(
            Set(model.assistantThreadSyncState.pendingSessionDeletionIDs),
            Set([firstSessionID, secondSessionID])
        )
    }

    func testDeleteAllAssistantThreadsMarksDeleteAllEvenWithoutLocalThreads() {
        let model = makeModel()

        model.deleteAllAssistantThreads()

        XCTAssertTrue(model.assistantThreadSyncState.pendingDeleteAll)
        XCTAssertTrue(model.assistantThreadSyncState.pendingSessionDeletionIDs.isEmpty)
    }

    private func makeModel() -> AppModel {
        let clerk = Clerk.preview { preview in
            preview.isSignedIn = false
        }
        return AppModel(clerk: clerk)
    }

    private func makeAssistantResponse(
        sessionID: UUID,
        text: String
    ) -> AssistantPlaintextQueryResponse {
        let json = """
        {
          "session_id": "\(sessionID.uuidString)",
          "capability": "general_chat",
          "display_text": "\(text)",
          "payload": {
            "title": "Assistant",
            "summary": "\(text)",
            "key_points": [],
            "follow_ups": []
          },
          "response_parts": [
            {
              "type": "chat_text",
              "text": "\(text)"
            }
          ]
        }
        """
        return try! JSONDecoder().decode(AssistantPlaintextQueryResponse.self, from: Data(json.utf8))
    }
}
