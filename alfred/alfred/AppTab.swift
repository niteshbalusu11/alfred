import Foundation

enum AppTab: Hashable, CaseIterable {
    case home
    case threads
    case activity
    case connectors

    var title: String {
        switch self {
        case .home:
            return "Home"
        case .threads:
            return "Threads"
        case .activity:
            return "Activity"
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
        case .activity:
            return "clock.arrow.circlepath"
        case .connectors:
            return "link"
        }
    }
}
