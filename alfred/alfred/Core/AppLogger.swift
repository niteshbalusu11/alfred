import Foundation
import OSLog

enum AppLogger {
    enum Category {
        case app
        case auth
        case network
        case oauth
    }

    private static let subsystem = Bundle.main.bundleIdentifier ?? "com.prodata.alfred"
    private static let appLogger = Logger(subsystem: subsystem, category: "app")
    private static let authLogger = Logger(subsystem: subsystem, category: "auth")
    private static let networkLogger = Logger(subsystem: subsystem, category: "network")
    private static let oauthLogger = Logger(subsystem: subsystem, category: "oauth")

    static func debug(_ message: String, category: Category = .app) {
        #if DEBUG
            logger(for: category).debug("\(message, privacy: .public)")
        #endif
    }

    static func info(_ message: String, category: Category = .app) {
        logger(for: category).info("\(message, privacy: .public)")
    }

    static func warning(_ message: String, category: Category = .app) {
        logger(for: category).warning("\(message, privacy: .public)")
    }

    static func error(_ message: String, category: Category = .app) {
        logger(for: category).error("\(message, privacy: .public)")
    }

    private static func logger(for category: Category) -> Logger {
        switch category {
        case .app:
            return appLogger
        case .auth:
            return authLogger
        case .network:
            return networkLogger
        case .oauth:
            return oauthLogger
        }
    }
}
