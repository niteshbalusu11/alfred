import AlfredAPIClient
import ClerkKit
import XCTest
@testable import alfred

@MainActor
final class AppModelConnectorHydrationTests: XCTestCase {
    func testApplyConnectorSnapshotHydratesActiveGoogleConnector() throws {
        let model = makeSignedOutModel()
        model.googleAuthURL = "https://accounts.google.com/oauth"
        model.googleState = "pending-state"

        let response = try decodeConnectorsResponse(
            """
            {
              "items": [
                {
                  "connector_id": "connector-active",
                  "provider": "google",
                  "status": "ACTIVE"
                }
              ]
            }
            """
        )

        model.applyConnectorSnapshot(response)

        XCTAssertEqual(model.connectorID, "connector-active")
        XCTAssertEqual(model.revokeStatus, "Connector status: ACTIVE.")
        XCTAssertEqual(model.googleAuthURL, "")
        XCTAssertEqual(model.googleState, "")
    }

    func testApplyConnectorSnapshotClearsConnectorForRevokedState() throws {
        let model = makeSignedOutModel()
        model.connectorID = "connector-active"

        let response = try decodeConnectorsResponse(
            """
            {
              "items": [
                {
                  "connector_id": "connector-active",
                  "provider": "google",
                  "status": "REVOKED"
                }
              ]
            }
            """
        )

        model.applyConnectorSnapshot(response)

        XCTAssertEqual(model.connectorID, "")
        XCTAssertEqual(model.revokeStatus, "Connector status: REVOKED.")
    }

    func testApplyConnectorSnapshotClearsStateWhenConnectorMissing() throws {
        let model = makeSignedOutModel()
        model.connectorID = "connector-active"
        model.revokeStatus = "Connector status: ACTIVE."
        model.googleAuthURL = "https://accounts.google.com/oauth"
        model.googleState = "pending-state"

        let response = try decodeConnectorsResponse(
            """
            {
              "items": []
            }
            """
        )

        model.applyConnectorSnapshot(response)

        XCTAssertEqual(model.connectorID, "")
        XCTAssertEqual(model.revokeStatus, "")
        XCTAssertEqual(model.googleAuthURL, "")
        XCTAssertEqual(model.googleState, "")
    }

    private func makeSignedOutModel() -> AppModel {
        let clerk = Clerk.preview { preview in
            preview.isSignedIn = false
        }
        return AppModel(clerk: clerk)
    }

    private func decodeConnectorsResponse(_ json: String) throws -> ListConnectorsResponse {
        let data = Data(json.utf8)
        let decoder = JSONDecoder()
        return try decoder.decode(ListConnectorsResponse.self, from: data)
    }
}
