import Foundation

public enum AutomationNotificationPreview {
    public static let defaultTitleMaxCharacters = 64
    public static let defaultBodyMaxCharacters = 180

    public static func makeVisiblePreview(
        from content: AutomationNotificationContent,
        titleMaxCharacters: Int = defaultTitleMaxCharacters,
        bodyMaxCharacters: Int = defaultBodyMaxCharacters
    ) -> AutomationNotificationContent {
        AutomationNotificationContent(
            title: truncate(content.title, maxCharacters: titleMaxCharacters),
            body: truncate(content.body, maxCharacters: bodyMaxCharacters)
        )
    }

    private static func truncate(_ value: String, maxCharacters: Int) -> String {
        let normalizedLimit = max(1, maxCharacters)
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.count > normalizedLimit else {
            return trimmed
        }

        let truncated = trimmed.prefix(normalizedLimit).trimmingCharacters(in: .whitespacesAndNewlines)
        return "\(truncated)..."
    }
}
