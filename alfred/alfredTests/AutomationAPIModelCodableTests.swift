import AlfredAPIClient
import Foundation
import XCTest

final class AutomationAPIModelCodableTests: XCTestCase {
    func testCreateAutomationRequestEncodesSnakeCaseFields() throws {
        let request = CreateAutomationRequest(
            intervalSeconds: 900,
            timeZone: "America/Los_Angeles",
            promptEnvelope: try makePromptEnvelope()
        )

        let data = try JSONEncoder().encode(request)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])

        XCTAssertEqual(json["interval_seconds"] as? Int, 900)
        XCTAssertEqual(json["time_zone"] as? String, "America/Los_Angeles")

        let envelope = try XCTUnwrap(json["prompt_envelope"] as? [String: Any])
        XCTAssertEqual(envelope["key_id"] as? String, "key-1")
        XCTAssertEqual(envelope["request_id"] as? String, "req-1")
        XCTAssertEqual(envelope["client_ephemeral_public_key"] as? String, "pub-key")
    }

    func testUpdateAutomationRequestEncodesStatusAndSchedule() throws {
        let request = UpdateAutomationRequest(
            schedule: AutomationScheduleUpdate(
                intervalSeconds: 3_600,
                timeZone: "UTC"
            ),
            status: .paused
        )

        let data = try JSONEncoder().encode(request)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])

        XCTAssertEqual(json["status"] as? String, "PAUSED")

        let schedule = try XCTUnwrap(json["schedule"] as? [String: Any])
        XCTAssertEqual(schedule["interval_seconds"] as? Int, 3_600)
        XCTAssertEqual(schedule["time_zone"] as? String, "UTC")
    }

    func testListAutomationsResponseDecodesDatesAndEnums() throws {
        let payload = """
        {
          "items": [
            {
              "rule_id": "f92ee2cb-1a34-4e40-aa4e-e8c2bd1522de",
              "status": "ACTIVE",
              "schedule_type": "INTERVAL_SECONDS",
              "interval_seconds": 1800,
              "time_zone": "UTC",
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
        XCTAssertEqual(response.items[0].status, .active)
        XCTAssertEqual(response.items[0].scheduleType, .intervalSeconds)
        XCTAssertEqual(response.items[0].intervalSeconds, 1_800)
        XCTAssertEqual(response.items[0].timeZone, "UTC")
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
