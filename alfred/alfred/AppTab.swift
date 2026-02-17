import Foundation

enum AppTab: Hashable, CaseIterable {
    case home
    case activity
    case connectors

    var title: String {
        switch self {
        case .home:
            return "Home"
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
        case .activity:
            return "clock.arrow.circlepath"
        case .connectors:
            return "link"
        }
    }
}
