import ClerkKit
import XCTest
@testable import alfred

@MainActor
final class AppModelStartupRoutingTests: XCTestCase {
    func testRetryAuthBootstrapRoutesSignedOutUserToSignedOut() async {
        let clerk = Clerk.preview { preview in
            preview.isSignedIn = false
        }
        let model = AppModel(clerk: clerk)

        await model.retryAuthBootstrap()

        XCTAssertFalse(model.isAuthenticated)
        XCTAssertEqual(model.startupRoute, .signedOut)
    }

    func testRetryAuthBootstrapClearsBootstrapErrorBanner() async {
        let clerk = Clerk.preview { preview in
            preview.isSignedIn = false
        }
        let model = AppModel(clerk: clerk)

        model.errorBanner = AppModel.ErrorBanner(
            message: "Failed to load preferences",
            retryAction: .loadConnectors,
            sourceAction: .loadConnectors
        )

        await model.retryAuthBootstrap()

        XCTAssertNil(model.errorBanner)
        XCTAssertEqual(model.startupRoute, .signedOut)
    }
}
