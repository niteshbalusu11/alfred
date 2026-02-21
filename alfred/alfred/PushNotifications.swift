import AlfredAPIClient
import Foundation
import UIKit
import UserNotifications

enum PushNotificationEvents {
    static let didUpdateAPNSToken = Notification.Name("alfred.didUpdateAPNSToken")
    static let didOpenAutomationNotification = Notification.Name("alfred.didOpenAutomationNotification")
}

final class PushAppDelegate: NSObject, UIApplicationDelegate, UNUserNotificationCenterDelegate {
    private let outputHistoryStore = AutomationOutputHistoryStore()

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        UNUserNotificationCenter.current().delegate = self
        return true
    }

    func application(
        _ application: UIApplication,
        didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data
    ) {
        let token = deviceToken.map { String(format: "%02x", $0) }.joined()
        NotificationCenter.default.post(
            name: PushNotificationEvents.didUpdateAPNSToken,
            object: nil,
            userInfo: ["token": token]
        )
    }

    func application(
        _ application: UIApplication,
        didFailToRegisterForRemoteNotificationsWithError error: Error
    ) {
        AppLogger.warning("APNs registration with Apple failed.", category: .network)
    }

    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        guard response.actionIdentifier == UNNotificationDefaultActionIdentifier else {
            completionHandler()
            return
        }

        let content = response.notification.request.content
        let requestID = AutomationNotificationCrypto.requestID(from: content.userInfo)
        let title = content.title.trimmingCharacters(in: .whitespacesAndNewlines)
        let body = content.body.trimmingCharacters(in: .whitespacesAndNewlines)

        Task {
            defer { completionHandler() }
            if let requestID, !title.isEmpty, !body.isEmpty {
                _ = try? await outputHistoryStore.upsertOpenedFromNotificationTap(
                    requestID: requestID,
                    title: title,
                    body: body
                )
            }
            NotificationCenter.default.post(
                name: PushNotificationEvents.didOpenAutomationNotification,
                object: nil
            )
        }
    }
}
