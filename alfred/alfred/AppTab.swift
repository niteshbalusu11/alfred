import Foundation

enum AppTab: Hashable, CaseIterable {
    case home
    case threads
    case automations
    case connectors

    var title: String {
        switch self {
        case .home:
            return "Home"
        case .threads:
            return "Threads"
        case .automations:
            return "Tasks"
        case .connectors:
            return "Connectors"
        }
    }

    var systemImage: String {
        switch self {
        case .home:
            return "house"
        case .threads:
            return "text.bubble"
        case .automations:
            return "calendar.badge.clock"
        case .connectors:
            return "link"
        }
    }
}
