import AlfredAPIClient
import Foundation
import XCTest

final class AutomationAPIModelCodableTests: XCTestCase {
    func testCreateAutomationRequestEncodesSnakeCaseFields() throws {
        let request = CreateAutomationRequest(
            title: "Morning Plan",
            schedule: AutomationSchedule(
                scheduleType: .weekly,
                timeZone: "America/Los_Angeles",
                localTime: "09:30"
            ),
            promptEnvelope: try makePromptEnvelope()
        )

        let data = try JSONEncoder().encode(request)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        XCTAssertEqual(json["title"] as? String, "Morning Plan")

        let schedule = try XCTUnwrap(json["schedule"] as? [String: Any])
        XCTAssertEqual(schedule["schedule_type"] as? String, "WEEKLY")
        XCTAssertEqual(schedule["time_zone"] as? String, "America/Los_Angeles")
        XCTAssertEqual(schedule["local_time"] as? String, "09:30")

        let envelope = try XCTUnwrap(json["prompt_envelope"] as? [String: Any])
        XCTAssertEqual(envelope["key_id"] as? String, "key-1")
        XCTAssertEqual(envelope["request_id"] as? String, "req-1")
        XCTAssertEqual(envelope["client_ephemeral_public_key"] as? String, "pub-key")
    }

    func testUpdateAutomationRequestEncodesStatusAndSchedule() throws {
        let request = UpdateAutomationRequest(
            title: "Updated plan",
            schedule: AutomationSchedule(
                scheduleType: .daily,
                timeZone: "UTC",
                localTime: "08:00"
            ),
            status: .paused
        )

        let data = try JSONEncoder().encode(request)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])

        XCTAssertEqual(json["title"] as? String, "Updated plan")
        XCTAssertEqual(json["status"] as? String, "PAUSED")

        let schedule = try XCTUnwrap(json["schedule"] as? [String: Any])
        XCTAssertEqual(schedule["schedule_type"] as? String, "DAILY")
        XCTAssertEqual(schedule["time_zone"] as? String, "UTC")
        XCTAssertEqual(schedule["local_time"] as? String, "08:00")
    }

    func testListAutomationsResponseDecodesDatesAndEnums() throws {
        let payload = """
        {
          "items": [
            {
              "rule_id": "f92ee2cb-1a34-4e40-aa4e-e8c2bd1522de",
              "title": "Daily summary",
              "status": "ACTIVE",
              "schedule": {
                "schedule_type": "MONTHLY",
                "time_zone": "UTC",
                "local_time": "11:45"
              },
              "next_run_at": "2026-02-21T12:00:00Z",
              "last_run_at": null,
              "prompt_sha256": "abc123",
              "created_at": "2026-02-21T10:00:00Z",
              "updated_at": "2026-02-21T11:00:00Z"
            }
          ]
        }
        """

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let response = try decoder.decode(ListAutomationsResponse.self, from: Data(payload.utf8))

        XCTAssertEqual(response.items.count, 1)
        XCTAssertEqual(response.items[0].title, "Daily summary")
        XCTAssertEqual(response.items[0].status, .active)
        XCTAssertEqual(response.items[0].schedule.scheduleType, .monthly)
        XCTAssertEqual(response.items[0].schedule.timeZone, "UTC")
        XCTAssertEqual(response.items[0].schedule.localTime, "11:45")
        XCTAssertNil(response.items[0].lastRunAt)
    }

    private func makePromptEnvelope() throws -> AssistantEncryptedRequestEnvelope {
        let payload = """
        {
          "version": "v1",
          "algorithm": "x25519-chacha20poly1305",
          "key_id": "key-1",
          "request_id": "req-1",
          "client_ephemeral_public_key": "pub-key",
          "nonce": "nonce",
          "ciphertext": "ciphertext"
        }
        """

        return try JSONDecoder().decode(
            AssistantEncryptedRequestEnvelope.self,
            from: Data(payload.utf8)
        )
    }
}
