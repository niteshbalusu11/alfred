import AlfredAPIClient
import XCTest
@testable import alfred

final class AssistantConversationMapperTests: XCTestCase {
    func testLegacyAssistantResponseBuildsResponsePartsAndToolSummary() throws {
        let response = try decodeAssistantResponse(
            """
            {
              "session_id": "2fce58f0-ecfb-4ea0-b7ef-4da9e663544e",
              "capability": "calendar_lookup",
              "display_text": "You have two meetings today.",
              "payload": {
                "title": "Calendar snapshot",
                "summary": "You have two meetings today.",
                "key_points": ["10:00 Team Sync", "15:00 Product Review"],
                "follow_ups": ["Ask for tomorrow's schedule."]
              }
            }
            """
        )

        XCTAssertEqual(response.responseParts.count, 2)
        XCTAssertEqual(response.responseParts.first?.type, .chatText)
        XCTAssertEqual(response.responseParts.last?.type, .toolSummary)

        let assistantMessage = AssistantConversationMapper.assistantMessage(from: response)
        XCTAssertEqual(assistantMessage.role, .assistant)
        XCTAssertEqual(assistantMessage.text, "You have two meetings today.")
        XCTAssertEqual(assistantMessage.toolSummaries.count, 1)
        XCTAssertEqual(assistantMessage.toolSummaries.first?.capability, .calendarLookup)
    }

    func testV2ResponsePartsDecodeAndMapMultipleToolSummaries() throws {
        let response = try decodeAssistantResponse(
            """
            {
              "session_id": "5a278243-89bb-483b-86c5-7c1512ac8be3",
              "response_parts": [
                {
                  "type": "chat_text",
                  "text": "Here is your calendar and inbox summary."
                },
                {
                  "type": "tool_summary",
                  "capability": "calendar_lookup",
                  "payload": {
                    "title": "Calendar",
                    "summary": "You have two meetings.",
                    "key_points": ["10:00 Team Sync"],
                    "follow_ups": []
                  }
                },
                {
                  "type": "tool_summary",
                  "capability": "email_lookup",
                  "payload": {
                    "title": "Inbox",
                    "summary": "One urgent sender matched.",
                    "key_points": ["finance@example.com - Invoice reminder"],
                    "follow_ups": ["Ask to filter this week only."]
                  }
                }
              ]
            }
            """
        )

        XCTAssertEqual(response.capability, .mixed)
        XCTAssertEqual(response.displayText, "Here is your calendar and inbox summary.")
        XCTAssertEqual(response.responseParts.count, 3)

        let assistantMessage = AssistantConversationMapper.assistantMessage(from: response)
        XCTAssertEqual(assistantMessage.toolSummaries.count, 2)
        XCTAssertEqual(assistantMessage.toolSummaries[0].capability, .calendarLookup)
        XCTAssertEqual(assistantMessage.toolSummaries[1].capability, .emailLookup)
    }

    func testChatOnlyResponsePartsRemainChatOnlyInConversation() throws {
        let response = try decodeAssistantResponse(
            """
            {
              "session_id": "f40f9b5f-90f3-4e56-852a-c8818700f625",
              "response_parts": [
                {
                  "type": "chat_text",
                  "text": "Sure, let's talk generally."
                }
              ]
            }
            """
        )

        XCTAssertEqual(response.capability, .generalChat)
        XCTAssertEqual(response.responseParts.count, 1)

        let assistantMessage = AssistantConversationMapper.assistantMessage(from: response)
        XCTAssertTrue(assistantMessage.toolSummaries.isEmpty)
    }

    func testAssistantMapperFallsBackToPayloadSummaryWhenDisplayTextIsMissing() throws {
        let response = try decodeAssistantResponse(
            """
            {
              "session_id": "f6fe77db-e7ae-4011-9653-8b9d82cc8eff",
              "response_parts": [
                {
                  "type": "tool_summary",
                  "capability": "calendar_lookup",
                  "payload": {
                    "title": "Calendar",
                    "summary": "You have one meeting in the next hour.",
                    "key_points": ["2:00 PM Product review"],
                    "follow_ups": []
                  }
                }
              ]
            }
            """
        )

        let assistantMessage = AssistantConversationMapper.assistantMessage(from: response)
        XCTAssertEqual(assistantMessage.text, "You have one meeting in the next hour.")
        XCTAssertEqual(assistantMessage.toolSummaries.count, 1)
    }

    func testLegacyGeneralChatResponseDoesNotProduceToolSummaryCard() throws {
        let response = try decodeAssistantResponse(
            """
            {
              "session_id": "b2bfb8ef-8ca9-4f06-84f1-62f66bb62e18",
              "capability": "general_chat",
              "display_text": "I can help with that.",
              "payload": {
                "title": "General conversation",
                "summary": "I can help with that.",
                "key_points": [],
                "follow_ups": []
              }
            }
            """
        )

        XCTAssertEqual(response.responseParts.count, 1)
        XCTAssertEqual(response.responseParts.first?.type, .chatText)

        let assistantMessage = AssistantConversationMapper.assistantMessage(from: response)
        XCTAssertTrue(assistantMessage.toolSummaries.isEmpty)
    }

    func testLegacyGeneralChatWithPlanKeyPointsDoesNotProduceToolSummaryCard() throws {
        let response = try decodeAssistantResponse(
            """
            {
              "session_id": "f8ab3b06-3fcc-42da-828b-6adfba598d2c",
              "capability": "general_chat",
              "display_text": "July is a great time. Here is a starting plan.",
              "payload": {
                "title": "Alaska in July",
                "summary": "July is a great time. Here is a starting plan.",
                "key_points": [
                  "Day 1-2: Anchorage",
                  "Day 3-4: Denali"
                ],
                "follow_ups": [
                  "Ask for a 10-day itinerary."
                ]
              }
            }
            """
        )

        let assistantMessage = AssistantConversationMapper.assistantMessage(from: response)
        XCTAssertTrue(assistantMessage.toolSummaries.isEmpty)
    }

    private func decodeAssistantResponse(_ json: String) throws -> AssistantPlaintextQueryResponse {
        let data = try XCTUnwrap(json.data(using: .utf8))
        return try JSONDecoder().decode(AssistantPlaintextQueryResponse.self, from: data)
    }
}
