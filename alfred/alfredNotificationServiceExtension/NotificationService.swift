import AlfredAPIClient
import UserNotifications

final class NotificationService: UNNotificationServiceExtension {
    private var contentHandler: ((UNNotificationContent) -> Void)?
    private var bestAttemptContent: UNMutableNotificationContent?
    private var processingTask: Task<Void, Never>?
    private let outputHistoryStore = AutomationOutputHistoryStore()
    private let stateLock = NSLock()
    private var didDeliver = false

    override func didReceive(
        _ request: UNNotificationRequest,
        withContentHandler contentHandler: @escaping (UNNotificationContent) -> Void
    ) {
        self.contentHandler = contentHandler
        let content = (request.content.mutableCopy() as? UNMutableNotificationContent)
            ?? UNMutableNotificationContent()
        bestAttemptContent = content

        processingTask = Task { [weak self] in
            guard let self else { return }
            let resolved = await AutomationNotificationCrypto.resolveVisibleContent(from: request.content.userInfo)
            let visiblePreview = AutomationNotificationPreview.makeVisiblePreview(from: resolved)
            content.title = visiblePreview.title
            content.body = visiblePreview.body
            if content.sound == nil {
                content.sound = .default
            }

            if resolved != .fallback,
               let requestID = AutomationNotificationCrypto.requestID(from: request.content.userInfo)
            {
                _ = try? await outputHistoryStore.upsertDelivered(
                    requestID: requestID,
                    title: resolved.title,
                    body: resolved.body
                )
            }

            self.deliver(content)
        }
    }

    override func serviceExtensionTimeWillExpire() {
        processingTask?.cancel()
        processingTask = nil

        if let content = bestAttemptContent {
            content.title = AutomationNotificationContent.fallback.title
            content.body = AutomationNotificationContent.fallback.body
            if content.sound == nil {
                content.sound = .default
            }
            deliver(content)
        } else {
            let fallback = UNMutableNotificationContent()
            fallback.title = AutomationNotificationContent.fallback.title
            fallback.body = AutomationNotificationContent.fallback.body
            fallback.sound = .default
            deliver(fallback)
        }
    }

    private func deliver(_ content: UNNotificationContent) {
        stateLock.lock()
        defer { stateLock.unlock() }

        guard !didDeliver else {
            return
        }
        didDeliver = true
        contentHandler?(content)
        contentHandler = nil
    }
}
